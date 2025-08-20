//! Advanced knowledge intelligence service implementation
//!
//! This service provides high-level operations for knowledge extraction,
//! pattern recognition, contribution workflows, and organizational learning.

use crate::{
    repositories::{
        AgentRepository, IssueRepository, KnowledgeIntelligenceRepository, KnowledgeRepository,
        MessageRepository,
    },
    Error, Result,
};
use uuid::Uuid;
use vibe_ensemble_core::{
    issue::{Issue, IssueStatus},
    knowledge::{AccessLevel, KnowledgeSearchCriteria, KnowledgeType},
    knowledge_intelligence::{
        CapabilityEnhancement, ContributionType, EnhancementType, ExtractedKnowledge,
        ExtractionMethod, ExtractionSource, KnowledgeContribution, KnowledgeReference,
        KnowledgeSuggestion, OrganizationalLearning, PatternType, QualityMetrics,
        RecognizedPattern, ReviewStatus, SuggestionTarget, VerificationMethod,
    },
    message::{Message, MessageType},
};

/// High-level knowledge intelligence service
pub struct KnowledgeIntelligenceService {
    ki_repository: KnowledgeIntelligenceRepository,
    knowledge_repository: KnowledgeRepository,
    issue_repository: IssueRepository,
    message_repository: MessageRepository,
    #[allow(dead_code)]
    agent_repository: AgentRepository,
}

impl KnowledgeIntelligenceService {
    /// Create new knowledge intelligence service
    pub fn new(
        ki_repository: KnowledgeIntelligenceRepository,
        knowledge_repository: KnowledgeRepository,
        issue_repository: IssueRepository,
        message_repository: MessageRepository,
        agent_repository: AgentRepository,
    ) -> Self {
        Self {
            ki_repository,
            knowledge_repository,
            issue_repository,
            message_repository,
            agent_repository,
        }
    }

    // ========== Automatic Knowledge Extraction ==========

    /// Extract knowledge from resolved issue
    pub async fn extract_knowledge_from_issue(
        &self,
        issue_id: Uuid,
        extractor_id: Uuid,
    ) -> Result<ExtractedKnowledge> {
        let issue = self
            .issue_repository
            .find_by_id(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Only extract from resolved issues
        if issue.status != IssueStatus::Resolved {
            return Err(Error::Validation {
                message: "Can only extract knowledge from resolved issues".to_string(),
            });
        }

        let extracted_title = format!("Solution: {}", issue.title);
        let extracted_content = self.generate_issue_knowledge_content(&issue);

        let mut extracted = ExtractedKnowledge::new(
            ExtractionSource::IssueResolution,
            issue_id,
            extracted_title,
            extracted_content,
            ExtractionMethod::StructuralAnalysis,
            extractor_id,
        );

        // Set suggested type and tags based on issue
        extracted.suggested_type = self.infer_knowledge_type_from_issue(&issue);
        extracted.suggested_tags = issue.tags.clone();

        // Calculate quality metrics
        extracted.quality_metrics = self.assess_extraction_quality(&extracted);
        extracted.calculate_confidence();

        // Store extracted knowledge
        self.ki_repository
            .create_extracted_knowledge(&extracted)
            .await?;

        Ok(extracted)
    }

    /// Extract knowledge from message thread
    pub async fn extract_knowledge_from_message_thread(
        &self,
        message_ids: Vec<Uuid>,
        extractor_id: Uuid,
    ) -> Result<ExtractedKnowledge> {
        let mut messages = Vec::new();
        for id in message_ids {
            if let Some(message) = self.message_repository.find_by_id(id).await? {
                messages.push(message);
            }
        }

        if messages.is_empty() {
            return Err(Error::Validation {
                message: "No valid messages found for extraction".to_string(),
            });
        }

        let extracted_title = self.generate_thread_title(&messages);
        let extracted_content = self.generate_thread_knowledge_content(&messages);

        let mut extracted = ExtractedKnowledge::new(
            ExtractionSource::MessageThread,
            messages[0].id, // Use first message as source
            extracted_title,
            extracted_content,
            ExtractionMethod::NaturalLanguageProcessing,
            extractor_id,
        );

        // Analyze messages for patterns and insights
        extracted.suggested_type = self.infer_knowledge_type_from_messages(&messages);
        extracted.suggested_tags = self.extract_tags_from_messages(&messages);

        // Calculate quality metrics
        extracted.quality_metrics = self.assess_extraction_quality(&extracted);
        extracted.calculate_confidence();

        // Store extracted knowledge
        self.ki_repository
            .create_extracted_knowledge(&extracted)
            .await?;

        Ok(extracted)
    }

    /// Review and approve extracted knowledge
    pub async fn review_extracted_knowledge(
        &self,
        extracted_id: Uuid,
        reviewer_id: Uuid,
        approved: bool,
        _feedback: Option<String>,
    ) -> Result<()> {
        let mut extracted = self
            .ki_repository
            .find_extracted_knowledge(extracted_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "ExtractedKnowledge".to_string(),
                id: extracted_id.to_string(),
            })?;

        if approved {
            extracted.review_status = ReviewStatus::Approved;

            // Create actual knowledge entry
            let knowledge = extracted.to_knowledge(reviewer_id, AccessLevel::Team)?;
            self.knowledge_repository.create(&knowledge).await?;
        } else {
            extracted.review_status = ReviewStatus::Rejected;
        }

        self.ki_repository
            .update_extracted_knowledge(&extracted)
            .await?;
        Ok(())
    }

    // ========== Pattern Recognition ==========

    /// Analyze issue resolution for patterns
    pub async fn analyze_issue_resolution_pattern(
        &self,
        issue_id: Uuid,
        success: bool,
    ) -> Result<Option<RecognizedPattern>> {
        let issue = self
            .issue_repository
            .find_by_id(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Look for existing patterns that match this issue
        let pattern_name = self.generate_pattern_name_from_issue(&issue);
        let existing_patterns = self
            .ki_repository
            .find_patterns_by_type(&PatternType::ProblemSolution)
            .await?;

        let mut pattern = existing_patterns
            .into_iter()
            .find(|p| p.pattern_name == pattern_name)
            .unwrap_or_else(|| {
                RecognizedPattern::new(
                    PatternType::ProblemSolution,
                    pattern_name,
                    format!("Pattern for resolving {}", issue.title),
                )
            });

        // Update pattern with this observation
        pattern.observe(success);
        pattern.context_tags.extend(issue.tags.clone());
        pattern.related_issues.push(issue_id);

        // Store updated pattern
        self.ki_repository
            .upsert_recognized_pattern(&pattern)
            .await?;

        Ok(Some(pattern))
    }

    /// Find patterns applicable to current issue
    pub async fn find_applicable_patterns(&self, issue: &Issue) -> Result<Vec<RecognizedPattern>> {
        let all_patterns = self.ki_repository.find_mature_patterns().await?;

        let mut applicable = Vec::new();
        for pattern in all_patterns {
            if self.pattern_matches_issue(&pattern, issue) {
                applicable.push(pattern);
            }
        }

        // Sort by confidence level
        applicable.sort_by(|a, b| b.confidence_level.partial_cmp(&a.confidence_level).unwrap());
        Ok(applicable)
    }

    // ========== Knowledge Suggestions ==========

    /// Generate knowledge suggestions for an issue
    pub async fn generate_issue_suggestions(
        &self,
        issue_id: Uuid,
        generator_id: Uuid,
    ) -> Result<KnowledgeSuggestion> {
        let issue = self
            .issue_repository
            .find_by_id(issue_id)
            .await?
            .ok_or_else(|| Error::NotFound {
                entity: "Issue".to_string(),
                id: issue_id.to_string(),
            })?;

        // Find relevant knowledge based on tags and content
        let mut knowledge_refs = Vec::new();

        // Search by tags
        for tag in &issue.tags {
            let criteria = KnowledgeSearchCriteria::new()
                .with_query(tag.clone())
                .with_limit(5);

            let results = self
                .knowledge_repository
                .search(&criteria, generator_id)
                .await?;
            for result in results {
                knowledge_refs.push(KnowledgeReference {
                    knowledge_id: result.knowledge.id,
                    relevance_score: result.relevance_score,
                    context_snippet: result
                        .snippet
                        .unwrap_or_else(|| result.knowledge.content.chars().take(200).collect()),
                    application_note: Some(format!("Relevant for tag: {}", tag)),
                });
            }
        }

        // Find patterns that might apply
        let _applicable_patterns = self.find_applicable_patterns(&issue).await?;

        // Deduplicate and rank suggestions
        knowledge_refs.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
        knowledge_refs.truncate(10); // Limit to top 10

        let suggestion = KnowledgeSuggestion::new(
            SuggestionTarget::Issue,
            issue_id,
            knowledge_refs,
            "Suggestions based on issue tags and similar patterns".to_string(),
            generator_id,
        );

        self.ki_repository
            .create_knowledge_suggestion(&suggestion)
            .await?;
        Ok(suggestion)
    }

    /// Accept or reject knowledge suggestion
    pub async fn respond_to_suggestion(
        &self,
        suggestion_id: Uuid,
        accepted: bool,
        feedback: Option<String>,
    ) -> Result<()> {
        let mut suggestion = self
            .ki_repository
            .find_suggestions_for_target(&SuggestionTarget::Issue, suggestion_id)
            .await?
            .into_iter()
            .find(|s| s.id == suggestion_id)
            .ok_or_else(|| Error::NotFound {
                entity: "KnowledgeSuggestion".to_string(),
                id: suggestion_id.to_string(),
            })?;

        if accepted {
            suggestion.accept(feedback);
        } else {
            suggestion.reject(feedback);
        }

        self.ki_repository
            .update_suggestion_acceptance(&suggestion)
            .await?;
        Ok(())
    }

    // ========== Knowledge Contribution Workflow ==========

    /// Create new knowledge contribution
    pub async fn create_knowledge_contribution(
        &self,
        contributor_id: Uuid,
        contribution_type: ContributionType,
        knowledge_id: Option<Uuid>,
        proposed_content: String,
        proposed_title: Option<String>,
    ) -> Result<KnowledgeContribution> {
        let proposed_changes = serde_json::json!({
            "content": proposed_content,
            "title": proposed_title,
            "type": contribution_type
        });

        let mut contribution =
            KnowledgeContribution::new(contributor_id, contribution_type, proposed_changes);
        contribution.knowledge_id = knowledge_id;

        self.ki_repository
            .create_knowledge_contribution(&contribution)
            .await?;
        Ok(contribution)
    }

    /// Submit contribution for review
    pub async fn submit_contribution(&self, contribution_id: Uuid) -> Result<()> {
        let contribution = self
            .ki_repository
            .list_pending_contributions()
            .await?
            .into_iter()
            .find(|c| c.id == contribution_id)
            .ok_or_else(|| Error::NotFound {
                entity: "KnowledgeContribution".to_string(),
                id: contribution_id.to_string(),
            })?;

        // Submit sets status and timestamp
        let mut contribution = contribution;
        contribution.submit()?;

        self.ki_repository
            .update_contribution_status(&contribution)
            .await?;
        Ok(())
    }

    // ========== Capability Enhancement ==========

    /// Record capability enhancement for agent
    pub async fn record_capability_enhancement(
        &self,
        agent_id: Uuid,
        enhancement_type: EnhancementType,
        knowledge_sources: Vec<Uuid>,
        capability_description: String,
    ) -> Result<CapabilityEnhancement> {
        let enhancement = CapabilityEnhancement::new(
            agent_id,
            enhancement_type,
            knowledge_sources,
            capability_description,
        );

        self.ki_repository
            .create_capability_enhancement(&enhancement)
            .await?;
        Ok(enhancement)
    }

    /// Validate capability enhancement
    pub async fn validate_capability_enhancement(
        &self,
        enhancement_id: Uuid,
        proficiency_level: f64,
        verification_method: VerificationMethod,
    ) -> Result<()> {
        let enhancements = self
            .ki_repository
            .find_enhancements_for_agent(Uuid::nil())
            .await?;
        let mut enhancement = enhancements
            .into_iter()
            .find(|e| e.id == enhancement_id)
            .ok_or_else(|| Error::NotFound {
                entity: "CapabilityEnhancement".to_string(),
                id: enhancement_id.to_string(),
            })?;

        enhancement.validate(proficiency_level, verification_method);
        self.ki_repository
            .update_enhancement_validation(&enhancement)
            .await?;
        Ok(())
    }

    // ========== Organizational Learning ==========

    /// Generate organizational learning insights
    pub async fn generate_organizational_insights(&self) -> Result<OrganizationalLearning> {
        let extraction_stats = self.ki_repository.get_extraction_statistics().await?;
        let mature_patterns = self.ki_repository.find_mature_patterns().await?;

        // Calculate growth rates (simplified)
        let knowledge_growth_rate = extraction_stats.approval_rate;
        let pattern_discovery_rate = mature_patterns.len() as f64 / 30.0; // patterns per month

        // Generate insights based on data
        let insights = self.generate_insights_from_patterns(&mature_patterns);
        let focus_areas = self.recommend_focus_areas(&mature_patterns, &extraction_stats);

        let learning = OrganizationalLearning {
            id: Uuid::new_v4(),
            learning_period: chrono::Utc::now(),
            knowledge_growth_rate,
            pattern_discovery_rate,
            knowledge_reuse_metrics: vibe_ensemble_core::knowledge_intelligence::ReuseMetrics {
                total_knowledge_items: extraction_stats.total_extracted as u32,
                actively_used_items: extraction_stats.approved_count as u32,
                reuse_frequency: 0.8,     // Simplified
                cross_agent_sharing: 0.7, // Simplified
                stale_knowledge_count: 0, // Simplified
            },
            agent_improvement_metrics: Vec::new(), // Would be calculated from enhancement data
            organizational_insights: insights,
            recommended_focus_areas: focus_areas,
        };

        Ok(learning)
    }

    // ========== Helper Methods ==========

    fn generate_issue_knowledge_content(&self, issue: &Issue) -> String {
        format!(
            "# Issue Resolution: {}\n\n## Problem\n{}\n\n## Tags\n{}\n\n## Resolution Notes\nThis issue was successfully resolved. The solution approach and methodology can be reused for similar problems.\n\n## Context\n- Priority: {:?}\n- Status: {:?}\n- Resolved at: {}",
            issue.title,
            issue.description,
            issue.tags.join(", "),
            issue.priority,
            issue.status,
            issue.resolved_at.map(|dt| dt.to_rfc3339()).unwrap_or_else(|| "Unknown".to_string())
        )
    }

    fn generate_thread_title(&self, messages: &[Message]) -> String {
        if messages.is_empty() {
            return "Empty Thread".to_string();
        }

        format!(
            "Knowledge from message thread starting: {}",
            messages[0].content.chars().take(50).collect::<String>()
        )
    }

    fn generate_thread_knowledge_content(&self, messages: &[Message]) -> String {
        let mut content = String::new();
        content.push_str("# Knowledge Extracted from Message Thread\n\n");

        for (i, message) in messages.iter().enumerate() {
            content.push_str(&format!(
                "## Message {} ({})\n",
                i + 1,
                message.created_at.format("%Y-%m-%d %H:%M")
            ));
            content.push_str(&message.content);
            content.push_str("\n\n");
        }

        content
    }

    fn infer_knowledge_type_from_issue(&self, issue: &Issue) -> KnowledgeType {
        if issue
            .tags
            .iter()
            .any(|tag| tag.contains("pattern") || tag.contains("solution"))
        {
            KnowledgeType::Solution
        } else if issue
            .tags
            .iter()
            .any(|tag| tag.contains("practice") || tag.contains("process"))
        {
            KnowledgeType::Practice
        } else {
            KnowledgeType::Pattern
        }
    }

    fn infer_knowledge_type_from_messages(&self, messages: &[Message]) -> KnowledgeType {
        // Simple heuristic based on message types and content
        if messages
            .iter()
            .any(|m| m.message_type == MessageType::IssueNotification)
        {
            KnowledgeType::Solution
        } else if messages
            .iter()
            .any(|m| m.content.to_lowercase().contains("process"))
        {
            KnowledgeType::Practice
        } else {
            KnowledgeType::Pattern
        }
    }

    fn extract_tags_from_messages(&self, messages: &[Message]) -> Vec<String> {
        let mut tags = Vec::new();

        for message in messages {
            // Simple tag extraction from message content
            let words: Vec<&str> = message.content.split_whitespace().collect();
            for word in words {
                if word.starts_with('#') && word.len() > 1 {
                    tags.push(word[1..].to_string());
                }
            }
        }

        tags.sort();
        tags.dedup();
        tags
    }

    fn assess_extraction_quality(&self, extracted: &ExtractedKnowledge) -> QualityMetrics {
        let mut metrics = QualityMetrics::new();

        // Simple quality assessment heuristics
        metrics.completeness_score = if extracted.extracted_content.len() > 100 {
            0.8
        } else {
            0.5
        };
        metrics.clarity_score = if extracted.extracted_title.len() > 10 {
            0.7
        } else {
            0.4
        };
        metrics.accuracy_score = 0.7; // Would need more sophisticated analysis
        metrics.relevance_score = if !extracted.suggested_tags.is_empty() {
            0.8
        } else {
            0.5
        };
        metrics.uniqueness_score = 0.6; // Would need duplicate detection

        metrics.calculate_overall();
        metrics
    }

    fn generate_pattern_name_from_issue(&self, issue: &Issue) -> String {
        format!(
            "Issue Resolution Pattern: {}",
            issue.tags.first().unwrap_or(&"General".to_string())
        )
    }

    fn pattern_matches_issue(&self, pattern: &RecognizedPattern, issue: &Issue) -> bool {
        // Check if pattern tags overlap with issue tags
        pattern
            .context_tags
            .iter()
            .any(|tag| issue.tags.contains(tag))
    }

    fn generate_insights_from_patterns(&self, patterns: &[RecognizedPattern]) -> Vec<String> {
        let mut insights = Vec::new();

        insights.push(format!(
            "Identified {} mature patterns with high success rates",
            patterns.len()
        ));

        if !patterns.is_empty() {
            let avg_success =
                patterns.iter().map(|p| p.success_rate).sum::<f64>() / patterns.len() as f64;
            insights.push(format!(
                "Average pattern success rate: {:.1}%",
                avg_success * 100.0
            ));
        }

        insights
    }

    fn recommend_focus_areas(
        &self,
        patterns: &[RecognizedPattern],
        _stats: &crate::repositories::knowledge_intelligence::ExtractionStatistics,
    ) -> Vec<String> {
        let mut focus_areas = Vec::new();

        if patterns.len() < 5 {
            focus_areas.push("Increase pattern recognition efforts".to_string());
        }

        focus_areas.push("Continue documenting successful resolution patterns".to_string());
        focus_areas.push("Improve knowledge extraction from agent interactions".to_string());

        focus_areas
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use sqlx::SqlitePool;
    use vibe_ensemble_core::{
        issue::{Issue, IssuePriority, IssueStatus},
    };

    async fn setup_test_service() -> Result<KnowledgeIntelligenceService> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        migrations::run_migrations(&pool).await.unwrap();

        let ki_repo = KnowledgeIntelligenceRepository::new(pool.clone());
        let knowledge_repo = KnowledgeRepository::new(pool.clone());
        let issue_repo = IssueRepository::new(pool.clone());
        let message_repo = MessageRepository::new(pool.clone());
        let agent_repo = AgentRepository::new(pool.clone());

        Ok(KnowledgeIntelligenceService::new(
            ki_repo,
            knowledge_repo,
            issue_repo,
            message_repo,
            agent_repo,
        ))
    }

    #[tokio::test]
    async fn test_extract_knowledge_from_issue() {
        let service = setup_test_service().await.unwrap();
        let _agent_id = Uuid::new_v4();

        // Create a resolved issue
        let mut issue = Issue::builder()
            .title("Test Issue")
            .description("Test description")
            .priority(IssuePriority::Medium)
            .tag("test")
            .build()
            .unwrap();

        issue.status = IssueStatus::Resolved;
        issue.resolved_at = Some(chrono::Utc::now());

        // Would need to create agent and issue in database first for full test
        // This is a simplified test of the extraction logic
        let content = service.generate_issue_knowledge_content(&issue);
        assert!(content.contains("Test Issue"));
        assert!(content.contains("Test description"));
    }

    #[tokio::test]
    async fn test_pattern_recognition() {
        let _service = setup_test_service().await.unwrap();

        let mut pattern = RecognizedPattern::new(
            PatternType::ProblemSolution,
            "Test Pattern".to_string(),
            "Test description".to_string(),
        );

        // Simulate successful observations
        pattern.observe(true);
        pattern.observe(true);
        pattern.observe(false);
        pattern.observe(true);
        pattern.observe(true);

        assert!(pattern.frequency_count >= 5);
        assert!(pattern.is_mature());
    }
}
