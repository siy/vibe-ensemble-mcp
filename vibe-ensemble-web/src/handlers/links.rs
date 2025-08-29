//! Simple link health page handlers
//!
//! Provides a basic link health dashboard page for manual testing

use axum::{extract::State, response::IntoResponse};
use serde_json::json;
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

use crate::{
    templates::{LinkHealthSummary, LinkHealthTemplate},
    Result,
};
use askama::Template;
use axum::response::Html;

/// Link health dashboard page
pub async fn link_health_page(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    // Static list of known application links for manual testing
    let discovered_links = vec![
        "http://127.0.0.1:8081/".to_string(),
        "http://127.0.0.1:8081/dashboard".to_string(),
        "http://127.0.0.1:8081/messages".to_string(),
        "http://127.0.0.1:8081/link-health".to_string(),
        "http://127.0.0.1:8081/api/health".to_string(),
        "http://127.0.0.1:8081/api/stats".to_string(),
        "http://127.0.0.1:8081/api/agents".to_string(),
        "http://127.0.0.1:8081/api/issues".to_string(),
        "http://127.0.0.1:8081/api/messages".to_string(),
        "http://127.0.0.1:8081/api/links/health".to_string(),
        "ws://127.0.0.1:8081/ws".to_string(),
    ];

    // Static summary for the template
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

/// Get link health summary (static data)
pub async fn link_health_summary(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let total_links = 11; // Static count of known links

    Ok(Json(json!({
        "total_links": total_links,
        "healthy_links": 0,
        "broken_links": 0,
        "warning_links": 0,
        "health_score": 100.0,
        "last_validation": null,
        "summary": {
            "total_links": total_links,
            "healthy_links": 0,
            "broken_links": 0,
            "warning_links": 0,
            "last_validation": null
        },
        "discovered_links": total_links,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "note": "Server-side validation removed. Use scripts/test-links.sh for validation."
    })))
}

/// Basic link status endpoint (returns static data)
pub async fn link_status_details(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "links": [],
        "total": 0,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "note": "Server-side validation removed. Use scripts/test-links.sh for validation."
    })))
}

/// Validation endpoint removed - returns informational message
pub async fn validate_links(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "status": "disabled",
        "message": "Server-side link validation has been removed for security and performance reasons.",
        "alternative": "Use scripts/test-links.sh to test links locally or in CI.",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Analytics endpoint (returns empty data)
pub async fn link_analytics(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "analytics": {
            "note": "Link analytics removed. Use scripts/test-links.sh for basic validation."
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Auto-repair endpoint removed
pub async fn auto_repair(State(_storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "status": "disabled",
        "message": "Auto-repair functionality has been removed.",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Repair suggestions endpoint removed
pub async fn repair_suggestions(
    State(_storage): State<Arc<StorageManager>>,
    _path: axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "suggestions": [],
        "message": "Repair suggestions functionality has been removed.",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}
