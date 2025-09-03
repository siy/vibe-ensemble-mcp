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

/// List all messages with optional filtering
pub async fn messages_list(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Value>> {
    // Enforce maximum limit to prevent excessive memory usage
    let limit = query.limit.unwrap_or(100).min(1000);
    let offset = query.offset.unwrap_or(0);

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
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    Ok(Json(json!({
        "messages": filtered_messages,
        "total": filtered_messages.len(),
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
