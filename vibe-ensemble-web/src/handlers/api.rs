//! API handlers

use crate::Result;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;
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
