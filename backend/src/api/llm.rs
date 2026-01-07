//! LLM/Doppelganger API handlers
//!
//! AI chat with profile-based roleplay.

#![allow(dead_code)]

use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

// ============ Types ============

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "user" or "assistant"
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub profile: serde_json::Value,
    pub message: String,
    pub history: Option<Vec<ChatMessage>>,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<ChatData>,
}

#[derive(Debug, Serialize)]
pub struct ChatData {
    pub reply: String,
}

const ROLEPLAY_PROMPT_TEMPLATE: &str = r#"你现在是 {name} 的数字分身。

## 个人档案
{profile}

## 扮演规则
1. 使用第一人称（我、我的）
2. 模仿档案中描述的语言风格
3. 做决策时参考档案中的价值观和历史决策案例
4. 遇到不确定的问题，承认"这个我不太确定"而非编造
5. 保持一致的人格特征
6. 使用中文回复
7. 回复要自然，像真人聊天一样

## 对话历史
{history}

## 当前消息
用户: {message}

## 回复
{name}:"#;

// ============ Chat Handler ============

/// Doppelganger chat with AI roleplay
pub async fn chat(Json(req): Json<ChatRequest>) -> Result<Json<ChatResponse>, AppError> {
    if req.profile.is_null() {
        return Ok(Json(ChatResponse {
            code: -1,
            message: Some("缺少档案数据".to_string()),
            data: None,
        }));
    }

    if req.message.is_empty() {
        return Ok(Json(ChatResponse {
            code: -1,
            message: Some("缺少消息内容".to_string()),
            data: None,
        }));
    }

    let name = req
        .profile
        .get("identity")
        .and_then(|i| i.get("Name"))
        .and_then(|n| n.as_str())
        .unwrap_or("分身");

    let profile_json = serde_json::to_string_pretty(&req.profile).unwrap_or_default();
    let history_text = req
        .history
        .as_ref()
        .map(|h| {
            h.iter()
                .map(|m| {
                    let role = if m.role == "user" { "用户" } else { name };
                    format!("{}: {}", role, m.content)
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_else(|| "(无历史对话)".to_string());

    let prompt = ROLEPLAY_PROMPT_TEMPLATE
        .replace("{name}", name)
        .replace("{profile}", &profile_json)
        .replace("{history}", &history_text)
        .replace("{message}", &req.message);

    // Try Gemini first, then DeepSeek, then fallback
    let gemini_key = std::env::var("GEMINI_API_KEY").ok();
    let deepseek_key = std::env::var("DEEPSEEK_API_KEY").ok();

    let reply = if let Some(key) = gemini_key {
        call_gemini_chat(&key, &prompt).await
    } else if let Some(key) = deepseek_key {
        call_deepseek_chat(&key, &prompt).await
    } else {
        // Fallback response
        Ok(format!(
            "（这是一个模拟回复，请配置 Gemini 或 DeepSeek API Key 以启用真实 AI 对话）\n\n作为 {}，我会这样回应：根据我的档案，我倾向于理性和务实地看待问题。关于你的问题\"{}\"，我需要更多信息才能给出具体想法。",
            name, req.message
        ))
    };

    match reply {
        Ok(text) => Ok(Json(ChatResponse {
            code: 0,
            message: None,
            data: Some(ChatData { reply: text }),
        })),
        Err(e) => Ok(Json(ChatResponse {
            code: -1,
            message: Some(e.to_string()),
            data: None,
        })),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestConnectionRequest {
    pub provider: String,
    pub gemini_api_key: Option<String>,
    pub gemini_model: Option<String>,
    pub gemini_proxy_enabled: Option<bool>,
    pub deepseek_api_key: Option<String>,
    pub deepseek_model: Option<String>,
    pub deepseek_proxy_enabled: Option<bool>,
    // OpenAI-compatible provider
    pub openai_compatible_base_url: Option<String>,
    pub openai_compatible_api_key: Option<String>,
    pub openai_compatible_model: Option<String>,
    pub openai_compatible_proxy_enabled: Option<bool>,
    // Proxy settings
    pub proxy_host: Option<String>,
    pub proxy_port: Option<u16>,
    pub proxy_username: Option<String>,
    pub proxy_password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestConnectionResponse {
    pub success: bool,
    pub message: String,
}

/// Test LLM connection
pub async fn test_connection(
    Json(req): Json<TestConnectionRequest>,
) -> Result<Json<TestConnectionResponse>, AppError> {
    let client = build_client(&req)?;

    match req.provider.as_str() {
        "gemini" => {
            let key = req.gemini_api_key.as_deref().unwrap_or("");
            if key.is_empty() {
                return Ok(Json(TestConnectionResponse {
                    success: false,
                    message: "Gemini API Key is empty".to_string(),
                }));
            }
            // Test with a simple model list or generate call
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                key
            );
            let resp = client.get(&url).send().await;

            match resp {
                Ok(r) => {
                    if r.status().is_success() {
                        Ok(Json(TestConnectionResponse {
                            success: true,
                            message: "Gemini connected successfully!".to_string(),
                        }))
                    } else {
                        Ok(Json(TestConnectionResponse {
                            success: false,
                            message: format!("Gemini Error: {}", r.status()),
                        }))
                    }
                }
                Err(e) => Ok(Json(TestConnectionResponse {
                    success: false,
                    message: format!("Connection failed: {:#?}\nURL: {}\nProxy: {:?}", e, url, client), // Debug info
                })),
            }
        }
        "deepseek" => {
            let key = req.deepseek_api_key.as_deref().unwrap_or("");
            if key.is_empty() {
                return Ok(Json(TestConnectionResponse {
                    success: false,
                    message: "DeepSeek API Key is empty".to_string(),
                }));
            }
            // Test user balance or models
            let resp = client
                .get("https://api.deepseek.com/user/balance")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await;

            match resp {
                Ok(r) => {
                    if r.status().is_success() {
                        Ok(Json(TestConnectionResponse {
                            success: true,
                            message: "DeepSeek connected successfully!".to_string(),
                        }))
                    } else {
                        // Some endpoints might return 401/403 if key is invalid
                        Ok(Json(TestConnectionResponse {
                            success: false,
                            message: format!("DeepSeek Error: {}", r.status()),
                        }))
                    }
                }
                Err(e) => Ok(Json(TestConnectionResponse {
                    success: false,
                    message: format!("Connection failed: {}", e),
                })),
            }
        }
        "openai_compatible" => {
            let base_url = req.openai_compatible_base_url.as_deref().unwrap_or("");
            let api_key = req.openai_compatible_api_key.as_deref().unwrap_or("");
            let model = req.openai_compatible_model.as_deref().unwrap_or("");
            let use_proxy = req.openai_compatible_proxy_enabled.unwrap_or(false);

            if base_url.is_empty() {
                return Ok(Json(TestConnectionResponse {
                    success: false,
                    message: "Base URL is empty".to_string(),
                }));
            }
            if api_key.is_empty() {
                return Ok(Json(TestConnectionResponse {
                    success: false,
                    message: "API Key is empty".to_string(),
                }));
            }
            if model.is_empty() {
                return Ok(Json(TestConnectionResponse {
                    success: false,
                    message: "Model name is empty".to_string(),
                }));
            }

            // Build proxy config if enabled
            let proxy_url = if use_proxy {
                if let (Some(host), Some(port)) = (&req.proxy_host, req.proxy_port) {
                    if !host.is_empty() && port > 0 {
                        Some(format!("http://{}:{}", host, port))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };

            match crate::llm::openai_compatible::test_connection_with_proxy(base_url, api_key, model, proxy_url.as_deref()).await {
                Ok(msg) => Ok(Json(TestConnectionResponse {
                    success: true,
                    message: msg,
                })),
                Err(e) => Ok(Json(TestConnectionResponse {
                    success: false,
                    message: format!("Connection failed: {}", e),
                })),
            }
        }
        _ => Ok(Json(TestConnectionResponse {
            success: false,
            message: "Unknown provider".to_string(),
        })),
    }
}

// ============ Ollama Test Connection ============

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestOllamaRequest {
    pub base_url: String,
    pub embedding_model: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestOllamaResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
}

/// Test Ollama connection by checking available models
pub async fn test_ollama_connection(
    Json(req): Json<TestOllamaRequest>,
) -> Result<Json<TestOllamaResponse>, AppError> {
    let base_url = if req.base_url.is_empty() {
        "http://127.0.0.1:11434".to_string()
    } else {
        req.base_url
    };

    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // First check if Ollama is reachable by listing models
    let url = format!("{}/api/tags", base_url);
    let resp = client.get(&url).send().await;

    match resp {
        Ok(r) => {
            if r.status().is_success() {
                let data: serde_json::Value = r.json().await.unwrap_or_default();
                let models: Vec<String> = data
                    .get("models")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_default();

                // Check if the required embedding model is available
                let embedding_model = req
                    .embedding_model
                    .unwrap_or_else(|| "qwen3-embedding:8b-q8_0".to_string());
                let has_model = models
                    .iter()
                    .any(|m| m.starts_with(&embedding_model.split(':').next().unwrap_or("")));

                if models.is_empty() {
                    Ok(Json(TestOllamaResponse {
                        success: true,
                        message: format!(
                            "✓ Ollama 连接成功！但未发现已下载的模型。请运行 `ollama pull {}`",
                            embedding_model
                        ),
                        models: Some(models),
                    }))
                } else if has_model {
                    Ok(Json(TestOllamaResponse {
                        success: true,
                        message: format!(
                            "✓ Ollama 连接成功！已发现 {} 个模型，包含所需的 embedding 模型。",
                            models.len()
                        ),
                        models: Some(models),
                    }))
                } else {
                    Ok(Json(TestOllamaResponse {
                        success: true,
                        message: format!("✓ Ollama 连接成功！发现 {} 个模型，但未找到 {}。请运行 `ollama pull {}`", models.len(), embedding_model, embedding_model),
                        models: Some(models),
                    }))
                }
            } else {
                Ok(Json(TestOllamaResponse {
                    success: false,
                    message: format!("Ollama 返回错误: HTTP {}", r.status()),
                    models: None,
                }))
            }
        }
        Err(e) => Ok(Json(TestOllamaResponse {
            success: false,
            message: format!("✗ 无法连接到 Ollama ({}): {}", base_url, e),
            models: None,
        })),
    }
}
fn build_client(req: &TestConnectionRequest) -> Result<reqwest::Client, AppError> {
    let mut builder = reqwest::Client::builder();

    let use_proxy = if req.provider == "gemini" {
        req.gemini_proxy_enabled.unwrap_or(false)
    } else {
        req.deepseek_proxy_enabled.unwrap_or(false)
    };

    if use_proxy {
        if let (Some(host), Some(port)) = (&req.proxy_host, req.proxy_port) {
            if !host.is_empty() && port > 0 {
                let proxy_url = format!("http://{}:{}", host, port);
                let mut proxy = reqwest::Proxy::all(&proxy_url)
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                if let (Some(u), Some(p)) = (&req.proxy_username, &req.proxy_password) {
                    if !u.is_empty() {
                        proxy = proxy.basic_auth(u, p);
                    }
                }
                builder = builder.proxy(proxy);
            }
        }
    }

    builder
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))
}

// ... existing helper functions (call_gemini_chat, call_deepseek_chat) need to be updated to accept a client instead of creating new one?
// Or I can keep them as is for now since `chat` endpoint handles creating its own client (which doesn't use the proxy config from frontend yet!).
// Wait, the `chat` endpoint reads keys from ENV vars, but the `test` endpoint uses keys from request.
// The `chat` endpoint implementation is currently using ENV vars, which means the frontend settings (saved in local storage) are NOT being used for actual chat?
// That seems like a separate issue. The user is asking about "configuration page not working".
// I will just implement the `test_connection` handler first.
// The existing `call_gemini_chat` logic is fine for the `chat` endpoint if we assume server-side config.
// But valid observation: the frontend configures keys, but the backend `chat` uses ENV.
// For now, I will leave `request` helper functions below but I am replacing lines 142-202 which contain them.
// I should preserve them.

async fn call_gemini_chat(api_key: &str, prompt: &str) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={}",
        api_key
    );

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "contents": [{"parts": [{"text": prompt}]}],
            "generationConfig": {
                "temperature": 0.8,
                "maxOutputTokens": 1024
            }
        }))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Gemini Request Failed: {:#?}", e)))?;

    let data: serde_json::Value = response.json().await?;
    let text = data
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.get(0))
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(text)
}

async fn call_deepseek_chat(api_key: &str, prompt: &str) -> Result<String, AppError> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://api.deepseek.com/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": "deepseek-chat",
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.8,
            "max_tokens": 1024
        }))
        .send()
        .await?;

    let data: serde_json::Value = response.json().await?;
    let text = data
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();

    Ok(text)
}
