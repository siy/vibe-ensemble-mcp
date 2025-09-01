//! Web handlers for the Vibe Ensemble dashboard

pub mod agents;
pub mod dashboard;
pub mod issues;
pub mod knowledge;
pub mod links;

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

use crate::handlers::dashboard::index;

use crate::Result;
use vibe_ensemble_core::{agent::AgentStatus, agent::AgentType, issue::IssueStatus};

/// Helper function to match agent status case-insensitively
fn matches_agent_status(status: &AgentStatus, filter: &str) -> bool {
    let filter_lower = filter.to_lowercase();
    match status {
        AgentStatus::Online => filter_lower == "online",
        AgentStatus::Offline => filter_lower == "offline",
        AgentStatus::Busy => filter_lower == "busy",
        _ => false,
    }
}

/// Helper function to match agent type case-insensitively
fn matches_agent_type(agent_type: &AgentType, filter: &str) -> bool {
    let filter_lower = filter.to_lowercase();
    match agent_type {
        AgentType::Worker => filter_lower == "worker",
        AgentType::Coordinator => filter_lower == "coordinator",
    }
}

/// Helper function to match issue status case-insensitively
fn matches_issue_status(status: &IssueStatus, filter: &str) -> bool {
    let filter_lower = filter.to_lowercase();
    match status {
        IssueStatus::Open => filter_lower == "open",
        IssueStatus::InProgress => filter_lower == "in_progress" || filter_lower == "inprogress",
        IssueStatus::Blocked { .. } => filter_lower == "blocked",
        IssueStatus::Resolved => filter_lower == "resolved",
        IssueStatus::Closed => filter_lower == "closed",
    }
}

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

/// Messages page handler
pub async fn messages_page(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    use crate::metrics::MetricsCollector;
    use crate::templates::MessagesTemplate;

    // Get message analytics
    let messages = storage
        .messages()
        .list_recent(1000)
        .await
        .map_err(crate::Error::Storage)?;

    let total_count = messages.len();
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let recent_count = messages
        .iter()
        .filter(|msg| msg.created_at > cutoff)
        .count();

    // Count by type and priority
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut priority_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut delivery_stats: (usize, usize) = (0usize, 0usize);

    for message in &messages {
        let msg_type = format!("{:?}", message.message_type);
        *type_counts.entry(msg_type).or_insert(0usize) += 1usize;

        let priority = format!("{:?}", message.metadata.priority);
        *priority_counts.entry(priority).or_insert(0usize) += 1usize;

        if message.is_delivered() {
            delivery_stats.0 += 1;
        } else {
            delivery_stats.1 += 1;
        }
    }

    // Count active conversations (messages with correlation_id)
    let conversation_count = messages
        .iter()
        .filter_map(|msg| msg.metadata.correlation_id)
        .collect::<std::collections::HashSet<_>>()
        .len();

    // Create message stats structure
    let message_stats = serde_json::json!({
        "total_messages": total_count,
        "recent_messages_24h": recent_count,
        "by_type": type_counts,
        "by_priority": priority_counts,
        "delivery_stats": {
            "delivered": delivery_stats.0,
            "undelivered": delivery_stats.1,
            "delivery_rate_percent": if total_count > 0 {
                (delivery_stats.0 as f64 / total_count as f64 * 100.0).round()
            } else {
                0.0
            }
        }
    });

    // Collect system metrics
    let metrics_collector = MetricsCollector::new(storage.clone());
    let system_metrics = metrics_collector.collect_system_metrics().await;
    let storage_metrics = metrics_collector.collect_storage_metrics().await;

    let template = MessagesTemplate::new(message_stats, conversation_count)
        .with_system_metrics(system_metrics)
        .with_storage_metrics(storage_metrics);

    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("{}", e)))?;
    Ok(Html(rendered))
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
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0).max(0);

    let agents = storage
        .agents()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Apply basic filtering
    let all_filtered: Vec<_> = agents
        .into_iter()
        .filter(|agent| {
            if let Some(status) = &query.status {
                matches_agent_status(&agent.status, status)
            } else {
                true
            }
        })
        .filter(|agent| {
            if let Some(agent_type) = &query.agent_type {
                matches_agent_type(&agent.agent_type, agent_type)
            } else {
                true
            }
        })
        .collect();

    let total = all_filtered.len();
    let filtered_agents: Vec<_> = all_filtered
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "agents": filtered_agents,
        "total": total,
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
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    let offset = query.offset.unwrap_or(0).max(0);

    let issues = storage
        .issues()
        .list()
        .await
        .map_err(crate::Error::Storage)?;

    // Apply basic filtering
    let all_filtered: Vec<_> = issues
        .into_iter()
        .filter(|issue| {
            if let Some(status) = &query.status {
                matches_issue_status(&issue.status, status)
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
        .collect();

    let total = all_filtered.len();
    let filtered_issues: Vec<_> = all_filtered
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "issues": filtered_issues,
        "total": total,
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
    pub status: Option<String>,
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
    use vibe_ensemble_core::issue::{IssuePriority, IssueStatus};

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

    // Update status if provided
    if let Some(status_str) = request.status {
        issue.status = match status_str.to_lowercase().as_str() {
            "open" => IssueStatus::Open,
            "in_progress" => IssueStatus::InProgress,
            "blocked" => IssueStatus::Blocked {
                reason: "Blocked via web interface".to_string(),
            },
            "resolved" => IssueStatus::Resolved,
            "closed" => IssueStatus::Closed,
            _ => return Err(crate::Error::BadRequest("Invalid status".to_string())),
        };
    }

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

/// Query parameters for message listing
#[derive(Debug, Deserialize)]
pub struct MessageQuery {
    pub limit: Option<i32>,
    pub offset: Option<i32>,
    pub sender_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub message_type: Option<String>,
    pub priority: Option<String>,
    pub correlation_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub search: Option<String>,
}

/// List messages API endpoint
pub async fn messages_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<impl IntoResponse> {
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let messages = storage
        .messages()
        .list_recent(500)
        .await
        .map_err(crate::Error::Storage)?;

    // Apply basic filtering
    let mut filtered_messages: Vec<_> = messages.into_iter().collect();

    // Filter by sender
    if let Some(sender_id) = query.sender_id {
        filtered_messages.retain(|msg| msg.sender_id == sender_id);
    }

    // Filter by recipient
    if let Some(recipient_id) = query.recipient_id {
        filtered_messages.retain(|msg| msg.recipient_id == Some(recipient_id));
    }

    // Filter by message type
    if let Some(msg_type) = &query.message_type {
        filtered_messages.retain(|msg| {
            format!("{:?}", msg.message_type).to_lowercase() == msg_type.to_lowercase()
        });
    }

    // Filter by priority
    if let Some(priority) = &query.priority {
        filtered_messages.retain(|msg| {
            format!("{:?}", msg.metadata.priority).to_lowercase() == priority.to_lowercase()
        });
    }

    // Filter by correlation ID
    if let Some(correlation_id) = query.correlation_id {
        filtered_messages.retain(|msg| msg.metadata.correlation_id == Some(correlation_id));
    }

    // Filter by issue ID
    if let Some(issue_id) = query.issue_id {
        filtered_messages.retain(|msg| msg.metadata.issue_id == Some(issue_id));
    }

    // Basic text search
    if let Some(search_term) = &query.search {
        let search_lower = search_term.to_lowercase();
        filtered_messages.retain(|msg| {
            msg.content.to_lowercase().contains(&search_lower)
                || msg
                    .metadata
                    .knowledge_refs
                    .iter()
                    .any(|kr| kr.to_lowercase().contains(&search_lower))
        });
    }

    // Sort by creation time (newest first)
    filtered_messages.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply pagination
    let total_count = filtered_messages.len();
    let paginated_messages: Vec<_> = filtered_messages
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "messages": paginated_messages,
        "total": total_count,
        "limit": limit,
        "offset": offset,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get message conversations (grouped by correlation ID)
pub async fn messages_conversations(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<impl IntoResponse> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let fetch = (limit + offset).clamp(1, 2000); // temp until storage supports pagination
    let messages = storage
        .messages()
        .list_recent(fetch as i64)
        .await
        .map_err(crate::Error::Storage)?;

    // Group messages by correlation ID
    let mut conversations: HashMap<Option<Uuid>, Vec<_>> = HashMap::new();

    for message in messages {
        let correlation_id = message.metadata.correlation_id;
        conversations
            .entry(correlation_id)
            .or_default()
            .push(message);
    }

    // Convert to structured format
    let mut conversation_list = Vec::new();
    for (correlation_id, mut messages) in conversations {
        // Skip single messages without correlation ID for conversations view
        if correlation_id.is_none() && messages.len() == 1 {
            continue;
        }

        // Sort messages in conversation by time
        messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        // Get unique participants
        let mut participants = std::collections::HashSet::new();
        for msg in &messages {
            participants.insert(msg.sender_id);
            if let Some(recipient) = msg.recipient_id {
                participants.insert(recipient);
            }
        }

        // Calculate conversation status
        let undelivered_count = messages.iter().filter(|m| !m.is_delivered()).count();
        let status = if undelivered_count > 0 {
            "pending"
        } else {
            "completed"
        };

        // Get latest message preview
        let latest_message = messages.last();
        let preview = latest_message
            .map(|m| {
                if m.content.len() > 100 {
                    format!("{}...", &m.content[..100])
                } else {
                    m.content.clone()
                }
            })
            .unwrap_or_default();

        conversation_list.push(json!({
            "correlation_id": correlation_id,
            "message_count": messages.len(),
            "participants": participants.into_iter().collect::<Vec<_>>(),
            "first_message_at": messages.first().map(|m| m.created_at),
            "last_message_at": messages.last().map(|m| m.created_at),
            "status": status,
            "undelivered_count": undelivered_count,
            "preview": preview,
            "messages": messages,
        }));
    }

    // Sort conversations by last activity
    conversation_list.sort_by(|a, b| {
        let a_time = a["last_message_at"].as_str().unwrap_or("");
        let b_time = b["last_message_at"].as_str().unwrap_or("");
        b_time.cmp(a_time)
    });

    let total = conversation_list.len();
    let conversations = conversation_list
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect::<Vec<_>>();
    Ok(Json(json!({
        "conversations": conversations,
        "total": total,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Search messages with full-text search
pub async fn messages_search(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<impl IntoResponse> {
    let search_term = query.search.unwrap_or_default();

    if search_term.trim().is_empty() {
        return Ok(Json(json!({
            "messages": [],
            "total": 0,
            "query": search_term,
            "timestamp": chrono::Utc::now().to_rfc3339()
        })));
    }

    let messages = storage
        .messages()
        .list_recent(500)
        .await
        .map_err(crate::Error::Storage)?;

    let search_lower = search_term.to_lowercase();
    let mut matching_messages: Vec<_> = messages
        .into_iter()
        .filter(|msg| {
            msg.content.to_lowercase().contains(&search_lower)
                || msg
                    .metadata
                    .knowledge_refs
                    .iter()
                    .any(|kr| kr.to_lowercase().contains(&search_lower))
                || msg
                    .metadata
                    .knowledge_context
                    .as_ref()
                    .map(|ctx| ctx.to_lowercase().contains(&search_lower))
                    .unwrap_or(false)
        })
        .collect();

    // Sort by relevance (for now, just by creation time)
    matching_messages.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    // Apply pagination
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let total_count = matching_messages.len();

    let paginated_messages: Vec<_> = matching_messages
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "messages": paginated_messages,
        "total": total_count,
        "query": search_term,
        "limit": limit,
        "offset": offset,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get specific message details
pub async fn message_get(
    State(storage): State<Arc<StorageManager>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let message = storage
        .messages()
        .find_by_id(id)
        .await
        .map_err(crate::Error::Storage)?
        .ok_or_else(|| crate::Error::NotFound(format!("Message with id {}", id)))?;

    Ok(Json(json!({
        "message": message,
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get message analytics/statistics
pub async fn messages_analytics(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let messages = storage
        .messages()
        .list_recent(500)
        .await
        .map_err(crate::Error::Storage)?;

    let total_count = messages.len();

    // Count by message type
    let mut type_counts: HashMap<String, usize> = HashMap::new();
    let mut priority_counts: HashMap<String, usize> = HashMap::new();
    let mut delivery_stats = (0, 0); // (delivered, undelivered)

    for message in &messages {
        let msg_type = format!("{:?}", message.message_type);
        *type_counts.entry(msg_type).or_insert(0) += 1;

        let priority = format!("{:?}", message.metadata.priority);
        *priority_counts.entry(priority).or_insert(0) += 1;

        if message.is_delivered() {
            delivery_stats.0 += 1;
        } else {
            delivery_stats.1 += 1;
        }
    }

    // Recent message volume (last 24 hours)
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let recent_count = messages
        .iter()
        .filter(|msg| msg.created_at > cutoff)
        .count();

    // Conversation analytics
    let mut conversation_counts = std::collections::HashMap::new();
    for message in &messages {
        if let Some(correlation_id) = message.metadata.correlation_id {
            *conversation_counts.entry(correlation_id).or_insert(0) += 1;
        }
    }

    Ok(Json(json!({
        "total_messages": total_count,
        "recent_messages_24h": recent_count,
        "active_conversations": conversation_counts.len(),
        "average_messages_per_conversation": if conversation_counts.is_empty() {
            0.0
        } else {
            conversation_counts.values().sum::<i32>() as f64 / conversation_counts.len() as f64
        },
        "by_type": type_counts,
        "by_priority": priority_counts,
        "delivery_stats": {
            "delivered": delivery_stats.0,
            "undelivered": delivery_stats.1,
            "delivery_rate_percent": if total_count > 0 {
                (delivery_stats.0 as f64 / total_count as f64 * 100.0).round()
            } else {
                0.0
            }
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    })))
}

/// Get messages by correlation ID (conversation thread)
pub async fn messages_by_correlation(
    State(storage): State<Arc<StorageManager>>,
    Path(correlation_id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    // Fetch a larger window to reduce risk of missing thread messages
    // TODO: Add find_by_correlation_id method to storage layer for better performance
    let messages = storage
        .messages()
        .list_recent(5000)
        .await
        .map_err(crate::Error::Storage)?;

    let thread_messages: Vec<_> = messages
        .into_iter()
        .filter(|msg| msg.metadata.correlation_id == Some(correlation_id))
        .collect();

    if thread_messages.is_empty() {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": "No messages found for correlation ID",
                "correlation_id": correlation_id,
                "timestamp": chrono::Utc::now().to_rfc3339()
            })),
        ));
    }

    // Sort by creation time
    let mut sorted_messages = thread_messages;
    sorted_messages.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    // Get participants
    let mut participants = std::collections::HashSet::new();
    for msg in &sorted_messages {
        participants.insert(msg.sender_id);
        if let Some(recipient) = msg.recipient_id {
            participants.insert(recipient);
        }
    }

    Ok((
        StatusCode::OK,
        Json(json!({
            "correlation_id": correlation_id,
            "messages": sorted_messages,
            "message_count": sorted_messages.len(),
            "participants": participants.into_iter().collect::<Vec<_>>(),
            "first_message_at": sorted_messages.first().map(|m| m.created_at),
            "last_message_at": sorted_messages.last().map(|m| m.created_at),
            "timestamp": chrono::Utc::now().to_rfc3339()
        })),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Query;
    use uuid::Uuid;
    use vibe_ensemble_core::message::{Message, MessagePriority};
    use vibe_ensemble_storage::{manager::DatabaseConfig, StorageManager};

    async fn setup_test_storage() -> Arc<StorageManager> {
        let config = DatabaseConfig {
            url: ":memory:".to_string(),
            max_connections: Some(1),
            migrate_on_startup: true,
            performance_config: None,
        };
        Arc::new(StorageManager::new(&config).await.unwrap())
    }

    async fn create_test_message(
        storage: &Arc<StorageManager>,
        correlation_id: Option<Uuid>,
    ) -> Message {
        // Create test agents first to avoid foreign key constraint issues
        let conn_metadata = vibe_ensemble_core::agent::ConnectionMetadata {
            endpoint: "http://localhost:8080".to_string(),
            protocol_version: "1.0".to_string(),
            session_id: None,
            version: None,
            transport: None,
            capabilities: None,
            session_type: None,
            project_context: None,
            coordination_scope: None,
            specialization: None,
            coordinator_managed: None,
            workspace_isolation: None,
        };

        let sender_agent = vibe_ensemble_core::agent::Agent::new(
            "test-sender".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata.clone(),
        )
        .unwrap();

        let recipient_agent = vibe_ensemble_core::agent::Agent::new(
            "test-recipient".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata,
        )
        .unwrap();

        let sender_id = sender_agent.id;
        let recipient_id = recipient_agent.id;

        storage.agents().create(&sender_agent).await.unwrap();
        storage.agents().create(&recipient_agent).await.unwrap();

        let mut message = Message::new_direct(
            sender_id,
            recipient_id,
            "Test message content".to_string(),
            MessagePriority::Normal,
        )
        .unwrap();

        if let Some(id) = correlation_id {
            message.set_correlation_id(id);
        }

        storage.messages().create(&message).await.unwrap();
        message
    }

    #[tokio::test]
    async fn test_messages_list() {
        let storage = setup_test_storage().await;

        // Create test messages
        create_test_message(&storage, None).await;
        create_test_message(&storage, None).await;

        let query = MessageQuery {
            limit: Some(10),
            offset: Some(0),
            sender_id: None,
            recipient_id: None,
            message_type: None,
            priority: None,
            correlation_id: None,
            issue_id: None,
            search: None,
        };

        let response = messages_list(State(storage), Query(query)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_messages_conversations() {
        let storage = setup_test_storage().await;

        // Create a conversation with multiple messages
        let correlation_id = Uuid::new_v4();
        create_test_message(&storage, Some(correlation_id)).await;
        create_test_message(&storage, Some(correlation_id)).await;
        create_test_message(&storage, None).await; // Single message

        let query = MessageQuery {
            limit: Some(100),
            offset: None,
            sender_id: None,
            recipient_id: None,
            message_type: None,
            priority: None,
            correlation_id: None,
            issue_id: None,
            search: None,
        };

        let response = messages_conversations(State(storage), Query(query)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_messages_search() {
        let storage = setup_test_storage().await;

        // Create test message with searchable content
        let conn_metadata = vibe_ensemble_core::agent::ConnectionMetadata {
            endpoint: "http://localhost:8080".to_string(),
            protocol_version: "1.0".to_string(),
            session_id: None,
            version: None,
            transport: None,
            capabilities: None,
            session_type: None,
            project_context: None,
            coordination_scope: None,
            specialization: None,
            coordinator_managed: None,
            workspace_isolation: None,
        };

        let sender_agent = vibe_ensemble_core::agent::Agent::new(
            "search-sender".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata.clone(),
        )
        .unwrap();

        let recipient_agent = vibe_ensemble_core::agent::Agent::new(
            "search-recipient".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata,
        )
        .unwrap();

        let sender_id = sender_agent.id;
        let recipient_id = recipient_agent.id;

        storage.agents().create(&sender_agent).await.unwrap();
        storage.agents().create(&recipient_agent).await.unwrap();

        let message = Message::new_direct(
            sender_id,
            recipient_id,
            "Unique search content for testing".to_string(),
            MessagePriority::High,
        )
        .unwrap();

        storage.messages().create(&message).await.unwrap();

        let query = MessageQuery {
            limit: Some(10),
            offset: Some(0),
            sender_id: None,
            recipient_id: None,
            message_type: None,
            priority: None,
            correlation_id: None,
            issue_id: None,
            search: Some("Unique search".to_string()),
        };

        let response = messages_search(State(storage), Query(query)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_messages_analytics() {
        let storage = setup_test_storage().await;

        // Create various types of test messages
        let conn_metadata = vibe_ensemble_core::agent::ConnectionMetadata {
            endpoint: "http://localhost:8080".to_string(),
            protocol_version: "1.0".to_string(),
            session_id: None,
            version: None,
            transport: None,
            capabilities: None,
            session_type: None,
            project_context: None,
            coordination_scope: None,
            specialization: None,
            coordinator_managed: None,
            workspace_isolation: None,
        };

        let sender_agent = vibe_ensemble_core::agent::Agent::new(
            "analytics-sender".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata.clone(),
        )
        .unwrap();

        let recipient_agent = vibe_ensemble_core::agent::Agent::new(
            "analytics-recipient".to_string(),
            vibe_ensemble_core::agent::AgentType::Worker,
            vec!["test".to_string()],
            conn_metadata,
        )
        .unwrap();

        let sender_id = sender_agent.id;
        let recipient_id = recipient_agent.id;

        storage.agents().create(&sender_agent).await.unwrap();
        storage.agents().create(&recipient_agent).await.unwrap();

        let msg1 = Message::new_direct(
            sender_id,
            recipient_id,
            "Direct message".to_string(),
            MessagePriority::High,
        )
        .unwrap();

        let msg2 = Message::new_broadcast(
            sender_id,
            "Broadcast message".to_string(),
            MessagePriority::Normal,
        )
        .unwrap();

        storage.messages().create(&msg1).await.unwrap();
        storage.messages().create(&msg2).await.unwrap();

        let response = messages_analytics(State(storage)).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_messages_by_correlation() {
        let storage = setup_test_storage().await;
        let correlation_id = Uuid::new_v4();

        // Create messages with the same correlation ID
        create_test_message(&storage, Some(correlation_id)).await;
        create_test_message(&storage, Some(correlation_id)).await;

        let response = messages_by_correlation(State(storage), Path(correlation_id)).await;

        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_message_get() {
        let storage = setup_test_storage().await;
        let message = create_test_message(&storage, None).await;

        let response = message_get(State(storage), Path(message.id)).await;

        assert!(response.is_ok());
    }
}
