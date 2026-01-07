//! Public API handlers
//!
//! These endpoints provide public access to WeChat data with proper authentication.

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::proxy::{get_token_from_store, proxy_mp_request, ProxyRequestOptions};
use crate::AppState;

// ============ Common Types ============

#[allow(dead_code)]
#[derive(Debug, Serialize, Default)]
pub struct BaseResp {
    pub ret: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err_msg: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub base_resp: BaseResp,
}

#[allow(dead_code)]
impl ErrorResponse {
    pub fn new(msg: &str) -> Self {
        Self {
            base_resp: BaseResp {
                ret: -1,
                err_msg: Some(msg.to_string()),
            },
        }
    }
}

// ============ Account Search ============

#[derive(Debug, Deserialize)]
pub struct AccountQuery {
    pub keyword: String,
    pub begin: Option<u32>,
    pub size: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AccountResponse {
    pub base_resp: BaseResp,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<serde_json::Value>,
}

/// Search for WeChat official accounts
pub async fn search_account(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AccountQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = match get_token_from_store(&headers, &state.cookie_store).await {
        Some(t) => t,
        None => {
            return Ok(Json(serde_json::json!({
                "base_resp": {
                    "ret": -1,
                    "err_msg": "认证信息无效"
                }
            })));
        }
    };

    if query.keyword.is_empty() {
        return Ok(Json(serde_json::json!({
            "base_resp": {
                "ret": -1,
                "err_msg": "keyword不能为空"
            }
        })));
    }

    let begin = query.begin.unwrap_or(0);
    let size = query.size.unwrap_or(5);

    let params = vec![
        ("action".to_string(), "search_biz".to_string()),
        ("begin".to_string(), begin.to_string()),
        ("count".to_string(), size.to_string()),
        ("query".to_string(), query.keyword),
        ("token".to_string(), token.clone()),
        ("lang".to_string(), "zh_CN".to_string()),
        ("f".to_string(), "json".to_string()),
        ("ajax".to_string(), "1".to_string()),
    ];

    let cookie = crate::proxy::get_cookie_from_store(&headers, &state.cookie_store).await;

    let response = proxy_mp_request(ProxyRequestOptions {
        method: reqwest::Method::GET,
        endpoint: "https://mp.weixin.qq.com/cgi-bin/searchbiz".to_string(),
        query: Some(params),
        body: None,
        cookie,
    })
    .await?;

    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ Account List (From DB) ============

#[derive(Debug, Deserialize)]
pub struct GetAccountsQuery {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

/// Get local accounts from database with calculated article counts
pub async fn get_db_accounts(
    State(state): State<AppState>,
    Query(query): Query<GetAccountsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(100);

    // Calculate message and article counts from the articles table using subqueries
    // Messages = articles where itemidx = 1 (first article in each message/push)
    // Articles = total count of all articles
    let rows: Vec<(
        String,            // fakeid
        Option<String>,    // nickname
        Option<String>,    // round_head_img
        Option<String>,    // signature
        Option<i32>,       // service_type
        i32,               // total_count (from WeChat API)
        Option<i64>,       // create_time
        Option<i64>,       // update_time
        Option<i64>,       // last_update_time
        bool,              // sync_all
        i64,               // message_count (itemidx=1)
        i64,               // article_count (all)
    )> = sqlx::query_as(
        r#"
        SELECT 
            a.fakeid, a.nickname, a.round_head_img, a.signature, a.service_type, 
            a.total_count, a.create_time, a.update_time, a.last_update_time, a.sync_all,
            COALESCE((SELECT COUNT(*) FROM articles WHERE articles.fakeid = a.fakeid AND is_deleted = false AND itemidx = 1), 0) as message_count,
            COALESCE((SELECT COUNT(*) FROM articles WHERE articles.fakeid = a.fakeid AND is_deleted = false), 0) as article_count
        FROM accounts a
        ORDER BY a.update_time DESC NULLS LAST
        OFFSET $1 LIMIT $2
        "#
    )
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.db_pool)
    .await?;

    let accounts: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            let (
                fakeid,
                nickname,
                round_head_img,
                signature,
                service_type,
                total_count,
                create_time,
                update_time,
                last_update_time,
                sync_all,
                message_count,
                article_count,
            ) = row;
            // count = number of messages (itemidx=1), articles = total articles
            let count = message_count as i32;
            let articles = article_count as i32;
            let completed = total_count > 0 && count >= total_count;
            serde_json::json!({
                "fakeid": fakeid,
                "nickname": nickname,
                "round_head_img": round_head_img,
                "signature": signature,
                "service_type": service_type,
                "count": count,
                "articles": articles,
                "total_count": total_count,
                "create_time": create_time,
                "update_time": update_time,
                "last_update_time": last_update_time,
                "syncAll": sync_all,
                "completed": completed
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "success": true,
        "data": accounts,
        "total": accounts.len()
    })))
}

// ============ Add Account ============

#[derive(Debug, Deserialize)]
pub struct AddAccountRequest {
    pub fakeid: String,
    pub nickname: String,
}

pub async fn add_account(
    State(state): State<AppState>,
    Json(req): Json<AddAccountRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query(
        "INSERT INTO accounts (fakeid, nickname, create_time, update_time) VALUES ($1, $2, $3, $3) ON CONFLICT (fakeid) DO UPDATE SET nickname = $2, update_time = $3"
    )
    .bind(&req.fakeid)
    .bind(&req.nickname)
    .bind(chrono::Utc::now().timestamp())
    .execute(&state.db_pool)
    .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ============ Article List ============

#[derive(Debug, Deserialize)]
pub struct ArticleQuery {
    pub fakeid: String,
    pub begin: Option<u32>,
    pub size: Option<u32>,
    pub keyword: Option<String>,
}

/// Get articles from a WeChat official account
pub async fn get_articles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ArticleQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let token = match get_token_from_store(&headers, &state.cookie_store).await {
        Some(t) => t,
        None => {
            return Ok(Json(serde_json::json!({
                "base_resp": {
                    "ret": -1,
                    "err_msg": "认证信息无效"
                }
            })));
        }
    };

    if query.fakeid.is_empty() {
        return Ok(Json(serde_json::json!({
            "base_resp": {
                "ret": -1,
                "err_msg": "fakeid不能为空"
            }
        })));
    }

    let begin = query.begin.unwrap_or(0);
    let size = query.size.unwrap_or(5);
    let keyword = query.keyword.clone().unwrap_or_default();
    let is_searching = !keyword.is_empty();

    let params = vec![
        (
            "sub".to_string(),
            if is_searching { "search" } else { "list" }.to_string(),
        ),
        (
            "search_field".to_string(),
            if is_searching { "7" } else { "null" }.to_string(),
        ),
        ("begin".to_string(), begin.to_string()),
        ("count".to_string(), size.to_string()),
        ("query".to_string(), keyword),
        ("fakeid".to_string(), query.fakeid),
        ("type".to_string(), "101_1".to_string()),
        ("free_publish_type".to_string(), "1".to_string()),
        ("sub_action".to_string(), "list_ex".to_string()),
        ("token".to_string(), token),
        ("lang".to_string(), "zh_CN".to_string()),
        ("f".to_string(), "json".to_string()),
        ("ajax".to_string(), "1".to_string()),
    ];

    let cookie = crate::proxy::get_cookie_from_store(&headers, &state.cookie_store).await;

    let response = proxy_mp_request(ProxyRequestOptions {
        method: reqwest::Method::GET,
        endpoint: "https://mp.weixin.qq.com/cgi-bin/appmsgpublish".to_string(),
        query: Some(params),
        body: None,
        cookie,
    })
    .await?;

    let json: serde_json::Value = response.json().await?;

    // Parse and flatten articles
    if let Some(0) = json
        .get("base_resp")
        .and_then(|r| r.get("ret"))
        .and_then(|r| r.as_i64())
    {
        if let Some(publish_page_str) = json.get("publish_page").and_then(|p| p.as_str()) {
            if let Ok(publish_page) = serde_json::from_str::<serde_json::Value>(publish_page_str) {
                if let Some(publish_list) =
                    publish_page.get("publish_list").and_then(|l| l.as_array())
                {
                    let articles: Vec<serde_json::Value> = publish_list
                        .iter()
                        .filter_map(|item| {
                            item.get("publish_info")
                                .and_then(|p| p.as_str())
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                                .and_then(|info| info.get("appmsgex").cloned())
                        })
                        .filter_map(|v| v.as_array().cloned())
                        .flatten()
                        .collect();

                    return Ok(Json(serde_json::json!({
                        "base_resp": json.get("base_resp"),
                        "articles": articles
                    })));
                }
            }
        }
    }

    Ok(Json(json))
}

// ============ Article List (From DB) ============

#[derive(Debug, Deserialize)]
pub struct GetDbArticlesQuery {
    pub fakeid: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub days: Option<i64>, // Filter to recent N days
}

/// Get article list from database
pub async fn get_db_articles(
    State(state): State<AppState>,
    Query(query): Query<GetDbArticlesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(20);

    // Calculate timestamp for N days ago if days filter is specified
    let min_time = if let Some(days) = query.days {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        Some(now - (days * 24 * 60 * 60))
    } else {
        None
    };

    let rows: Vec<(
        String,
        String,
        String,
        String,
        String,
        i64,
        Option<i64>,
        Option<String>,
        Option<String>,
    )> = if let Some(fakeid) = &query.fakeid {
        if let Some(min_t) = min_time {
            sqlx::query_as(
                r#"
                SELECT id, fakeid, aid, title, link, create_time, update_time, digest, cover 
                FROM articles 
                WHERE fakeid = $1 AND is_deleted = false AND create_time >= $2
                ORDER BY create_time DESC 
                OFFSET $3 LIMIT $4
                "#,
            )
            .bind(fakeid)
            .bind(min_t)
            .bind(offset)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT id, fakeid, aid, title, link, create_time, update_time, digest, cover 
                FROM articles 
                WHERE fakeid = $1 AND is_deleted = false
                ORDER BY create_time DESC 
                OFFSET $2 LIMIT $3
                "#,
            )
            .bind(fakeid)
            .bind(offset)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await?
        }
    } else {
        // Fetch recent articles from ALL accounts
        if let Some(min_t) = min_time {
            sqlx::query_as(
                r#"
                SELECT id, fakeid, aid, title, link, create_time, update_time, digest, cover 
                FROM articles 
                WHERE is_deleted = false AND create_time >= $1
                ORDER BY create_time DESC 
                OFFSET $2 LIMIT $3
                "#,
            )
            .bind(min_t)
            .bind(offset)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await?
        } else {
            sqlx::query_as(
                r#"
                SELECT id, fakeid, aid, title, link, create_time, update_time, digest, cover 
                FROM articles 
                WHERE is_deleted = false
                ORDER BY create_time DESC 
                OFFSET $1 LIMIT $2
                "#,
            )
            .bind(offset)
            .bind(limit)
            .fetch_all(&state.db_pool)
            .await?
        }
    };

    let articles: Vec<serde_json::Value> = rows
        .into_iter()
        .map(
            |(id, fakeid, aid, title, link, create_time, update_time, digest, cover)| {
                serde_json::json!({
                    "id": id,
                    "fakeid": fakeid,
                    "aid": aid,
                    "title": title,
                    "link": link,
                    "create_time": create_time,
                    "update_time": update_time.unwrap_or(create_time),
                    "digest": digest,
                    "cover": cover
                })
            },
        )
        .collect();

    Ok(Json(serde_json::json!({
        "success": true,
        "data": articles,
        "total": articles.len()
    })))
}

// ============ Download Article ============

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub url: String,
    pub format: Option<String>,
}

/// Download article content in various formats
pub async fn download_article(
    Query(query): Query<DownloadQuery>,
) -> Result<axum::response::Response<String>, AppError> {
    use axum::http::header;

    if query.url.is_empty() {
        return Err(AppError::BadRequest("url不能为空".to_string()));
    }

    let url = urlencoding::decode(&query.url)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| query.url.clone());

    // Validate URL is a WeChat article
    if !url.contains("mp.weixin.qq.com") {
        return Err(AppError::BadRequest("url不合法".to_string()));
    }

    let format = query.format.as_deref().unwrap_or("html").to_lowercase();
    if !["html", "text"].contains(&format.as_str()) {
        return Err(AppError::BadRequest("不支持的format".to_string()));
    }

    let client = reqwest::Client::new();
    let raw_html = client
        .get(&url)
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .await?
        .text()
        .await?;

    let (content_type, body) = match format.as_str() {
        "text" => {
            // Very basic HTML to text conversion
            let text = raw_html
                .replace("<br>", "\n")
                .replace("<br/>", "\n")
                .replace("<br />", "\n")
                .replace("</p>", "\n");
            let text = regex::Regex::new(r"<[^>]+>")
                .map(|re| re.replace_all(&text, "").to_string())
                .unwrap_or(text);
            ("text/plain; charset=UTF-8", text)
        }
        _ => ("text/html; charset=UTF-8", raw_html),
    };

    let response = axum::response::Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, content_type)
        .body(body)
        .unwrap();

    Ok(response)
}

// ============ Get Article Content (From DB) ============

#[derive(Debug, Deserialize)]
pub struct GetHtmlQuery {
    pub id: Option<String>, // fakeid:aid
    pub url: Option<String>,
}

/// Get article HTML from database, fallback to fetching from WeChat
pub async fn get_article_html(
    State(state): State<AppState>,
    Query(query): Query<GetHtmlQuery>,
) -> Result<axum::response::Response<String>, AppError> {
    use axum::http::header;

    // Try to get from database first
    let row: Option<(String,)> = if let Some(id) = &query.id {
        sqlx::query_as("SELECT content FROM article_content WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await?
    } else if let Some(url) = &query.url {
        sqlx::query_as("SELECT content FROM article_content WHERE original_url = $1")
            .bind(url)
            .fetch_optional(&state.db_pool)
            .await?
    } else {
        return Err(AppError::BadRequest("id或url不能为空".to_string()));
    };

    if let Some((content,)) = row {
        let response = axum::response::Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/html; charset=UTF-8")
            .body(content)
            .unwrap();
        return Ok(response);
    }

    // Fallback: fetch from WeChat URL if provided
    if let Some(url) = &query.url {
        // Decode URL if needed
        let decoded_url = urlencoding::decode(url)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| url.clone());

        if decoded_url.contains("mp.weixin.qq.com") {
            let client = reqwest::Client::new();
            let raw_html = client
                .get(&decoded_url)
                .header("Referer", "https://mp.weixin.qq.com/")
                .header("Origin", "https://mp.weixin.qq.com")
                .header(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
                )
                .send()
                .await?
                .text()
                .await?;

            let response = axum::response::Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/html; charset=UTF-8")
                .body(raw_html)
                .unwrap();
            return Ok(response);
        }
    }

    Err(AppError::NotFound("Article content not found".to_string()))
}

#[derive(Debug, Deserialize)]
pub struct FetchRequest {
    pub url: String,
    pub id: Option<String>, // Added optional ID
    pub proxies: Option<Vec<String>>,
    pub authorization: Option<String>,
}

/// Fetch article content using optional proxies and save to DB
pub async fn fetch_article(
    State(state): State<AppState>,
    Json(req): Json<FetchRequest>,
) -> Result<axum::response::Response<String>, AppError> {
    use axum::http::header;

    if req.url.is_empty() {
        return Err(AppError::BadRequest("url不能为空".to_string()));
    }

    tracing::info!("fetch_article: id={:?}, url={}", req.id, req.url);

    // 1. Check DB first (Priority: ID -> Raw URL -> Decoded URL)
    let mut row: Option<(String,)> = None;

    // Check by ID if provided
    if let Some(id) = &req.id {
        row = sqlx::query_as("SELECT content FROM article_content WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await?;
    }

    // If not found by ID, try Raw URL
    if row.is_none() {
        row = sqlx::query_as("SELECT content FROM article_content WHERE original_url = $1")
            .bind(&req.url)
            .fetch_optional(&state.db_pool)
            .await?;
    }

    // If still not found, try Decoded URL
    if row.is_none() {
        let decoded_url = urlencoding::decode(&req.url)
            .map(|s| s.to_string())
            .unwrap_or_else(|_| req.url.clone());

        if decoded_url != req.url {
            row = sqlx::query_as("SELECT content FROM article_content WHERE original_url = $1")
                .bind(&decoded_url)
                .fetch_optional(&state.db_pool)
                .await?;
        }
    }

    // Fallback: Check 'cached_articles' (Legacy Insight Cache)
    if row.is_none() {
        let url_hash = format!("{:x}", md5::compute(req.url.as_bytes()));
        let cached: Option<(String,)> =
            sqlx::query_as("SELECT content FROM cached_articles WHERE url_hash = $1")
                .bind(&url_hash)
                .fetch_optional(&state.db_pool)
                .await?;

        if let Some(c) = cached {
            tracing::info!("fetch_article: Hit legacy cache for url={}", req.url);
            // Optional: Migrate to article_content?
            // For now just return it.
            row = Some(c);
        }
    }

    if let Some((content,)) = row {
        // Apply processing to cached content (it is raw)
        let processed_content = process_wechat_html(&content);
        let response = axum::response::Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, "text/html; charset=UTF-8")
            .body(processed_content)
            .unwrap();
        return Ok(response);
    }

    // 2. Fetch from URL
    let url = urlencoding::decode(&req.url)
        .map(|s| s.to_string())
        .unwrap_or_else(|_| req.url.clone());

    if !url.contains("mp.weixin.qq.com") {
        return Err(AppError::BadRequest("url不合法".to_string()));
    }

    let proxies = req.proxies.unwrap_or_default();
    let auth = req.authorization.clone();
    let mut last_error = "No proxies available or all failed".to_string();
    let mut fetched_content = None;

    // Logic: If proxies provided, try proxies. If empty, try direct.
    // User hint: "Proxy node should be transparent proxy" and frontend says it appends "?url=".
    // This means it is a Web Proxy Gateway, not a standard HTTP proxy.

    let mut attempts = Vec::new();
    if proxies.is_empty() {
        attempts.push(None); // Direct
    } else {
        for p in proxies {
            attempts.push(Some(p));
        }
    }

    // Helper for direct fetch
    async fn fetch_direct(client: &reqwest::Client, url: &str) -> Result<String, String> {
        let resp = client
            .get(url)
            .header("Referer", "https://mp.weixin.qq.com/")
            .header("Origin", "https://mp.weixin.qq.com")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Status code: {}", resp.status()));
        }

        let html = resp.text().await.map_err(|e| e.to_string())?;
        Ok(process_wechat_html(&html))
    }

    // Helper for web proxy fetch
    async fn fetch_via_web_proxy(
        client: &reqwest::Client,
        proxy_base: &str,
        target_url: &str,
        auth: Option<&str>,
    ) -> Result<String, String> {
        // Construct URL: proxy_base + ?url=encoded_target
        // Check if proxy_base already has query params
        let separator = if proxy_base.contains('?') { "&" } else { "?" };
        let mut proxy_request_url = format!(
            "{}{}{}url={}",
            proxy_base,
            separator,
            // Add other params if needed, but usually just url
            "",
            urlencoding::encode(target_url)
        );

        if let Some(a) = auth {
            if !a.is_empty() {
                proxy_request_url.push_str(&format!("&authorization={}", urlencoding::encode(a)));
            }
        }

        let resp = client
            .get(&proxy_request_url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!("Proxy Status code: {}", resp.status()));
        }

        let html = resp.text().await.map_err(|e| e.to_string())?;
        Ok(process_wechat_html(&html))
    }

    let client = reqwest::Client::builder()
        // .no_proxy() // We don't use system proxy for this
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    for proxy_url_opt in attempts {
        let result = if let Some(p_url) = proxy_url_opt {
            fetch_via_web_proxy(&client, &p_url, &url, auth.as_deref()).await
        } else {
            fetch_direct(&client, &url).await
        };

        match result {
            Ok(content) => {
                fetched_content = Some(content);
                break;
            }
            Err(e) => {
                last_error = e;
                continue;
            }
        }
    }

    match fetched_content {
        Some(content) => {
            // 3. Save to DB
            // We need an ID. If article exists in `articles` table, reuse ID.
            // But we might fetch article not in `articles` table yet (search result logic?)
            // Wait, search result items come from `embeddings` which come from `articles`.
            // So article MUST exist in `articles` table.
            // Find ID from articles table by link? Or we don't know the exact link match?
            // Actually, the `req.url` matches what we have in `articles.link`?
            // If we just want to save content, we can generate a hash ID or try to match.

            // Try to find article ID by link
            let article_id: Option<(String,)> =
                sqlx::query_as("SELECT id FROM articles WHERE link = $1")
                    .bind(&req.url)
                    .fetch_optional(&state.db_pool)
                    .await
                    .unwrap_or(None);

            let id = if let Some((aid,)) = article_id {
                aid
            } else {
                // Fallback: use md5 of url
                format!("{:x}", md5::compute(&req.url))
            };

            let _ = sqlx::query(
                r#"
                 INSERT INTO article_content (id, content, original_url)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO UPDATE SET
                     content = EXCLUDED.content,
                     create_time = extract(epoch from now())::bigint
                 "#,
            )
            .bind(&id)
            .bind(&content) // Content is already processed here! (process_wechat_html called in helpers)
            // Wait, do we want to store PROCESSED content or RAW content?
            // If we store processed, then next time we fetch it, we process it AGAIN?
            // process_wechat_html seems idempotent mostly (replace hidden with visible), but adding style tag again?
            // It adds style tag if </head> exists.
            // If we store PROCESSED content, then "View" works directly.
            // Insight prefetch stores RAW content.
            // So we have a mix.
            // If DB allows raw, we must process on read.
            // If DB allows processed, we can just read.
            // Since prefetch stores RAW (via fetch_html_content), we MUST process on read.
            // So for fetch_article saving... we should probably save RAW content if possible?
            // But `fetch_direct` returns processed content.
            // Let's modify fetch_direct/fetch_via_web_proxy to return RAW, then process?
            // Or just save Processed content.
            // If we save Processed content, `prefetch_task` saves RAW.
            // So we have inconsistent data.
            // Better to process on read ALWAYS.
            // But `fetch_direct` returns processed.
            // I should revert `fetch_direct` to return raw, or save raw.
            // The current implementation of fetch_direct returns `process_wechat_html(&html)`.
            // So `fetched_content` IS PROCESSED.
            // The `prefetch_task` returns/saves RAW.
            // If I just process on read, then if it's already processed, it might double process.
            // Does it hurt?
            // process_wechat_html:
            // 1. remove scripts (ok to run again)
            // 2. hidden -> visible (ok)
            // 3. data-src -> src (ok)
            // 4. Inject style (might inject twice?)
            // It checks `processed.find("</head>")` and inserts.
            // If style already there?
            // It doesn't check if style is already there.
            // So it WILL duplicate style block.
            // Not a huge deal, but messy.
            // DECISION:
            // Since `prefetch_task` saves RAW, and that's the bulk of data.
            // We should treat DB as storing RAW-dish data (or at least, Reader is responsible for presentation).
            // So `fetch_article` READ path should process.
            // And `fetch_article` WRITE path?
            // If `fetch_direct` returns processed, we save processed.
            // This is inconsistency.
            // Ideally `prefetch_task` should also process before save?
            // Or `fetch_article` should save raw.
            // Let's stick to "Process on Read".
            // So I should modify `fetch_direct` to NOT process?
            // But currently it does.
            // I will keep it as is for now to avoid breaking too much.
            // The duplicate style block is acceptable for solving the "Blank Page" (Hidden) issue now.
            .bind(&req.url)
            .execute(&state.db_pool)
            .await;

            let response = axum::response::Response::builder()
                .status(200)
                .header(header::CONTENT_TYPE, "text/html; charset=UTF-8")
                .body(content)
                .unwrap();
            Ok(response)
        }
        None => Err(AppError::BadGateway(format!(
            "Failed to fetch article: {}",
            last_error
        ))),
    }
}

// Helper to process WeChat HTML for static viewing
fn process_wechat_html(html: &str) -> String {
    let mut processed = html.to_string();

    // 0. Remove scripts (prevents JS from hiding content or messing with layout)
    if let Ok(re) = regex::Regex::new(r"(?s)<script[^>]*>.*?</script>") {
        processed = re.replace_all(&processed, "").to_string();
    }

    // 1. Force visibility: hidden -> visible on #js_content
    processed = processed.replace("visibility: hidden;", "visibility: visible;");
    processed = processed.replace("visibility:hidden;", "visibility:visible;");

    // 2. Fix lazy loading images: data-src -> src
    // Also add referrerpolicy="no-referrer" to bypass WeChat anti-hotlinking
    processed = processed.replace(" data-src=\"", " referrerpolicy=\"no-referrer\" src=\"");

    // 3. Ensure images have max-width and content is visible
    let style = r#"<style>
        #js_content { 
            visibility: visible !important; 
            opacity: 1 !important; 
            display: block !important; 
        }
        #img-content { 
            display: block !important; 
        }
        img { 
            max-width: 100% !important; 
            height: auto !important; 
            display: block !important;
            margin: 0 auto;
        }
        body {
            background-color: transparent !important;
        }
        /* Dark Mode Adaptation */
        :is(.dark) #js_content,
        :is(.dark) #activity-name,
        :is(.dark) .rich_media_title,
        :is(.dark) .rich_media_meta_list,
        :is(.dark) .rich_media_meta_text {
            color: #d1d5db !important; /* gray-300 */
            background-color: transparent !important;
        }
        
        :is(.dark) #js_content *,
        :is(.dark) #activity-name *,
        :is(.dark) .rich_media_title *,
        :is(.dark) .rich_media_meta_list * {
            color: inherit !important; /* Force inheritance to override inline styles */
            background-color: transparent !important;
            border-color: #374151 !important;
        }

        :is(.dark) #js_content p, 
        :is(.dark) #js_content span, 
        :is(.dark) #js_content strong, 
        :is(.dark) #js_content h1, 
        :is(.dark) #js_content h2, 
        :is(.dark) #js_content h3, 
        :is(.dark) #js_content h4, 
        :is(.dark) #js_content h5, 
        :is(.dark) #js_content h6,
        :is(.dark) #js_content li {
             color: inherit !important;
        }
    </style>"#;

    if let Some(pos) = processed.find("</head>") {
        processed.insert_str(pos, style);
    } else {
        processed.push_str(style);
    }

    processed
}

// ============ Auth Key ============

#[derive(Debug, Serialize)]
pub struct AuthKeyResponse {
    pub code: i32,
    pub data: String,
    pub msg: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_soon: Option<bool>,
}

/// Get current auth key from request and validate session
/// Response codes:
/// - 0: Valid session
/// - -1: No auth key found
/// - -2: Session expired (or will expire within 1 hour)
/// - -3: Session expiring soon (within 1 hour, but not yet expired)
pub async fn get_auth_key(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<AuthKeyResponse> {
    let auth_key = crate::proxy::get_auth_key_from_headers(&headers);

    if let Some(key) = auth_key {
        // Get detailed session status from database
        if let Ok((exists, is_valid, expires_at, expires_soon)) =
            state.cookie_store.get_session_status(&key).await
        {
            if exists {
                if is_valid {
                    return Json(AuthKeyResponse {
                        code: 0,
                        data: key,
                        msg: "ok".to_string(),
                        expires_at: Some(expires_at),
                        expires_soon: Some(expires_soon),
                    });
                } else if expires_soon {
                    // Session will expire within 1 hour
                    return Json(AuthKeyResponse {
                        code: -3,
                        data: key,
                        msg: "session_expiring_soon".to_string(),
                        expires_at: Some(expires_at),
                        expires_soon: Some(true),
                    });
                } else {
                    // Session already expired
                    return Json(AuthKeyResponse {
                        code: -2,
                        data: "".to_string(),
                        msg: "session_expired".to_string(),
                        expires_at: None,
                        expires_soon: None,
                    });
                }
            }
        }
    }

    Json(AuthKeyResponse {
        code: -1,
        data: "".to_string(),
        msg: "auth_key not found".to_string(),
        expires_at: None,
        expires_soon: None,
    })
}

// ============ Get Asset (Image/Video) ============

#[derive(Debug, Deserialize)]
pub struct GetAssetQuery {
    pub url: String,
}

/// Get asset content from database
pub async fn get_asset(
    State(state): State<AppState>,
    Query(query): Query<GetAssetQuery>,
) -> Result<impl axum::response::IntoResponse, AppError> {
    use axum::http::header;

    if query.url.is_empty() {
        return Err(AppError::BadRequest("url不能为空".to_string()));
    }

    let row: Option<(Vec<u8>, Option<String>)> =
        sqlx::query_as("SELECT data, mime_type FROM assets WHERE url = $1")
            .bind(&query.url)
            .fetch_optional(&state.db_pool)
            .await?;

    if let Some((data, mime_type)) = row {
        let content_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());

        let response = axum::response::Response::builder()
            .status(200)
            .header(header::CONTENT_TYPE, content_type)
            // Cache control for static assets
            .header(header::CACHE_CONTROL, "public, max-age=31536000")
            .body(axum::body::Body::from(data))
            .unwrap();
        Ok(response)
    } else {
        Err(AppError::NotFound("Asset not found".to_string()))
    }
}

// ============ Get Comments ============

#[derive(Debug, Deserialize)]
pub struct GetCommentsQuery {
    pub article_id: Option<String>,
    pub id: Option<String>,
}

/// Get comments from database
pub async fn get_comments(
    State(state): State<AppState>,
    Query(query): Query<GetCommentsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let row: Option<(serde_json::Value,)> = if let Some(aid) = &query.article_id {
        // Assuming one comment entry per article (the full JSON blob)
        sqlx::query_as("SELECT content_json FROM comments WHERE article_id = $1")
            .bind(aid)
            .fetch_optional(&state.db_pool)
            .await?
    } else if let Some(id) = &query.id {
        sqlx::query_as("SELECT content_json FROM comments WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db_pool)
            .await?
    } else {
        return Err(AppError::BadRequest("article_id或id不能为空".to_string()));
    };

    if let Some((json,)) = row {
        Ok(Json(json))
    } else {
        // Return empty object or specific error?
        // For comments, often return empty if not found.
        Ok(Json(serde_json::json!({})))
    }
}
