//! WeChat MP request proxy
//!
//! Handles forwarding requests to WeChat API with proper authentication.

use axum::http::HeaderMap;
use reqwest::header::{COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};

use crate::cookie::CookieStore;
use crate::error::AppError;

const WECHAT_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Options for proxying a request to WeChat
#[derive(Debug)]
pub struct ProxyRequestOptions {
    pub method: reqwest::Method,
    pub endpoint: String,
    pub query: Option<Vec<(String, String)>>,
    pub body: Option<Vec<(String, String)>>,
    pub cookie: Option<String>,
}

/// Response from WeChat API
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct WeChatResponse {
    #[serde(flatten)]
    pub data: serde_json::Value,
}

/// Proxy a request to WeChat MP API
pub async fn proxy_mp_request(options: ProxyRequestOptions) -> Result<reqwest::Response, AppError> {
    let client = reqwest::Client::new();

    let mut url = options.endpoint.clone();

    // Add query parameters
    if let Some(query) = &options.query {
        if !query.is_empty() {
            let query_string: String = query
                .iter()
                .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&");
            url = format!("{}?{}", url, query_string);
        }
    }

    let mut request = client.request(options.method.clone(), &url);

    // Set headers
    request = request
        .header(REFERER, "https://mp.weixin.qq.com/")
        .header(ORIGIN, "https://mp.weixin.qq.com")
        .header(USER_AGENT, WECHAT_USER_AGENT);

    // Add cookie if provided
    if let Some(cookie) = &options.cookie {
        request = request.header(COOKIE, cookie);
    }

    // Add form body for POST requests
    if options.method == reqwest::Method::POST {
        if let Some(body) = &options.body {
            request = request.form(body);
        }
    }

    let response = request.send().await?;
    Ok(response)
}

/// Proxy a request and return JSON
#[allow(dead_code)]
pub async fn proxy_mp_request_json<T: for<'de> Deserialize<'de>>(
    options: ProxyRequestOptions,
) -> Result<T, AppError> {
    let response = proxy_mp_request(options).await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "WeChat API error {}: {}",
            status, error_text
        )));
    }

    let json = response.json::<T>().await?;
    Ok(json)
}

/// Extract auth key from request headers
pub fn get_auth_key_from_headers(headers: &HeaderMap) -> Option<String> {
    // Try X-Auth-Key header first
    if let Some(auth_key) = headers.get("X-Auth-Key") {
        if let Ok(key) = auth_key.to_str() {
            return Some(key.to_string());
        }
    }

    // Try Cookie header (look for auth-key cookie)
    if let Some(cookie) = headers.get(COOKIE) {
        if let Ok(cookie_str) = cookie.to_str() {
            for part in cookie_str.split(';') {
                let part = part.trim();
                if let Some((name, value)) = part.split_once('=') {
                    if name.trim() == "auth-key" {
                        return Some(value.trim().to_string());
                    }
                }
            }
        }
    }

    None
}

/// Get cookie string from store using auth key in headers
pub async fn get_cookie_from_store(
    headers: &HeaderMap,
    cookie_store: &CookieStore,
) -> Option<String> {
    let auth_key = get_auth_key_from_headers(headers)?;
    let account_cookie = cookie_store.get_cookie(&auth_key).await.ok()??;
    Some(account_cookie.to_cookie_header())
}

/// Get token from store using auth key in headers
pub async fn get_token_from_store(
    headers: &HeaderMap,
    cookie_store: &CookieStore,
) -> Option<String> {
    let auth_key = get_auth_key_from_headers(headers)?;
    cookie_store.get_token(&auth_key).await.ok()?
}
