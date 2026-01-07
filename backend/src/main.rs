//! WeChat Article Insights - Rust Backend
//!
//! A high-performance backend for semantic search and WeChat API proxy.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    routing::{get, post},
    Router,
};
use clap::Parser;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod api;
mod cookie;
mod db;
mod error;
mod llm;
mod proxy;

use cookie::CookieStore;

/// Command line arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Enable debug level logging
    #[arg(long, default_value_t = false)]
    debug: bool,
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub cookie_store: Arc<CookieStore>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Determine log level based on --debug flag
    let log_level = if args.debug { "debug" } else { "info" };
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    // Initialize logging (File + Stdout)
    let file_appender = tracing_appender::rolling::daily("logs", "wechat_insights.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false), // File output matches plain text
        )
        .with(
            tracing_subscriber::fmt::layer().with_writer(std::io::stdout), // Keep stdout for dev
        )
        .with(env_filter)
        .init();

    tracing::info!("Log level: {}", log_level);

    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize database
    let db_pool = db::init_db().await?;

    // Startup Cleanup: Reset any tasks stuck in processing/cancelling state
    tracing::info!("Cleaning up stuck tasks...");
    sqlx::query(
        "UPDATE insight_tasks SET status = 'failed' WHERE status IN ('processing', 'cancelling')",
    )
    .execute(&db_pool)
    .await?;

    // Initialize cookie store
    let cookie_store = CookieStore::new(db_pool.clone());
    cookie_store.init().await?;

    // Cleanup expired sessions on startup
    let cleaned = cookie_store.cleanup_expired().await?;
    if cleaned > 0 {
        tracing::info!("Cleaned up {} expired session(s)", cleaned);
    }

    // Create app state
    let app_state = AppState {
        db_pool: db_pool.clone(),
        cookie_store: Arc::new(cookie_store),
    };

    // Setup CORS - Allow credentials by mirroring request origin
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::PATCH,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::COOKIE]);

    // Build router
    let app = Router::new()
        // ============ Embedding API ============
        .route("/api/embedding/generate", post(api::embedding::generate))
        .route("/api/embedding/batch", post(api::embedding::batch))
        .route("/api/embedding/store", post(api::embedding::store_handler))
        .route(
            "/api/embedding/search",
            post(api::embedding::search_handler),
        )
        .route("/api/embedding/stats", get(api::embedding::stats_handler))
        .route("/api/embedding/clear", post(api::embedding::clear_handler))
        .route("/api/embedding/clean", post(api::embedding::clean_handler))
        .route(
            "/api/embedding/unindexed_count",
            get(api::embedding::unindexed_count_handler),
        )
        .route(
            "/api/embedding/auto_index",
            post(api::embedding::auto_index_handler),
        )
        // ============ Public API v1 ============
        .route("/api/public/v1/account", get(api::public::search_account))
        .route("/api/account/add", post(api::public::add_account)) // New endpoint for Insight "Add to Monitor"
        .route(
            "/api/public/v1/accounts/db",
            get(api::public::get_db_accounts),
        ) // New DB-backed endpoint
        .route("/api/public/v1/article", get(api::public::get_articles))
        .route(
            "/api/public/v1/article/fetch",
            post(api::public::fetch_article),
        ) // New fetch endpoint
        .route(
            "/api/public/v1/articles/db",
            get(api::public::get_db_articles),
        ) // New DB-backed article list
        .route(
            "/api/public/v1/download",
            get(api::public::download_article),
        )
        .route("/api/public/v1/html", get(api::public::get_article_html))
        .route("/api/public/v1/asset", get(api::public::get_asset))
        .route("/api/public/v1/comments", get(api::public::get_comments))
        .route("/api/public/v1/authkey", get(api::public::get_auth_key))
        // ============ Web Login API ============
        .route(
            "/api/web/login/session/:sid",
            post(api::web::start_login_session),
        )
        .route("/api/web/login/getqrcode", get(api::web::get_qrcode))
        .route("/api/web/login/scan", get(api::web::check_scan))
        .route("/api/web/login/bizlogin", post(api::web::biz_login))
        .route("/api/web/mp/info", get(api::web::get_mp_info))
        .route("/api/web/mp/logout", get(api::web::logout))
        .route("/api/web/mp/searchbiz", get(api::web::mp_searchbiz))
        .route("/api/web/mp/appmsgpublish", get(api::web::mp_appmsgpublish))
        .route(
            "/api/web/misc/appmsgalbum",
            get(api::web::mp_appmsgalbum_proxy),
        )
        // ============ Web Misc API ============
        .route("/api/web/misc/status", get(api::web::misc_status))
        .route("/api/web/misc/accountname", get(api::web::misc_accountname))
        .route("/api/web/misc/comment", get(api::web::misc_comment))
        // ============ LLM API ============
        .route("/api/llm/test", post(api::llm::test_connection))
        .route(
            "/api/llm/test-ollama",
            post(api::llm::test_ollama_connection),
        )
        // ============ Insight API ============
        .route("/api/insight/create", post(api::insight::create_task))
        .route("/api/insight/list", get(api::insight::list_tasks))
        .route("/api/insight/cancel", post(api::insight::cancel_task))
        .route("/api/insight/delete", post(api::insight::delete_task))
        .route("/api/insight/export", post(api::insight::export_task))
        .route("/api/insight/prefetch", post(api::insight::prefetch_task))
        .route("/api/insight/:id", get(api::insight::get_task))
        // ============ PDF API ============
        .route("/api/pdf", post(api::pdf::generate_pdf))
        // ============ Health Check ============
        .route("/health", get(|| async { "OK" }))
        .layer(cors)
        .with_state(app_state)
        // Increase body limit to 300MB for large batch embedding uploads
        // 10,000 items * 4096 dimensions * 4 bytes = ~160MB raw data
        .layer(DefaultBodyLimit::max(300 * 1024 * 1024));

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
