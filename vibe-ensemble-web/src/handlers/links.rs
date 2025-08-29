//! Link validation handlers

use axum::{extract::State, response::IntoResponse, Json};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use vibe_ensemble_storage::StorageManager;

use crate::Result;

/// Link validation request
#[derive(Debug, Deserialize)]
pub struct LinkValidationRequest {
    pub urls: Vec<String>,
}

/// Link validation result for a single URL
#[derive(Debug, Serialize)]
pub struct LinkResult {
    pub url: String,
    pub status: String,
    pub status_code: Option<u16>,
    pub response_time_ms: Option<u64>,
    pub error: Option<String>,
}

/// Link validation response
#[derive(Debug, Serialize)]
pub struct LinkValidationResponse {
    pub results: Vec<LinkResult>,
    pub summary: LinkValidationSummary,
}

/// Summary statistics for link validation
#[derive(Debug, Serialize)]
pub struct LinkValidationSummary {
    pub total: usize,
    pub healthy: usize,
    pub broken: usize,
    pub warning: usize,
}

/// Validate a list of links and return their status
pub async fn validate_links(
    State(_storage): State<Arc<StorageManager>>,
    Json(request): Json<LinkValidationRequest>,
) -> Result<impl IntoResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Vibe-Ensemble-Link-Validator/1.0")
        .build()
        .map_err(|e| {
            crate::Error::Internal(anyhow::anyhow!("Failed to create HTTP client: {}", e))
        })?;

    let mut results = Vec::new();
    let mut healthy = 0;
    let mut broken = 0;
    let mut warning = 0;

    for url in request.urls {
        let start_time = std::time::Instant::now();

        let result = match validate_single_link(&client, &url).await {
            Ok((status_code, response_time)) => {
                let status = determine_link_status(status_code);
                match status.as_str() {
                    "healthy" => healthy += 1,
                    "warning" => warning += 1,
                    "broken" => broken += 1,
                    _ => broken += 1,
                }

                LinkResult {
                    url: url.clone(),
                    status,
                    status_code: Some(status_code),
                    response_time_ms: Some(response_time),
                    error: None,
                }
            }
            Err(error) => {
                broken += 1;
                LinkResult {
                    url: url.clone(),
                    status: "broken".to_string(),
                    status_code: None,
                    response_time_ms: Some(start_time.elapsed().as_millis() as u64),
                    error: Some(error.to_string()),
                }
            }
        };

        results.push(result);
    }

    let total = results.len();
    let summary = LinkValidationSummary {
        total,
        healthy,
        broken,
        warning,
    };

    let response = LinkValidationResponse { results, summary };

    Ok(Json(json!({
        "results": response.results,
        "summary": response.summary,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Validate a single link
async fn validate_single_link(
    client: &Client,
    url: &str,
) -> std::result::Result<(u16, u64), String> {
    let start_time = std::time::Instant::now();

    // Add timeout for the entire request
    let response = match timeout(Duration::from_secs(10), client.head(url).send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(e)) => return Err(format!("Request failed: {}", e)),
        Err(_) => return Err("Request timed out".to_string()),
    };

    let status_code = response.status().as_u16();
    let response_time = start_time.elapsed().as_millis() as u64;

    Ok((status_code, response_time))
}

/// Determine the status of a link based on its HTTP status code
fn determine_link_status(status_code: u16) -> String {
    match status_code {
        200..=299 => "healthy".to_string(),
        300..=399 => "warning".to_string(), // Redirects
        _ => "broken".to_string(),
    }
}

/// Trigger link validation with default URLs (for testing/demo purposes)
pub async fn trigger_link_validation(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    // Default set of URLs for demonstration
    let default_urls = vec![
        "https://httpbin.org/status/200".to_string(),
        "https://httpbin.org/status/404".to_string(),
        "https://httpbin.org/status/301".to_string(),
        "https://github.com".to_string(),
        "https://invalid-domain-that-does-not-exist-12345.com".to_string(),
    ];

    let request = LinkValidationRequest { urls: default_urls };

    validate_links(State(storage), Json(request)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_link_status() {
        assert_eq!(determine_link_status(200), "healthy");
        assert_eq!(determine_link_status(201), "healthy");
        assert_eq!(determine_link_status(301), "warning");
        assert_eq!(determine_link_status(302), "warning");
        assert_eq!(determine_link_status(404), "broken");
        assert_eq!(determine_link_status(500), "broken");
    }
}
