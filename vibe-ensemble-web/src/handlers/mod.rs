//! Web handlers for the Vibe Ensemble dashboard

pub mod dashboard;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

use crate::handlers::dashboard::index;

use crate::Result;

/// Health check endpoint
pub async fn health(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    // Check database health
    storage
        .health_check()
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "status": "healthy",
        "service": "vibe-ensemble-web",
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Dashboard page handler
pub async fn dashboard(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    // Delegate to the proper dashboard handler
    index(State(storage)).await
}

/// Query parameters for agent listing
#[derive(Debug, Deserialize)]
pub struct AgentQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub status: Option<String>,
    pub agent_type: Option<String>,
}

/// List agents API endpoint
pub async fn agents_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<AgentQuery>,
) -> Result<impl IntoResponse> {
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let agents = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Apply basic filtering
    let filtered_agents: Vec<_> = agents
        .into_iter()
        .filter(|agent| {
            if let Some(status) = &query.status {
                format!("{:?}", agent.status).to_lowercase() == status.to_lowercase()
            } else {
                true
            }
        })
        .filter(|agent| {
            if let Some(agent_type) = &query.agent_type {
                format!("{:?}", agent.agent_type).to_lowercase() == agent_type.to_lowercase()
            } else {
                true
            }
        })
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "agents": filtered_agents,
        "total": filtered_agents.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Query parameters for issue listing
#[derive(Debug, Deserialize)]
pub struct IssueQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub assigned_agent_id: Option<Uuid>,
}

/// List issues API endpoint  
pub async fn issues_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<IssueQuery>,
) -> Result<impl IntoResponse> {
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Apply basic filtering
    let filtered_issues: Vec<_> = issues
        .into_iter()
        .filter(|issue| {
            if let Some(status) = &query.status {
                format!("{:?}", issue.status).to_lowercase() == status.to_lowercase()
            } else {
                true
            }
        })
        .filter(|issue| {
            if let Some(priority) = &query.priority {
                format!("{:?}", issue.priority).to_lowercase() == priority.to_lowercase()
            } else {
                true
            }
        })
        .filter(|issue| {
            if let Some(agent_id) = query.assigned_agent_id {
                issue.assigned_agent_id == Some(agent_id)
            } else {
                true
            }
        })
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "issues": filtered_issues,
        "total": filtered_issues.len(),
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Issue creation request
#[derive(Debug, Deserialize)]
pub struct IssueRequest {
    pub title: String,
    pub description: String,
    pub priority: String,
    pub assigned_agent_id: Option<Uuid>,
}

/// Create issue API endpoint
pub async fn issues_create(
    State(storage): State<Arc<StorageManager>>,
    Json(request): Json<IssueRequest>,
) -> Result<impl IntoResponse> {
    use vibe_ensemble_core::issue::{Issue, IssuePriority};

    let priority = match request.priority.to_lowercase().as_str() {
        "low" => IssuePriority::Low,
        "medium" => IssuePriority::Medium,
        "high" => IssuePriority::High,
        "critical" => IssuePriority::Critical,
        _ => return Err(crate::Error::BadRequest("Invalid priority".to_string())),
    };

    let mut issue = Issue::new(request.title.clone(), request.description, priority)
        .map_err(crate::Error::Core)?;
    issue.assigned_agent_id = request.assigned_agent_id;

    storage
        .issues()
        .create(&issue)
        .await
        .map_err(crate::Error::Storage)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "issue": issue,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    ))
}

/// System statistics API endpoint
pub async fn system_stats(State(storage): State<Arc<StorageManager>>) -> Result<impl IntoResponse> {
    let stats = storage.stats().await.map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "agents": {
            "total": stats.agents_count,
            "active": stats.agents_count // TODO: implement active agent counting
        },
        "issues": {
            "total": stats.issues_count,
            "open": stats.issues_count // TODO: implement open issue counting
        },
        "messages": stats.messages_count,
        "knowledge": stats.knowledge_count,
        "prompts": stats.prompts_count,
        "templates": stats.templates_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get specific agent details
pub async fn agent_get(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let agent = storage
        .agents()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound(format!("Agent with id {}", id)))?;

    Ok(Json(json!({
        "agent": agent,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get specific issue details
pub async fn issue_get(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound(format!("Issue with id {}", id)))?;

    Ok(Json(json!({
        "issue": issue,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Update an existing issue
pub async fn issue_update(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
    Json(request): Json<IssueRequest>,
) -> Result<impl IntoResponse> {
    use vibe_ensemble_core::issue::IssuePriority;

    let mut issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound(format!("Issue with id {}", id)))?;

    // Update issue fields
    issue.title = request.title;
    issue.description = request.description;
    issue.priority = match request.priority.to_lowercase().as_str() {
        "low" => IssuePriority::Low,
        "medium" => IssuePriority::Medium,
        "high" => IssuePriority::High,
        "critical" => IssuePriority::Critical,
        _ => return Err(crate::Error::BadRequest("Invalid priority".to_string())),
    };
    issue.assigned_agent_id = request.assigned_agent_id;
    issue.updated_at = chrono::Utc::now();

    storage
        .issues()
        .update(&issue)
        .await
        .map_err(crate::Error::Storage)?;

    Ok(Json(json!({
        "issue": issue,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Delete an issue
pub async fn issue_delete(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let _issue = storage
        .issues()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound(format!("Issue with id {}", id)))?;

    storage
        .issues()
        .delete(id)
        .await
        .map_err(crate::Error::Storage)?;

    Ok(StatusCode::NO_CONTENT)
}
