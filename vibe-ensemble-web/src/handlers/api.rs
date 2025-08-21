//! API handlers

use crate::{websocket::WebSocketManager, Error, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
use vibe_ensemble_core::{
    agent::Agent,
    issue::{Issue, IssuePriority, IssueStatus},
    knowledge::KnowledgeEntry,
    message::Message,
};
use vibe_ensemble_storage::StorageManager;

/// Health check endpoint
pub async fn health(State(storage): State<Arc<StorageManager>>) -> Result<Json<Value>> {
    // Check database health
    storage.health_check().await?;

    Ok(Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now(),
    })))
}

/// System statistics endpoint
pub async fn stats(State(storage): State<Arc<StorageManager>>) -> Result<Json<Value>> {
    let stats = storage.stats().await?;

    Ok(Json(json!({
        "agents": stats.agents_count,
        "issues": stats.issues_count,
        "messages": stats.messages_count,
        "knowledge": stats.knowledge_count,
        "prompts": stats.prompts_count,
        "timestamp": chrono::Utc::now(),
    })))
}

// ======================
// AGENT API ENDPOINTS
// ======================

/// Query parameters for agent listing
#[derive(Debug, Deserialize)]
pub struct AgentQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub status: Option<String>,
    pub agent_type: Option<String>,
}

/// List all agents with optional filtering
pub async fn agents_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<AgentQuery>,
) -> Result<Json<Value>> {
    let agents = storage.agents().list().await?;

    // Apply basic filtering
    let filtered_agents: Vec<Agent> = agents
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
        .skip(query.offset.unwrap_or(0) as usize)
        .take(query.limit.unwrap_or(100) as usize)
        .collect();

    Ok(Json(json!({
        "agents": filtered_agents,
        "total": filtered_agents.len(),
        "timestamp": chrono::Utc::now(),
    })))
}

/// Get specific agent details
pub async fn agent_detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let agent = storage
        .agents()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Agent with id {}", id)))?;

    Ok(Json(json!({
        "agent": agent,
        "timestamp": chrono::Utc::now(),
    })))
}

// ======================
// ISSUE API ENDPOINTS
// ======================

/// Issue creation/update request
#[derive(Debug, Deserialize)]
pub struct IssueRequest {
    pub title: String,
    pub description: String,
    pub priority: String,
    pub assigned_agent_id: Option<Uuid>,
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

/// List all issues with optional filtering
pub async fn issues_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<IssueQuery>,
) -> Result<Json<Value>> {
    let issues = storage.issues().list().await?;

    // Apply basic filtering
    let filtered_issues: Vec<Issue> = issues
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
        .skip(query.offset.unwrap_or(0) as usize)
        .take(query.limit.unwrap_or(100) as usize)
        .collect();

    Ok(Json(json!({
        "issues": filtered_issues,
        "total": filtered_issues.len(),
        "timestamp": chrono::Utc::now(),
    })))
}

/// Create a new issue
pub async fn issue_create(
    State(storage): State<Arc<StorageManager>>,
    State(ws_manager): State<Arc<WebSocketManager>>,
    Json(request): Json<IssueRequest>,
) -> Result<(StatusCode, Json<Value>)> {
    let priority = match request.priority.as_str() {
        "Low" => IssuePriority::Low,
        "Medium" => IssuePriority::Medium,
        "High" => IssuePriority::High,
        "Critical" => IssuePriority::Critical,
        _ => return Err(Error::BadRequest("Invalid priority".to_string())),
    };

    let mut issue = Issue::new(request.title.clone(), request.description, priority)?;
    issue.assigned_agent_id = request.assigned_agent_id;

    storage.issues().create(&issue).await?;

    // Broadcast to WebSocket clients
    let _ = ws_manager.broadcast_issue_created(issue.id, request.title, format!("{:?}", priority));

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "issue": issue,
            "timestamp": chrono::Utc::now(),
        })),
    ))
}

/// Get specific issue details
pub async fn issue_detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    Ok(Json(json!({
        "issue": issue,
        "timestamp": chrono::Utc::now(),
    })))
}

/// Update an existing issue
pub async fn issue_update(
    State(storage): State<Arc<StorageManager>>,
    State(ws_manager): State<Arc<WebSocketManager>>,
    Path(id): Path<Uuid>,
    Json(request): Json<IssueRequest>,
) -> Result<Json<Value>> {
    let mut issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    // Update issue fields
    issue.title = request.title;
    issue.description = request.description;
    issue.priority = match request.priority.as_str() {
        "Low" => IssuePriority::Low,
        "Medium" => IssuePriority::Medium,
        "High" => IssuePriority::High,
        "Critical" => IssuePriority::Critical,
        _ => return Err(Error::BadRequest("Invalid priority".to_string())),
    };
    issue.assigned_agent_id = request.assigned_agent_id;
    issue.updated_at = chrono::Utc::now();

    storage.issues().update(&issue).await?;

    // Broadcast to WebSocket clients
    let _ = ws_manager.broadcast_issue_status(issue.id, format!("{:?}", issue.status));

    Ok(Json(json!({
        "issue": issue,
        "timestamp": chrono::Utc::now(),
    })))
}

/// Delete an issue
pub async fn issue_delete(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let issue = storage
        .issues()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Issue with id {}", id)))?;

    storage.issues().delete(id).await?;

    Ok(StatusCode::NO_CONTENT)
}

// ======================
// KNOWLEDGE API ENDPOINTS
// ======================

/// Knowledge entry creation request
#[derive(Debug, Deserialize)]
pub struct KnowledgeRequest {
    pub title: String,
    pub content: String,
    pub category: String,
    pub tags: Vec<String>,
}

/// Query parameters for knowledge listing
#[derive(Debug, Deserialize)]
pub struct KnowledgeQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub category: Option<String>,
    pub tag: Option<String>,
    pub search: Option<String>,
}

/// List all knowledge entries with optional filtering
pub async fn knowledge_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<KnowledgeQuery>,
) -> Result<Json<Value>> {
    let entries = storage.knowledge().list().await?;

    // Apply basic filtering
    let filtered_entries: Vec<KnowledgeEntry> = entries
        .into_iter()
        .filter(|entry| {
            if let Some(category) = &query.category {
                entry.category == *category
            } else {
                true
            }
        })
        .filter(|entry| {
            if let Some(tag) = &query.tag {
                entry.tags.contains(tag)
            } else {
                true
            }
        })
        .filter(|entry| {
            if let Some(search) = &query.search {
                entry.title.to_lowercase().contains(&search.to_lowercase())
                    || entry
                        .content
                        .to_lowercase()
                        .contains(&search.to_lowercase())
            } else {
                true
            }
        })
        .skip(query.offset.unwrap_or(0) as usize)
        .take(query.limit.unwrap_or(100) as usize)
        .collect();

    Ok(Json(json!({
        "knowledge": filtered_entries,
        "total": filtered_entries.len(),
        "timestamp": chrono::Utc::now(),
    })))
}

/// Get specific knowledge entry details
pub async fn knowledge_detail(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>> {
    let entry = storage
        .knowledge()
        .find_by_id(id)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Knowledge entry with id {}", id)))?;

    Ok(Json(json!({
        "knowledge": entry,
        "timestamp": chrono::Utc::now(),
    })))
}

// ======================
// MESSAGE API ENDPOINTS
// ======================

/// Query parameters for message listing
#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub from_agent: Option<String>,
    pub to_agent: Option<String>,
    pub message_type: Option<String>,
}

/// List all messages with optional filtering
pub async fn messages_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Value>> {
    let messages = storage.messages().list().await?;

    // Apply basic filtering
    let filtered_messages: Vec<Message> = messages
        .into_iter()
        .filter(|message| {
            if let Some(from_agent) = &query.from_agent {
                message.from_agent == *from_agent
            } else {
                true
            }
        })
        .filter(|message| {
            if let Some(to_agent) = &query.to_agent {
                message
                    .to_agent
                    .as_ref()
                    .map(|to| to == to_agent)
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .filter(|message| {
            if let Some(msg_type) = &query.message_type {
                format!("{:?}", message.message_type).to_lowercase() == msg_type.to_lowercase()
            } else {
                true
            }
        })
        .skip(query.offset.unwrap_or(0) as usize)
        .take(query.limit.unwrap_or(100) as usize)
        .collect();

    Ok(Json(json!({
        "messages": filtered_messages,
        "total": filtered_messages.len(),
        "timestamp": chrono::Utc::now(),
    })))
}
