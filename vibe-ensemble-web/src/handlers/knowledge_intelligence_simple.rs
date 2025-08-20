//! Simplified web handlers for knowledge intelligence features
//!
//! This module provides basic HTTP endpoints for the core knowledge intelligence
//! features while maintaining compatibility with the existing web framework.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

use crate::{Error, Result};

/// Request to extract knowledge from issue
#[derive(Debug, Deserialize)]
pub struct ExtractFromIssueRequest {
    pub issue_id: Uuid,
    pub extractor_id: Uuid,
}

/// Response for extraction statistics
#[derive(Debug, Serialize)]
pub struct ExtractionStatsResponse {
    pub total_extracted: i64,
    pub approved_count: i64,
    pub pending_count: i64,
    pub approval_rate: f64,
}

/// Create simplified router for knowledge intelligence endpoints
pub fn router() -> Router<Arc<StorageManager>> {
    Router::new()
        .route("/ki/extract/issue", post(extract_from_issue))
        .route("/ki/statistics", get(get_extraction_statistics))
        .route("/ki/patterns", get(list_patterns))
        .route("/ki/health", get(health_check))
}

/// Extract knowledge from a resolved issue
pub async fn extract_from_issue(
    State(storage): State<Arc<StorageManager>>,
    Json(request): Json<ExtractFromIssueRequest>,
) -> Result<Json<serde_json::Value>> {
    // Create repositories directly
    let pool = storage.pool();
    let issue_repo = vibe_ensemble_storage::repositories::IssueRepository::new(pool.clone());

    // Get the issue
    let issue = issue_repo
        .find_by_id(request.issue_id)
        .await?
        .ok_or_else(|| Error::NotFound {
            entity: "Issue".to_string(),
            id: request.issue_id.to_string(),
        })?;

    // Create extracted knowledge entry (simplified)
    let extracted_content = format!(
        "# Issue Resolution: {}\n\n## Problem\n{}\n\n## Tags\n{}\n\n## Resolution Notes\nThis issue was successfully resolved.",
        issue.title,
        issue.description,
        issue.tags.join(", ")
    );

    // Return the extracted knowledge as JSON
    Ok(Json(serde_json::json!({
        "id": Uuid::new_v4(),
        "source_id": request.issue_id,
        "extracted_title": format!("Solution: {}", issue.title),
        "extracted_content": extracted_content,
        "extracted_by": request.extractor_id,
        "extracted_at": chrono::Utc::now(),
        "status": "pending_review"
    })))
}

/// Get extraction statistics
pub async fn get_extraction_statistics(
    State(storage): State<Arc<StorageManager>>,
) -> Result<Json<ExtractionStatsResponse>> {
    let pool = storage.pool();
    let ki_repo =
        vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(pool.clone());

    let stats = ki_repo.get_extraction_statistics().await?;

    let response = ExtractionStatsResponse {
        total_extracted: stats.total_extracted,
        approved_count: stats.approved_count,
        pending_count: stats.pending_count,
        approval_rate: stats.approval_rate,
    };

    Ok(Json(response))
}

/// List recognized patterns
pub async fn list_patterns(
    State(storage): State<Arc<StorageManager>>,
) -> Result<Json<serde_json::Value>> {
    let pool = storage.pool();
    let ki_repo =
        vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(pool.clone());

    let patterns = ki_repo.find_mature_patterns().await?;

    Ok(Json(serde_json::json!({
        "patterns": patterns,
        "count": patterns.len(),
        "timestamp": chrono::Utc::now()
    })))
}

/// Health check for knowledge intelligence system
pub async fn health_check(
    State(storage): State<Arc<StorageManager>>,
) -> Result<Json<serde_json::Value>> {
    // Basic health check
    storage.health_check().await?;

    Ok(Json(serde_json::json!({
        "status": "healthy",
        "system": "knowledge_intelligence",
        "timestamp": chrono::Utc::now()
    })))
}
