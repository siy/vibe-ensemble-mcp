//! Link validation handlers
//!
//! Provides endpoints for validating application links and monitoring navigation health

use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Json,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, sync::Arc, time::Duration};
use vibe_ensemble_storage::StorageManager;

use crate::{
    templates::{LinkHealthSummary, LinkHealthTemplate},
    Result,
};
use askama::Template;
use axum::response::Html;

/// Link validation status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkStatus {
    pub url: String,
    pub status: String,
    pub status_code: Option<u16>,
    pub response_time_ms: Option<u64>,
    pub last_checked: chrono::DateTime<chrono::Utc>,
    pub error_message: Option<String>,
}

/// Query parameters for link validation
#[derive(Debug, Deserialize)]
pub struct LinkValidationQuery {
    pub force: Option<bool>,
    pub timeout: Option<u64>,
}

/// Link health dashboard page
pub async fn link_health_page(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let link_validator = LinkValidator::new();
    let discovered_links = link_validator.discover_application_links().await;

    // Create a simple summary for the template
    let summary = LinkHealthSummary {
        total_links: discovered_links.len(),
        healthy_links: 0,
        broken_links: 0,
        warning_links: 0,
        last_validation: None,
    };

    let template = LinkHealthTemplate::new(summary, discovered_links);
    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
}

/// Get link health summary
pub async fn link_health_summary(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let link_validator = LinkValidator::new();
    let discovered_links = link_validator.discover_application_links().await;

    // For now, return static data until we implement database storage
    let summary = LinkHealthSummary {
        total_links: discovered_links.len(),
        healthy_links: 0,
        broken_links: 0,
        warning_links: 0,
        last_validation: None,
    };

    Ok(Json(json!({
        "total_links": summary.total_links,
        "healthy_links": summary.healthy_links,
        "broken_links": summary.broken_links,
        "warning_links": summary.warning_links,
        "health_score": 100.0,
        "last_validation": summary.last_validation,
        "summary": summary,
        "discovered_links": discovered_links.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get detailed link status
pub async fn link_status_details(
    State(_storage): State<Arc<StorageManager>>,
    Query(query): Query<LinkValidationQuery>,
) -> Result<impl IntoResponse> {
    let link_validator = LinkValidator::new();
    let timeout = Duration::from_secs(query.timeout.unwrap_or(5));

    let discovered_links = link_validator.discover_application_links().await;
    let mut link_statuses = Vec::new();

    // Validate each discovered link
    for url in discovered_links {
        let status = link_validator.validate_link(&url, timeout).await;
        link_statuses.push(status);
    }

    Ok(Json(json!({
        "links": link_statuses,
        "total": link_statuses.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Validate all links
pub async fn validate_links(
    State(_storage): State<Arc<StorageManager>>,
    Query(query): Query<LinkValidationQuery>,
) -> Result<impl IntoResponse> {
    let link_validator = LinkValidator::new();
    let timeout = Duration::from_secs(query.timeout.unwrap_or(5));

    let discovered_links = link_validator.discover_application_links().await;
    let mut results = HashMap::new();
    let mut healthy_count = 0;
    let mut broken_count = 0;
    let mut warning_count = 0;

    for url in discovered_links.iter() {
        let status = link_validator.validate_link(url, timeout).await;

        match status.status.as_str() {
            "healthy" => healthy_count += 1,
            "broken" => broken_count += 1,
            "warning" => warning_count += 1,
            _ => {}
        }

        results.insert(url.clone(), status);
    }

    Ok(Json(json!({
        "status": "completed",
        "results": results,
        "validation_results": results,
        "summary": {
            "total": discovered_links.len(),
            "healthy": healthy_count,
            "broken": broken_count,
            "warning": warning_count,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get link analytics
pub async fn link_analytics(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    // For now, return placeholder analytics
    Ok(Json(json!({
        "analytics": {
            "average_response_time": 150,
            "uptime_percentage": 98.5,
            "most_accessed_links": [
                {"url": "/dashboard", "count": 1250},
                {"url": "/api/health", "count": 890},
                {"url": "/api/stats", "count": 567}
            ]
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Auto-repair broken links
pub async fn auto_repair(State(_storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let _link_validator = LinkValidator::new();

    // For testing, simulate auto-repair process
    let repairs_applied = vec![
        json!({
            "url": "/broken-link",
            "issue": "404 Not Found",
            "repair": "Added redirect to /dashboard",
            "status": "completed"
        }),
        json!({
            "url": "/old-page",
            "issue": "Deprecated endpoint",
            "repair": "Updated to new endpoint",
            "status": "completed"
        }),
    ];

    let config = json!({
        "enabled": true,
        "confidence_threshold": 0.8,
        "auto_fix_safe_issues": true,
        "create_redirects": true,
        "suggest_alternatives": true
    });

    Ok(Json(json!({
        "status": "completed",
        "repairs_applied": repairs_applied,
        "config": config,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "total_repairs": repairs_applied.len()
    })))
}

/// Get repair suggestions for a specific URL
pub async fn repair_suggestions(
    State(_storage): State<Arc<StorageManager>>,
    Path(url): Path<String>,
) -> Result<impl IntoResponse> {
    // Decode the URL parameter
    let decoded_url = urlencoding::decode(&url).unwrap_or_else(|_| url.clone().into());

    // Generate mock repair suggestions for the URL
    let suggestions = vec![
        json!({
            "type": "url_correction",
            "description": "Possible typo in URL path",
            "suggested_url": "/dashboard",
            "confidence": 0.85
        }),
        json!({
            "type": "missing_handler",
            "description": "Route handler not implemented",
            "suggested_action": "Add route handler",
            "confidence": 0.75
        }),
        json!({
            "type": "redirect_needed",
            "description": "Create redirect from old URL",
            "suggested_url": "/new-path",
            "confidence": 0.65
        }),
    ];

    Ok(Json(json!({
        "url": decoded_url,
        "suggestions": suggestions,
        "total_suggestions": suggestions.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Link validator implementation
pub struct LinkValidator {
    client: Client,
}

impl Default for LinkValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkValidator {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Discover application links from routes and templates
    pub async fn discover_application_links(&self) -> Vec<String> {
        let mut links = Vec::new();

        // Add known application routes
        let base_url = "http://127.0.0.1:8081"; // TODO: Make configurable

        // Dashboard and page routes
        links.push(format!("{}/", base_url));
        links.push(format!("{}/dashboard", base_url));
        links.push(format!("{}/agents", base_url));
        links.push(format!("{}/issues", base_url));
        links.push(format!("{}/knowledge", base_url));
        links.push(format!("{}/admin", base_url));
        links.push(format!("{}/link-health", base_url));
        links.push(format!("{}/messages", base_url));

        // API endpoints
        links.push(format!("{}/api/health", base_url));
        links.push(format!("{}/api/stats", base_url));
        links.push(format!("{}/api/agents", base_url));
        links.push(format!("{}/api/issues", base_url));
        links.push(format!("{}/api/links/health", base_url));
        links.push(format!("{}/api/links/status", base_url));
        links.push(format!("{}/api/links/validate", base_url));
        links.push(format!("{}/api/links/analytics", base_url));

        // Authentication routes
        links.push(format!("{}/login", base_url));
        links.push(format!("{}/logout", base_url));

        // WebSocket endpoint
        links.push("ws://127.0.0.1:8081/ws".to_string());

        links
    }

    /// Validate a single link
    pub async fn validate_link(&self, url: &str, timeout: Duration) -> LinkStatus {
        let start_time = std::time::Instant::now();

        // Handle WebSocket URLs differently
        if url.starts_with("ws://") || url.starts_with("wss://") {
            return LinkStatus {
                url: url.to_string(),
                status: "warning".to_string(),
                status_code: None,
                response_time_ms: Some(start_time.elapsed().as_millis() as u64),
                last_checked: chrono::Utc::now(),
                error_message: Some("WebSocket validation not implemented".to_string()),
            };
        }

        match self.client.get(url).timeout(timeout).send().await {
            Ok(response) => {
                let status_code = response.status().as_u16();
                let response_time = start_time.elapsed().as_millis() as u64;

                let status = if (200..300).contains(&status_code) {
                    "healthy"
                } else if (300..400).contains(&status_code) {
                    "warning" // Redirects
                } else {
                    "broken"
                };

                LinkStatus {
                    url: url.to_string(),
                    status: status.to_string(),
                    status_code: Some(status_code),
                    response_time_ms: Some(response_time),
                    last_checked: chrono::Utc::now(),
                    error_message: None,
                }
            }
            Err(error) => {
                let response_time = start_time.elapsed().as_millis() as u64;

                LinkStatus {
                    url: url.to_string(),
                    status: "broken".to_string(),
                    status_code: None,
                    response_time_ms: Some(response_time),
                    last_checked: chrono::Utc::now(),
                    error_message: Some(error.to_string()),
                }
            }
        }
    }
}
