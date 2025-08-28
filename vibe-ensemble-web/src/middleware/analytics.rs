//! Navigation analytics middleware
//!
//! Tracks navigation patterns, response times, and errors for link validation
//! and navigation integrity monitoring.

use crate::{link_validator::LinkValidator, Result};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::{sync::Arc, time::Instant};
use vibe_ensemble_storage::StorageManager;

/// Navigation analytics middleware
pub async fn navigation_analytics_middleware(
    State(storage): State<Arc<StorageManager>>,
    request: Request,
    next: Next,
) -> Result<Response> {
    let start_time = Instant::now();
    let path = request.uri().path().to_string();
    let user_agent = extract_user_agent(request.headers());

    // Process the request
    let response = next.run(request).await;
    let response_time = start_time.elapsed();
    let status_code = response.status();

    // Record navigation analytics asynchronously
    tokio::spawn(async move {
        if let Err(e) =
            record_navigation_analytics(storage, path, user_agent, response_time, status_code).await
        {
            tracing::error!("Failed to record navigation analytics: {}", e);
        }
    });

    Ok(response)
}

/// Extract user agent from request headers
fn extract_user_agent(headers: &HeaderMap) -> Option<String> {
    headers
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(|s| s.to_string())
}

/// Record navigation analytics
async fn record_navigation_analytics(
    storage: Arc<StorageManager>,
    path: String,
    user_agent: Option<String>,
    response_time: std::time::Duration,
    status_code: StatusCode,
) -> Result<()> {
    // Create a link validator instance for analytics recording
    let validator = LinkValidator::new(
        crate::link_validator::ValidationConfig::default(),
        Some(storage),
    );

    // Record the navigation event
    validator.record_navigation(&path, user_agent.as_deref(), Some(response_time));

    // If this was an error response, we might want to record it separately
    if status_code.is_client_error() || status_code.is_server_error() {
        tracing::warn!(
            "Navigation error recorded: {} {} ({}ms)",
            status_code.as_u16(),
            path,
            response_time.as_millis()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_extract_user_agent() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "user-agent",
            HeaderValue::from_static("Mozilla/5.0 (Test Browser)"),
        );

        let user_agent = extract_user_agent(&headers);
        assert_eq!(user_agent, Some("Mozilla/5.0 (Test Browser)".to_string()));
    }

    #[test]
    fn test_extract_user_agent_missing() {
        let headers = HeaderMap::new();
        let user_agent = extract_user_agent(&headers);
        assert_eq!(user_agent, None);
    }
}
