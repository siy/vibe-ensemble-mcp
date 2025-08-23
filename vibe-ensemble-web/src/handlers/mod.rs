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
pub async fn dashboard(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    // Get real data from storage
    let agents = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Create some sample recent activity if data exists
    let recent_activity = if !agents.is_empty() || !issues.is_empty() {
        use crate::templates::ActivityEntry;
        vec![ActivityEntry {
            timestamp: chrono::Utc::now().format("%H:%M").to_string(),
            message: format!(
                "Dashboard initialized with {} agents and {} issues",
                agents.len(),
                issues.len()
            ),
            activity_type: "system".to_string(),
        }]
    } else {
        vec![]
    };

    let template = DashboardTemplate {
        title: "Vibe Ensemble Dashboard".to_string(),
        active_agents: agents.len(),
        open_issues: issues.len(),
        recent_activity,
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
    State(storage): State<Arc<StorageManager>>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse> {
    use vibe_ensemble_core::issue::{Issue, IssuePriority};

    // Extract basic fields from payload
    let title = payload
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled Issue")
        .to_string();

    let description = payload
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let priority = payload
        .get("priority")
        .and_then(|v| v.as_str())
        .and_then(|s| match s.to_lowercase().as_str() {
            "low" => Some(IssuePriority::Low),
            "medium" => Some(IssuePriority::Medium),
            "high" => Some(IssuePriority::High),
            "critical" => Some(IssuePriority::Critical),
            _ => None,
        })
        .unwrap_or(IssuePriority::Medium);

    // Create the issue
    let issue = Issue::new(title, description, priority).map_err(crate::Error::Core)?;

    // Store the issue
    storage
        .issues()
        .create(&issue)
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "status": "created",
        "issue": issue,
        "message": "Issue created successfully"
    })))
}

/// Get single issue API endpoint
pub async fn issue_get(
    State(storage): State<Arc<StorageManager>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?;

    match issue {
        Some(issue) => Ok(Json(json!({
            "issue": issue,
            "status": "found"
        }))),
        None => Ok(Json(json!({
            "status": "not_found",
            "message": "Issue not found"
        }))),
    }
}

/// Update issue API endpoint
pub async fn issue_update(
    State(storage): State<Arc<StorageManager>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse> {
    use vibe_ensemble_core::issue::IssueStatus;

    // Get existing issue
    let mut issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound("Issue not found".to_string()))?;

    // Update fields if provided
    if let Some(title) = payload.get("title").and_then(|v| v.as_str()) {
        issue.title = title.to_string();
    }

    if let Some(description) = payload.get("description").and_then(|v| v.as_str()) {
        issue.description = description.to_string();
    }

    if let Some(status_str) = payload.get("status").and_then(|v| v.as_str()) {
        if let Some(status) = match status_str.to_lowercase().as_str() {
            "open" => Some(IssueStatus::Open),
            "in_progress" | "in-progress" => Some(IssueStatus::InProgress),
            "resolved" => Some(IssueStatus::Resolved),
            "closed" => Some(IssueStatus::Closed),
            _ => None,
        } {
            issue.status = status;
        }
    }

    // Update the updated_at timestamp
    issue.updated_at = chrono::Utc::now();

    // Update the issue
    storage
        .issues()
        .update(&issue)
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "status": "updated",
        "issue": issue,
        "message": "Issue updated successfully"
    })))
}

/// Delete issue API endpoint
pub async fn issue_delete(
    State(storage): State<Arc<StorageManager>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse> {
    storage
        .issues()
        .delete(id)
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "status": "deleted",
        "message": "Issue deleted successfully"
    })))
}

/// Get single agent API endpoint
pub async fn agent_get(
    State(storage): State<Arc<StorageManager>>,
    axum::extract::Path(id): axum::extract::Path<uuid::Uuid>,
) -> Result<impl IntoResponse> {
    let agent = storage
        .agents()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?;

    match agent {
        Some(agent) => Ok(Json(json!({
            "agent": agent,
            "status": "found"
        }))),
        None => Ok(Json(json!({
            "status": "not_found",
            "message": "Agent not found"
        }))),
    }
}

/// System statistics API endpoint
pub async fn system_stats(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    use vibe_ensemble_core::{agent::AgentStatus, issue::IssueStatus};

    let agents = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Count active agents (assuming all listed agents are active for now)
    let active_agents = agents
        .iter()
        .filter(|agent| matches!(agent.status, AgentStatus::Online))
        .count();

    // Count open issues
    let open_issues = issues
        .iter()
        .filter(|issue| matches!(issue.status, IssueStatus::Open | IssueStatus::InProgress))
        .count();

    // Get basic system stats
    let start_time = std::time::SystemTime::now();
    let uptime_secs = start_time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(Json(json!({
        "agents": {
            "total": agents.len(),
            "active": active_agents,
            "connected": active_agents
        },
        "issues": {
            "total": issues.len(),
            "open": open_issues,
            "closed": issues.len() - open_issues
        },
        "system": {
            "uptime_seconds": uptime_secs,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION")
        }
    })))
}
