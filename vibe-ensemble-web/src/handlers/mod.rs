//! Web handlers for the Vibe Ensemble dashboard

use axum::{extract::State, response::IntoResponse, Json};
use serde_json::json;
use std::sync::Arc;
use vibe_ensemble_storage::StorageManager;

use crate::{templates::DashboardTemplate, Result};

/// Health check endpoint
pub async fn health() -> Result<impl IntoResponse> {
    Ok(Json(json!({
        "status": "ok",
        "service": "vibe-ensemble-web",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Dashboard page handler
pub async fn dashboard(State(_storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let template = DashboardTemplate {
        title: "Vibe Ensemble Dashboard".to_string(),
        active_agents: 0,
        open_issues: 0,
        recent_activity: vec![],
        current_page: "dashboard".to_string(),
    };

    Ok(template)
}

/// List agents API endpoint
pub async fn agents_list(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let agents = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "agents": agents,
        "count": agents.len()
    })))
}

/// List issues API endpoint  
pub async fn issues_list(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "issues": issues,
        "count": issues.len()
    })))
}

/// Create issue API endpoint
pub async fn issues_create(
    State(_storage): State<Arc<StorageManager>>,
    Json(_payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse> {
    // This is a placeholder - would need proper issue creation logic
    Ok(Json(json!({
        "status": "created",
        "message": "Issue creation not yet implemented"
    })))
}

/// System statistics API endpoint
pub async fn system_stats(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let agent_count = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?
        .len();

    let issue_count = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?
        .len();

    Ok(Json(json!({
        "agents": {
            "total": agent_count,
            "active": agent_count // Placeholder
        },
        "issues": {
            "total": issue_count,
            "open": issue_count // Placeholder
        },
        "system": {
            "uptime": "N/A",
            "memory_usage": "N/A"
        }
    })))
}
