//! Knowledge intelligence repository implementation
//!
//! This module provides persistence and retrieval operations for advanced
//! knowledge management features including extraction, pattern recognition,
//! contribution workflows, and organizational learning.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;
use vibe_ensemble_core::knowledge_intelligence::{
    CapabilityEnhancement, ExtractedKnowledge, KnowledgeContribution, KnowledgeSuggestion,
    PatternType, RecognizedPattern, ReviewStatus, SuggestionTarget, WorkflowStatus,
};

/// Repository for knowledge intelligence entities
pub struct KnowledgeIntelligenceRepository {
    pool: Pool<Sqlite>,
}

impl KnowledgeIntelligenceRepository {
    /// Create a new knowledge intelligence repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    // ========== Extracted Knowledge Operations ==========

    /// Create a new extracted knowledge entry
    pub async fn create_extracted_knowledge(&self, extracted: &ExtractedKnowledge) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO extracted_knowledge 
            (id, source_type, source_id, extracted_title, extracted_content, confidence_score,
             extraction_method, suggested_type, suggested_tags, extracted_by, extracted_at,
             review_status, quality_metrics)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
        )
        .bind(extracted.id.to_string())
        .bind(serde_json::to_string(&extracted.source_type)?)
        .bind(extracted.source_id.to_string())
        .bind(&extracted.extracted_title)
        .bind(&extracted.extracted_content)
        .bind(extracted.confidence_score)
        .bind(serde_json::to_string(&extracted.extraction_method)?)
        .bind(serde_json::to_string(&extracted.suggested_type)?)
        .bind(serde_json::to_string(&extracted.suggested_tags)?)
        .bind(extracted.extracted_by.to_string())
        .bind(extracted.extracted_at.to_rfc3339())
        .bind(serde_json::to_string(&extracted.review_status)?)
        .bind(serde_json::to_string(&extracted.quality_metrics)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find extracted knowledge by ID
    pub async fn find_extracted_knowledge(&self, id: Uuid) -> Result<Option<ExtractedKnowledge>> {
        let row = sqlx::query("SELECT * FROM extracted_knowledge WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let extracted = Self::extracted_knowledge_from_row(&row)?;
                Ok(Some(extracted))
            }
            None => Ok(None),
        }
    }

    /// Update extracted knowledge
    pub async fn update_extracted_knowledge(&self, extracted: &ExtractedKnowledge) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE extracted_knowledge 
            SET confidence_score = ?2, review_status = ?3, quality_metrics = ?4,
                suggested_type = ?5, suggested_tags = ?6
            WHERE id = ?1
            "#,
        )
        .bind(extracted.id.to_string())
        .bind(extracted.confidence_score)
        .bind(serde_json::to_string(&extracted.review_status)?)
        .bind(serde_json::to_string(&extracted.quality_metrics)?)
        .bind(serde_json::to_string(&extracted.suggested_type)?)
        .bind(serde_json::to_string(&extracted.suggested_tags)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List extracted knowledge pending review
    pub async fn list_pending_extracted_knowledge(&self) -> Result<Vec<ExtractedKnowledge>> {
        let rows = sqlx::query(
            "SELECT * FROM extracted_knowledge WHERE review_status = ?1 ORDER BY extracted_at ASC",
        )
        .bind(serde_json::to_string(&ReviewStatus::Pending)?)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::extracted_knowledge_from_row(&row)?);
        }
        Ok(results)
    }

    /// List extracted knowledge by source
    pub async fn list_extracted_knowledge_by_source(
        &self,
        source_id: Uuid,
    ) -> Result<Vec<ExtractedKnowledge>> {
        let rows = sqlx::query(
            "SELECT * FROM extracted_knowledge WHERE source_id = ?1 ORDER BY extracted_at DESC",
        )
        .bind(source_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::extracted_knowledge_from_row(&row)?);
        }
        Ok(results)
    }

    // ========== Pattern Recognition Operations ==========

    /// Create or update a recognized pattern
    pub async fn upsert_recognized_pattern(&self, pattern: &RecognizedPattern) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO recognized_patterns
            (id, pattern_type, pattern_name, pattern_description, frequency_count, success_rate,
             context_tags, related_issues, related_agents, first_observed, last_observed, confidence_level)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(pattern.id.to_string())
        .bind(serde_json::to_string(&pattern.pattern_type)?)
        .bind(&pattern.pattern_name)
        .bind(&pattern.pattern_description)
        .bind(pattern.frequency_count as i64)
        .bind(pattern.success_rate)
        .bind(serde_json::to_string(&pattern.context_tags)?)
        .bind(serde_json::to_string(&pattern.related_issues)?)
        .bind(serde_json::to_string(&pattern.related_agents)?)
        .bind(pattern.first_observed.to_rfc3339())
        .bind(pattern.last_observed.to_rfc3339())
        .bind(pattern.confidence_level)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find pattern by ID
    pub async fn find_pattern(&self, id: Uuid) -> Result<Option<RecognizedPattern>> {
        let row = sqlx::query("SELECT * FROM recognized_patterns WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let pattern = Self::pattern_from_row(&row)?;
                Ok(Some(pattern))
            }
            None => Ok(None),
        }
    }

    /// Find patterns by type
    pub async fn find_patterns_by_type(
        &self,
        pattern_type: &PatternType,
    ) -> Result<Vec<RecognizedPattern>> {
        let rows = sqlx::query(
            "SELECT * FROM recognized_patterns WHERE pattern_type = ?1 ORDER BY confidence_level DESC"
        )
        .bind(serde_json::to_string(pattern_type)?)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::pattern_from_row(&row)?);
        }
        Ok(results)
    }

    /// Find mature patterns (high confidence and frequency)
    pub async fn find_mature_patterns(&self) -> Result<Vec<RecognizedPattern>> {
        let rows = sqlx::query(
            "SELECT * FROM recognized_patterns WHERE confidence_level >= 0.7 AND frequency_count >= 5 ORDER BY confidence_level DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::pattern_from_row(&row)?);
        }
        Ok(results)
    }

    // ========== Knowledge Contribution Operations ==========

    /// Create knowledge contribution
    pub async fn create_knowledge_contribution(
        &self,
        contribution: &KnowledgeContribution,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO knowledge_contributions
            (id, contributor_id, knowledge_id, extracted_knowledge_id, contribution_type,
             proposed_changes, workflow_status, submitted_at, reviewed_at, approved_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(contribution.id.to_string())
        .bind(contribution.contributor_id.to_string())
        .bind(contribution.knowledge_id.map(|id| id.to_string()))
        .bind(contribution.extracted_knowledge_id.map(|id| id.to_string()))
        .bind(serde_json::to_string(&contribution.contribution_type)?)
        .bind(contribution.proposed_changes.to_string())
        .bind(serde_json::to_string(&contribution.workflow_status)?)
        .bind(contribution.submitted_at.to_rfc3339())
        .bind(contribution.reviewed_at.map(|dt| dt.to_rfc3339()))
        .bind(contribution.approved_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update knowledge contribution status
    pub async fn update_contribution_status(
        &self,
        contribution: &KnowledgeContribution,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE knowledge_contributions 
            SET workflow_status = ?2, reviewed_at = ?3, approved_at = ?4
            WHERE id = ?1
            "#,
        )
        .bind(contribution.id.to_string())
        .bind(serde_json::to_string(&contribution.workflow_status)?)
        .bind(contribution.reviewed_at.map(|dt| dt.to_rfc3339()))
        .bind(contribution.approved_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// List contributions pending review
    pub async fn list_pending_contributions(&self) -> Result<Vec<KnowledgeContribution>> {
        let rows = sqlx::query(
            "SELECT * FROM knowledge_contributions WHERE workflow_status = ?1 ORDER BY submitted_at ASC"
        )
        .bind(serde_json::to_string(&WorkflowStatus::Submitted)?)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::contribution_from_row(&row)?);
        }
        Ok(results)
    }

    // ========== Knowledge Suggestions Operations ==========

    /// Create knowledge suggestion
    pub async fn create_knowledge_suggestion(
        &self,
        suggestion: &KnowledgeSuggestion,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO knowledge_suggestions
            (id, target_type, target_id, suggested_knowledge, suggestion_reason, confidence_score,
             generated_by, generated_at, accepted, feedback)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(suggestion.id.to_string())
        .bind(serde_json::to_string(&suggestion.target_type)?)
        .bind(suggestion.target_id.to_string())
        .bind(serde_json::to_string(&suggestion.suggested_knowledge)?)
        .bind(&suggestion.suggestion_reason)
        .bind(suggestion.confidence_score)
        .bind(suggestion.generated_by.to_string())
        .bind(suggestion.generated_at.to_rfc3339())
        .bind(suggestion.accepted.map(|b| b as i32))
        .bind(&suggestion.feedback)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update suggestion acceptance
    pub async fn update_suggestion_acceptance(
        &self,
        suggestion: &KnowledgeSuggestion,
    ) -> Result<()> {
        sqlx::query("UPDATE knowledge_suggestions SET accepted = ?2, feedback = ?3 WHERE id = ?1")
            .bind(suggestion.id.to_string())
            .bind(suggestion.accepted.map(|b| b as i32))
            .bind(&suggestion.feedback)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Find suggestions for target
    pub async fn find_suggestions_for_target(
        &self,
        target_type: &SuggestionTarget,
        target_id: Uuid,
    ) -> Result<Vec<KnowledgeSuggestion>> {
        let rows = sqlx::query(
            "SELECT * FROM knowledge_suggestions WHERE target_type = ?1 AND target_id = ?2 ORDER BY confidence_score DESC"
        )
        .bind(serde_json::to_string(target_type)?)
        .bind(target_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::suggestion_from_row(&row)?);
        }
        Ok(results)
    }

    // ========== Capability Enhancement Operations ==========

    /// Create capability enhancement record
    pub async fn create_capability_enhancement(
        &self,
        enhancement: &CapabilityEnhancement,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO capability_enhancements
            (id, agent_id, enhancement_type, knowledge_sources, capability_gained,
             proficiency_level, verification_method, enhanced_at, validated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(enhancement.id.to_string())
        .bind(enhancement.agent_id.to_string())
        .bind(serde_json::to_string(&enhancement.enhancement_type)?)
        .bind(serde_json::to_string(&enhancement.knowledge_sources)?)
        .bind(&enhancement.capability_gained)
        .bind(enhancement.proficiency_level)
        .bind(serde_json::to_string(&enhancement.verification_method)?)
        .bind(enhancement.enhanced_at.to_rfc3339())
        .bind(enhancement.validated_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Update capability enhancement validation
    pub async fn update_enhancement_validation(
        &self,
        enhancement: &CapabilityEnhancement,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE capability_enhancements 
            SET proficiency_level = ?2, verification_method = ?3, validated_at = ?4
            WHERE id = ?1
            "#,
        )
        .bind(enhancement.id.to_string())
        .bind(enhancement.proficiency_level)
        .bind(serde_json::to_string(&enhancement.verification_method)?)
        .bind(enhancement.validated_at.map(|dt| dt.to_rfc3339()))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find enhancements for agent
    pub async fn find_enhancements_for_agent(
        &self,
        agent_id: Uuid,
    ) -> Result<Vec<CapabilityEnhancement>> {
        let rows = sqlx::query(
            "SELECT * FROM capability_enhancements WHERE agent_id = ?1 ORDER BY enhanced_at DESC",
        )
        .bind(agent_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            results.push(Self::enhancement_from_row(&row)?);
        }
        Ok(results)
    }

    // ========== Utility Methods ==========

    /// Get knowledge extraction statistics
    pub async fn get_extraction_statistics(&self) -> Result<ExtractionStatistics> {
        let total_row = sqlx::query("SELECT COUNT(*) as count FROM extracted_knowledge")
            .fetch_one(&self.pool)
            .await?;
        let total_extracted = total_row.get::<i64, _>("count");

        let approved_row = sqlx::query(
            "SELECT COUNT(*) as count FROM extracted_knowledge WHERE review_status = ?1",
        )
        .bind(serde_json::to_string(&ReviewStatus::Approved)?)
        .fetch_one(&self.pool)
        .await?;
        let approved_count = approved_row.get::<i64, _>("count");

        let pending_row = sqlx::query(
            "SELECT COUNT(*) as count FROM extracted_knowledge WHERE review_status = ?1",
        )
        .bind(serde_json::to_string(&ReviewStatus::Pending)?)
        .fetch_one(&self.pool)
        .await?;
        let pending_count = pending_row.get::<i64, _>("count");

        Ok(ExtractionStatistics {
            total_extracted,
            approved_count,
            pending_count,
            approval_rate: if total_extracted > 0 {
                approved_count as f64 / total_extracted as f64
            } else {
                0.0
            },
        })
    }

    // ========== Row Conversion Methods ==========

    fn extracted_knowledge_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<ExtractedKnowledge> {
        Ok(ExtractedKnowledge {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            source_type: serde_json::from_str(&row.get::<String, _>("source_type"))?,
            source_id: Uuid::parse_str(&row.get::<String, _>("source_id"))?,
            extracted_title: row.get("extracted_title"),
            extracted_content: row.get("extracted_content"),
            confidence_score: row.get("confidence_score"),
            extraction_method: serde_json::from_str(&row.get::<String, _>("extraction_method"))?,
            suggested_type: serde_json::from_str(&row.get::<String, _>("suggested_type"))?,
            suggested_tags: serde_json::from_str(&row.get::<String, _>("suggested_tags"))?,
            extracted_by: Uuid::parse_str(&row.get::<String, _>("extracted_by"))?,
            extracted_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("extracted_at"))?
                .with_timezone(&Utc),
            review_status: serde_json::from_str(&row.get::<String, _>("review_status"))?,
            quality_metrics: serde_json::from_str(&row.get::<String, _>("quality_metrics"))?,
        })
    }

    fn pattern_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<RecognizedPattern> {
        Ok(RecognizedPattern {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            pattern_type: serde_json::from_str(&row.get::<String, _>("pattern_type"))?,
            pattern_name: row.get("pattern_name"),
            pattern_description: row.get("pattern_description"),
            frequency_count: row.get::<i64, _>("frequency_count") as u32,
            success_rate: row.get("success_rate"),
            context_tags: serde_json::from_str(&row.get::<String, _>("context_tags"))?,
            related_issues: serde_json::from_str(&row.get::<String, _>("related_issues"))?,
            related_agents: serde_json::from_str(&row.get::<String, _>("related_agents"))?,
            first_observed: DateTime::parse_from_rfc3339(&row.get::<String, _>("first_observed"))?
                .with_timezone(&Utc),
            last_observed: DateTime::parse_from_rfc3339(&row.get::<String, _>("last_observed"))?
                .with_timezone(&Utc),
            confidence_level: row.get("confidence_level"),
        })
    }

    fn contribution_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<KnowledgeContribution> {
        Ok(KnowledgeContribution {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            contributor_id: Uuid::parse_str(&row.get::<String, _>("contributor_id"))?,
            knowledge_id: row
                .get::<Option<String>, _>("knowledge_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()?,
            extracted_knowledge_id: row
                .get::<Option<String>, _>("extracted_knowledge_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()?,
            contribution_type: serde_json::from_str(&row.get::<String, _>("contribution_type"))?,
            proposed_changes: serde_json::from_str(&row.get::<String, _>("proposed_changes"))?,
            review_comments: Vec::new(), // Comments loaded separately if needed
            workflow_status: serde_json::from_str(&row.get::<String, _>("workflow_status"))?,
            submitted_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("submitted_at"))?
                .with_timezone(&Utc),
            reviewed_at: row
                .get::<Option<String>, _>("reviewed_at")
                .map(|s| DateTime::parse_from_rfc3339(&s))
                .transpose()?
                .map(|dt| dt.with_timezone(&Utc)),
            approved_at: row
                .get::<Option<String>, _>("approved_at")
                .map(|s| DateTime::parse_from_rfc3339(&s))
                .transpose()?
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }

    fn suggestion_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<KnowledgeSuggestion> {
        Ok(KnowledgeSuggestion {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            target_type: serde_json::from_str(&row.get::<String, _>("target_type"))?,
            target_id: Uuid::parse_str(&row.get::<String, _>("target_id"))?,
            suggested_knowledge: serde_json::from_str(
                &row.get::<String, _>("suggested_knowledge"),
            )?,
            suggestion_reason: row.get("suggestion_reason"),
            confidence_score: row.get("confidence_score"),
            generated_by: Uuid::parse_str(&row.get::<String, _>("generated_by"))?,
            generated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("generated_at"))?
                .with_timezone(&Utc),
            accepted: row.get::<Option<i32>, _>("accepted").map(|i| i != 0),
            feedback: row.get("feedback"),
        })
    }

    fn enhancement_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<CapabilityEnhancement> {
        Ok(CapabilityEnhancement {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            agent_id: Uuid::parse_str(&row.get::<String, _>("agent_id"))?,
            enhancement_type: serde_json::from_str(&row.get::<String, _>("enhancement_type"))?,
            knowledge_sources: serde_json::from_str(&row.get::<String, _>("knowledge_sources"))?,
            capability_gained: row.get("capability_gained"),
            proficiency_level: row.get("proficiency_level"),
            verification_method: serde_json::from_str(
                &row.get::<String, _>("verification_method"),
            )?,
            enhanced_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("enhanced_at"))?
                .with_timezone(&Utc),
            validated_at: row
                .get::<Option<String>, _>("validated_at")
                .map(|s| DateTime::parse_from_rfc3339(&s))
                .transpose()?
                .map(|dt| dt.with_timezone(&Utc)),
        })
    }
}

/// Statistics about knowledge extraction
#[derive(Debug, Clone)]
pub struct ExtractionStatistics {
    pub total_extracted: i64,
    pub approved_count: i64,
    pub pending_count: i64,
    pub approval_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use crate::repositories::AgentRepository;
    use sqlx::SqlitePool;
    use vibe_ensemble_core::agent::{Agent, AgentType, ConnectionMetadata};
    use vibe_ensemble_core::knowledge_intelligence::*;

    async fn create_test_agent(pool: &SqlitePool, name: &str) -> Uuid {
        let agent_repo = AgentRepository::new(pool.clone());
        
        let metadata = ConnectionMetadata::builder()
            .endpoint("https://localhost:8080")
            .protocol_version("1.0")
            .build()
            .unwrap();

        let agent = Agent::builder()
            .name(name)
            .agent_type(AgentType::Worker)
            .capability("testing")
            .connection_metadata(metadata)
            .build()
            .unwrap();

        agent_repo.create(&agent).await.unwrap();
        agent.id
    }

    #[tokio::test]
    async fn test_extracted_knowledge_operations() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        let repo = KnowledgeIntelligenceRepository::new(pool.clone());

        let agent_id = create_test_agent(&pool, "test-agent").await;

        let extracted = ExtractedKnowledge::new(
            ExtractionSource::IssueResolution,
            Uuid::new_v4(),
            "Test Pattern".to_string(),
            "Test content".to_string(),
            ExtractionMethod::PatternMatching,
            agent_id,
        );

        // Test create
        repo.create_extracted_knowledge(&extracted).await.unwrap();

        // Test find
        let found = repo.find_extracted_knowledge(extracted.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().extracted_title, "Test Pattern");

        // Test list pending
        let pending = repo.list_pending_extracted_knowledge().await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn test_pattern_operations() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        let repo = KnowledgeIntelligenceRepository::new(pool.clone());

        let mut pattern = RecognizedPattern::new(
            PatternType::ProblemSolution,
            "Test Pattern".to_string(),
            "Test description".to_string(),
        );

        // Simulate some observations to make it mature (more to overcome exponential moving average)
        for _ in 0..10 {
            pattern.observe(true);
        }

        // Test upsert
        repo.upsert_recognized_pattern(&pattern).await.unwrap();

        // Test find
        let found = repo.find_pattern(pattern.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().pattern_name, "Test Pattern");

        // Test find mature patterns
        let mature = repo.find_mature_patterns().await.unwrap();
        assert_eq!(mature.len(), 1);
    }
}
