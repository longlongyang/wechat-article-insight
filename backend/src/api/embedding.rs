//! Embedding API handlers with PostgreSQL + pgvector

use axum::{extract::State, Json};
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::error::AppError;
use crate::AppState;

// ============ Types ============

#[derive(Debug, Deserialize)]
pub struct GenerateRequest {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct GenerateResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchItem {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct BatchRequest {
    pub items: Vec<BatchItem>,
}

#[derive(Debug, Serialize)]
pub struct BatchResultItem {
    pub id: String,
    pub embedding: Vec<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    pub success: bool,
    pub results: Option<Vec<BatchResultItem>>,
    pub completed: usize,
    pub failed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EmbeddingData {
    pub id: String,
    pub fakeid: String,
    pub aid: Option<String>,
    pub title: String,
    pub source: String,
    #[serde(rename = "textHash")]
    pub text_hash: String,
    pub vector: Vec<f32>,
    #[serde(rename = "indexedAt")]
    pub indexed_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct StoreRequest {
    pub embeddings: Vec<EmbeddingData>,
}

#[derive(Debug, Serialize)]
pub struct StoreResponse {
    pub success: bool,
    pub stored: usize,
    pub failed: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub vector: Vec<f32>,
    #[serde(rename = "topK")]
    pub top_k: Option<usize>,
    #[serde(rename = "minScore")]
    pub min_score: Option<f32>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct SearchResultItem {
    pub id: String,
    pub title: String,
    pub fakeid: String,
    pub source: String,
    pub link: Option<String>, // Added link
    pub score: f32,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results: Option<Vec<SearchResultItem>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "searchTime")]
    pub search_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub success: bool,
    pub count: usize,
    #[serde(rename = "bySource")]
    pub by_source: BySourceStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct BySourceStats {
    pub title: usize,
    pub content: usize,
    pub comment: usize,
}

#[derive(Debug, Serialize)]
pub struct ClearResponse {
    pub success: bool,
    pub cleared: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ============ Ollama Client ============

const DEFAULT_OLLAMA_BASE_URL: &str = "http://127.0.0.1:11434";
const DEFAULT_EMBEDDING_MODEL: &str = "qwen3-embedding:8b-q8_0";

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

async fn call_ollama_embed(texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
    let base_url =
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| DEFAULT_OLLAMA_BASE_URL.to_string());
    let model = std::env::var("OLLAMA_EMBEDDING_MODEL")
        .unwrap_or_else(|_| DEFAULT_EMBEDDING_MODEL.to_string());

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(600)) // 10 minutes timeout for large batches
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build client: {}", e)))?;

    let url = format!("{}/api/embed", base_url);

    let payload = serde_json::json!({
        "model": model,
        "input": texts,
    });

    tracing::info!("[Ollama] Sending request to {} with model '{}'", url, model);
    tracing::debug!("[Ollama] Payload: {}", payload);

    let response = client.post(&url).json(&payload).send().await.map_err(|e| {
        tracing::error!("[Ollama] Failed to connect to {}: {}", url, e);
        e
    })?;

    let status = response.status();
    tracing::info!("[Ollama] Response Status: {}", status);

    if !status.is_success() {
        let headers = response.headers().clone();
        tracing::warn!("[Ollama] Response Headers: {:?}", headers);
        let error_text = response.text().await.unwrap_or_default();
        tracing::error!("[Ollama] Error Body: '{}'", error_text);

        return Err(AppError::BadRequest(format!(
            "Ollama error (Status: {}): {}",
            status,
            if error_text.is_empty() {
                "(Empty response body)"
            } else {
                &error_text
            }
        )));
    }

    let result: OllamaEmbedResponse = response.json().await?;
    Ok(result.embeddings)
}

/// Helper for internal use (e.g. from other modules)
#[allow(dead_code)]
pub async fn generate_embedding_ollama(text: &str) -> Result<Vec<f32>, AppError> {
    let embeddings = call_ollama_embed(vec![text.to_string()]).await?;
    embeddings
        .into_iter()
        .next()
        .ok_or(AppError::Internal("No embedding returned".to_string()))
}

// ============ Handlers ============

/// Generate embedding for a single text
pub async fn generate(
    Json(req): Json<GenerateRequest>,
) -> Result<Json<GenerateResponse>, AppError> {
    if req.text.is_empty() {
        return Ok(Json(GenerateResponse {
            success: false,
            embedding: None,
            dimensions: None,
            error: Some("请提供文本内容".to_string()),
        }));
    }

    let embeddings = call_ollama_embed(vec![req.text]).await?;

    if let Some(embedding) = embeddings.into_iter().next() {
        let dimensions = embedding.len();
        Ok(Json(GenerateResponse {
            success: true,
            embedding: Some(embedding),
            dimensions: Some(dimensions),
            error: None,
        }))
    } else {
        Ok(Json(GenerateResponse {
            success: false,
            embedding: None,
            dimensions: None,
            error: Some("无效的 embedding 响应".to_string()),
        }))
    }
}

/// Generate embeddings for multiple texts
pub async fn batch(Json(req): Json<BatchRequest>) -> Result<Json<BatchResponse>, AppError> {
    if req.items.is_empty() {
        return Ok(Json(BatchResponse {
            success: true,
            results: Some(vec![]),
            completed: 0,
            failed: 0,
            error: None,
        }));
    }

    let valid_items: Vec<_> = req
        .items
        .into_iter()
        .filter(|item| !item.text.trim().is_empty())
        .collect();

    if valid_items.is_empty() {
        return Ok(Json(BatchResponse {
            success: true,
            results: Some(vec![]),
            completed: 0,
            failed: 0,
            error: None,
        }));
    }

    let texts: Vec<String> = valid_items.iter().map(|item| item.text.clone()).collect();
    let embeddings = call_ollama_embed(texts).await?;

    let mut results = Vec::new();
    let mut completed = 0;
    let mut failed = 0;

    for (i, item) in valid_items.iter().enumerate() {
        if let Some(embedding) = embeddings.get(i) {
            if !embedding.is_empty() {
                results.push(BatchResultItem {
                    id: item.id.clone(),
                    embedding: embedding.clone(),
                    error: None,
                });
                completed += 1;
            } else {
                results.push(BatchResultItem {
                    id: item.id.clone(),
                    embedding: vec![],
                    error: Some("No embedding returned".to_string()),
                });
                failed += 1;
            }
        } else {
            results.push(BatchResultItem {
                id: item.id.clone(),
                embedding: vec![],
                error: Some("No embedding returned".to_string()),
            });
            failed += 1;
        }
    }

    Ok(Json(BatchResponse {
        success: failed == 0,
        results: Some(results),
        completed,
        failed,
        error: None,
    }))
}

/// Store embeddings in PostgreSQL with pgvector
pub async fn store(
    State(pool): State<PgPool>,
    Json(req): Json<StoreRequest>,
) -> Result<Json<StoreResponse>, AppError> {
    let mut stored = 0;
    let mut failed = 0;

    for emb in req.embeddings {
        if emb.id.is_empty() || emb.vector.is_empty() {
            failed += 1;
            continue;
        }

        // Convert to pgvector Vector type
        let vector = Vector::from(emb.vector.clone());

        let result = sqlx::query(
            r#"
            INSERT INTO embeddings (id, fakeid, aid, title, source, text_hash, vector, indexed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                fakeid = EXCLUDED.fakeid,
                aid = EXCLUDED.aid,
                title = EXCLUDED.title,
                source = EXCLUDED.source,
                text_hash = EXCLUDED.text_hash,
                vector = EXCLUDED.vector,
                indexed_at = EXCLUDED.indexed_at
            "#,
        )
        .bind(&emb.id)
        .bind(&emb.fakeid)
        .bind(&emb.aid)
        .bind(&emb.title)
        .bind(&emb.source)
        .bind(&emb.text_hash)
        .bind(&vector)
        .bind(emb.indexed_at)
        .execute(&pool)
        .await;

        match result {
            Ok(_) => stored += 1,
            Err(e) => {
                tracing::error!("Failed to store {}: {}", emb.id, e);
                failed += 1;
            }
        }
    }

    tracing::info!("[Store] Stored: {}, Failed: {}", stored, failed);

    Ok(Json(StoreResponse {
        success: failed == 0,
        stored,
        failed,
        error: None,
    }))
}

/// Search for similar embeddings using pgvector native cosine similarity
/// This is MUCH faster than loading all vectors into memory!
pub async fn search(
    State(pool): State<PgPool>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    let start_time = std::time::Instant::now();

    if req.vector.is_empty() {
        return Ok(Json(SearchResponse {
            success: false,
            results: None,
            total: None,
            search_time: None,
            error: Some("请提供查询向量".to_string()),
        }));
    }

    let top_k = req.top_k.unwrap_or(50) as i32;
    let min_score = req.min_score.unwrap_or(0.3);
    let offset = req.offset.unwrap_or(0) as i64;

    // Convert to pgvector
    let query_vector = Vector::from(req.vector.clone());

    // Native pgvector similarity search - uses index for O(log N) performance!
    // 1 - (vector <=> query) converts cosine distance to cosine similarity
    let rows: Vec<(String, String, String, String, Option<String>, f64)> = sqlx::query_as(
        r#"
        SELECT e.id, e.fakeid, e.title, e.source, a.link,
               1 - (e.vector <=> $1::vector) as score
        FROM embeddings e
        LEFT JOIN articles a ON e.fakeid = a.fakeid AND e.aid = a.aid
        WHERE 1 - (e.vector <=> $1::vector) >= $2
        ORDER BY e.vector <=> $1::vector
        LIMIT $3 OFFSET $4
        "#,
    )
    .bind(&query_vector)
    .bind(min_score as f64)
    .bind(top_k)
    .bind(offset)
    .fetch_all(&pool)
    .await?;

    let results: Vec<SearchResultItem> = rows
        .into_iter()
        .map(
            |(id, fakeid, title, source, link, score)| SearchResultItem {
                id,
                fakeid,
                title,
                source,
                link,
                score: score as f32,
            },
        )
        .collect();

    let total = results.len();
    let search_time = start_time.elapsed().as_millis() as u64;

    tracing::info!(
        "[Search] Found {} matches in {}ms (pgvector native search)",
        total,
        search_time
    );

    Ok(Json(SearchResponse {
        success: true,
        results: Some(results),
        total: Some(total),
        search_time: Some(search_time),
        error: None,
    }))
}

/// Get embedding statistics
pub async fn stats(State(pool): State<PgPool>) -> Result<Json<StatsResponse>, AppError> {
    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM embeddings")
        .fetch_one(&pool)
        .await?;

    let title: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM embeddings WHERE source = 'title'")
        .fetch_one(&pool)
        .await?;

    let content: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM embeddings WHERE source = 'content'")
            .fetch_one(&pool)
            .await?;

    let comment: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM embeddings WHERE source = 'comment'")
            .fetch_one(&pool)
            .await?;

    Ok(Json(StatsResponse {
        success: true,
        count: total.0 as usize,
        by_source: BySourceStats {
            title: title.0 as usize,
            content: content.0 as usize,
            comment: comment.0 as usize,
        },
        error: None,
    }))
}

/// Clear all embeddings
pub async fn clear(State(pool): State<PgPool>) -> Result<Json<ClearResponse>, AppError> {
    let result = sqlx::query("DELETE FROM embeddings").execute(&pool).await?;

    let cleared = result.rows_affected() as usize;
    tracing::info!("[Clear] Cleared {} embeddings", cleared);

    Ok(Json(ClearResponse {
        success: true,
        cleared,
        error: None,
    }))
}

#[derive(Debug, Serialize)]
pub struct CleanResponse {
    pub success: bool,
    pub cleaned: usize,
    pub error: Option<String>,
}

/// Clean orphaned embeddings (articles that no longer exist)
pub async fn clean(State(pool): State<PgPool>) -> Result<Json<CleanResponse>, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM embeddings e 
        WHERE NOT EXISTS (
            SELECT 1 FROM articles a 
            WHERE a.fakeid = e.fakeid AND a.aid = e.aid
        )
        "#,
    )
    .execute(&pool)
    .await?;

    let cleaned = result.rows_affected() as usize;
    tracing::info!("[Clean] Removed {} orphaned embeddings", cleaned);

    Ok(Json(CleanResponse {
        success: true,
        cleaned,
        error: None,
    }))
}

#[derive(Debug, Serialize)]
pub struct UnindexedCountResponse {
    pub success: bool,
    pub count: usize,
    pub error: Option<String>,
}

/// Get count of unindexed articles
pub async fn unindexed_count(
    State(pool): State<PgPool>,
) -> Result<Json<UnindexedCountResponse>, AppError> {
    // Check if title embedding exists for the article
    let count: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) 
        FROM articles a 
        WHERE NOT EXISTS (
            SELECT 1 FROM embeddings e 
            WHERE e.fakeid = a.fakeid AND e.aid = a.aid AND e.source = 'title'
        )
        "#,
    )
    .fetch_one(&pool)
    .await?;

    Ok(Json(UnindexedCountResponse {
        success: true,
        count: count.0 as usize,
        error: None,
    }))
}

#[derive(Debug, Deserialize)]
pub struct AutoIndexRequest {
    pub limit: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct AutoIndexResponse {
    pub success: bool,
    pub indexed: usize,
    pub failed: usize,
    pub remaining: usize,
    pub error: Option<String>,
}

/// Auto index a batch of articles
pub async fn auto_index(
    State(pool): State<PgPool>,
    Json(req): Json<AutoIndexRequest>,
) -> Result<Json<AutoIndexResponse>, AppError> {
    let limit = req.limit.unwrap_or(20);

    // 1. Fetch unindexed articles
    let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT a.id, a.fakeid, a.aid, a.title, a.digest
        FROM articles a 
        WHERE NOT EXISTS (
            SELECT 1 FROM embeddings e 
            WHERE e.fakeid = a.fakeid AND e.aid = a.aid AND e.source = 'title'
        )
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?;

    if rows.is_empty() {
        return Ok(Json(AutoIndexResponse {
            success: true,
            indexed: 0,
            failed: 0,
            remaining: 0,
            error: None,
        }));
    }

    let mut indexed = 0;
    let mut failed = 0;

    // Prepare items for batch embedding
    // We will process titles and digests separately but in same batch to save round trips if possible
    // But simplistic approach: just batch all titles for now.

    let mut texts_to_embed = Vec::new();
    let mut metadata = Vec::new();

    for (id, fakeid, aid, title, digest) in &rows {
        if !title.is_empty() {
            texts_to_embed.push(title.clone());
            metadata.push((
                id.clone(),
                fakeid.clone(),
                aid.clone(),
                title.clone(),
                "title".to_string(),
            ));
        }

        // Also index digest if present
        if let Some(d) = digest {
            if !d.is_empty() {
                texts_to_embed.push(d.clone());
                // Use digest as text, but title field in valid DB record still needs to be the article title?
                // Actually embedding table struct has: id, fakeid, aid, title, source, text_hash, vector
                // "title" field in database is usually the text content's title or the article title?
                // Let's assume it is the article title for reference.
                metadata.push((
                    id.clone(),
                    fakeid.clone(),
                    aid.clone(),
                    title.clone(),
                    "digest".to_string(),
                ));
            }
        }
    }

    // Call Ollama
    if !texts_to_embed.is_empty() {
        match call_ollama_embed(texts_to_embed).await {
            Ok(embeddings) => {
                // Store embeddings
                for (i, embedding) in embeddings.into_iter().enumerate() {
                    if i >= metadata.len() {
                        break;
                    }
                    let (_article_id, fakeid, aid, title, source) = &metadata[i];

                    // Generate a deterministic ID for the embedding record
                    // fakeid:aid:source
                    let embedding_id = format!("{}:{}:{}", fakeid, aid, source);

                    let vector = Vector::from(embedding);
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;

                    // Simple hash for change detection
                    let text_hash = format!("{:x}", md5::compute(format!("{}{}", title, source))); // Simplified

                    let result = sqlx::query(
                        r#"
                        INSERT INTO embeddings (id, fakeid, aid, title, source, text_hash, vector, indexed_at)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                        ON CONFLICT (id) DO UPDATE SET
                            vector = EXCLUDED.vector,
                            indexed_at = EXCLUDED.indexed_at
                        "#
                    )
                    .bind(&embedding_id)
                    .bind(fakeid)
                    .bind(aid)
                    .bind(title)
                    .bind(source)
                    .bind(&text_hash)
                    .bind(&vector)
                    .bind(now)
                    .execute(&pool)
                    .await;

                    if let Err(e) = result {
                        tracing::error!("Failed to save embedding {}: {}", embedding_id, e);
                        failed += 1; // Count as failed specific item
                    } else {
                        // Count unique articles indexed, not just embeddings rows
                        // But for simplicity in this loop, we just count specific embeddings
                    }
                }
                indexed = rows.len(); // Approximate: we processed this batch of articles
            }
            Err(e) => {
                tracing::error!("Ollama batch failed: {}", e);
                failed = rows.len();
                return Ok(Json(AutoIndexResponse {
                    success: false,
                    indexed: 0,
                    failed,
                    remaining: 0,
                    error: Some(format!("Ollama failed: {}", e)),
                }));
            }
        }
    }

    // Check remaining
    let remaining: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) 
        FROM articles a 
        WHERE NOT EXISTS (
            SELECT 1 FROM embeddings e 
            WHERE e.fakeid = a.fakeid AND e.aid = a.aid AND e.source = 'title'
        )
        "#,
    )
    .fetch_one(&pool)
    .await?;

    Ok(Json(AutoIndexResponse {
        success: true,
        indexed,
        failed,
        remaining: remaining.0 as usize,
        error: None,
    }))
}

// ============ AppState Wrapper Handlers ============

/// Store embeddings (AppState wrapper)
pub async fn store_handler(
    State(state): State<AppState>,
    body: Json<StoreRequest>,
) -> Result<Json<StoreResponse>, AppError> {
    store(State(state.db_pool), body).await
}

/// Search embeddings (AppState wrapper)
pub async fn search_handler(
    State(state): State<AppState>,
    body: Json<SearchRequest>,
) -> Result<Json<SearchResponse>, AppError> {
    search(State(state.db_pool), body).await
}

/// Get stats (AppState wrapper)
pub async fn stats_handler(State(state): State<AppState>) -> Result<Json<StatsResponse>, AppError> {
    stats(State(state.db_pool)).await
}

/// Clear embeddings (AppState wrapper)
pub async fn clear_handler(State(state): State<AppState>) -> Result<Json<ClearResponse>, AppError> {
    clear(State(state.db_pool)).await
}

/// Clean index (AppState wrapper)
pub async fn clean_handler(State(state): State<AppState>) -> Result<Json<CleanResponse>, AppError> {
    clean(State(state.db_pool)).await
}

/// Unindexed count (AppState wrapper)
pub async fn unindexed_count_handler(
    State(state): State<AppState>,
) -> Result<Json<UnindexedCountResponse>, AppError> {
    unindexed_count(State(state.db_pool)).await
}

/// Auto index (AppState wrapper)
pub async fn auto_index_handler(
    State(state): State<AppState>,
    body: Json<AutoIndexRequest>,
) -> Result<Json<AutoIndexResponse>, AppError> {
    auto_index(State(state.db_pool), body).await
}
