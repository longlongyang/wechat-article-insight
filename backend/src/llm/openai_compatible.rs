//! OpenAI-Compatible API Provider
//! Supports any service that implements the OpenAI Chat Completions API format
//! (e.g., POE, OpenRouter, Azure OpenAI, local deployments)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

/// Build HTTP client with optional proxy
fn build_client(proxy_url: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();

    if let Some(proxy) = proxy_url {
        let proxy = reqwest::Proxy::all(proxy)?;
        builder = builder.proxy(proxy);
    }

    Ok(builder.build()?)
}

/// Generate text using an OpenAI-compatible API
#[allow(dead_code)]
pub async fn generate_text(
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    proxy_url: Option<&str>,
) -> Result<String> {
    let client = build_client(proxy_url)?;
    
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    
    let request = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        max_tokens: None,
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow!("API error {}: {}", status, error_text));
    }

    let data: ChatCompletionResponse = response.json().await?;
    
    data.choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| anyhow!("No response content from OpenAI-compatible API"))
}

/// Test connection to an OpenAI-compatible API (with proxy support)
pub async fn test_connection_with_proxy(
    base_url: &str,
    api_key: &str,
    model: &str,
    proxy_url: Option<&str>,
) -> Result<String> {
    let client = build_client(proxy_url)?;
    
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    
    let request = ChatCompletionRequest {
        model: model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Say 'OK' if you can hear me.".to_string(),
        }],
        max_tokens: Some(50),
    };

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow!("API error {}: {}", status, error_text));
    }

    let data: ChatCompletionResponse = response.json().await?;
    
    let content = data
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    let proxy_note = if proxy_url.is_some() { " (通过代理)" } else { "" };
    Ok(format!("✓ 连接成功{}！模型响应: {}", proxy_note, content.trim()))
}
