//! Knowledge repository implementation with core functionality
//!
//! This module provides CRUD operations, search capabilities, and
//! basic knowledge management for the Vibe Ensemble system.

use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Row, Sqlite};
use uuid::Uuid;
use vibe_ensemble_core::knowledge::{Knowledge, KnowledgeSearchCriteria, KnowledgeSearchResult};

/// Repository for knowledge entities
pub struct KnowledgeRepository {
    pool: Pool<Sqlite>,
}

impl KnowledgeRepository {
    /// Create a new knowledge repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    /// Create a new knowledge entry
    pub async fn create(&self, knowledge: &Knowledge) -> Result<()> {
        let tags_json = serde_json::to_string(&knowledge.tags)?;

        sqlx::query(
            r#"
            INSERT INTO knowledge 
            (id, title, content, knowledge_type, tags, created_by, created_at, updated_at, version, access_level)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
        )
        .bind(knowledge.id.to_string())
        .bind(&knowledge.title)
        .bind(&knowledge.content)
        .bind(serde_json::to_string(&knowledge.knowledge_type)?)
        .bind(tags_json)
        .bind(knowledge.created_by.to_string())
        .bind(knowledge.created_at.to_rfc3339())
        .bind(knowledge.updated_at.to_rfc3339())
        .bind(knowledge.version as i64)
        .bind(serde_json::to_string(&knowledge.access_level)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find a knowledge entry by ID
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Knowledge>> {
        let row = sqlx::query("SELECT * FROM knowledge WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await?;

        match row {
            Some(row) => {
                let knowledge = Self::knowledge_from_row(&row)?;
                Ok(Some(knowledge))
            }
            None => Ok(None),
        }
    }

    /// Update a knowledge entry
    pub async fn update(&self, knowledge: &Knowledge) -> Result<()> {
        let tags_json = serde_json::to_string(&knowledge.tags)?;

        sqlx::query(
            r#"
            UPDATE knowledge 
            SET title = ?2, content = ?3, knowledge_type = ?4, tags = ?5, 
                updated_at = ?6, version = ?7, access_level = ?8
            WHERE id = ?1
            "#,
        )
        .bind(knowledge.id.to_string())
        .bind(&knowledge.title)
        .bind(&knowledge.content)
        .bind(serde_json::to_string(&knowledge.knowledge_type)?)
        .bind(tags_json)
        .bind(knowledge.updated_at.to_rfc3339())
        .bind(knowledge.version as i64)
        .bind(serde_json::to_string(&knowledge.access_level)?)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Delete a knowledge entry
    pub async fn delete(&self, id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM knowledge WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// List knowledge entries accessible by an agent
    pub async fn list_accessible_by(&self, agent_id: Uuid) -> Result<Vec<Knowledge>> {
        let rows = sqlx::query(
            r#"
            SELECT * FROM knowledge 
            WHERE access_level = '"Public"' 
               OR access_level = '"Team"' 
               OR (access_level = '"Private"' AND created_by = ?1)
            ORDER BY updated_at DESC
            "#,
        )
        .bind(agent_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut knowledge_entries = Vec::new();
        for row in rows {
            knowledge_entries.push(Self::knowledge_from_row(&row)?);
        }

        Ok(knowledge_entries)
    }

    /// Search knowledge entries using criteria
    pub async fn search(
        &self,
        criteria: &KnowledgeSearchCriteria,
        agent_id: Uuid,
    ) -> Result<Vec<KnowledgeSearchResult>> {
        let mut query_str = r#"
            SELECT * FROM knowledge 
            WHERE (access_level = '"Public"' OR access_level = '"Team"' OR (access_level = '"Private"' AND created_by = ?1))
        "#.to_string();

        let mut bindings = vec![agent_id.to_string()];

        // Add simple text search if query is provided
        if let Some(search_query) = &criteria.query {
            query_str.push_str(" AND (title LIKE ?2 OR content LIKE ?2)");
            bindings.push(format!("%{}%", search_query));
        }

        // Add creator filter
        if let Some(creator) = criteria.created_by {
            query_str.push_str(&format!(" AND created_by = ?{}", bindings.len() + 1));
            bindings.push(creator.to_string());
        }

        query_str.push_str(" ORDER BY updated_at DESC");

        // Apply pagination
        if let Some(limit) = criteria.limit {
            query_str.push_str(&format!(" LIMIT ?{}", bindings.len() + 1));
            bindings.push(limit.to_string());
        }

        if let Some(offset) = criteria.offset {
            query_str.push_str(&format!(" OFFSET ?{}", bindings.len() + 1));
            bindings.push(offset.to_string());
        }

        // Execute query
        let mut query = sqlx::query(&query_str);
        for binding in bindings {
            query = query.bind(binding);
        }

        let rows = query.fetch_all(&self.pool).await?;

        let mut results = Vec::new();
        for row in rows {
            let knowledge = Self::knowledge_from_row(&row)?;
            results.push(KnowledgeSearchResult {
                knowledge,
                relevance_score: 1.0,
                matched_fields: vec!["title".to_string(), "content".to_string()],
                snippet: None,
            });
        }

        Ok(results)
    }

    /// Count knowledge entries
    pub async fn count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM knowledge")
            .fetch_one(&self.pool)
            .await?;

        Ok(row.get::<i64, _>("count"))
    }

    /// Count knowledge entries accessible by an agent
    pub async fn count_accessible_by(&self, agent_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count FROM knowledge 
            WHERE access_level = '"Public"' 
               OR access_level = '"Team"' 
               OR (access_level = '"Private"' AND created_by = ?1)
            "#,
        )
        .bind(agent_id.to_string())
        .fetch_one(&self.pool)
        .await?;

        Ok(row.get::<i64, _>("count"))
    }

    /// Convert database row to Knowledge struct
    fn knowledge_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Knowledge> {
        Ok(Knowledge {
            id: Uuid::parse_str(&row.get::<String, _>("id"))?,
            title: row.get("title"),
            content: row.get("content"),
            knowledge_type: serde_json::from_str(&row.get::<String, _>("knowledge_type"))?,
            tags: serde_json::from_str(&row.get::<String, _>("tags"))?,
            created_by: Uuid::parse_str(&row.get::<String, _>("created_by"))?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))?
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))?
                .with_timezone(&Utc),
            version: row.get::<i64, _>("version") as u32,
            access_level: serde_json::from_str(&row.get::<String, _>("access_level"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrations;
    use sqlx::SqlitePool;
    use uuid::Uuid;
    use vibe_ensemble_core::knowledge::{AccessLevel, Knowledge, KnowledgeType};

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

    async fn create_test_knowledge(
        repo: &KnowledgeRepository,
        pool: &Pool<Sqlite>,
        creator_id: Uuid,
    ) -> Knowledge {
        // Ensure agent exists first
        create_test_agent(pool, creator_id).await;

        let knowledge = Knowledge::builder()
            .title("Test Knowledge Entry")
            .content("This is a test knowledge entry for testing purposes.")
            .knowledge_type(KnowledgeType::Pattern)
            .created_by(creator_id)
            .access_level(AccessLevel::Public)
            .tag("test")
            .tag("pattern")
            .build()
            .unwrap();

        repo.create(&knowledge).await.unwrap();
        knowledge
    }

    #[tokio::test]
    async fn test_basic_knowledge_operations() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        let repo = KnowledgeRepository::new(pool.clone());
        let creator_id = Uuid::new_v4();

        // Create knowledge
        let knowledge = create_test_knowledge(&repo, &pool, creator_id).await;

        // Test find by ID
        let found = repo.find_by_id(knowledge.id).await.unwrap();
        assert!(found.is_some());

        let found_knowledge = found.unwrap();
        assert_eq!(found_knowledge.id, knowledge.id);
        assert_eq!(found_knowledge.title, knowledge.title);

        // Test count
        let count = repo.count().await.unwrap();
        assert_eq!(count, 1);

        // Test delete
        repo.delete(knowledge.id).await.unwrap();
        let found_after_delete = repo.find_by_id(knowledge.id).await.unwrap();
        assert!(found_after_delete.is_none());
    }

    #[tokio::test]
    async fn test_search_knowledge() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        migrations::run_migrations(&pool).await.unwrap();
        let repo = KnowledgeRepository::new(pool.clone());
        let creator_id = Uuid::new_v4();

        // Create knowledge
        let _knowledge = create_test_knowledge(&repo, &pool, creator_id).await;

        // Test search with query
        let criteria = KnowledgeSearchCriteria::new().with_query("test");
        let results = repo.search(&criteria, creator_id).await.unwrap();
        assert_eq!(results.len(), 1);

        // Test search with creator filter
        let criteria = KnowledgeSearchCriteria::new().with_created_by(creator_id);
        let results = repo.search(&criteria, creator_id).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
