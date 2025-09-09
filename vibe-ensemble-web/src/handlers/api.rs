//! Additional API handlers for knowledge and message management

use crate::{Error, Result};
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
    knowledge::KnowledgeEntry, message::Message, orchestration::worker_manager::OutputType,
};
use vibe_ensemble_storage::StorageManager;

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
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

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
        .skip(offset as usize)
        .take(limit as usize)
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

/// Query parameters for pending message retrieval
#[derive(Debug, Deserialize)]
pub struct PendingMessageQuery {
    pub since: Option<String>, // RFC3339 timestamp
}

/// List all messages with optional filtering
pub async fn messages_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Value>> {
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

    let messages = storage.messages().list().await?;

    // Apply basic filtering using correct field names
    let filtered_messages: Vec<Message> = messages
        .into_iter()
        .filter(|message| {
            if let Some(from_agent) = &query.from_agent {
                let sender_id_str = message.sender_id.to_string();
                sender_id_str == *from_agent || sender_id_str.starts_with(from_agent)
            } else {
                true
            }
        })
        .filter(|message| {
            if let Some(to_agent) = &query.to_agent {
                message
                    .recipient_id
                    .map(|id| {
                        let recipient_id_str = id.to_string();
                        recipient_id_str == *to_agent || recipient_id_str.starts_with(to_agent)
                    })
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
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "messages": filtered_messages,
        "total": filtered_messages.len(),
        "timestamp": chrono::Utc::now(),
    })))
}

/// Get pending messages for a specific agent
/// Supports HTTP fallback strategy for workers with SSE connection issues
pub async fn pending_messages_for_agent(
    State(storage): State<Arc<StorageManager>>,
    Path(agent_id): Path<Uuid>,
    Query(query): Query<PendingMessageQuery>,
) -> Result<Json<Value>> {
    // Parse the 'since' parameter or default to 1 hour ago
    let since = if let Some(since_str) = query.since {
        chrono::DateTime::parse_from_rfc3339(&since_str)
            .map_err(|e| Error::BadRequest(format!("Invalid timestamp format: {}", e)))?
            .with_timezone(&chrono::Utc)
    } else {
        // Default to 1 hour ago if no timestamp provided
        chrono::Utc::now() - chrono::Duration::hours(1)
    };

    let pending_messages = storage
        .messages()
        .get_pending_messages_for_agent(agent_id, since)
        .await?;

    // Extract message IDs for deduplication tracking
    let message_ids: Vec<Uuid> = pending_messages.iter().map(|m| m.id).collect();
    let latest_timestamp = pending_messages
        .iter()
        .map(|m| m.created_at)
        .max()
        .unwrap_or(since);

    Ok(Json(json!({
        "messages": pending_messages,
        "message_ids": message_ids,
        "agent_id": agent_id,
        "since": since,
        "latest_timestamp": latest_timestamp,
        "count": pending_messages.len(),
        "timestamp": chrono::Utc::now(),
        "deduplication_hint": "Track message_ids to avoid processing duplicates from SSE"
    })))
}

/// Mark messages as delivered (batch operation)
/// Used by workers to acknowledge receipt of messages retrieved via HTTP
#[derive(Debug, Deserialize)]
pub struct MessageDeliveryAck {
    pub message_ids: Vec<Uuid>,
}

pub async fn acknowledge_message_delivery(
    State(storage): State<Arc<StorageManager>>,
    Json(ack): Json<MessageDeliveryAck>,
) -> Result<Json<Value>> {
    let mut acknowledged = Vec::new();
    let mut failed = Vec::new();

    for message_id in ack.message_ids {
        match storage
            .messages()
            .mark_message_delivered_fast(message_id)
            .await
        {
            Ok(()) => acknowledged.push(message_id),
            Err(e) => {
                tracing::warn!("Failed to mark message {} as delivered: {}", message_id, e);
                failed.push(json!({
                    "message_id": message_id,
                    "error": e.to_string()
                }));
            }
        }
    }

    Ok(Json(json!({
        "acknowledged": acknowledged,
        "failed": failed,
        "acknowledged_count": acknowledged.len(),
        "failed_count": failed.len(),
        "timestamp": chrono::Utc::now(),
    })))
}

// ======================
// WORKER OUTPUT STRUCTURES
// ======================

/// Worker output line for API responses
#[derive(Debug, Clone, Serialize)]
pub struct WorkerOutputLine {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub output_type: OutputType,
    pub content: String,
}
