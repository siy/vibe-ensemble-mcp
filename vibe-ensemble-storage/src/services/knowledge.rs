//! Knowledge service implementation providing high-level operations
//!
//! This service layer provides business logic for knowledge management,
//! including integration with messaging and issue tracking systems.

use crate::{repositories::KnowledgeRepository, Error, Result};
use uuid::Uuid;
use vibe_ensemble_core::{
    issue::Issue,
    knowledge::{Knowledge, KnowledgeSearchCriteria, KnowledgeSearchResult},
    message::{Message, MessageType},
};

/// High-level knowledge service providing business operations
pub struct KnowledgeService {
    repository: KnowledgeRepository,
}

impl KnowledgeService {
    /// Create a new knowledge service
    pub fn new(repository: KnowledgeRepository) -> Self {
        Self { repository }
    }

    /// Create a new knowledge entry
    pub async fn create_knowledge(&self, knowledge: &Knowledge) -> Result<()> {
        self.repository.create(knowledge).await
    }

    /// Find knowledge by ID with access control
    pub async fn find_knowledge(&self, id: Uuid, agent_id: Uuid) -> Result<Option<Knowledge>> {
        if let Some(knowledge) = self.repository.find_by_id(id).await? {
            if knowledge.is_accessible_by(agent_id) {
                Ok(Some(knowledge))
            } else {
                Err(Error::Unauthorized {
                    message: "Access denied to knowledge entry".to_string(),
                })
            }
        } else {
            Ok(None)
        }
    }

    /// Search knowledge with criteria
    pub async fn search_knowledge(
        &self,
        criteria: &KnowledgeSearchCriteria,
        agent_id: Uuid,
    ) -> Result<Vec<KnowledgeSearchResult>> {
        self.repository.search(criteria, agent_id).await
    }

    /// Integration: Extract knowledge from resolved issues
    pub async fn extract_knowledge_from_issue(
        &self,
        issue: &Issue,
        extracted_by: Uuid,
    ) -> Result<Knowledge> {
        // Create knowledge entry from issue resolution
        let knowledge = Knowledge::builder()
            .title(format!("Solution: {}", issue.title))
            .content(format!(
                "Issue: {}\n\nDescription: {}\n\nResolution: Resolved at {}",
                issue.title,
                issue.description,
                issue
                    .resolved_at
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| "Unknown".to_string())
            ))
            .knowledge_type(vibe_ensemble_core::knowledge::KnowledgeType::Solution)
            .created_by(extracted_by)
            .access_level(vibe_ensemble_core::knowledge::AccessLevel::Team)
            .tags(issue.tags.clone())
            .build()?;

        self.create_knowledge(&knowledge).await?;

        Ok(knowledge)
    }

    /// Integration: Create knowledge sharing message
    pub async fn create_knowledge_share_message(
        &self,
        knowledge: &Knowledge,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        share_context: Option<String>,
    ) -> Result<Message> {
        // Create message with knowledge reference
        let message_content = serde_json::json!({
            "knowledge_id": knowledge.id,
            "knowledge_title": knowledge.title,
            "knowledge_type": knowledge.knowledge_type,
            "share_context": share_context,
            "snippet": knowledge.content.chars().take(200).collect::<String>()
        });

        let message = if let Some(recipient) = recipient_id {
            Message::new_direct(
                sender_id,
                recipient,
                message_content.to_string(),
                vibe_ensemble_core::message::MessagePriority::Normal,
            )?
        } else {
            Message::new_broadcast(
                sender_id,
                message_content.to_string(),
                vibe_ensemble_core::message::MessagePriority::Normal,
            )?
        };
        Ok(message)
    }

    /// Integration: Process knowledge from message
    pub async fn process_knowledge_message(
        &self,
        message: &Message,
        processor_id: Uuid,
    ) -> Result<Option<Knowledge>> {
        if message.message_type != MessageType::KnowledgeShare {
            return Ok(None);
        }

        // Parse knowledge ID from message
        let content: serde_json::Value = serde_json::from_str(&message.content)?;
        if let Some(knowledge_id_str) = content.get("knowledge_id").and_then(|v| v.as_str()) {
            let knowledge_id = Uuid::parse_str(knowledge_id_str)?;

            // Retrieve and return the knowledge if accessible
            self.find_knowledge(knowledge_id, processor_id).await
        } else {
            Ok(None)
        }
    }

    /// Get knowledge statistics for an agent
    pub async fn get_agent_statistics(&self, agent_id: Uuid) -> Result<KnowledgeStatistics> {
        let total_accessible = self.repository.count_accessible_by(agent_id).await?;
        let total_created = self
            .repository
            .search(
                &KnowledgeSearchCriteria::new().with_created_by(agent_id),
                agent_id,
            )
            .await?
            .len() as i64;

        Ok(KnowledgeStatistics {
            total_accessible,
            total_created,
            recent_views: 0,     // Simplified for core implementation
            knowledge_shared: 0, // Simplified for core implementation
        })
    }
}

/// Statistics about knowledge usage for an agent
#[derive(Debug, Clone)]
pub struct KnowledgeStatistics {
    pub total_accessible: i64,
    pub total_created: i64,
    pub recent_views: i64,
    pub knowledge_shared: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Pool, Sqlite, SqlitePool};

    async fn setup_test_service() -> Result<(KnowledgeService, Pool<Sqlite>)> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Run migrations
        crate::migrations::run_migrations(&pool).await.unwrap();

        let repository = KnowledgeRepository::new(pool.clone());
        Ok((KnowledgeService::new(repository), pool))
    }

    async fn create_test_agent(pool: &Pool<Sqlite>, agent_id: Uuid) {
        sqlx::query(
            r#"
            INSERT INTO agents (id, name, agent_type, capabilities, status, connection_metadata, created_at, last_seen)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#
        )
        .bind(agent_id.to_string())
        .bind("Test Agent")
        .bind("test")
        .bind("[]")
        .bind("active")
        .bind("{}")
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_create_and_find_knowledge() {
        let (service, pool) = setup_test_service().await.unwrap();
        let creator_id = Uuid::new_v4();

        // Create test agent first
        create_test_agent(&pool, creator_id).await;

        let knowledge = Knowledge::builder()
            .title("Test Knowledge")
            .content("Test content for knowledge management")
            .knowledge_type(vibe_ensemble_core::knowledge::KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(vibe_ensemble_core::knowledge::AccessLevel::Public)
            .tag("test")
            .build()
            .unwrap();

        // Create knowledge
        service.create_knowledge(&knowledge).await.unwrap();

        // Find knowledge
        let found = service
            .find_knowledge(knowledge.id, creator_id)
            .await
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().title, "Test Knowledge");
    }

    #[tokio::test]
    async fn test_access_control() {
        let (service, pool) = setup_test_service().await.unwrap();
        let creator_id = Uuid::new_v4();
        let other_agent_id = Uuid::new_v4();

        // Create test agents first
        create_test_agent(&pool, creator_id).await;
        create_test_agent(&pool, other_agent_id).await;

        let private_knowledge = Knowledge::builder()
            .title("Private Knowledge")
            .content("Private content")
            .knowledge_type(vibe_ensemble_core::knowledge::KnowledgeType::Practice)
            .created_by(creator_id)
            .access_level(vibe_ensemble_core::knowledge::AccessLevel::Private)
            .build()
            .unwrap();

        service.create_knowledge(&private_knowledge).await.unwrap();

        // Creator should have access
        let found_by_creator = service
            .find_knowledge(private_knowledge.id, creator_id)
            .await
            .unwrap();
        assert!(found_by_creator.is_some());

        // Other agent should not have access
        let found_by_other = service
            .find_knowledge(private_knowledge.id, other_agent_id)
            .await;
        assert!(found_by_other.is_err());
    }
}
