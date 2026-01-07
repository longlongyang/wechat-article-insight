//! Gemini LLM provider implementation

use anyhow::Result;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Generate embedding using Gemini gemini-embedding-001
/// Supports flexible output dimensions: 128-3072 (recommended: 768, 1536, 3072)
pub async fn generate_embedding(api_key: &str, text: &str) -> Result<Vec<f32>> {
    generate_embedding_with_dim(api_key, text, None).await
}

/// Generate embedding with custom output dimension
pub async fn generate_embedding_with_dim(
    api_key: &str,
    text: &str,
    output_dim: Option<i32>,
) -> Result<Vec<f32>> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}/models/gemini-embedding-001:embedContent?key={}",
        GEMINI_API_BASE, api_key
    );

    let mut request_body = serde_json::json!({
        "content": {
            "parts": [{"text": text}]
        }
    });

    // Add output dimension if specified (MRL technique allows truncation)
    if let Some(dim) = output_dim {
        request_body["outputDimensionality"] = serde_json::json!(dim);
    }

    let response = client.post(&url).json(&request_body).send().await?;

    if !response.status().is_success() {
        let error_text = response.text().await?;
        return Err(anyhow::anyhow!(
            "Gemini Embedding API error: {}",
            error_text
        ));
    }

    let json: serde_json::Value = response.json().await?;

    let values = json
        .get("embedding")
        .and_then(|e| e.get("values"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("Invalid Gemini embedding response"))?;

    let embedding: Vec<f32> = values
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect();

    if embedding.is_empty() {
        return Err(anyhow::anyhow!("Empty embedding returned from Gemini"));
    }

    Ok(embedding)
}
