//! Cookie management for WeChat authentication
//!
//! Handles parsing, storage, and retrieval of WeChat session cookies.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;

/// A single parsed cookie entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CookieEntity {
    pub name: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_timestamp: Option<i64>,
}

/// Parsed cookies for a WeChat account session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountCookie {
    pub token: String,
    pub cookies: Vec<CookieEntity>,
}

impl AccountCookie {
    /// Create from raw set-cookie header strings
    pub fn new(token: String, raw_cookies: Vec<String>) -> Self {
        let cookies = Self::parse_cookies(&raw_cookies);
        Self { token, cookies }
    }

    /// Parse set-cookie header strings into CookieEntity list
    pub fn parse_cookies(raw_cookies: &[String]) -> Vec<CookieEntity> {
        let mut cookie_map: HashMap<String, CookieEntity> = HashMap::new();

        for cookie_str in raw_cookies {
            let parts: Vec<&str> = cookie_str.split(';').map(|s| s.trim()).collect();

            if parts.is_empty() {
                continue;
            }

            // First part is name=value
            if let Some((name, value)) = parts[0].split_once('=') {
                let cookie_name = name.trim().to_string();
                let cookie_value = value.trim().to_string();

                let mut entity = CookieEntity {
                    name: cookie_name.clone(),
                    value: cookie_value,
                    domain: None,
                    path: None,
                    expires: None,
                    expires_timestamp: None,
                };

                // Process other attributes
                for part in parts.iter().skip(1) {
                    if let Some((key, val)) = part.split_once('=') {
                        let key_lower = key.trim().to_lowercase();
                        let val_str = val.trim().to_string();

                        match key_lower.as_str() {
                            "domain" => entity.domain = Some(val_str),
                            "path" => entity.path = Some(val_str),
                            "expires" => {
                                entity.expires = Some(val_str.clone());
                                // Try to parse timestamp
                                if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(&val_str) {
                                    entity.expires_timestamp = Some(dt.timestamp_millis());
                                }
                            }
                            _ => {}
                        }
                    }
                }

                cookie_map.insert(cookie_name, entity);
            }
        }

        cookie_map.into_values().collect()
    }

    /// Convert cookies to a Cookie header string for HTTP requests
    pub fn to_cookie_header(&self) -> String {
        self.cookies
            .iter()
            .filter(|c| c.value != "EXPIRED" && !c.value.is_empty())
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        // Check if any essential cookie is expired
        let now = chrono::Utc::now().timestamp_millis();
        for cookie in &self.cookies {
            if let Some(expires) = cookie.expires_timestamp {
                if expires < now {
                    return true;
                }
            }
        }
        false
    }
}

/// Cookie store with PostgreSQL persistence
pub struct CookieStore {
    pool: PgPool,
}

impl CookieStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the cookies table
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS cookies (
                auth_key TEXT PRIMARY KEY,
                token TEXT NOT NULL,
                cookies_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Store cookies for an auth key
    pub async fn set_cookie(
        &self,
        auth_key: &str,
        account_cookie: &AccountCookie,
    ) -> Result<bool, sqlx::Error> {
        tracing::info!("Setting cookie for auth_key: {}", auth_key);
        let now = chrono::Utc::now().timestamp();
        let expires_at = now + (4 * 24 * 60 * 60); // 4 days
        let cookies_json = serde_json::to_string(&account_cookie.cookies).unwrap_or_default();

        sqlx::query(
            r#"
            INSERT INTO cookies (auth_key, token, cookies_json, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (auth_key) DO UPDATE SET
                token = EXCLUDED.token,
                cookies_json = EXCLUDED.cookies_json,
                created_at = EXCLUDED.created_at,
                expires_at = EXCLUDED.expires_at
            "#,
        )
        .bind(auth_key)
        .bind(&account_cookie.token)
        .bind(&cookies_json)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(true)
    }

    /// Get cookies for an auth key
    pub async fn get_cookie(&self, auth_key: &str) -> Result<Option<AccountCookie>, sqlx::Error> {
        tracing::info!("Getting cookie for auth_key: {}", auth_key);
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT token, cookies_json FROM cookies WHERE auth_key = $1 AND expires_at > $2",
        )
        .bind(auth_key)
        .bind(chrono::Utc::now().timestamp())
        .fetch_optional(&self.pool)
        .await?;

        if let Some((token, cookies_json)) = row {
            let cookies: Vec<CookieEntity> =
                serde_json::from_str(&cookies_json).unwrap_or_default();
            tracing::info!(
                "Found cookie for auth_key: {}, token: {}, cookies count: {}",
                auth_key,
                token,
                cookies.len()
            );
            Ok(Some(AccountCookie { token, cookies }))
        } else {
            tracing::warn!(
                "No valid/non-expired cookie found for auth_key: {}",
                auth_key
            );
            Ok(None)
        }
    }

    /// Get token for an auth key
    pub async fn get_token(&self, auth_key: &str) -> Result<Option<String>, sqlx::Error> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT token FROM cookies WHERE auth_key = $1 AND expires_at > $2")
                .bind(auth_key)
                .bind(chrono::Utc::now().timestamp())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(|(token,)| token))
    }

    /// Get session status for an auth key
    /// Returns (exists, is_valid, expires_at, expires_soon)
    /// - exists: whether the session exists in DB
    /// - is_valid: whether the session is not expired (with 1 hour buffer)
    /// - expires_at: Unix timestamp when session expires
    /// - expires_soon: whether session expires within 1 hour
    pub async fn get_session_status(
        &self,
        auth_key: &str,
    ) -> Result<(bool, bool, i64, bool), sqlx::Error> {
        let row: Option<(i64,)> =
            sqlx::query_as("SELECT expires_at FROM cookies WHERE auth_key = $1")
                .bind(auth_key)
                .fetch_optional(&self.pool)
                .await?;

        if let Some((expires_at,)) = row {
            let now = chrono::Utc::now().timestamp();
            let one_hour = 60 * 60; // 1 hour in seconds

            // Consider expired if within 1 hour of actual expiry
            let effective_expires = expires_at - one_hour;
            let is_valid = now < effective_expires;
            let expires_soon = now >= effective_expires && now < expires_at;

            Ok((true, is_valid, expires_at, expires_soon))
        } else {
            Ok((false, false, 0, false))
        }
    }

    /// Delete expired cookies
    pub async fn cleanup_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("DELETE FROM cookies WHERE expires_at <= $1")
            .bind(chrono::Utc::now().timestamp())
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cookies() {
        let raw = vec![
            "session=abc123; Path=/; Expires=Thu, 01 Jan 2030 00:00:00 GMT".to_string(),
            "token=xyz789; Domain=.example.com".to_string(),
        ];

        let cookies = AccountCookie::parse_cookies(&raw);
        assert_eq!(cookies.len(), 2);
    }

    #[test]
    fn test_to_cookie_header() {
        let account = AccountCookie {
            token: "test".to_string(),
            cookies: vec![
                CookieEntity {
                    name: "a".to_string(),
                    value: "1".to_string(),
                    domain: None,
                    path: None,
                    expires: None,
                    expires_timestamp: None,
                },
                CookieEntity {
                    name: "b".to_string(),
                    value: "2".to_string(),
                    domain: None,
                    path: None,
                    expires: None,
                    expires_timestamp: None,
                },
            ],
        };

        assert_eq!(account.to_cookie_header(), "a=1; b=2");
    }
}
