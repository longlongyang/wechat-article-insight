//! Ollama local LLM provider implementation

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Generate embedding using Ollama
pub async fn generate_embedding(base_url: &str, model: &str, text: &str) -> Result<Vec<f32>> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let url = format!("{}/api/embed", base_url);

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "model": model,
            "input": text
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!("Ollama Embedding error: {}", error_text));
    }

    let result: OllamaEmbedResponse = response.json().await?;

    result
        .embeddings
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("No embedding returned from Ollama"))
}
