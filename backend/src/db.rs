//! Database module for PostgreSQL + pgvector operations

use sqlx::postgres::{PgPool, PgPoolOptions};

/// Initialize the PostgreSQL database
pub async fn init_db() -> anyhow::Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/wechat_insights".to_string()
    });

    tracing::info!("Connecting to PostgreSQL: {}", database_url);

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    // Create pgvector extension if not exists
    sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&pool)
        .await?;

    // Create embeddings table with vector column (4096 dimensions for qwen3-embedding:8b-q8_0)
    // Get embedding dimension from environment
    // - Gemini gemini-embedding-001: supports 768, 1536, 3072 (recommended: 768)
    // - Ollama qwen3-embedding:8b-q8_0: 4096
    let embedding_dim = std::env::var("EMBEDDING_DIMENSION")
        .ok()
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(768); // Default to Gemini recommended dimension

    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS embeddings (
            id TEXT PRIMARY KEY,
            fakeid TEXT NOT NULL,
            aid TEXT,
            title TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'title',
            text_hash TEXT NOT NULL,
            vector vector({}) NOT NULL,
            indexed_at BIGINT NOT NULL
        )
        "#,
        embedding_dim
    ))
    .execute(&pool)
    .await?;

    // Create indexes separately
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_embeddings_fakeid ON embeddings(fakeid)")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_embeddings_source ON embeddings(source)")
        .execute(&pool)
        .await?;

    // Create accounts table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS accounts (
            fakeid TEXT PRIMARY KEY,
            nickname TEXT,
            round_head_img TEXT,
            signature TEXT,
            service_type INTEGER,
            count INTEGER NOT NULL DEFAULT 0,
            articles INTEGER NOT NULL DEFAULT 0,
            total_count INTEGER NOT NULL DEFAULT 0,
            create_time BIGINT,
            update_time BIGINT,
            last_update_time BIGINT,
            sync_all BOOLEAN NOT NULL DEFAULT FALSE,
            raw_json JSONB
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create articles table
    // id = fakeid:aid
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS articles (
            id TEXT PRIMARY KEY,
            fakeid TEXT NOT NULL,
            aid TEXT NOT NULL,
            title TEXT NOT NULL,
            link TEXT NOT NULL,
            create_time BIGINT NOT NULL,
            update_time BIGINT,
            digest TEXT,
            cover TEXT,
            is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
            itemidx INTEGER NOT NULL DEFAULT 1,
            content_json JSONB,
            raw_json JSONB
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create indexes for articles
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_articles_fakeid_time ON articles(fakeid, create_time DESC)",
    )
    .execute(&pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_articles_link ON articles(link)")
        .execute(&pool)
        .await?;

    // Create article_content table for HTML storage
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS article_content (
            id TEXT PRIMARY KEY, -- Same as article id (fakeid:aid)
            content TEXT NOT NULL,
            original_url TEXT,
            create_time BIGINT DEFAULT (extract(epoch from now())::bigint)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create indexes for article_content
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_article_content_url ON article_content(original_url)",
    )
    .execute(&pool)
    .await?;

    // Create assets table for images/media
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS assets (
            url TEXT PRIMARY KEY,
            data BYTEA NOT NULL,
            mime_type TEXT,
            size INTEGER,
            create_time BIGINT DEFAULT (extract(epoch from now())::bigint)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create comments table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS comments (
            id TEXT PRIMARY KEY, -- using id/fakeid:aid as key? Comment usually belongs to article
            article_id TEXT NOT NULL,
            content_json JSONB NOT NULL,
            create_time BIGINT DEFAULT (extract(epoch from now())::bigint)
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create index for comments
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_comments_article_id ON comments(article_id)")
        .execute(&pool)
        .await?;

    // Create vector index for fast similarity search (IVFFlat)
    // This may fail if already exists or if table is empty (needs data to create IVFFlat)
    let _ = sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_embeddings_vector 
        ON embeddings USING ivfflat (vector vector_cosine_ops) WITH (lists = 100)
        "#,
    )
    .execute(&pool)
    .await;

    // Create cookies table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cookies (
            auth_key TEXT PRIMARY KEY,
            token TEXT NOT NULL,
            cookies_json TEXT NOT NULL,
            created_at BIGINT NOT NULL,
            expires_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    tracing::info!("PostgreSQL database initialized with pgvector extension");

    // Create insight_tasks table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS insight_tasks (
            id UUID PRIMARY KEY,
            prompt TEXT NOT NULL,
            status VARCHAR(50) NOT NULL,
            keywords TEXT[] NOT NULL DEFAULT '{}',
            target_count INTEGER NOT NULL DEFAULT 30,
            processed_count INTEGER NOT NULL DEFAULT 0,
            created_at BIGINT NOT NULL,
            updated_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Create insight_articles table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS insight_articles (
            id UUID PRIMARY KEY,
            task_id UUID NOT NULL REFERENCES insight_tasks(id),
            title TEXT NOT NULL,
            url TEXT NOT NULL,
            account_name TEXT,
            account_fakeid TEXT,  -- Added column
            publish_time BIGINT,
            similarity FLOAT,
            insight TEXT,
            relevance_score FLOAT,
            created_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Migration for existing tables
    let _ =
        sqlx::query("ALTER TABLE insight_articles ADD COLUMN IF NOT EXISTS account_fakeid TEXT")
            .execute(&pool)
            .await;

    let _ =
        sqlx::query("ALTER TABLE insight_tasks ADD COLUMN IF NOT EXISTS completion_reason TEXT")
            .execute(&pool)
            .await;

    // Create index for insight_articles
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_insight_articles_task_id ON insight_articles(task_id)",
    )
    .execute(&pool)
    .await?;

    // Create cached_articles table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS cached_articles (
            url_hash TEXT PRIMARY KEY,
            url TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at BIGINT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

/// Embedding record in database
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Embedding {
    pub id: String,
    pub fakeid: String,
    pub aid: Option<String>,
    pub title: String,
    pub source: String,
    pub text_hash: String,
    pub vector: Vec<f32>,
    pub indexed_at: i64,
}
