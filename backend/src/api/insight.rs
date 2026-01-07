use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use uuid::Uuid;

use crate::error::AppError;
use crate::AppState;

use rand::Rng;

const WECHAT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ============ Types ============

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct InsightTask {
    pub id: Uuid,
    pub prompt: String,
    pub status: String,
    pub keywords: Vec<String>,
    pub target_count: i32,
    pub processed_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
    pub completion_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct InsightArticle {
    pub id: Uuid,
    pub task_id: Uuid,
    pub title: String,
    pub url: String,
    pub account_name: Option<String>,
    pub account_fakeid: Option<String>, // Added field
    pub publish_time: Option<i64>,
    pub similarity: Option<f64>,
    pub insight: Option<String>,
    pub relevance_score: Option<f64>,
    pub created_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub prompt: String,
    pub target_count: Option<i32>,
    pub deepseek_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
    pub specific_account_fakeid: Option<String>,
    pub specific_account_name: Option<String>,
    // LLM Provider Configuration
    pub keyword_provider: Option<String>, // "gemini" or "deepseek"
    pub reasoning_provider: Option<String>, // "gemini" or "deepseek"
    pub embedding_provider: Option<String>, // "gemini" or "ollama"
    pub ollama_base_url: Option<String>,
    pub ollama_embedding_model: Option<String>,
    // Search Speed: "high" (0.5s), "medium" (1-2s), "low" (2-3s)
    pub search_speed: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateTaskResponse {
    pub id: Uuid,
}

// ============ Handlers ============

use regex::Regex;
use std::path::{Path as StdPath, PathBuf};

#[derive(Debug, Deserialize)]
pub struct ExportTaskRequest {
    pub task_id: Uuid,
    pub target_dir: String,
    pub format: String, // "markdown" or "pdf"
    pub proxies: Option<Vec<String>>,
    pub authorization: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ExportTaskResponse {
    pub success: bool,
    pub message: String,
}

pub async fn export_task(
    State(state): State<AppState>,
    Json(req): Json<ExportTaskRequest>,
) -> Result<Json<ExportTaskResponse>, AppError> {
    // 1. Fetch Task and Articles
    let task = sqlx::query_as::<_, InsightTask>("SELECT * FROM insight_tasks WHERE id = $1")
        .bind(req.task_id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(AppError::NotFound("Task not found".to_string()))?;

    let articles = sqlx::query_as::<_, InsightArticle>(
        "SELECT * FROM insight_articles WHERE task_id = $1 ORDER BY similarity DESC NULLS LAST",
    )
    .bind(req.task_id)
    .fetch_all(&state.db_pool)
    .await?;

    if articles.is_empty() {
        return Ok(Json(ExportTaskResponse {
            success: false,
            message: "No articles to export".to_string(),
        }));
    }

    // 2. Prepare Directory
    let safe_prompt = task
        .prompt
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "_");
    let export_dir = StdPath::new(&req.target_dir).join(format!(
        "{}_export_{}",
        safe_prompt,
        chrono::Utc::now().format("%Y%m%d%H%M")
    ));

    // Create export dir
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create directory: {}", e)))?;
    }

    // Create images dir
    let images_dir = export_dir.join("images");
    std::fs::create_dir_all(&images_dir)
        .map_err(|e| AppError::Internal(format!("Failed to create images directory: {}", e)))?;

    tracing::info!("Exporting task {} to {:?}", task.id, export_dir);

    // Sanitize proxies: remove trailing slashes
    let sanitized_proxies = if let Some(proxies) = &req.proxies {
        Some(
            proxies
                .iter()
                .map(|p| p.trim_end_matches('/').to_string())
                .collect::<Vec<_>>(),
        )
    } else {
        None
    };

    // 3. Process Articles
    // Build a single client for all requests (proxies are handled via URL rewriting now)
    let client = reqwest::Client::builder()
        .user_agent(WECHAT_USER_AGENT)
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build client: {}", e)))?;

    let mut summary_content = String::new();
    summary_content.push_str(&format!("Task Prompt: {}\n", task.prompt));
    summary_content.push_str(&format!("Target: {}\n", task.target_count));
    summary_content.push_str(&format!("Processed: {}\n", task.processed_count));
    summary_content.push_str(&format!("Keywords: {:?}\n\n", task.keywords));

    let total_articles = articles.len();

    // --- Parallel Processing Start ---
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;

    let shared_proxies = Arc::new(sanitized_proxies);
    let shared_auth = Arc::new(req.authorization.clone());
    let shared_export_dir = Arc::new(export_dir.clone());
    let shared_images_dir = Arc::new(images_dir.clone());
    let shared_format = Arc::new(req.format.clone());
    let shared_db_pool = state.db_pool.clone();

    let concurrency = if req.format == "pdf" {
        // PDF generation is heavy, but user has high-performance CPU
        10
    } else if let Some(p) = shared_proxies.as_ref() {
        if p.is_empty() {
            2
        } else {
            (p.len() / 2).clamp(3, 20)
        }
    } else {
        1
    };
    tracing::info!("Concurrency: {}", concurrency);

    let script_regex = Arc::new(Regex::new(r"(?s)<script[^>]*>.*?</script>").unwrap());
    let style_regex = Arc::new(Regex::new(r"(?s)<style[^>]*>.*?</style>").unwrap());
    let js_link_regex = Arc::new(
        Regex::new(r#"(?i)<a[^>]+href\s*=\s*["']javascript:[^"']*["'][^>]*>.*?</a>"#).unwrap(),
    );

    let tasks = stream::iter(articles.into_iter().enumerate()).map(|(i, article)| {
        let db_pool = shared_db_pool.clone();
        let client = client.clone();
        let proxies = shared_proxies.clone();
        let auth = shared_auth.clone();
        let export_dir = shared_export_dir.clone();
        let images_dir = shared_images_dir.clone();
        let fmt = shared_format.clone();
        let script_re = script_regex.clone();
        let style_re = style_regex.clone();
        let js_link_re = js_link_regex.clone();

        async move {
            tracing::info!(
                "Processing article {}/{}: {}",
                i + 1,
                total_articles,
                article.title
            );

            if i > 0 {
                let delay = rand::random::<u64>() % 900 + 100;
                tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
            }

            let mut log_entry = String::new();

            let gateway = if let Some(ps) = proxies.as_ref() {
                if !ps.is_empty() {
                    use rand::seq::SliceRandom;
                    let mut rng = rand::thread_rng();
                    ps.choose(&mut rng).map(|s| s.as_str())
                } else {
                    None
                }
            } else {
                None
            };

            let gateway_auth = auth.as_deref();

            log_entry.push_str(&format!("{}. {} ({})\n", i + 1, article.title, article.url));
            if let Some(insight) = &article.insight {
                log_entry.push_str(&format!("   Insight: {}\n", insight));
            }

            let url_hash = format!("{:x}", md5::compute(article.url.as_bytes()));
            let cached_content: Option<String> = sqlx::query_scalar("SELECT content FROM cached_articles WHERE url_hash = $1")
                .bind(&url_hash)
                .fetch_optional(&db_pool)
                .await
                .unwrap_or(None);

            let html_content = if let Some(content) = cached_content {
                log_entry.push_str("   [Cache] Hit\n");
                content
            } else {
                match fetch_html_content(&client, &article.url, gateway, gateway_auth).await {
                    Ok(c) => {
                        if c.trim().len() < 500 {
                            tracing::warn!(
                                "Content too short for {}: {} items",
                                article.title,
                                c.len()
                            );
                            log_entry.push_str("   [Error] Download failed: Content too short\n");
                            return (i, log_entry);
                        }

                        // Save to cache
                        let _ = sqlx::query("INSERT INTO cached_articles (url_hash, url, content, created_at) VALUES ($1, $2, $3, $4) ON CONFLICT (url_hash) DO NOTHING")
                            .bind(&url_hash)
                            .bind(&article.url)
                            .bind(&c)
                            .bind(chrono::Utc::now().timestamp())
                            .execute(&db_pool)
                            .await;

                        c
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch article {}: {}", article.url, e);
                        log_entry.push_str(&format!("   [Error] Download failed: {}\n", e));
                        return (i, log_entry);
                    }
                }
            };

            // Process Images & Content (Pass gateway info for image downloads)
            let (processed_html, _) = process_html_images(
                &client,
                &html_content,
                &images_dir,
                &article.id.to_string(),
                gateway,
                gateway_auth,
                &db_pool,
                false, // Revert to relative paths as requested
            )
            .await;

            let filename = format!(
                "{}_{}",
                i + 1,
                article
                    .title
                    .replace(|c: char| !c.is_alphanumeric() && c != ' ', "_")
            );

            if *fmt == "markdown" {
                let s1 = script_re.replace_all(&processed_html, "");
                let s2 = style_re.replace_all(&s1, "");
                let clean_html = js_link_re.replace_all(&s2, "");

                // Convert to Markdown
                let markdown_body = html2md::parse_html(&clean_html);
                let full_md = format!(
                    "---\ntitle: {}\nurl: {}\ndate: {}\n---\n\n# {}\n\n> Insight: {}\n\n{}",
                    article.title,
                    article.url,
                    article.publish_time.unwrap_or(0),
                    article.title,
                    article.insight.as_deref().unwrap_or(""),
                    markdown_body
                );

                let file_path = export_dir.join(format!("{}.md", filename));
                if let Err(e) = std::fs::write(&file_path, full_md) {
                    log_entry.push_str(&format!("   [Error] Write MD failed: {}\n", e));
                } else {
                    log_entry.push_str("   [Success] Markdown saved.\n");
                }
            } else {
                let pdf_html = processed_html;

                let file_path = export_dir.join(format!("{}.pdf", filename));
                if let Err(e) =
                    crate::api::pdf::convert_html_to_pdf(&pdf_html, &file_path, &article.title, Some(&export_dir))
                        .await
                {
                    log_entry.push_str(&format!("   [Error] PDF gen failed: {}\n", e));
                } else {
                    log_entry.push_str("   [Success] PDF generated.\n");
                }
            }

            (i, log_entry)
        }
    });

    let mut results: Vec<(usize, String)> = tasks.buffer_unordered(concurrency).collect().await;
    results.sort_by_key(|k| k.0);
    for (_, log) in results {
        summary_content.push_str(&log);
    }

    let _ = std::fs::write(export_dir.join("summary.txt"), summary_content);

    Ok(Json(ExportTaskResponse {
        success: true,
        message: format!("Export completed to {:?}", export_dir),
    }))
}

// Helper code to be inserted or appended later (fetch_html_content, process_html_images) or inlined.
// I will inline them inside this replacing block or ensure they exist.
// Wait, I can't define valid functions inside a handler block if I replace `// ============ Handlers ============`.
// I should better place the handler at the END of the file and include helpers.

// Reverting to adding imports at TOP and Handler at BOTTOM logic is tedious with specific line replacement.
// I'll assume I can add imports here (Rust allows inner imports but better at top).
// I'll put imports at the top of the function or try to add them to top of file in a separate call?
// No, I'll just put `use` inside the function or ignore if already imported. `regex` is external, needs careful handling.
// I will add the Handler at the END of `api/insight.rs`.
// And I will add `use regex::Regex;` to the top of the file in another step or just rely on `regex::Regex` if I added it to Cargo.toml.
// I'll use fully qualified `regex::Regex`.

#[derive(Debug, Deserialize)]
pub struct PrefetchTaskRequest {
    pub task_id: Uuid,
    pub proxies: Option<Vec<String>>,
    pub authorization: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct PrefetchStats {
    pub article_success: usize,
    pub article_failed: usize,
    pub image_success: usize,
    pub image_failed: usize,
}

#[derive(Debug, Serialize)]
pub struct PrefetchTaskResponse {
    pub success: bool,
    pub message: String,
    pub stats: PrefetchStats,
}

pub async fn prefetch_task(
    State(state): State<AppState>,
    Json(req): Json<PrefetchTaskRequest>,
) -> Result<Json<PrefetchTaskResponse>, AppError> {
    // 1. Fetch Task and Articles
    let _task = sqlx::query_as::<_, InsightTask>("SELECT * FROM insight_tasks WHERE id = $1")
        .bind(req.task_id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(AppError::NotFound("Task not found".to_string()))?;

    let articles = sqlx::query_as::<_, InsightArticle>(
        "SELECT * FROM insight_articles WHERE task_id = $1 ORDER BY similarity DESC NULLS LAST",
    )
    .bind(req.task_id)
    .fetch_all(&state.db_pool)
    .await?;

    let sanitized_proxies = if let Some(proxies) = req.proxies {
        Some(
            proxies
                .into_iter()
                .map(|p| p.trim_end_matches('/').to_string())
                .collect::<Vec<String>>(),
        )
    } else {
        None
    };

    // 2. Setup Concurrency
    use futures::stream::{self, StreamExt};
    use std::sync::Arc;

    let shared_proxies = Arc::new(sanitized_proxies);
    let shared_auth = Arc::new(req.authorization.clone());
    let shared_db_pool = state.db_pool.clone();

    // Compile regex once (Allow http, https, and protocol-relative)
    let img_regex = Arc::new(Regex::new(r#"(?i)(?:data-src|src)\s*=\s*["']((?:https?:)?//[^"']+)["']"#).unwrap());

    // Client for requests
    let client = reqwest::Client::builder()
        .user_agent(WECHAT_USER_AGENT)
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build client: {}", e)))?;

    let concurrency = if let Some(p) = shared_proxies.as_ref() {
        if p.is_empty() {
            2
        } else {
            (p.len() / 2).clamp(3, 20)
        }
    } else {
        1
    };

    tracing::info!("Prefetch Concurrency: {}", concurrency);

    let tasks = stream::iter(articles.into_iter().enumerate()).map(|(i, article)| {
        let db_pool = shared_db_pool.clone();
        let client = client.clone();
        let proxies = shared_proxies.clone();
        let auth = shared_auth.clone();
        let img_re = img_regex.clone();

        async move {
            let mut log_entry = String::new();
            let mut stats = PrefetchStats::default();
            log_entry.push_str(&format!("{}. {} ({})\n", i + 1, article.title, article.url));

            // --- A. Content Fetching ---
            let cached_content: Option<String> = sqlx::query_scalar("SELECT content FROM article_content WHERE id = $1")
                .bind(article.id)
                .fetch_optional(&db_pool)
                .await
                .unwrap_or(None);

            let html_content = if let Some(content) = cached_content {
                // Check word count threshold (> 500 chars)
                if content.trim().len() < 500 {
                    log_entry.push_str("   [Cache] Invalid (< 500 chars). Re-fetching...\n");
                    None
                } else {
                    log_entry.push_str("   [Cache] Hit\n");
                    Some(content)
                }
            } else {
                None
            };

            let html_content = if let Some(c) = html_content {
                stats.article_success += 1;
                c
            } else {
                // Fetch
                let gateway = if let Some(ps) = proxies.as_ref() {
                    if !ps.is_empty() {
                        use rand::seq::SliceRandom;
                        let mut rng = rand::thread_rng();
                        ps.choose(&mut rng).map(|s| s.as_str())
                    } else { None }
                } else { None };
                let gateway_auth = auth.as_deref();

                match fetch_html_content(&client, &article.url, gateway, gateway_auth).await {
                    Ok(c) => {
                        if c.trim().len() < 500 {
                            log_entry.push_str("   [Warning] Fetched content short < 500\n");
                        }
                        // Save to cache (article_content)
                        let _ = sqlx::query("INSERT INTO article_content (id, content, original_url, create_time) VALUES ($1, $2, $3, $4) ON CONFLICT (id) DO UPDATE SET content = EXCLUDED.content, create_time = EXCLUDED.create_time")
                            .bind(article.id)
                            .bind(&c)
                            .bind(&article.url)
                            .bind(chrono::Utc::now().timestamp())
                            .execute(&db_pool)
                            .await;
                        log_entry.push_str("   [Success] Fetched & Saved\n");
                        stats.article_success += 1;
                        c
                    }
                    Err(e) => {
                        log_entry.push_str(&format!("   [Error] Fetch failed: {}\n", e));
                        stats.article_failed += 1;
                        return (i, log_entry, stats); // Stop if main content fails
                    }
                }
            };

            // --- B. Image Prefetch & Compression ---
            let mut img_total = 0;
            let mut img_ok = 0;

            for cap in img_re.captures_iter(&html_content) {
                if let Some(url_match) = cap.get(1) {
                    let raw_url = url_match.as_str();
                    let img_url_string = if raw_url.starts_with("//") {
                        format!("https:{}", raw_url)
                    } else {
                        raw_url.to_string()
                    };
                    let img_url = img_url_string.as_str();
                    img_total += 1;

                    // Check assets table
                    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM assets WHERE url = $1)")
                        .bind(img_url)
                        .fetch_one(&db_pool)
                        .await
                        .unwrap_or(false);

                    if exists {
                        img_ok += 1;
                        continue;
                    }

                    // Download
                    let gateway = if let Some(ps) = proxies.as_ref() {
                        if !ps.is_empty() {
                            use rand::seq::SliceRandom;
                            let mut rng = rand::thread_rng();
                            ps.choose(&mut rng).map(|s| s.as_str())
                        } else { None }
                    } else { None };
                    let gateway_auth = auth.as_deref();

                    let final_url = if let Some(gw) = gateway {
                         let mut u = reqwest::Url::parse(gw).unwrap();
                         {
                            let mut p = u.query_pairs_mut();
                            p.append_pair("url", img_url);
                            if let Some(a) = gateway_auth { p.append_pair("authorization", a); }
                         }
                         u.to_string()
                    } else { img_url.to_string() };

                    match client.get(&final_url).send().await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                if let Ok(bytes) = resp.bytes().await {
                                    // Compress
                                    let compressed_data = if let Ok(img) = image::load_from_memory(&bytes) {
                                        // Resize if too large (max 1280 width)
                                        let img = if img.width() > 1280 {
                                            img.resize(1280, 1280 * img.height() / img.width(), image::imageops::FilterType::Lanczos3)
                                        } else {
                                            img
                                        };
                                        let mut comp_bytes: Vec<u8> = Vec::new();
                                        // Encode to JPEG q=75
                                        if let Ok(_) = img.write_to(&mut std::io::Cursor::new(&mut comp_bytes), image::ImageOutputFormat::Jpeg(75)) {
                                            comp_bytes
                                        } else {
                                            bytes.to_vec() // Fallback
                                        }
                                    } else {
                                        bytes.to_vec() // Fallback
                                    };

                                    // Store
                                    let _ = sqlx::query("INSERT INTO assets (url, data, mime_type, size, create_time) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (url) DO NOTHING")
                                        .bind(img_url)
                                        .bind(&compressed_data)
                                        .bind("image/jpeg")
                                        .bind(compressed_data.len() as i32)
                                        .bind(chrono::Utc::now().timestamp())
                                        .execute(&db_pool)
                                        .await;
                                    img_ok += 1;
                                }
                            }
                        }
                        Err(_) => {} // Ignore image failure
                    }
                }
            }
            stats.image_success = img_ok;
            stats.image_failed = img_total - img_ok;

            log_entry.push_str(&format!("   [Images] Processed {}/{} (Compressed)\n", img_ok, img_total));

            (i, log_entry, stats)
        }
    });

    let results: Vec<(usize, String, PrefetchStats)> =
        tasks.buffer_unordered(concurrency).collect().await;

    // Aggregation
    let mut total_stats = PrefetchStats::default();
    for (_, _, s) in results {
        total_stats.article_success += s.article_success;
        total_stats.article_failed += s.article_failed;
        total_stats.image_success += s.image_success;
        total_stats.image_failed += s.image_failed;
    }

    Ok(Json(PrefetchTaskResponse {
        success: true,
        message: format!("Prefetch completed."),
        stats: total_stats,
    }))
}

#[derive(Debug, Deserialize)]
pub struct DeleteTaskRequest {
    pub id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CancelTaskRequest {
    pub id: Uuid,
}

/// Delete a task and its articles
pub async fn delete_task(
    State(state): State<AppState>,
    Json(req): Json<DeleteTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Delete articles first due to FK
    sqlx::query("DELETE FROM insight_articles WHERE task_id = $1")
        .bind(req.id)
        .execute(&state.db_pool)
        .await?;

    // Delete task
    sqlx::query("DELETE FROM insight_tasks WHERE id = $1")
        .bind(req.id)
        .execute(&state.db_pool)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Cancel a running task
pub async fn cancel_task(
    State(state): State<AppState>,
    Json(req): Json<CancelTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query("UPDATE insight_tasks SET status = 'cancelling', updated_at = $1 WHERE id = $2")
        .bind(chrono::Utc::now().timestamp())
        .bind(req.id)
        .execute(&state.db_pool)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

/// Create a new insight task
pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<CreateTaskResponse>, AppError> {
    // Pre-validation: Check if WeChat session is valid before creating task
    let auth_key = get_valid_auth_key(&state)
        .await
        .ok_or_else(|| AppError::BadRequest("请先登录微信公众平台".to_string()))?;

    // Validate the session is actually working by making a simple API call
    if let Err(e) = validate_wechat_session(&state, &auth_key).await {
        return Err(AppError::BadRequest(format!(
            "微信登录已过期，请重新登录: {}",
            e
        )));
    }

    let task_id = Uuid::new_v4();
    let now = chrono::Utc::now().timestamp();
    let target = req.target_count.unwrap_or(30);

    // Insert task into DB
    sqlx::query(
        "INSERT INTO insight_tasks (id, prompt, status, keywords, target_count, processed_count, created_at, updated_at, completion_reason) VALUES ($1, $2, $3, $4::text[], $5, $6, $7, $8, $9)"
    )
    .bind(task_id)
    .bind(&req.prompt)
    .bind("pending") // Initial status
    .bind(&Vec::<String>::new())
    .bind(target)
    .bind(0)
    .bind(now)
    .bind(now)
    .bind(Option::<String>::None) // completion_reason starts as None
    .execute(&state.db_pool)
    .await?;

    // Spawn background worker
    let state_clone = state.clone();
    let prompt_clone = req.prompt.clone();
    let deepseek_key = req.deepseek_api_key.clone();
    let gemini_key = req.gemini_api_key.clone();
    let target_count = target;
    let specific_fakeid = req.specific_account_fakeid.clone();
    let specific_name = req.specific_account_name.clone();
    // LLM Provider Config
    let keyword_provider = req
        .keyword_provider
        .clone()
        .unwrap_or_else(|| "gemini".to_string());
    let reasoning_provider = req
        .reasoning_provider
        .clone()
        .unwrap_or_else(|| "gemini".to_string());
    let embedding_provider = req
        .embedding_provider
        .clone()
        .unwrap_or_else(|| "gemini".to_string());
    let ollama_base_url = req.ollama_base_url.clone();
    let ollama_embedding_model = req.ollama_embedding_model.clone();
    let search_speed = req.search_speed.clone().unwrap_or_else(|| "medium".to_string());

    tokio::spawn(async move {
        if let Err(e) = process_task(
            state_clone,
            task_id,
            prompt_clone,
            target_count,
            deepseek_key,
            gemini_key,
            specific_fakeid,
            specific_name,
            keyword_provider,
            reasoning_provider,
            embedding_provider,
            ollama_base_url,
            ollama_embedding_model,
            search_speed,
        )
        .await
        {
            tracing::error!("Task {} failed: {}", task_id, e);
            // Update status to failed
            let log_path = std::env::current_dir()
                .unwrap_or_default()
                .join("logs")
                .join("wechat_insights.log");
            let reason = format!("Unexpected Error: {}. Log: {:?}", e, log_path);
            let _ = update_task_status(&state.clone(), task_id, "failed", Some(reason)).await;
        }
    });

    Ok(Json(CreateTaskResponse { id: task_id }))
}

/// List all tasks
pub async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<InsightTask>>, AppError> {
    let tasks =
        sqlx::query_as::<_, InsightTask>("SELECT * FROM insight_tasks ORDER BY created_at DESC")
            .fetch_all(&state.db_pool)
            .await?;

    Ok(Json(tasks))
}

/// Get task details and articles
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = sqlx::query_as::<_, InsightTask>("SELECT * FROM insight_tasks WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db_pool)
        .await?
        .ok_or(AppError::NotFound("Task not found".to_string()))?;

    let articles = sqlx::query_as::<_, InsightArticle>(
        "SELECT * FROM insight_articles WHERE task_id = $1 ORDER BY similarity DESC NULLS LAST",
    )
    .bind(id)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(Json(serde_json::json!({
        "task": task,
        "articles": articles
    })))
}

// ============ Worker Logic ============

async fn update_task_status(
    state: &AppState,
    id: Uuid,
    status: &str,
    reason: Option<String>,
) -> anyhow::Result<()> {
    if let Some(r) = reason {
        sqlx::query("UPDATE insight_tasks SET status = $1, updated_at = $2, completion_reason = $3 WHERE id = $4")
            .bind(status)
            .bind(chrono::Utc::now().timestamp())
            .bind(r)
            .bind(id)
            .execute(&state.db_pool)
            .await?;
    } else {
        sqlx::query("UPDATE insight_tasks SET status = $1, updated_at = $2 WHERE id = $3")
            .bind(status)
            .bind(chrono::Utc::now().timestamp())
            .bind(id)
            .execute(&state.db_pool)
            .await?;
    }
    Ok(())
}

async fn is_task_cancelled(state: &AppState, id: Uuid) -> anyhow::Result<bool> {
    let status: String = sqlx::query_scalar("SELECT status FROM insight_tasks WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db_pool)
        .await?;
    Ok(status == "cancelling" || status == "cancelled")
}

async fn process_task(
    state: AppState,
    task_id: Uuid,
    prompt: String,
    target_count: i32,
    deepseek_key: Option<String>,
    gemini_key: Option<String>,
    specific_fakeid: Option<String>,
    specific_name: Option<String>,
    keyword_provider: String,
    reasoning_provider: String,
    embedding_provider: String,
    ollama_base_url: Option<String>,
    ollama_embedding_model: Option<String>,
    search_speed: String,
) -> anyhow::Result<()> {
    tracing::info!(
        "Starting processing for task: {} (keyword:{}, reasoning:{}, embedding:{})",
        task_id,
        keyword_provider,
        reasoning_provider,
        embedding_provider
    );
    update_task_status(&state, task_id, "processing", None).await?;

    // Dynamic Scaling Configuration
    let (keyword_count, account_limit, article_limit) = if target_count <= 50 {
        (10, 20, 20) // Fast mode for small targets
    } else if target_count <= 200 {
        (15, 30, 30) // Medium mode
    } else {
        (20, 50, 50) // Heavy mode for large targets (e.g. 2000)
    };

    tracing::info!(
        "Task {}: Scaling config - Keywords: {}, Accounts: {}, Articles: {}",
        task_id,
        keyword_count,
        account_limit,
        article_limit
    );

    // 1. Determine Search Space
    let accounts_to_scan = if let (Some(fakeid), Some(nickname)) = (specific_fakeid, specific_name)
    {
        // Mode A: Specific Account Targeting
        if is_task_cancelled(&state, task_id).await? {
            update_task_status(
                &state,
                task_id,
                "cancelled",
                Some("Cancelled by user".to_string()),
            )
            .await?;
            return Ok(());
        } // Clean exit

        tracing::info!(
            "Task {}: Targeting specific account: {} ({})",
            task_id,
            nickname,
            fakeid
        );
        vec![AccountInfo { fakeid, nickname }]
    } else {
        // Mode B: Keyword Discovery
        // 1. Generate Keywords (DeepSeek)
        if is_task_cancelled(&state, task_id).await? {
            update_task_status(
                &state,
                task_id,
                "cancelled",
                Some("Cancelled by user".to_string()),
            )
            .await?;
            return Ok(());
        }

        let keywords = generate_keywords(&keyword_provider, &prompt, keyword_count, deepseek_key.as_deref(), gemini_key.as_deref()).await?;
        tracing::info!("Task {}: Generated keywords: {:?}", task_id, keywords);

        sqlx::query("UPDATE insight_tasks SET keywords = $1 WHERE id = $2")
            .bind(&keywords)
            .bind(task_id)
            .execute(&state.db_pool)
            .await?;

        // 2. Discover Accounts
        let auth_key = get_valid_auth_key(&state)
            .await
            .ok_or(anyhow::anyhow!("No valid WeChat login session found"))?;

        let mut discovered_accounts = Vec::new();
        // Simple deduplication
        let mut seen_fakeids = std::collections::HashSet::new();

        for keyword in keywords {
            if is_task_cancelled(&state, task_id).await? {
                update_task_status(
                    &state,
                    task_id,
                    "cancelled",
                    Some("Cancelled by user".to_string()),
                )
                .await?;
                return Ok(());
            }

            if is_task_cancelled(&state, task_id).await? {
                update_task_status(
                    &state,
                    task_id,
                    "cancelled",
                    Some("Cancelled by user".to_string()),
                )
                .await?;
                return Ok(());
            }

            // Rate Limiting: delay based on search_speed setting
            let delay = match search_speed.as_str() {
                "high" => rand::thread_rng().gen_range(400..=600),   // 0.4-0.6s (high risk)
                "medium" => rand::thread_rng().gen_range(1000..=2000), // 1-2s (medium risk)
                "low" | _ => rand::thread_rng().gen_range(2000..=3000), // 2-3s (low risk, default)
            };
            tracing::info!(
                "Task {}: Waiting {}ms before searching keyword '{}' (speed: {})",
                task_id,
                delay,
                keyword,
                search_speed
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

            if is_task_cancelled(&state, task_id).await? {
                update_task_status(
                    &state,
                    task_id,
                    "cancelled",
                    Some("Cancelled by user".to_string()),
                )
                .await?;
                return Ok(());
            }

            // Robustness: Handle search errors gracefully
            let accounts =
                match search_accounts(&state, &auth_key, &keyword, account_limit as u32).await {
                    Ok(accs) => accs,
                    Err(e) => {
                        tracing::error!(
                            "Task {}: Search failed for keyword '{}': {}",
                            task_id,
                            keyword,
                            e
                        );
                        continue; // Skip this keyword
                    }
                };

            for acc in accounts {
                if !seen_fakeids.contains(&acc.fakeid) {
                    seen_fakeids.insert(acc.fakeid.clone());
                    discovered_accounts.push(acc);
                }
            }
        }
        discovered_accounts
    };

    // 2. Prepare for Scanning
    let auth_key = get_valid_auth_key(&state)
        .await
        .ok_or(anyhow::anyhow!("No valid WeChat login session found"))?;

    // Generate prompt embedding using configured provider
    let prompt_embedding = generate_embedding_configurable(
        &embedding_provider,
        gemini_key.as_deref(),
        ollama_base_url.as_deref(),
        ollama_embedding_model.as_deref(),
        &prompt,
    )
    .await?;

    if prompt_embedding.is_empty() {
        return Err(anyhow::anyhow!("Embedding generation failed"));
    }

    let mut unique_urls = std::collections::HashSet::new();
    let mut article_count = 0;

    // Safety break to prevent infinite loops if we can't find enough relevant articles
    // Increased limit to support large target counts (e.g. 1000)
    let max_scan_limit = (target_count * 50).min(100000).max(1000);
    let mut scanned_count = 0;

    for account in accounts_to_scan {
        if article_count >= target_count {
            break;
        }
        if scanned_count >= max_scan_limit {
            break;
        }
        if is_task_cancelled(&state, task_id).await? {
            tracing::info!("Task {} cancelled by user", task_id);
            update_task_status(
                &state,
                task_id,
                "cancelled",
                Some("User Cancelled".to_string()),
            )
            .await?;
            return Ok(());
        }

        // Reuse inner logic
        let account = account; // Rebind for clarity matching previous logic context if needed

        if article_count >= target_count {
            break;
        }
        if scanned_count >= max_scan_limit {
            break;
        }
        // if unique_urls.len() >= 50 { break; } // REMOVED global limit

        let fakeid = account.fakeid;

        // Rate Limiting: 2~5s delay before fetching articles
        let delay = rand::thread_rng().gen_range(2000..=5000);
        tracing::info!(
            "Task {}: Waiting {}ms before fetching articles for '{}'",
            task_id,
            delay,
            account.nickname
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;

        tracing::info!(
            "Task {}: Fetching articles for account {} ({})",
            task_id,
            account.nickname,
            fakeid
        );

        // Robustness: Retry mechanism for fetching articles
        let mut articles = Vec::new();
        let mut fetch_attempts = 0;
        while fetch_attempts < 3 {
            match fetch_account_articles(&state, &auth_key, &fakeid, article_limit as u32).await {
                Ok(res) => {
                    articles = res;
                    break;
                }
                Err(e) => {
                    fetch_attempts += 1;
                    tracing::warn!(
                        "Task {}: Fetch articles failed for {} (Attempt {}/3): {}",
                        task_id,
                        account.nickname,
                        fetch_attempts,
                        e
                    );
                    if fetch_attempts < 3 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            2000 * fetch_attempts as u64,
                        ))
                        .await;
                    }
                }
            }
        }

        if articles.is_empty() && fetch_attempts >= 3 {
            tracing::error!(
                "Task {}: Failed to fetch articles for {} after 3 attempts. Skipping.",
                task_id,
                account.nickname
            );
            continue;
        }
        tracing::info!(
            "Task {}: Fetched {} articles from {}",
            task_id,
            articles.len(),
            account.nickname
        );

        for article in articles {
            if article_count >= target_count {
                break;
            }
            if unique_urls.contains(&article.url) {
                continue;
            }

            // Deep check cancellations per article if needed (optional, maybe overkill to check PER article)
            // But good for responsiveness
            if scanned_count % 5 == 0 {
                if is_task_cancelled(&state, task_id).await? {
                    tracing::info!("Task {} cancelled by user", task_id);
                    update_task_status(
                        &state,
                        task_id,
                        "cancelled",
                        Some("User Cancelled".to_string()),
                    )
                    .await?;
                    return Ok(());
                }
            }

            unique_urls.insert(article.url.clone());
            scanned_count += 1;

            let text_to_embed = format!("{} {}", article.title, article.digest);
            let embedding = match generate_embedding_configurable(
                &embedding_provider,
                gemini_key.as_deref(),
                ollama_base_url.as_deref(),
                ollama_embedding_model.as_deref(),
                &text_to_embed,
            )
            .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "Task {}: Failed to embed article '{}': {}",
                        task_id,
                        article.title,
                        e
                    );
                    continue;
                }
            };

            let similarity = cosine_similarity(&prompt_embedding, &embedding);
            tracing::info!(
                "Task {}: Article '{}' similarity: {:.4}",
                task_id,
                article.title,
                similarity
            );

            if similarity > 0.4 {
                // ... generation & filtering logic ...
                // Retry mechanism for robustness
                let mut attempts = 0;
                let mut success = false;
                let mut is_relevant = false;
                let mut insight = String::new();

                while attempts < 3 {
                    match generate_insight(
                        &reasoning_provider,
                        &prompt,
                        &article.title,
                        &article.digest,
                        deepseek_key.as_deref(),
                        gemini_key.as_deref(),
                    )
                    .await
                    {
                        Ok((rel, ins)) => {
                            is_relevant = rel;
                            insight = ins;
                            success = true;
                            break;
                        }
                        Err(e) => {
                            attempts += 1;
                            tracing::warn!(
                                "Task {}: generate_insight failed for '{}' (attempt {}/3): {}",
                                task_id,
                                article.title,
                                attempts,
                                e
                            );
                            if attempts < 3 {
                                tokio::time::sleep(tokio::time::Duration::from_millis(
                                    2000 * attempts as u64,
                                ))
                                .await;
                            }
                        }
                    }
                }

                if !success {
                    tracing::error!("Task {}: Failed to generate insight for article '{}' after 3 attempts. Skipping.", task_id, article.title);
                    continue; // Skip this article, do NOT fail the task
                }

                // let (is_relevant, insight) = ... (Removed)

                if !is_relevant {
                    tracing::info!(
                        "Task {}: Article '{}' filtered as IRRELEVANT by AI.",
                        task_id,
                        article.title
                    );
                    continue;
                }

                let id = Uuid::new_v4();
                sqlx::query(
                         "INSERT INTO insight_articles (id, task_id, title, url, account_name, account_fakeid, publish_time, similarity, insight, relevance_score, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"
                     )
                     .bind(id)
                     .bind(task_id)
                     .bind(&article.title)
                     .bind(&article.url)
                     .bind(&account.nickname)
                     .bind(&fakeid) // Save fakeid
                     .bind(article.create_time)
                     .bind(similarity)
                     .bind(&insight)
                     .bind(0.8)
                     .bind(chrono::Utc::now().timestamp())
                     .execute(&state.db_pool)
                     .await?;

                article_count += 1;

                sqlx::query("UPDATE insight_tasks SET processed_count = $1 WHERE id = $2")
                    .bind(article_count)
                    .bind(task_id)
                    .execute(&state.db_pool)
                    .await?;
            }
        }
    } // End accounts_to_scan loop

    // Determine final reason
    let reason = if article_count >= target_count {
        format!("Target Reached ({}/{})", article_count, target_count)
    } else if scanned_count >= max_scan_limit {
        format!("Max Scan Limit Reached ({})", scanned_count)
    } else {
        "All Keywords Searched".to_string()
    };

    update_task_status(&state, task_id, "completed", Some(reason)).await?;
    tracing::info!(
        "Task {} completed. Total articles: {} (Scanned: {})",
        task_id,
        article_count,
        scanned_count
    );
    Ok(())
}

// ============ Helpers ============

// Simple cosine similarity
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        (dot_product / (norm_a * norm_b)) as f64
    }
}

async fn get_valid_auth_key(state: &AppState) -> Option<String> {
    // Return the most recently created valid auth key (not expired, ordered by created_at DESC)
    let now = chrono::Utc::now().timestamp();
    let row = sqlx::query(
        "SELECT auth_key FROM cookies WHERE expires_at > $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(now)
    .fetch_optional(&state.db_pool)
    .await
    .ok()??;

    Some(row.get(0))
}

/// Validate WeChat session by making a simple API call
async fn validate_wechat_session(state: &AppState, auth_key: &str) -> anyhow::Result<()> {
    let token = state
        .cookie_store
        .get_token(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Token not found"))?;
    let cookie = state
        .cookie_store
        .get_cookie(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Cookie not found"))?;
    let cookie_str = cookie.to_cookie_header();

    // Make a simple search request to validate session
    let client = reqwest::Client::builder().no_proxy().build()?;
    let resp = client
        .get("https://mp.weixin.qq.com/cgi-bin/searchbiz")
        .query(&[
            ("action", "search_biz"),
            ("begin", "0"),
            ("count", "1"),
            ("query", "test"),
            ("token", &token),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Cookie", cookie_str)
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?;

    let text = resp.text().await?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("JSON parse error: {}", e))?;

    // Check for session error
    if let Some(ret) = json
        .get("base_resp")
        .and_then(|r| r.get("ret"))
        .and_then(|v| v.as_i64())
    {
        if ret != 0 {
            let msg = json
                .get("base_resp")
                .and_then(|r| r.get("err_msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(anyhow::anyhow!("Session invalid ({}): {}", ret, msg));
        }
    }

    Ok(())
}

#[derive(Debug)]
struct AccountInfo {
    fakeid: String,
    nickname: String,
}

#[derive(Debug)]
struct SimpleArticle {
    title: String,
    digest: String,
    url: String,
    create_time: i64,
}

async fn search_accounts(
    state: &AppState,
    auth_key: &str,
    keyword: &str,
    limit: u32,
) -> anyhow::Result<Vec<AccountInfo>> {
    // NOTE: This duplicates logic from web.rs, ideally refactor.
    // For now, implementing specialized client logic.
    let token = state
        .cookie_store
        .get_token(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Token not found"))?;
    let cookie = state
        .cookie_store
        .get_cookie(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Cookie not found"))?;
    let cookie_str = cookie.to_cookie_header();
    let count_str = limit.to_string();

    let client = reqwest::Client::builder().no_proxy().build()?;
    let resp = client
        .get("https://mp.weixin.qq.com/cgi-bin/searchbiz")
        .query(&[
            ("action", "search_biz"),
            ("begin", "0"),
            ("count", &count_str),
            ("query", keyword),
            ("token", &token),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Cookie", cookie_str)
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?;

    let text = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("WeChat Search Biz JSON Error: {} | Body: {}", e, text))?;

    // Check for base_resp error
    if let Some(ret) = json
        .get("base_resp")
        .and_then(|r| r.get("ret"))
        .and_then(|v| v.as_i64())
    {
        if ret != 0 {
            let msg = json
                .get("base_resp")
                .and_then(|r| r.get("err_msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            tracing::error!("WeChat Search Biz Error: ret={} msg={}", ret, msg);
            return Err(anyhow::anyhow!("WeChat Search Error ({}): {}", ret, msg));
        }
    }

    let mut accounts = Vec::new();
    if let Some(list) = json.get("list").and_then(|l| l.as_array()) {
        for item in list {
            if let (Some(fakeid), Some(nickname)) = (
                item.get("fakeid").and_then(|s| s.as_str()),
                item.get("nickname").and_then(|s| s.as_str()),
            ) {
                accounts.push(AccountInfo {
                    fakeid: fakeid.to_string(),
                    nickname: nickname.to_string(),
                });
            }
        }
    }
    Ok(accounts)
}

async fn fetch_account_articles(
    state: &AppState,
    auth_key: &str,
    fakeid: &str,
    limit: u32,
) -> anyhow::Result<Vec<SimpleArticle>> {
    let token = state
        .cookie_store
        .get_token(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Token not found"))?;
    let cookie = state
        .cookie_store
        .get_cookie(auth_key)
        .await?
        .ok_or(anyhow::anyhow!("Cookie not found"))?;
    let cookie_str = cookie.to_cookie_header();
    let count_str = limit.to_string();

    let client = reqwest::Client::builder().no_proxy().build()?;
    let resp = client
        .get("https://mp.weixin.qq.com/cgi-bin/appmsgpublish")
        .query(&[
            ("sub", "list"),
            ("search_field", "null"),
            ("begin", "0"),
            ("count", &count_str),
            ("fakeid", fakeid),
            ("type", "101_1"),
            ("token", &token),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Cookie", cookie_str)
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?;

    let text = resp.text().await?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("WeChat Article Fetch JSON Error: {} | Body: {}", e, text))?;

    // Check for base_resp error
    if let Some(ret) = json
        .get("base_resp")
        .and_then(|r| r.get("ret"))
        .and_then(|v| v.as_i64())
    {
        if ret != 0 {
            let msg = json
                .get("base_resp")
                .and_then(|r| r.get("err_msg"))
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            tracing::warn!(
                "WeChat Article Fetch Error for fakeid {}: ret={} msg={}",
                fakeid,
                ret,
                msg
            );
            // Don't fail the whole task for one account failure, but log it.
            return Ok(vec![]);
        }
    }

    let mut articles = Vec::new();

    // publish_page is returned as a JSON string, we need to parse it first
    let publish_page_json = if let Some(s) = json.get("publish_page").and_then(|s| s.as_str()) {
        serde_json::from_str::<serde_json::Value>(s).ok()
    } else {
        None
    };

    if let Some(page_obj) = publish_page_json {
        if let Some(list) = page_obj.get("publish_list").and_then(|l| l.as_array()) {
            for item in list {
                if let Some(info_str) = item.get("publish_info").and_then(|s| s.as_str()) {
                    // Safe to clean html entities?
                    let clean_info = info_str.replace("&quot;", "\""); // Basic unescape
                    if let Ok(info) = serde_json::from_str::<serde_json::Value>(&clean_info) {
                        // 1. Get create_time from sent_info (shared for this push)
                        let shared_time = info
                            .get("sent_info")
                            .and_then(|s| s.get("time"))
                            .and_then(|v| v.as_f64())
                            .map(|f| f as i64)
                            .unwrap_or(0);

                        // 2. Try appmsg_info (Primary now)
                        if let Some(appmsg_info) =
                            info.get("appmsg_info").and_then(|l| l.as_array())
                        {
                            for msg in appmsg_info {
                                if let (Some(title), Some(url)) = (
                                    msg.get("title").and_then(|s| s.as_str()),
                                    msg.get("content_url").and_then(|s| s.as_str()),
                                ) {
                                    let digest = msg
                                        .get("digest")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    articles.push(SimpleArticle {
                                        title: title.to_string(),
                                        digest,
                                        url: url.replace("\\", ""), // clean escaped slashes if any
                                        create_time: shared_time,
                                    });
                                }
                            }
                        }
                        // 3. Fallback to appmsgex (Old format)
                        else if let Some(appmsg_list) =
                            info.get("appmsgex").and_then(|l| l.as_array())
                        {
                            for msg in appmsg_list {
                                if let (Some(title), Some(digest), Some(link), Some(create_time)) = (
                                    msg.get("title").and_then(|s| s.as_str()),
                                    msg.get("digest").and_then(|s| s.as_str()),
                                    msg.get("link").and_then(|s| s.as_str()),
                                    msg.get("create_time")
                                        .and_then(|v| v.as_f64())
                                        .map(|f| f as i64),
                                ) {
                                    articles.push(SimpleArticle {
                                        title: title.to_string(),
                                        digest: digest.to_string(),
                                        url: link.to_string(),
                                        create_time,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Debug log only if empty (can remove later)
    if articles.is_empty() {
        tracing::debug!(
            "Fetched 0 articles for fakeid {}. JSON Response: {:?}",
            fakeid,
            json
        );
    }

    Ok(articles)
}

// ============ LLM Logic (DeepSeek & Gemini) ============

/// Configurable embedding generation - dispatches to Gemini or Ollama based on provider
async fn generate_embedding_configurable(
    provider: &str,
    gemini_key: Option<&str>,
    ollama_base_url: Option<&str>,
    ollama_model: Option<&str>,
    text: &str,
) -> anyhow::Result<Vec<f32>> {
    match provider.to_lowercase().as_str() {
        "ollama" => {
            crate::llm::ollama::generate_embedding(
                ollama_base_url.unwrap_or("http://127.0.0.1:11434"),
                ollama_model.unwrap_or("qwen3-embedding:8b-q8_0"),
                text,
            )
            .await
        }
        "gemini" | _ => {
            let api_key = gemini_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Gemini API Key required for embedding"))?;
            crate::llm::gemini::generate_embedding(&api_key, text).await
        }
    }
}

async fn generate_keywords(
    provider: &str,
    prompt: &str,
    count: usize,
    deepseek_key: Option<&str>,
    gemini_key: Option<&str>,
) -> anyhow::Result<Vec<String>> {
    let sys_prompt = format!("You are a keyword generator helper. The user needs to search for WeChat Official Accounts. \n\
    Generate {} search keywords based on the user's topic. \n\
    Output specific, short terms (e.g. '不良资产', '债权处置'). \n\
    \n\
    IMPORTANT: You must return a valid JSON object in this format: \n\
    {{ \"keywords\": [\"keyword1\", \"keyword2\"] }}", count);

    // Common JSON parsing logic
    fn parse_keywords(text: &str) -> anyhow::Result<Vec<String>> {
         let json: serde_json::Value = serde_json::from_str(text).map_err(|e| {
            anyhow::anyhow!("JSON Parse Error: {} | Body: {}", e, text)
        })?;

        // Handle DeepSeek/Gemini structure differences if needed, but usually we just want the content
        // DeepSeek: choices[0].message.content
        // Gemini: candidates[0].content.parts[0].text
        
        let content = if let Some(c) = json.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|m| m.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str()) {
                c.to_string()
        } else if let Some(c) = json.get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|parts| parts.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|t| t.get("text"))
            .and_then(|s| s.as_str()) {
                c.to_string()
        } else {
            return Err(anyhow::anyhow!("Unknown JSON structure or empty content"));
        };

        let clean_content = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```");

        #[derive(serde::Deserialize)]
        struct KeywordsResp {
            keywords: Vec<String>,
        }

        let resp_obj: KeywordsResp = serde_json::from_str(clean_content).map_err(|e| {
            anyhow::anyhow!("Content Parse Error: {} | Content: {}", e, clean_content)
        })?;
        Ok(resp_obj.keywords)
    }

    match provider.to_lowercase().as_str() {
        "gemini" => {
             let api_key = gemini_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Gemini API Key required for keywords"))?;

            let client = reqwest::Client::new();
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
                api_key
            );
            
            let full_prompt = format!("{}\n\nUser Topic: {}", sys_prompt, prompt);

            let mut attempt = 0;
            while attempt < 5 {
                attempt += 1;
                let resp = client
                    .post(&url)
                    .json(&serde_json::json!({
                        "contents": [{"parts": [{"text": full_prompt}]}],
                         "generationConfig": { "response_mime_type": "application/json" }
                    }))
                    .send()
                    .await;

                match resp {
                    Ok(r) => {
                        if r.status().is_success() {
                            let text = r.text().await?;
                            return parse_keywords(&text);
                        } else {
                             tracing::warn!("Gemini API Error (Attempt {}/5): Status {}", attempt, r.status());
                        }
                    }
                    Err(e) => tracing::warn!("Gemini Network Error (Attempt {}/5): {}", attempt, e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            Err(anyhow::anyhow!("Gemini API failed after 5 attempts"))
        }
        "deepseek" | _ => {
            let api_key = deepseek_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("DeepSeek API Key not found"))?;

            let client = reqwest::Client::new();
            let mut attempt = 0;
            while attempt < 5 {
                attempt += 1;
                let resp = client
                    .post("https://api.deepseek.com/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&serde_json::json!({
                         "model": "deepseek-chat",
                         "messages": [
                             {"role": "system", "content": &sys_prompt},
                             {"role": "user", "content": format!("Topic: {}", prompt)}
                         ],
                         "temperature": 0.3,
                         "response_format": { "type": "json_object" }
                    }))
                    .send()
                    .await;
                
                  match resp {
                    Ok(r) => {
                        if r.status().is_success() {
                            let text = r.text().await?;
                            return parse_keywords(&text);
                        } else {
                             tracing::warn!("DeepSeek API Error (Attempt {}/5): Status {}", attempt, r.status());
                        }
                    }
                    Err(e) => tracing::warn!("DeepSeek Network Error (Attempt {}/5): {}", attempt, e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
             Err(anyhow::anyhow!("DeepSeek API failed after 5 attempts"))
        }
    }
}

async fn generate_insight(
    provider: &str,
    intent: &str,
    title: &str,
    digest: &str,
    deepseek_key: Option<&str>,
    gemini_key: Option<&str>,
) -> anyhow::Result<(bool, String)> {
     let user_prompt = format!(
        "Intent: {}\n\nArticle Title: {}\nDigest: {}\n\nEvaluate if this article is RELEVANT to the Intent. \n\
        STRICT RULES: \n\
        1. If it is an advertisement, course promotion (training camp, free lessons), or selling anxiety, MARK AS FALSE (is_relevant: false).\n\
        2. If it is a simple notification, recruitment info, or low-value content, MARK AS FALSE.\n\
        3. Only mark as TRUE if it provides substantive knowledge, analysis, or industry insights.\n\
        If relevant, provide a concise insight (2-3 sentences max) in Simplified Chinese. \n\
        Return JSON ONLY: {{ \"is_relevant\": boolean, \"insight\": \"string\" }}", 
        intent, title, digest
    );

    // Common Parsing Logic
    fn parse_insight(text: &str) -> anyhow::Result<(bool, String)> {
        let json: serde_json::Value = serde_json::from_str(text).map_err(|e| {
            anyhow::anyhow!("JSON Error: {} | Body: {}", e, text)
        })?;

        // Extract content depending on structure
        let content = if let Some(c) = json.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|m| m.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|s| s.as_str()) {
                c.to_string()
        } else if let Some(c) = json.get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|parts| parts.get("parts"))
            .and_then(|p| p.get(0))
            .and_then(|t| t.get("text"))
            .and_then(|s| s.as_str()) {
            c.to_string()
        } else {
             // Try parsing the root if it's already the object (unlikely for API response but safety)
             return Err(anyhow::anyhow!("Unknown JSON structure"));
        };

        let clean_text = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```");

        #[derive(serde::Deserialize)]
        struct InsightResp {
            is_relevant: bool,
            insight: String,
        }

        let parsed: InsightResp = serde_json::from_str(clean_text).unwrap_or(InsightResp {
            is_relevant: false,
            insight: "Failed to parse AI response".to_string(),
        });
        Ok((parsed.is_relevant, parsed.insight))
    }

    match provider.to_lowercase().as_str() {
        "deepseek" => {
               let api_key = deepseek_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("DeepSeek API Key required"))?;
            
            let client = reqwest::Client::new();
            let mut attempt = 0;
             while attempt < 5 {
                attempt += 1;
                let resp = client
                    .post("https://api.deepseek.com/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&serde_json::json!({
                        "model": "deepseek-chat",
                        "messages": [{"role": "user", "content": user_prompt}],
                        "temperature": 0.2, // Lower temp for classification
                        "response_format": { "type": "json_object" }
                    }))
                    .send()
                    .await;
                
                  match resp {
                    Ok(r) => {
                        if r.status().is_success() {
                            let text = r.text().await?;
                            return parse_insight(&text);
                        } else {
                             tracing::warn!("DeepSeek Insight API Error (Attempt {}/5): Status {}", attempt, r.status());
                        }
                    }
                    Err(e) => tracing::warn!("DeepSeek Insight Network Error (Attempt {}/5): {}", attempt, e),
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
             Err(anyhow::anyhow!("DeepSeek API failed after 5 attempts"))
        },
        "gemini" | _ => {
            // Use Gemini
            let api_key = gemini_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("GEMINI_API_KEY").ok())
                .ok_or_else(|| anyhow::anyhow!("Gemini API Key not found"))?;

            let client = reqwest::Client::new();
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
                api_key
            );

            let mut attempt = 0;
            while attempt < 5 {
                attempt += 1;
                let response_result = client
                    .post(&url)
                    .json(&serde_json::json!({
                        "contents": [{"parts": [{"text": user_prompt}]}],
                        "generationConfig": { "response_mime_type": "application/json" }
                    }))
                    .send()
                    .await;

                match response_result {
                    Ok(response) => {
                        if response.status().is_success() {
                            let body_text = response.text().await?;
                            return parse_insight(&body_text);
                        } else {
                            tracing::warn!("Gemini Insight API Error (Attempt {}/5): Status={}", attempt, response.status());
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Gemini Network Error (Attempt {}/5): {}", attempt, e);
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
            Err(anyhow::anyhow!("Gemini Insight API failed after 5 attempts"))
        }

    }
}

// Export Helpers
async fn fetch_html_content(
    client: &reqwest::Client,
    target_url: &str,
    gateway: Option<&str>,
    gateway_auth: Option<&str>,
) -> anyhow::Result<String> {
    let final_url = if let Some(gw) = gateway {
        // Construct Gateway URL: gw?url=encoded_target&authorization=auth
        let mut url =
            reqwest::Url::parse(gw).map_err(|e| anyhow::anyhow!("Invalid gateway URL: {}", e))?;

        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("url", target_url);
            if let Some(auth) = gateway_auth {
                pairs.append_pair("authorization", auth);
            }
        }
        url.to_string()
    } else {
        target_url.to_string()
    };

    let mut attempt = 0;
    loop {
        attempt += 1;
        match client.get(&final_url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    return Ok(resp.text().await?);
                } else if attempt >= 3 {
                    return Err(anyhow::anyhow!(
                        "Request failed with status: {}",
                        resp.status()
                    ));
                }
            }
            Err(e) => {
                if attempt >= 3 {
                    return Err(e.into());
                }
                // Retry on timeout, connection error, or body decoding error
                if e.is_timeout()
                    || e.is_connect()
                    || e.is_request()
                    || e.is_body()
                    || e.is_decode()
                {
                    tracing::warn!(
                        "Fetch error for {} (Attempt {}/3): {}",
                        final_url,
                        attempt,
                        e
                    );
                } else {
                    return Err(e.into()); // Other errors might be fatal
                }
            }
        }
        // Simple backoff
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
}

pub async fn process_html_images(
    client: &reqwest::Client,
    html: &str,
    images_dir: &StdPath,
    _prefix: &str,
    gateway: Option<&str>,
    gateway_auth: Option<&str>,
    db_pool: &sqlx::PgPool,
    use_absolute_paths: bool, // Kept for API compatibility, but effectively ignored if using base64 logic below (I will repurpose this or add new arg)
    // Actually, I should just repurpose `use_absolute_paths` -> `embed_base64` or add a new arg.
    // To minimize signature changes in call sites I haven't seen, let's overload `use_absolute_paths`.
    // If true, we will use Base64. If false, we use relative paths.
    // Wait, PDF export passed `true`. Batch export passed `false`.
    // Perfect. PDF needs Base64. Batch export needs relative paths to files.
    // So `use_absolute_paths` == true -> generate Base64.
) -> (String, Vec<PathBuf>) {

    let mut processed_html = html.to_string();
    let mut downloaded_images = Vec::new();

    // 1. Unhide content
    let style_regex = Regex::new(r#"style="[^"]*visibility:\s*hidden[^"]*""#).unwrap();
    processed_html = style_regex.replace_all(&processed_html, "").to_string();

    // 2. Normalize data-src to src just in case (Case-insensitive)
    let data_src_regex = Regex::new(r#"(?i)data-src\s*="#).unwrap();
    processed_html = data_src_regex.replace_all(&processed_html, "src=").to_string();

    // 3. Brute-force find ALL WeChat image URLs (ignoring tags/attributes)
    //    WeChat URLs have complex query strings with various encodings.
    //    Instead of trying to enumerate all characters, match EVERYTHING
    //    from mmbiz.qpic.cn until we hit a quote (") or (') or whitespace.
    //    Pattern: (https?:)?//mmbiz\.qpic\.cn/[^"'\s]+
    //    This is greedy and will capture the entire URL no matter what it contains.
    let url_regex = Regex::new(r#"(?:https?:)?//mmbiz\.qpic\.cn/[^\"'\s]+"#).unwrap();
    
    let mut replacements: Vec<(String, PathBuf, String, Option<String>)> = Vec::new();
    let mut seen_urls = std::collections::HashSet::new();

    for cap in url_regex.captures_iter(&processed_html) {
        if let Some(match_str) = cap.get(0) {
            let raw_url = match_str.as_str();
            
            // Normalize URL
            let mut url = raw_url.to_string();
            if url.starts_with("//") {
                url = format!("https:{}", url);
            }
            
            // Dedup
            if seen_urls.contains(&url) { continue; }
            seen_urls.insert(url.clone());

            // Generate filename
            // Default to jpg, but check url for hints
            let ext = if url.contains("wx_fmt=png") { "png" } 
                      else if url.contains("wx_fmt=gif") { "gif" }
                      else if url.contains("wx_fmt=webp") { "webp" }
                      else { "jpg" };
            
            let filename = format!("{}.{}", Uuid::new_v4(), ext);
            let file_path = images_dir.join(&filename);
            let rel_path = format!("images/{}", filename);

            // We must replace the RAW string found in HTML, not the normalized URL
            replacements.push((raw_url.to_string(), file_path, rel_path, None)); 
        }
    }
    
    tracing::info!("Brute-force scan found {} unique WeChat images", replacements.len());

    // Import futures
    use base64::Engine;
    use futures::stream::{self, StreamExt};

    tracing::info!("Starting parallel download for {} images...", replacements.len());

    let download_futures = stream::iter(replacements).map(|(target_url, file_path, rel_path, _)| {
        let client = client.clone();
        let gateway = gateway.map(|s| s.to_string());
        let gateway_auth = gateway_auth.map(|s| s.to_string());
        let db_pool = db_pool.clone();
        let should_embed = use_absolute_paths; // Reuse flag: true = embed base64

        async move {
            let mut image_data: Option<Vec<u8>> = None;
            // Decode URL and Normalize
            let decoded_url = html_escape::decode_html_entities(&target_url).to_string();
            let dl_url = if decoded_url.starts_with("//") {
                format!("https:{}", decoded_url)
            } else {
                decoded_url
            };
            tracing::info!("Processing image: {}", dl_url);

            // A. Check Cache (Use NORMALIZED URL)
            let cached: Option<Vec<u8>> = sqlx::query_scalar("SELECT data FROM assets WHERE url = $1")
                .bind(&dl_url) 
                .fetch_optional(&db_pool)
                .await
                .unwrap_or(None);
            
            // Validate cache quality: must be > 100 bytes and look like an image
            if let Some(data) = cached {
                if data.len() > 100 && (
                   data.starts_with(&[0xff, 0xd8, 0xff]) || // JPG
                   data.starts_with(&[0x89, 0x50, 0x4e, 0x47]) || // PNG
                   data.starts_with(b"GIF8") || // GIF
                   (data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP") // WebP
                ) {
                     image_data = Some(data);
                } else {
                     tracing::warn!("Invalid/Corrupted cache for {}, triggering re-download.", target_url);
                }
            }
            
            if image_data.is_none() {
                // B. Download
                 let final_url = if let Some(gw) = gateway {
                    let mut url = reqwest::Url::parse(&gw).unwrap_or(reqwest::Url::parse("http://err").unwrap());
                    {
                        let mut p = url.query_pairs_mut();
                        p.append_pair("url", &dl_url);
                        if let Some(a) = &gateway_auth { p.append_pair("authorization", a); }
                    }
                    url.to_string()
                } else { dl_url.clone() };

                // Retry loop (3 attempts)
                for i in 0..3 {
                    // Add Referer header which is often required by WeChat images
                    // Add User-Agent and Accept to look like a browser
                    match client.get(&final_url)
                        .header("Referer", "https://mp.weixin.qq.com/")
                        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                        .header("Accept", "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8")
                        .send().await 
                    {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                if let Ok(bytes) = resp.bytes().await {
                                    image_data = Some(bytes.to_vec());
                                    break; 
                                }
                            } else {
                                tracing::warn!("Image download failed (status {}): {}", resp.status(), target_url);
                            }
                        }
                        Err(e) => {
                             tracing::warn!("Image download network error (attempt {}): {} - {}", i+1, target_url, e);
                        }
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }

            if let Some(data) = &image_data {
                // Determine mime based on magic bytes
                let mime_type = if data.starts_with(&[0xff, 0xd8, 0xff]) { "image/jpeg" }
                                else if data.starts_with(&[0x89, 0x50, 0x4e, 0x47]) { "image/png" }
                                else if data.starts_with(b"GIF8") { "image/gif" }
                                else if data.len() > 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" { "image/webp" }
                                else { "application/octet-stream" };

                // Determine extension correction if needed?
                // The filename was already generated with an extension based on URL.
                // If URL was wrong, filename might be .jpg but content is .webp.
                // Prince handles mismatches reasonably well, but let's stick to no transcoding.

                // Cache the fresh download using NORMALIZED URL
                 let _ = sqlx::query("INSERT INTO assets (url, data, mime_type) VALUES ($1, $2, $3) ON CONFLICT (url) DO UPDATE SET data = $2, mime_type = $3")
                    .bind(&dl_url)
                    .bind(data)
                    .bind(mime_type) 
                    .bind("application/octet-stream")
                    .execute(&db_pool).await;

                // Always write to file for batch export consistency (or just backup)
                if !data.is_empty() {
                    match std::fs::write(&file_path, data) {
                        Ok(_) => tracing::info!("Wrote image to file: {:?} (size: {})", file_path, data.len()),
                        Err(e) => tracing::error!("Failed to write image file {:?}: {}", file_path, e),
                    }
                } else {
                     tracing::warn!("Skipping file write for empty data: {:?}", file_path);
                }
                
                let replacement_str = if should_embed {
                    // Base64 logic (Disabled for batch by User request, but kept for Single Export)
                     let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                     format!("data:{};base64,{}", mime_type, b64)
                } else {
                    // Use absolute file:// path for Prince to find the image
                    let abs_path = file_path.canonicalize().unwrap_or(file_path.clone());
                    let path_str = abs_path.display().to_string().replace("\\", "/");
                    // Ensure exactly 3 slashes: file:///path (Unix) or file:///C:/path (Windows)
                    // If path already starts with /, use file:// + path, else file:/// + path
                    if path_str.starts_with("/") {
                        format!("file://{}", path_str)
                    } else {
                        format!("file:///{}", path_str)
                    }
                };

                Some((target_url, rel_path, file_path, replacement_str))
            } else {
                tracing::error!("Failed to acquire image after retries: {}", target_url);
                None
            }
        }
    });

    let results: Vec<Option<(String, String, PathBuf, String)>> =
        download_futures.buffer_unordered(15).collect().await;

    let mut success_count = 0;
    for res in results {
        if let Some((target_url, _, file_path, replacement)) = res {
            downloaded_images.push(file_path); // Track downloaded files
            
            // Log the replacement to see if it is Base64 or File URL
            if replacement.len() > 200 {
                 // Use char-safe truncation to avoid panic on multi-byte chars
                 let truncated: String = replacement.chars().take(100).collect();
                 tracing::info!("Image replacement (trunc): {}...", truncated);
            } else {
                 tracing::info!("Image replacement: {}", replacement);
            }

            processed_html = processed_html.replace(&target_url, &replacement);
            success_count += 1;
        }
    }
    tracing::info!("Processed images: {}/{}", success_count, downloaded_images.len());

    (processed_html, downloaded_images)
}
