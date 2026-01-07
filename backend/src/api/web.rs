//! WeChat Web API handlers
//!
//! Login flow, account info, and misc utilities.

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Response, StatusCode},
    Json,
};
use reqwest::header::{COOKIE, SET_COOKIE};
use serde::{Deserialize, Serialize};

use crate::cookie::AccountCookie;
use crate::error::AppError;
use crate::AppState;

const WECHAT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

// ============ Login: Session ============

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct BaseResp {
    pub ret: i32,
    pub err_msg: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct StartLoginResponse {
    pub base_resp: BaseResp,
}

/// Start login session
pub async fn start_login_session(
    headers: HeaderMap,
    axum::extract::Path(sid): axum::extract::Path<String>,
) -> Result<Response<Body>, AppError> {
    let cookie = get_cookies_from_request(&headers);

    let client = reqwest::Client::new();
    let mut request = client
        .post("https://mp.weixin.qq.com/cgi-bin/bizlogin")
        .query(&[("action", "startlogin")])
        .form(&[
            ("userlang", "zh_CN"),
            ("redirect_url", ""),
            ("login_type", "3"),
            ("sessionid", &sid),
            ("token", ""),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(c) = cookie {
        request = request.header(COOKIE, c);
    }

    let response = request.send().await?;

    // Forward the response including set-cookie headers (specifically uuid)
    let mut builder = Response::builder().status(response.status().as_u16());

    for (name, value) in response.headers() {
        if name == SET_COOKIE {
            // Forward uuid cookie
            if let Ok(v) = value.to_str() {
                if v.contains("uuid=") {
                    builder = builder.header(SET_COOKIE, v);
                }
            }
        } else if name == header::CONTENT_TYPE {
            builder = builder.header(name, value);
        }
    }

    let body = response.bytes().await?;
    Ok(builder.body(Body::from(body)).unwrap())
}

// ============ Login: Get QR Code ============

/// Get login QR code from WeChat
pub async fn get_qrcode(headers: HeaderMap) -> Result<Response<Body>, AppError> {
    let cookie = get_cookies_from_request(&headers);

    let client = reqwest::Client::new();
    let mut request = client
        .get("https://mp.weixin.qq.com/cgi-bin/scanloginqrcode")
        .query(&[
            ("action", "getqrcode"),
            ("random", &chrono::Utc::now().timestamp_millis().to_string()),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(c) = cookie {
        request = request.header(COOKIE, c);
    }

    let response = request.send().await?;

    // Forward the response including set-cookie headers
    let mut builder = Response::builder().status(response.status().as_u16());

    for (name, value) in response.headers() {
        if name == SET_COOKIE {
            // Only forward uuid cookie
            if let Ok(v) = value.to_str() {
                if v.starts_with("uuid=") {
                    builder = builder.header(SET_COOKIE, v);
                }
            }
        } else if name == header::CONTENT_TYPE {
            builder = builder.header(name, value);
        }
    }

    let body = response.bytes().await?;
    Ok(builder.body(Body::from(body)).unwrap())
}

// ============ Login: Scan Status ============

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub status: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_img_url: Option<String>,
}

/// Check QR code scan status
pub async fn check_scan(headers: HeaderMap) -> Result<Json<serde_json::Value>, AppError> {
    let cookie = get_cookies_from_request(&headers);

    let client = reqwest::Client::new();
    let mut request = client
        .get("https://mp.weixin.qq.com/cgi-bin/scanloginqrcode")
        .query(&[
            ("action", "ask"),
            ("token", ""),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(c) = cookie {
        request = request.header(COOKIE, c);
    }

    let response = request.send().await?;
    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ Login: Biz Login ============

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct BizLoginResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
}

/// Complete login and get auth key
pub async fn biz_login(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response<Body>, AppError> {
    let cookie = get_cookies_from_request(&headers);

    let client = reqwest::Client::new();
    let mut request = client
        .post("https://mp.weixin.qq.com/cgi-bin/bizlogin")
        .query(&[("action", "login")])
        .form(&[
            ("userlang", "zh_CN"),
            ("redirect_url", ""),
            ("cookie_forbidden", "0"),
            ("cookie_cleaned", "0"),
            ("plugin_used", "0"),
            ("login_type", "3"),
            ("token", ""),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(c) = cookie {
        request = request.header(COOKIE, c);
    }

    let response = request.send().await?;
    let set_cookies: Vec<String> = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
        .collect();

    let json: serde_json::Value = response.json().await?;

    // Extract token from redirect_url
    let token = json
        .get("redirect_url")
        .and_then(|u| u.as_str())
        .and_then(|url| {
            url::Url::parse(&format!("http://localhost{}", url))
                .ok()?
                .query_pairs()
                .find(|(k, _)| k == "token")
                .map(|(_, v)| v.to_string())
        });

    if let Some(token) = token {
        // Generate auth key and store cookies
        let auth_key = uuid::Uuid::new_v4().to_string().replace("-", "");
        let account_cookie = AccountCookie::new(token.clone(), set_cookies);

        state
            .cookie_store
            .set_cookie(&auth_key, &account_cookie)
            .await?;

        // Get account info
        let info = get_mp_info_internal(&state, &auth_key).await;

        let expires = chrono::Utc::now() + chrono::Duration::days(4);
        let body = serde_json::json!({
            "nickname": info.as_ref().map(|i| i.nick_name.as_str()).unwrap_or(""),
            "avatar": info.as_ref().and_then(|i| i.head_img.as_deref()).unwrap_or(""),
            "expires": expires.to_rfc3339(),
        });

        let response = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                SET_COOKIE,
                format!(
                    "auth-key={}; Path=/; Expires={}; SameSite=Lax",
                    auth_key,
                    expires.format("%a, %d %b %Y %H:%M:%S GMT")
                ),
            )
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap();

        Ok(response)
    } else {
        let body = serde_json::json!({
            "err": "登录失败，请稍后重试"
        });
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap())
    }
}

// ============ MP Info ============

#[derive(Debug, Serialize, Deserialize)]
pub struct MpInfo {
    pub nick_name: String,
    pub head_img: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

async fn get_mp_info_internal(state: &AppState, auth_key: &str) -> Option<MpInfo> {
    let account_cookie = state.cookie_store.get_cookie(auth_key).await.ok()??;
    let cookie_str = account_cookie.to_cookie_header();
    let token = account_cookie.token;

    let client = reqwest::Client::new();
    let response = client
        .get("https://mp.weixin.qq.com/cgi-bin/home")
        .query(&[("t", "home/index"), ("token", &token), ("lang", "zh_CN")])
        .header(COOKIE, &cookie_str)
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await
        .ok()?;

    let html = response.text().await.ok()?;

    // Parse nick_name and head_img from HTML
    let nick_name = regex::Regex::new(r#"nick_name\s*:\s*["']([^"']+)["']"#)
        .ok()?
        .captures(&html)?
        .get(1)?
        .as_str()
        .to_string();

    let head_img = regex::Regex::new(r#"head_img\s*:\s*["']([^"']+)["']"#)
        .ok()
        .and_then(|re| re.captures(&html))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    Some(MpInfo {
        nick_name,
        head_img,
        extra: serde_json::Value::Null,
    })
}

/// Get MP account info
pub async fn get_mp_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_key = crate::proxy::get_auth_key_from_headers(&headers);

    if let Some(auth_key) = auth_key {
        tracing::info!("get_mp_info: found auth_key: {}", auth_key);
        if let Some(info) = get_mp_info_internal(&state, &auth_key).await {
            return Ok(Json(serde_json::json!({
                "nick_name": info.nick_name,
                "head_img": info.head_img,
            })));
        } else {
            tracing::warn!("get_mp_info: failed to get info for auth_key: {}", auth_key);
        }
    } else {
        tracing::warn!("get_mp_info: no auth_key found in headers");
    }

    Ok(Json(serde_json::json!({
        "nick_name": "",
        "head_img": "",
    })))
}

/// Logout
pub async fn logout(_headers: HeaderMap) -> Json<serde_json::Value> {
    // Just return success - client should clear auth-key cookie
    Json(serde_json::json!({
        "success": true
    }))
}

// ============ Helpers ============

fn get_cookies_from_request(headers: &HeaderMap) -> Option<String> {
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

// ============ Misc: Status ============

/// Get proxy status from external service
pub async fn misc_status() -> Result<Json<serde_json::Value>, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://my-cron-service.deno.dev/api/worker-proxy")
        .send()
        .await?;
    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ Misc: Account Name ============

#[derive(Debug, Deserialize)]
pub struct AccountNameQuery {
    pub url: String,
}

/// Get WeChat account name from article URL
pub async fn misc_accountname(
    axum::extract::Query(query): axum::extract::Query<AccountNameQuery>,
) -> Result<String, AppError> {
    let url = urlencoding::decode(&query.url)
        .map(|s| s.to_string())
        .unwrap_or(query.url);

    let client = reqwest::Client::new();
    let html = client
        .get(&url)
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("Origin", "https://mp.weixin.qq.com")
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?
        .text()
        .await?;

    // Extract account name using regex (simulating cheerio)
    let name = regex::Regex::new(r#"class="wx_follow_nickname[^"]*"[^>]*>([^<]+)<"#)
        .ok()
        .and_then(|re| re.captures(&html))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();

    Ok(name)
}

// ============ Misc: Comment ============

#[derive(Debug, Deserialize)]
pub struct CommentQuery {
    pub __biz: String,
    pub comment_id: String,
    pub key: String,
    pub uin: String,
    pub pass_ticket: String,
}

/// Get article comments
pub async fn misc_comment(
    axum::extract::Query(query): axum::extract::Query<CommentQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://mp.weixin.qq.com/mp/appmsg_comment")
        .query(&[
            ("action", "getcomment"),
            ("__biz", &query.__biz),
            ("comment_id", &query.comment_id),
            ("uin", &query.uin),
            ("key", &query.key),
            ("pass_ticket", &query.pass_ticket),
            ("limit", "1000"),
            ("f", "json"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ MP: Search Biz ============

#[derive(Debug, Deserialize)]
pub struct SearchBizQuery {
    pub keyword: String,
    pub begin: Option<u32>,
    pub size: Option<u32>,
}

/// Search for WeChat official accounts (authenticated version)
pub async fn mp_searchbiz(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<SearchBizQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_key = crate::proxy::get_auth_key_from_headers(&headers);

    let token = if let Some(key) = &auth_key {
        state.cookie_store.get_token(key).await.ok().flatten()
    } else {
        None
    };

    let token = match token {
        Some(t) => t,
        None => {
            return Ok(Json(serde_json::json!({
                "base_resp": {"ret": -1, "err_msg": "认证信息无效"}
            })));
        }
    };

    let begin = query.begin.unwrap_or(0);
    let size = query.size.unwrap_or(5);

    let account_cookie = if let Some(key) = &auth_key {
        state.cookie_store.get_cookie(key).await.ok().flatten()
    } else {
        None
    };
    let cookie_str = account_cookie.map(|c| c.to_cookie_header());

    let client = reqwest::Client::new();
    let mut request = client
        .get("https://mp.weixin.qq.com/cgi-bin/searchbiz")
        .query(&[
            ("action", "search_biz"),
            ("begin", &begin.to_string()),
            ("count", &size.to_string()),
            ("query", &query.keyword),
            ("token", &token),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(cookie) = cookie_str {
        request = request.header(COOKIE, cookie);
    }

    let response = request.send().await?;
    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ MP: App Msg Publish ============

#[derive(Debug, Deserialize)]
pub struct AppMsgPublishQuery {
    pub fakeid: String,
    pub begin: Option<u32>,
    pub size: Option<u32>,
    pub keyword: Option<String>,
}

/// Get published articles from an official account
pub async fn mp_appmsgpublish(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Query(query): axum::extract::Query<AppMsgPublishQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let auth_key = crate::proxy::get_auth_key_from_headers(&headers);

    let token = if let Some(key) = &auth_key {
        state.cookie_store.get_token(key).await.ok().flatten()
    } else {
        None
    };

    let token = match token {
        Some(t) => t,
        None => {
            return Ok(Json(serde_json::json!({
                "base_resp": {"ret": -1, "err_msg": "认证信息无效"}
            })));
        }
    };

    let begin = query.begin.unwrap_or(0);
    let size = query.size.unwrap_or(5);

    let account_cookie = if let Some(key) = &auth_key {
        state.cookie_store.get_cookie(key).await.ok().flatten()
    } else {
        None
    };
    let cookie_str = account_cookie.map(|c| c.to_cookie_header());

    let client = reqwest::Client::new();
    let mut request = client
        .get("https://mp.weixin.qq.com/cgi-bin/appmsgpublish")
        .query(&[
            ("sub", "list"),
            ("search_field", "null"),
            ("begin", &begin.to_string()),
            ("count", &size.to_string()),
            ("query", query.keyword.as_deref().unwrap_or("")),
            ("fakeid", &query.fakeid),
            ("type", "101_1"),
            ("free_publish_type", "1"),
            ("sub_action", "list_ex"),
            ("token", &token),
            ("lang", "zh_CN"),
            ("f", "json"),
            ("ajax", "1"),
        ])
        .header("Referer", "https://mp.weixin.qq.com/")
        .header("User-Agent", WECHAT_USER_AGENT);

    if let Some(cookie) = cookie_str {
        request = request.header(COOKIE, cookie);
    }

    let response = request.send().await?;
    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}

// ============ MP: App Msg Album ============

#[derive(Debug, Deserialize)]
pub struct AppMsgAlbumQuery {
    pub fakeid: String,
    pub album_id: String,
    #[serde(default)]
    pub is_reverse: String,
    #[serde(default)]
    pub begin_msgid: String,
    #[serde(default)]
    pub begin_itemidx: String,
}

/// Get album info (proxy)
pub async fn mp_appmsgalbum_proxy(
    axum::extract::Query(query): axum::extract::Query<AppMsgAlbumQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let client = reqwest::Client::new();
    let mut req_query = vec![
        ("action", "getalbum"),
        ("album_id", &query.album_id),
        ("fakeid", &query.fakeid),
        ("is_reverse", &query.is_reverse),
        ("f", "json"),
        ("count", "10"),
    ];

    if !query.begin_msgid.is_empty() {
        req_query.push(("begin_msgid", &query.begin_msgid));
    }
    if !query.begin_itemidx.is_empty() {
        req_query.push(("begin_itemidx", &query.begin_itemidx));
    }

    // Usually this endpoint is public, we just proxy it
    let response = client
        .get("https://mp.weixin.qq.com/mp/appmsgalbum")
        .query(&req_query)
        .header("User-Agent", WECHAT_USER_AGENT)
        .send()
        .await?;

    let json: serde_json::Value = response.json().await?;
    Ok(Json(json))
}
