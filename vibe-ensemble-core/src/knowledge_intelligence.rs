//! Advanced knowledge intelligence and extraction module
//!
//! This module provides sophisticated knowledge management capabilities including:
//! - Automatic knowledge extraction from agent interactions
//! - Pattern recognition from issue resolutions
//! - Quality assessment and validation
//! - Knowledge contribution workflows
//! - Context-aware suggestions and recommendations

use crate::{
    issue::{Issue, IssueStatus},
    knowledge::{AccessLevel, Knowledge, KnowledgeSearchCriteria, KnowledgeType},
    message::{Message, MessageType},
    Error, Result,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents extracted knowledge from agent interactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedKnowledge {
    pub id: Uuid,
    pub source_type: ExtractionSource,
    pub source_id: Uuid,
    pub extracted_title: String,
    pub extracted_content: String,
    pub confidence_score: f64,
    pub extraction_method: ExtractionMethod,
    pub suggested_type: KnowledgeType,
    pub suggested_tags: Vec<String>,
    pub extracted_by: Uuid,
    pub extracted_at: DateTime<Utc>,
    pub review_status: ReviewStatus,
    pub quality_metrics: QualityMetrics,
}

/// Source of knowledge extraction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionSource {
    IssueResolution,
    AgentConversation,
    MessageThread,
    WorkflowCompletion,
    ErrorResolution,
}

/// Method used for knowledge extraction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionMethod {
    PatternMatching,
    NaturalLanguageProcessing,
    StructuralAnalysis,
    SuccessMetrics,
    ManualCuration,
}

/// Review status for extracted knowledge
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReviewStatus {
    Pending,
    UnderReview,
    Approved,
    Rejected,
    NeedsRevision,
}

/// Quality metrics for knowledge assessment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QualityMetrics {
    pub completeness_score: f64, // 0.0 - 1.0
    pub clarity_score: f64,      // 0.0 - 1.0
    pub accuracy_score: f64,     // 0.0 - 1.0
    pub relevance_score: f64,    // 0.0 - 1.0
    pub uniqueness_score: f64,   // 0.0 - 1.0
    pub overall_quality: f64,    // Computed aggregate score
}

/// Pattern recognized in agent interactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecognizedPattern {
    pub id: Uuid,
    pub pattern_type: PatternType,
    pub pattern_name: String,
    pub pattern_description: String,
    pub frequency_count: u32,
    pub success_rate: f64,
    pub context_tags: Vec<String>,
    pub related_issues: Vec<Uuid>,
    pub related_agents: Vec<Uuid>,
    pub first_observed: DateTime<Utc>,
    pub last_observed: DateTime<Utc>,
    pub confidence_level: f64,
}

/// Type of recognized pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    ProblemSolution,
    WorkflowProcess,
    CommunicationPattern,
    ErrorHandling,
    BestPractice,
    AntiPattern,
}

/// Knowledge contribution workflow entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeContribution {
    pub id: Uuid,
    pub contributor_id: Uuid,
    pub knowledge_id: Option<Uuid>,
    pub extracted_knowledge_id: Option<Uuid>,
    pub contribution_type: ContributionType,
    pub proposed_changes: serde_json::Value,
    pub review_comments: Vec<ReviewComment>,
    pub workflow_status: WorkflowStatus,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub approved_at: Option<DateTime<Utc>>,
}

/// Type of knowledge contribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContributionType {
    NewKnowledge,
    Enhancement,
    Correction,
    Translation,
    Tagging,
    Categorization,
}

/// Workflow status for contributions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum WorkflowStatus {
    Draft,
    Submitted,
    UnderReview,
    ApprovalPending,
    Approved,
    Rejected,
    Revision,
}

/// Review comment on knowledge contribution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewComment {
    pub id: Uuid,
    pub reviewer_id: Uuid,
    pub comment_text: String,
    pub comment_type: CommentType,
    pub created_at: DateTime<Utc>,
}

/// Type of review comment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CommentType {
    Suggestion,
    Question,
    Issue,
    Approval,
    Rejection,
}

/// Knowledge suggestion for issues or contexts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeSuggestion {
    pub id: Uuid,
    pub target_type: SuggestionTarget,
    pub target_id: Uuid,
    pub suggested_knowledge: Vec<KnowledgeReference>,
    pub suggestion_reason: String,
    pub confidence_score: f64,
    pub generated_by: Uuid,
    pub generated_at: DateTime<Utc>,
    pub accepted: Option<bool>,
    pub feedback: Option<String>,
}

/// Target for knowledge suggestions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SuggestionTarget {
    Issue,
    Agent,
    Message,
    Workflow,
}

/// Reference to knowledge with context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KnowledgeReference {
    pub knowledge_id: Uuid,
    pub relevance_score: f64,
    pub context_snippet: String,
    pub application_note: Option<String>,
}

/// Context-enhanced message with knowledge integration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnhancedMessage {
    pub base_message: Message,
    pub knowledge_references: Vec<KnowledgeReference>,
    pub extracted_patterns: Vec<Uuid>,
    pub context_analysis: ContextAnalysis,
    pub suggested_actions: Vec<String>,
}

/// Analysis of message context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextAnalysis {
    pub primary_topics: Vec<String>,
    pub sentiment_score: f64,
    pub complexity_level: ComplexityLevel,
    pub urgency_indicators: Vec<String>,
    pub knowledge_gaps: Vec<String>,
}

/// Complexity level assessment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComplexityLevel {
    Simple,
    Moderate,
    Complex,
    Expert,
}

/// Agent capability enhancement record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityEnhancement {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub enhancement_type: EnhancementType,
    pub knowledge_sources: Vec<Uuid>,
    pub capability_gained: String,
    pub proficiency_level: f64,
    pub verification_method: VerificationMethod,
    pub enhanced_at: DateTime<Utc>,
    pub validated_at: Option<DateTime<Utc>>,
}

/// Type of capability enhancement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EnhancementType {
    SkillAcquisition,
    KnowledgeIntegration,
    ProcessImprovement,
    ToolMastery,
    DomainExpertise,
}

/// Method for verifying capability enhancement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VerificationMethod {
    TaskCompletion,
    PeerReview,
    MetricImprovement,
    KnowledgeTest,
    RealWorldApplication,
}

/// Organizational learning metrics and insights
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrganizationalLearning {
    pub id: Uuid,
    pub learning_period: DateTime<Utc>,
    pub knowledge_growth_rate: f64,
    pub pattern_discovery_rate: f64,
    pub knowledge_reuse_metrics: ReuseMetrics,
    pub agent_improvement_metrics: Vec<AgentImprovement>,
    pub organizational_insights: Vec<String>,
    pub recommended_focus_areas: Vec<String>,
}

/// Knowledge reuse metrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReuseMetrics {
    pub total_knowledge_items: u32,
    pub actively_used_items: u32,
    pub reuse_frequency: f64,
    pub cross_agent_sharing: f64,
    pub stale_knowledge_count: u32,
}

/// Agent improvement metrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentImprovement {
    pub agent_id: Uuid,
    pub skill_growth_rate: f64,
    pub knowledge_contribution_rate: f64,
    pub collaboration_effectiveness: f64,
    pub problem_solving_improvement: f64,
}

impl ExtractedKnowledge {
    /// Create new extracted knowledge entry
    pub fn new(
        source_type: ExtractionSource,
        source_id: Uuid,
        title: String,
        content: String,
        extraction_method: ExtractionMethod,
        extracted_by: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source_type,
            source_id,
            extracted_title: title,
            extracted_content: content,
            confidence_score: 0.0,
            extraction_method,
            suggested_type: KnowledgeType::Pattern,
            suggested_tags: Vec::new(),
            extracted_by,
            extracted_at: Utc::now(),
            review_status: ReviewStatus::Pending,
            quality_metrics: QualityMetrics::default(),
        }
    }

    /// Calculate overall confidence based on quality metrics
    pub fn calculate_confidence(&mut self) {
        self.confidence_score = (self.quality_metrics.completeness_score * 0.2
            + self.quality_metrics.clarity_score * 0.2
            + self.quality_metrics.accuracy_score * 0.3
            + self.quality_metrics.relevance_score * 0.2
            + self.quality_metrics.uniqueness_score * 0.1)
            .min(1.0)
            .max(0.0);
    }

    /// Convert to Knowledge entry if approved
    pub fn to_knowledge(&self, creator_id: Uuid, access_level: AccessLevel) -> Result<Knowledge> {
        if self.review_status != ReviewStatus::Approved {
            return Err(Error::Validation {
                message: "Only approved extracted knowledge can be converted to Knowledge"
                    .to_string(),
            });
        }

        let mut knowledge = Knowledge::builder()
            .title(self.extracted_title.clone())
            .content(self.extracted_content.clone())
            .knowledge_type(self.suggested_type.clone())
            .created_by(creator_id)
            .access_level(access_level)
            .build()?;

        // Add suggested tags
        for tag in &self.suggested_tags {
            knowledge.add_tag(tag.clone())?;
        }

        Ok(knowledge)
    }
}

impl QualityMetrics {
    /// Create default quality metrics
    pub fn new() -> Self {
        Self {
            completeness_score: 0.0,
            clarity_score: 0.0,
            accuracy_score: 0.0,
            relevance_score: 0.0,
            uniqueness_score: 0.0,
            overall_quality: 0.0,
        }
    }

    /// Calculate overall quality score
    pub fn calculate_overall(&mut self) {
        self.overall_quality = (self.completeness_score * 0.25
            + self.clarity_score * 0.2
            + self.accuracy_score * 0.3
            + self.relevance_score * 0.2
            + self.uniqueness_score * 0.05)
            .min(1.0)
            .max(0.0);
    }

    /// Check if quality meets minimum threshold
    pub fn meets_threshold(&self, threshold: f64) -> bool {
        self.overall_quality >= threshold
    }
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl RecognizedPattern {
    /// Create new recognized pattern
    pub fn new(pattern_type: PatternType, name: String, description: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            pattern_type,
            pattern_name: name,
            pattern_description: description,
            frequency_count: 1,
            success_rate: 0.0,
            context_tags: Vec::new(),
            related_issues: Vec::new(),
            related_agents: Vec::new(),
            first_observed: now,
            last_observed: now,
            confidence_level: 0.0,
        }
    }

    /// Update pattern with new observation
    pub fn observe(&mut self, success: bool) {
        self.frequency_count += 1;
        self.last_observed = Utc::now();

        // Update success rate using exponential moving average
        let weight = 0.1;
        let success_value = if success { 1.0 } else { 0.0 };
        self.success_rate = (1.0 - weight) * self.success_rate + weight * success_value;

        // Update confidence based on frequency and success rate
        self.confidence_level = (self.frequency_count as f64 * self.success_rate / 10.0).min(1.0);
    }

    /// Check if pattern is mature (reliable)
    pub fn is_mature(&self) -> bool {
        self.frequency_count >= 5 && self.confidence_level >= 0.7
    }
}

impl KnowledgeContribution {
    /// Create new knowledge contribution
    pub fn new(
        contributor_id: Uuid,
        contribution_type: ContributionType,
        proposed_changes: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            contributor_id,
            knowledge_id: None,
            extracted_knowledge_id: None,
            contribution_type,
            proposed_changes,
            review_comments: Vec::new(),
            workflow_status: WorkflowStatus::Draft,
            submitted_at: Utc::now(),
            reviewed_at: None,
            approved_at: None,
        }
    }

    /// Submit contribution for review
    pub fn submit(&mut self) -> Result<()> {
        if self.workflow_status != WorkflowStatus::Draft {
            return Err(Error::Validation {
                message: "Only draft contributions can be submitted".to_string(),
            });
        }
        self.workflow_status = WorkflowStatus::Submitted;
        self.submitted_at = Utc::now();
        Ok(())
    }

    /// Add review comment
    pub fn add_review_comment(&mut self, comment: ReviewComment) {
        self.review_comments.push(comment);
        if self.reviewed_at.is_none() {
            self.reviewed_at = Some(Utc::now());
        }
    }

    /// Approve contribution
    pub fn approve(&mut self) -> Result<()> {
        if !matches!(
            self.workflow_status,
            WorkflowStatus::UnderReview | WorkflowStatus::ApprovalPending
        ) {
            return Err(Error::Validation {
                message: "Only contributions under review can be approved".to_string(),
            });
        }
        self.workflow_status = WorkflowStatus::Approved;
        self.approved_at = Some(Utc::now());
        Ok(())
    }
}

impl KnowledgeSuggestion {
    /// Create new knowledge suggestion
    pub fn new(
        target_type: SuggestionTarget,
        target_id: Uuid,
        suggested_knowledge: Vec<KnowledgeReference>,
        reason: String,
        generated_by: Uuid,
    ) -> Self {
        let confidence_score = if suggested_knowledge.is_empty() {
            0.0
        } else {
            suggested_knowledge
                .iter()
                .map(|k| k.relevance_score)
                .sum::<f64>()
                / suggested_knowledge.len() as f64
        };

        Self {
            id: Uuid::new_v4(),
            target_type,
            target_id,
            suggested_knowledge,
            suggestion_reason: reason,
            confidence_score,
            generated_by,
            generated_at: Utc::now(),
            accepted: None,
            feedback: None,
        }
    }

    /// Accept suggestion with optional feedback
    pub fn accept(&mut self, feedback: Option<String>) {
        self.accepted = Some(true);
        self.feedback = feedback;
    }

    /// Reject suggestion with optional feedback
    pub fn reject(&mut self, feedback: Option<String>) {
        self.accepted = Some(false);
        self.feedback = feedback;
    }
}

impl CapabilityEnhancement {
    /// Create new capability enhancement record
    pub fn new(
        agent_id: Uuid,
        enhancement_type: EnhancementType,
        knowledge_sources: Vec<Uuid>,
        capability_gained: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            agent_id,
            enhancement_type,
            knowledge_sources,
            capability_gained,
            proficiency_level: 0.0,
            verification_method: VerificationMethod::TaskCompletion,
            enhanced_at: Utc::now(),
            validated_at: None,
        }
    }

    /// Validate enhancement with proficiency assessment
    pub fn validate(&mut self, proficiency_level: f64, verification_method: VerificationMethod) {
        self.proficiency_level = proficiency_level.min(1.0).max(0.0);
        self.verification_method = verification_method;
        self.validated_at = Some(Utc::now());
    }

    /// Check if enhancement is validated
    pub fn is_validated(&self) -> bool {
        self.validated_at.is_some() && self.proficiency_level >= 0.7
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extracted_knowledge_creation() {
        let source_id = Uuid::new_v4();
        let extractor_id = Uuid::new_v4();

        let extracted = ExtractedKnowledge::new(
            ExtractionSource::IssueResolution,
            source_id,
            "Test Pattern".to_string(),
            "Test content for pattern".to_string(),
            ExtractionMethod::PatternMatching,
            extractor_id,
        );

        assert_eq!(extracted.source_type, ExtractionSource::IssueResolution);
        assert_eq!(extracted.source_id, source_id);
        assert_eq!(extracted.extracted_title, "Test Pattern");
        assert_eq!(extracted.review_status, ReviewStatus::Pending);
    }

    #[test]
    fn test_quality_metrics_calculation() {
        let mut metrics = QualityMetrics::new();
        metrics.completeness_score = 0.8;
        metrics.clarity_score = 0.9;
        metrics.accuracy_score = 0.85;
        metrics.relevance_score = 0.75;
        metrics.uniqueness_score = 0.6;

        metrics.calculate_overall();
        assert!(metrics.overall_quality > 0.8);
        assert!(metrics.meets_threshold(0.8));
    }

    #[test]
    fn test_pattern_recognition_lifecycle() {
        let mut pattern = RecognizedPattern::new(
            PatternType::ProblemSolution,
            "Database Connection Pattern".to_string(),
            "Pattern for handling database connections".to_string(),
        );

        assert_eq!(pattern.frequency_count, 1);
        assert!(!pattern.is_mature());

        // Simulate successful observations (more to overcome exponential moving average)
        for _ in 0..10 {
            pattern.observe(true);
        }

        assert!(pattern.frequency_count >= 5);
        assert!(pattern.success_rate > 0.5);
        assert!(pattern.is_mature());
    }

    #[test]
    fn test_knowledge_contribution_workflow() {
        let contributor_id = Uuid::new_v4();
        let reviewer_id = Uuid::new_v4();

        let mut contribution = KnowledgeContribution::new(
            contributor_id,
            ContributionType::NewKnowledge,
            serde_json::json!({"title": "New Pattern", "content": "Pattern content"}),
        );

        assert_eq!(contribution.workflow_status, WorkflowStatus::Draft);

        // Submit for review
        contribution.submit().unwrap();
        assert_eq!(contribution.workflow_status, WorkflowStatus::Submitted);

        // Add review comment
        let comment = ReviewComment {
            id: Uuid::new_v4(),
            reviewer_id,
            comment_text: "Looks good, just needs minor fixes".to_string(),
            comment_type: CommentType::Suggestion,
            created_at: Utc::now(),
        };
        contribution.add_review_comment(comment);
        assert!(!contribution.review_comments.is_empty());
        assert!(contribution.reviewed_at.is_some());
    }

    #[test]
    fn test_knowledge_suggestion() {
        let target_id = Uuid::new_v4();
        let generator_id = Uuid::new_v4();

        let knowledge_refs = vec![KnowledgeReference {
            knowledge_id: Uuid::new_v4(),
            relevance_score: 0.8,
            context_snippet: "Relevant pattern for this issue".to_string(),
            application_note: Some("Apply this pattern for better results".to_string()),
        }];

        let mut suggestion = KnowledgeSuggestion::new(
            SuggestionTarget::Issue,
            target_id,
            knowledge_refs,
            "This knowledge is relevant to your current issue".to_string(),
            generator_id,
        );

        assert_eq!(suggestion.confidence_score, 0.8);
        assert!(suggestion.accepted.is_none());

        suggestion.accept(Some("Very helpful!".to_string()));
        assert_eq!(suggestion.accepted, Some(true));
        assert_eq!(suggestion.feedback, Some("Very helpful!".to_string()));
    }

    #[test]
    fn test_capability_enhancement() {
        let agent_id = Uuid::new_v4();
        let knowledge_sources = vec![Uuid::new_v4(), Uuid::new_v4()];

        let mut enhancement = CapabilityEnhancement::new(
            agent_id,
            EnhancementType::SkillAcquisition,
            knowledge_sources,
            "Advanced debugging techniques".to_string(),
        );

        assert!(!enhancement.is_validated());

        enhancement.validate(0.85, VerificationMethod::TaskCompletion);
        assert!(enhancement.is_validated());
        assert_eq!(enhancement.proficiency_level, 0.85);
    }
}
