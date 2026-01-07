//! PDF generation API
//!
//! Converts HTML to PDF using Prince XML.

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::process::Command;
use tokio::fs;

use crate::api::insight;
use crate::error::AppError;
use crate::AppState;

use lazy_static::lazy_static;

lazy_static! {
    /// Prince XML executable path - configurable via PRINCE_PATH env var
    static ref PRINCE_PATH: String = std::env::var("PRINCE_PATH").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            "C:\\Program Files\\Prince\\engine\\bin\\prince.exe".to_string()
        } else {
            "/usr/bin/prince".to_string() // Linux/macOS default
        }
    });
}

#[derive(Debug, Deserialize)]
pub struct PdfRequest {
    pub html: String,
    pub filename: Option<String>,
}

/// Generate PDF from HTML using Prince
pub async fn generate_pdf(
    State(state): State<AppState>, // Inject State
    Json(req): Json<PdfRequest>,
) -> Result<Response<axum::body::Body>, AppError> {
    if req.html.is_empty() {
        return Err(AppError::BadRequest("Missing html content".to_string()));
    }

    let filename = req.filename.as_deref().unwrap_or("article");
    let temp_id = uuid::Uuid::new_v4().to_string();
    let temp_dir = std::env::temp_dir()
        .join("wechat-insights-pdf")
        .join(&temp_id); // Use a unique subdir
    let temp_pdf = temp_dir.join(format!("{}.pdf", temp_id));
    let images_dir = temp_dir.join("images"); // Subdir for images

    // Ensure temp directories exist
    fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create temp dir: {}", e)))?;

    // --- Process Images with Cache Check ---
    // Create a client (direct connection, no proxy for single export for now, or could trust system proxy)
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| AppError::Internal(format!("Failed to build client: {}", e)))?;

    // Call process_html_images to rewrite HTML to point to local temp images (fetched from DB or net)
    // We pass None for gateway as single export doesn't currently support custom gateway selection
    let (processed_html, _downloaded_images) = insight::process_html_images(
        &client,
        &req.html,
        &images_dir,
        &temp_id, // Prefix not really used in current impl but required
        None,
        None,
        &state.db_pool,
        true, // Single export PDF uses absolute paths
    )
    .await;

    // Call helper with PROCESSED HTML
    match convert_html_to_pdf(&processed_html, &temp_pdf, filename, Some(&temp_dir)).await {
        Ok(_) => {}
        Err(e) => {
            // cleanup on error
            let _ = fs::remove_dir_all(&temp_dir).await;
            return Err(e);
        }
    }

    // Read the generated PDF
    let pdf_bytes = match fs::read(&temp_pdf).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let _ = fs::remove_dir_all(&temp_dir).await;
            return Err(AppError::Internal(format!("Failed to read PDF: {}", e)));
        }
    };

    // Cleanup temp directory (includes images and html and pdf)
    let _ = fs::remove_dir_all(&temp_dir).await;

    // Build response with PDF
    let encoded_filename = urlencoding::encode(filename);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}.pdf\"", encoded_filename),
        )
        .header(header::CONTENT_LENGTH, pdf_bytes.len())
        .body(axum::body::Body::from(pdf_bytes))
        .unwrap();

    Ok(response)
}

/// Helper: Convert HTML string to PDF at specified path
pub async fn convert_html_to_pdf(
    html: &str,
    output_path: &std::path::Path,
    title: &str,
    working_dir: Option<&std::path::Path>, // Added optional working_dir
) -> Result<(), AppError> {
    let temp_id = uuid::Uuid::new_v4().to_string();
    let default_temp_dir = std::env::temp_dir().join("wechat-insights-pdf");

    // Use provided working_dir or default
    let temp_dir = working_dir.unwrap_or(default_temp_dir.as_path());
    let temp_html = temp_dir.join(format!("{}.html", temp_id));

    if working_dir.is_none() {
        fs::create_dir_all(&temp_dir).await?;
    }

    // Build full HTML with Prince-friendly styles
    let full_html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>{}</title>
  <style>
    /* Force font override with !important to ignore article inline styles */
    * {{
      font-family: "Noto Sans CJK SC", "WenQuanYi Micro Hei", "Microsoft YaHei", "SimHei", sans-serif !important;
      overflow-wrap: break-word;
      word-wrap: break-word;
      /* Aggressive Layout Resets */
      max-width: 100% !important;
      height: auto !important;
      position: static !important; /* Disable absolute positioning which causes overlaps */
      float: none !important;      /* Disable floats */
      margin-left: 0 !important;   /* Reset horizontal margins that might cause shift */
      margin-right: 0 !important;
      text-indent: 0 !important;   /* Fix weird indents */
    }}
    html, body {{
      font-family: "Noto Sans CJK SC", "WenQuanYi Micro Hei", "Microsoft YaHei", "SimHei", sans-serif !important;
      font-size: 14px;
      line-height: 1.6;
      color: #333;
      margin: 0;
      padding: 0;
    }}
    img {{
      max-width: 100% !important;
      height: auto !important;
      display: block; 
      margin: 10px auto !important;
    }}
    /* Reset common WeChat article containers */
    section, div, p {{
        max-width: 100% !important;
        box-sizing: border-box !important;
        height: auto !important;
    }}
    h1, h2, h3 {{
      font-weight: bold;
      page-break-after: avoid;
      line-height: 1.4;
      margin-top: 1em !important;
      margin-bottom: 0.5em !important;
    }}
    p {{
      orphans: 3;
      widows: 3;
      margin-bottom: 1em !important;
    }}
  </style>
</head>
<body>
{}
</body>
</html>"#,
        title, 
        html
    );

    // Write HTML to temp file
    fs::write(&temp_html, &full_html).await?;

    // Execute Prince
    tracing::info!("[PDF] Generating PDF with Prince: {}", temp_html.display());

    let output = Command::new(PRINCE_PATH.as_str())
        .arg(&temp_html)
        .arg("--verbose") // Enable verbose logging
        .arg("-o")
        .arg(output_path)
        .output();

    match output {
        Ok(result) => {
            if !result.status.success() {
                let stderr = String::from_utf8_lossy(&result.stderr);
                tracing::error!("[PDF] Prince failed: {}", stderr);

                // Cleanup (only clean the file we created)
                let _ = fs::remove_file(&temp_html).await;

                return Err(AppError::Internal(format!("Prince failed: {}", stderr)));
            }
        }
        Err(e) => {
            // Cleanup
            let _ = fs::remove_file(&temp_html).await;

            // Check if Prince is not installed
            if e.kind() == std::io::ErrorKind::NotFound {
                return Err(AppError::Internal(
                    "Prince XML not found. Please install from https://www.princexml.com/"
                        .to_string(),
                ));
            }
            return Err(AppError::Internal(format!(
                "Failed to execute Prince: {}",
                e
            )));
        }
    }

    // Cleanup HTML temp
    let _ = fs::remove_file(&temp_html).await;

    Ok(())
}
