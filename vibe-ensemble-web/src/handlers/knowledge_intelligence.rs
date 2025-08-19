//! Web handlers for advanced knowledge intelligence features
//!
//! This module provides HTTP endpoints for knowledge extraction, pattern recognition,
//! contribution workflows, suggestions, and organizational learning insights.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_core::knowledge_intelligence::{
    ExtractedKnowledge, RecognizedPattern, KnowledgeContribution, KnowledgeSuggestion,
    CapabilityEnhancement, OrganizationalLearning, ContributionType, EnhancementType,
    VerificationMethod, ReviewStatus, WorkflowStatus,
};
use vibe_ensemble_storage::services::KnowledgeIntelligenceService;

use crate::{Error, Result};
use vibe_ensemble_storage::StorageManager;

/// Request to extract knowledge from issue
#[derive(Debug, Deserialize)]
pub struct ExtractFromIssueRequest {
    pub issue_id: Uuid,
    pub extractor_id: Uuid,
}

/// Request to extract knowledge from message thread
#[derive(Debug, Deserialize)]
pub struct ExtractFromThreadRequest {
    pub message_ids: Vec<Uuid>,
    pub extractor_id: Uuid,
}

/// Request to review extracted knowledge
#[derive(Debug, Deserialize)]
pub struct ReviewExtractionRequest {
    pub reviewer_id: Uuid,
    pub approved: bool,
    pub feedback: Option<String>,
}

/// Request to analyze issue resolution pattern
#[derive(Debug, Deserialize)]
pub struct AnalyzePatternRequest {
    pub issue_id: Uuid,
    pub success: bool,
}

/// Request to create knowledge contribution
#[derive(Debug, Deserialize)]
pub struct CreateContributionRequest {
    pub contributor_id: Uuid,
    pub contribution_type: ContributionType,
    pub knowledge_id: Option<Uuid>,
    pub proposed_content: String,
    pub proposed_title: Option<String>,
}

/// Request to record capability enhancement
#[derive(Debug, Deserialize)]
pub struct RecordEnhancementRequest {
    pub agent_id: Uuid,
    pub enhancement_type: EnhancementType,
    pub knowledge_sources: Vec<Uuid>,
    pub capability_description: String,
}

/// Request to validate capability enhancement
#[derive(Debug, Deserialize)]
pub struct ValidateEnhancementRequest {
    pub proficiency_level: f64,
    pub verification_method: VerificationMethod,
}

/// Request to respond to knowledge suggestion
#[derive(Debug, Deserialize)]
pub struct SuggestionResponseRequest {
    pub accepted: bool,
    pub feedback: Option<String>,
}

/// Query parameters for listing extracted knowledge
#[derive(Debug, Deserialize)]
pub struct ListExtractedQuery {
    pub status: Option<ReviewStatus>,
    pub source_id: Option<Uuid>,
    pub limit: Option<u32>,
}

/// Query parameters for finding patterns
#[derive(Debug, Deserialize)]
pub struct FindPatternsQuery {
    pub mature_only: Option<bool>,
    pub pattern_type: Option<String>,
}

/// Response containing extraction statistics
#[derive(Debug, Serialize)]
pub struct ExtractionStatsResponse {
    pub total_extracted: i64,
    pub approved_count: i64,
    pub pending_count: i64,
    pub approval_rate: f64,
}

/// Response containing organizational insights
#[derive(Debug, Serialize)]
pub struct InsightsResponse {
    pub learning: OrganizationalLearning,
    pub pattern_count: usize,
    pub extraction_stats: ExtractionStatsResponse,
}

/// Create router for knowledge intelligence endpoints
pub fn router() -> Router<Arc<StorageManager>> {
    Router::new()
        // Knowledge extraction endpoints
        .route("/extract/issue", post(extract_from_issue))
        .route("/extract/thread", post(extract_from_thread))
        .route("/extract/:id/review", put(review_extraction))
        .route("/extract", get(list_extracted_knowledge))
        .route("/extract/:id", get(get_extracted_knowledge))
        
        // Pattern recognition endpoints
        .route("/patterns/analyze", post(analyze_pattern))
        .route("/patterns", get(find_patterns))
        .route("/patterns/:id", get(get_pattern))
        .route("/patterns/applicable/:issue_id", get(find_applicable_patterns))
        
        // Knowledge suggestions endpoints
        .route("/suggestions/issue/:issue_id", post(generate_issue_suggestions))
        .route("/suggestions/:id/respond", put(respond_to_suggestion))
        .route("/suggestions/target/:target_type/:target_id", get(get_suggestions_for_target))
        
        // Knowledge contribution workflow endpoints
        .route("/contributions", post(create_contribution))
        .route("/contributions/:id/submit", put(submit_contribution))
        .route("/contributions/pending", get(list_pending_contributions))
        .route("/contributions/:id", get(get_contribution))
        
        // Capability enhancement endpoints
        .route("/enhancements", post(record_enhancement))
        .route("/enhancements/:id/validate", put(validate_enhancement))
        .route("/enhancements/agent/:agent_id", get(get_agent_enhancements))
        
        // Organizational learning endpoints
        .route("/insights", get(get_organizational_insights))
        .route("/statistics", get(get_extraction_statistics))
}

// ========== Knowledge Extraction Handlers ==========

/// Extract knowledge from a resolved issue
pub async fn extract_from_issue(
    State(storage): State<Arc<StorageManager>>,
    Json(request): Json<ExtractFromIssueRequest>,
) -> Result<Json<ExtractedKnowledge>> {
    let service = create_ki_service(&storage).await?;
    
    let extracted = service
        .extract_knowledge_from_issue(request.issue_id, request.extractor_id)
        .await?;

    Ok(Json(extracted))
}

/// Extract knowledge from a message thread
pub async fn extract_from_thread(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ExtractFromThreadRequest>,
) -> WebResult<Json<ExtractedKnowledge>> {
    let service = create_ki_service(&state).await?;
    
    let extracted = service
        .extract_knowledge_from_message_thread(request.message_ids, request.extractor_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to extract knowledge from thread: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(extracted))
}

/// Review extracted knowledge
pub async fn review_extraction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReviewExtractionRequest>,
) -> WebResult<StatusCode> {
    let service = create_ki_service(&state).await?;
    
    service
        .review_extracted_knowledge(id, request.reviewer_id, request.approved, request.feedback)
        .await
        .map_err(|e| {
            tracing::error!("Failed to review extracted knowledge: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

/// List extracted knowledge with filters
pub async fn list_extracted_knowledge(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListExtractedQuery>,
) -> WebResult<Json<Vec<ExtractedKnowledge>>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let extracted = if query.status == Some(ReviewStatus::Pending) {
        ki_repo.list_pending_extracted_knowledge().await
    } else if let Some(source_id) = query.source_id {
        ki_repo.list_extracted_knowledge_by_source(source_id).await
    } else {
        ki_repo.list_pending_extracted_knowledge().await // Default to pending
    }
    .map_err(|e| {
        tracing::error!("Failed to list extracted knowledge: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(extracted))
}

/// Get specific extracted knowledge
pub async fn get_extracted_knowledge(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> WebResult<Json<ExtractedKnowledge>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let extracted = ki_repo
        .find_extracted_knowledge(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get extracted knowledge: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(extracted))
}

// ========== Pattern Recognition Handlers ==========

/// Analyze issue resolution for patterns
pub async fn analyze_pattern(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AnalyzePatternRequest>,
) -> WebResult<Json<Option<RecognizedPattern>>> {
    let service = create_ki_service(&state).await?;
    
    let pattern = service
        .analyze_issue_resolution_pattern(request.issue_id, request.success)
        .await
        .map_err(|e| {
            tracing::error!("Failed to analyze pattern: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(pattern))
}

/// Find patterns with optional filters
pub async fn find_patterns(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FindPatternsQuery>,
) -> WebResult<Json<Vec<RecognizedPattern>>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let patterns = if query.mature_only.unwrap_or(false) {
        ki_repo.find_mature_patterns().await
    } else {
        ki_repo.find_mature_patterns().await // For now, always return mature patterns
    }
    .map_err(|e| {
        tracing::error!("Failed to find patterns: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(patterns))
}

/// Get specific pattern
pub async fn get_pattern(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> WebResult<Json<RecognizedPattern>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let pattern = ki_repo
        .find_pattern(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get pattern: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(pattern))
}

/// Find patterns applicable to an issue
pub async fn find_applicable_patterns(
    State(state): State<Arc<AppState>>,
    Path(issue_id): Path<Uuid>,
) -> WebResult<Json<Vec<RecognizedPattern>>> {
    let service = create_ki_service(&state).await?;
    
    // First get the issue
    let issue_repo = vibe_ensemble_storage::repositories::IssueRepository::new(state.pool.clone());
    let issue = issue_repo
        .find_by_id(issue_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let patterns = service
        .find_applicable_patterns(&issue)
        .await
        .map_err(|e| {
            tracing::error!("Failed to find applicable patterns: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(patterns))
}

// ========== Knowledge Suggestions Handlers ==========

/// Generate knowledge suggestions for an issue
pub async fn generate_issue_suggestions(
    State(state): State<Arc<AppState>>,
    Path(issue_id): Path<Uuid>,
    Json(generator_id): Json<Uuid>,
) -> WebResult<Json<KnowledgeSuggestion>> {
    let service = create_ki_service(&state).await?;
    
    let suggestion = service
        .generate_issue_suggestions(issue_id, generator_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to generate suggestions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(suggestion))
}

/// Respond to a knowledge suggestion
pub async fn respond_to_suggestion(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<SuggestionResponseRequest>,
) -> WebResult<StatusCode> {
    let service = create_ki_service(&state).await?;
    
    service
        .respond_to_suggestion(id, request.accepted, request.feedback)
        .await
        .map_err(|e| {
            tracing::error!("Failed to respond to suggestion: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

/// Get suggestions for a target
pub async fn get_suggestions_for_target(
    State(state): State<Arc<AppState>>,
    Path((target_type, target_id)): Path<(String, Uuid)>,
) -> WebResult<Json<Vec<KnowledgeSuggestion>>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let target_type = match target_type.as_str() {
        "issue" => vibe_ensemble_core::knowledge_intelligence::SuggestionTarget::Issue,
        "agent" => vibe_ensemble_core::knowledge_intelligence::SuggestionTarget::Agent,
        "message" => vibe_ensemble_core::knowledge_intelligence::SuggestionTarget::Message,
        "workflow" => vibe_ensemble_core::knowledge_intelligence::SuggestionTarget::Workflow,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let suggestions = ki_repo
        .find_suggestions_for_target(&target_type, target_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get suggestions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(suggestions))
}

// ========== Knowledge Contribution Handlers ==========

/// Create new knowledge contribution
pub async fn create_contribution(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateContributionRequest>,
) -> WebResult<Json<KnowledgeContribution>> {
    let service = create_ki_service(&state).await?;
    
    let contribution = service
        .create_knowledge_contribution(
            request.contributor_id,
            request.contribution_type,
            request.knowledge_id,
            request.proposed_content,
            request.proposed_title,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to create contribution: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(contribution))
}

/// Submit contribution for review
pub async fn submit_contribution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> WebResult<StatusCode> {
    let service = create_ki_service(&state).await?;
    
    service
        .submit_contribution(id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to submit contribution: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

/// List pending contributions
pub async fn list_pending_contributions(
    State(state): State<Arc<AppState>>,
) -> WebResult<Json<Vec<KnowledgeContribution>>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let contributions = ki_repo
        .list_pending_contributions()
        .await
        .map_err(|e| {
            tracing::error!("Failed to list pending contributions: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(contributions))
}

/// Get specific contribution
pub async fn get_contribution(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> WebResult<Json<KnowledgeContribution>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    // This is a simplified implementation - in a real system you'd have a proper find method
    let contributions = ki_repo.list_pending_contributions().await
        .map_err(|e| {
            tracing::error!("Failed to get contribution: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let contribution = contributions.into_iter()
        .find(|c| c.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(contribution))
}

// ========== Capability Enhancement Handlers ==========

/// Record capability enhancement
pub async fn record_enhancement(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RecordEnhancementRequest>,
) -> WebResult<Json<CapabilityEnhancement>> {
    let service = create_ki_service(&state).await?;
    
    let enhancement = service
        .record_capability_enhancement(
            request.agent_id,
            request.enhancement_type,
            request.knowledge_sources,
            request.capability_description,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to record enhancement: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(enhancement))
}

/// Validate capability enhancement
pub async fn validate_enhancement(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(request): Json<ValidateEnhancementRequest>,
) -> WebResult<StatusCode> {
    let service = create_ki_service(&state).await?;
    
    service
        .validate_capability_enhancement(
            id,
            request.proficiency_level,
            request.verification_method,
        )
        .await
        .map_err(|e| {
            tracing::error!("Failed to validate enhancement: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::OK)
}

/// Get enhancements for an agent
pub async fn get_agent_enhancements(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<Uuid>,
) -> WebResult<Json<Vec<CapabilityEnhancement>>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let enhancements = ki_repo
        .find_enhancements_for_agent(agent_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get agent enhancements: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(enhancements))
}

// ========== Organizational Learning Handlers ==========

/// Get organizational learning insights
pub async fn get_organizational_insights(
    State(state): State<Arc<AppState>>,
) -> WebResult<Json<InsightsResponse>> {
    let service = create_ki_service(&state).await?;
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let learning = service
        .generate_organizational_insights()
        .await
        .map_err(|e| {
            tracing::error!("Failed to generate insights: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let patterns = ki_repo
        .find_mature_patterns()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get patterns: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let stats = ki_repo
        .get_extraction_statistics()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get extraction stats: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response = InsightsResponse {
        learning,
        pattern_count: patterns.len(),
        extraction_stats: ExtractionStatsResponse {
            total_extracted: stats.total_extracted,
            approved_count: stats.approved_count,
            pending_count: stats.pending_count,
            approval_rate: stats.approval_rate,
        },
    };

    Ok(Json(response))
}

/// Get extraction statistics
pub async fn get_extraction_statistics(
    State(state): State<Arc<AppState>>,
) -> WebResult<Json<ExtractionStatsResponse>> {
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(state.pool.clone());
    
    let stats = ki_repo
        .get_extraction_statistics()
        .await
        .map_err(|e| {
            tracing::error!("Failed to get extraction statistics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response = ExtractionStatsResponse {
        total_extracted: stats.total_extracted,
        approved_count: stats.approved_count,
        pending_count: stats.pending_count,
        approval_rate: stats.approval_rate,
    };

    Ok(Json(response))
}

// ========== Helper Functions ==========

/// Create knowledge intelligence service with all dependencies  
async fn create_ki_service(storage: &StorageManager) -> Result<KnowledgeIntelligenceService> {
    let pool = storage.pool();
    let ki_repo = vibe_ensemble_storage::repositories::KnowledgeIntelligenceRepository::new(pool.clone());
    let knowledge_repo = vibe_ensemble_storage::repositories::KnowledgeRepository::new(pool.clone());
    let issue_repo = vibe_ensemble_storage::repositories::IssueRepository::new(pool.clone());
    let message_repo = vibe_ensemble_storage::repositories::MessageRepository::new(pool.clone());
    let agent_repo = vibe_ensemble_storage::repositories::AgentRepository::new(pool.clone());

    Ok(KnowledgeIntelligenceService::new(
        ki_repo,
        knowledge_repo,
        issue_repo,
        message_repo,
        agent_repo,
    ))
}