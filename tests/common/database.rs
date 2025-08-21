//! Database testing utilities for vibe-ensemble-mcp
//!
//! Provides utilities for database testing including:
//! - Test database creation and cleanup
//! - Transaction management for isolated tests
//! - Database state verification helpers

use sqlx::{Pool, Sqlite, SqlitePool, Transaction};
use std::sync::Arc;
use tempfile::NamedTempFile;
use uuid::Uuid;

/// Database test helper that provides isolated test databases
pub struct DatabaseTestHelper {
    pub pool: Arc<SqlitePool>,
    temp_file: NamedTempFile,
}

impl DatabaseTestHelper {
    /// Creates a new test database with all migrations applied
    pub async fn new() -> Result<Self, sqlx::Error> {
        let temp_file = NamedTempFile::new()
            .map_err(|e| sqlx::Error::Io(e))?;
        
        let db_url = format!("sqlite:{}", temp_file.path().display());
        let pool = SqlitePool::connect(&db_url).await?;
        
        // Run all migrations
        sqlx::migrate!("../vibe-ensemble-storage/migrations")
            .run(&pool)
            .await?;
        
        Ok(Self {
            pool: Arc::new(pool),
            temp_file,
        })
    }

    /// Begins a new transaction for isolated testing
    pub async fn begin_transaction(&self) -> Result<Transaction<Sqlite>, sqlx::Error> {
        self.pool.begin().await
    }

    /// Verifies that a table exists in the database
    pub async fn table_exists(&self, table_name: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?"
        )
        .bind(table_name)
        .fetch_one(self.pool.as_ref())
        .await?;
        
        Ok(result > 0)
    }

    /// Counts rows in a given table
    pub async fn count_rows(&self, table_name: &str) -> Result<i32, sqlx::Error> {
        let query = format!("SELECT COUNT(*) FROM {}", table_name);
        sqlx::query_scalar(&query)
            .fetch_one(self.pool.as_ref())
            .await
    }

    /// Clears all data from a table (useful for test cleanup)
    pub async fn clear_table(&self, table_name: &str) -> Result<(), sqlx::Error> {
        let query = format!("DELETE FROM {}", table_name);
        sqlx::query(&query)
            .execute(self.pool.as_ref())
            .await?;
        Ok(())
    }

    /// Seeds the database with test data
    pub async fn seed_test_data(&self) -> Result<(), sqlx::Error> {
        // Create test agents
        let agent_ids = self.create_test_agents(5).await?;
        
        // Create test issues
        self.create_test_issues(&agent_ids, 10).await?;
        
        // Create test messages
        self.create_test_messages(&agent_ids, 20).await?;
        
        // Create test knowledge
        self.create_test_knowledge(&agent_ids, 15).await?;
        
        Ok(())
    }

    async fn create_test_agents(&self, count: usize) -> Result<Vec<Uuid>, sqlx::Error> {
        let mut agent_ids = Vec::new();
        
        for i in 0..count {
            let agent_id = Uuid::new_v4();
            let name = format!("test-agent-{}", i);
            
            sqlx::query!(
                r#"
                INSERT INTO agents (id, name, status, capabilities, connection_metadata, created_at, last_seen)
                VALUES (?1, ?2, 'active', ?3, ?4, datetime('now'), datetime('now'))
                "#,
                agent_id.to_string(),
                name,
                serde_json::to_string(&vec!["test", "automation"]).unwrap(),
                serde_json::to_string(&serde_json::json!({
                    "host": "localhost",
                    "port": 8080,
                    "protocol": "http",
                    "last_heartbeat": chrono::Utc::now().to_rfc3339(),
                    "connection_id": Uuid::new_v4().to_string()
                })).unwrap()
            )
            .execute(self.pool.as_ref())
            .await?;
            
            agent_ids.push(agent_id);
        }
        
        Ok(agent_ids)
    }

    async fn create_test_issues(&self, agent_ids: &[Uuid], count: usize) -> Result<(), sqlx::Error> {
        for i in 0..count {
            let issue_id = Uuid::new_v4();
            let assigned_to = agent_ids[i % agent_ids.len()];
            
            sqlx::query!(
                r#"
                INSERT INTO issues (id, title, description, status, priority, assigned_to, created_at)
                VALUES (?1, ?2, ?3, 'open', 'medium', ?4, datetime('now'))
                "#,
                issue_id.to_string(),
                format!("Test Issue {}", i),
                format!("This is test issue number {} for automated testing", i),
                assigned_to.to_string()
            )
            .execute(self.pool.as_ref())
            .await?;
        }
        
        Ok(())
    }

    async fn create_test_messages(&self, agent_ids: &[Uuid], count: usize) -> Result<(), sqlx::Error> {
        for i in 0..count {
            let message_id = Uuid::new_v4();
            let sender_id = agent_ids[i % agent_ids.len()];
            let recipient_id = if i % 3 == 0 { 
                None // Broadcast message
            } else { 
                Some(agent_ids[(i + 1) % agent_ids.len()])
            };
            
            sqlx::query!(
                r#"
                INSERT INTO messages (id, sender_id, recipient_id, message_type, content, created_at)
                VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
                "#,
                message_id.to_string(),
                sender_id.to_string(),
                recipient_id.map(|id| id.to_string()),
                if recipient_id.is_some() { "direct" } else { "broadcast" },
                format!("Test message content {}", i)
            )
            .execute(self.pool.as_ref())
            .await?;
        }
        
        Ok(())
    }

    async fn create_test_knowledge(&self, agent_ids: &[Uuid], count: usize) -> Result<(), sqlx::Error> {
        for i in 0..count {
            let knowledge_id = Uuid::new_v4();
            let created_by = agent_ids[i % agent_ids.len()];
            
            sqlx::query!(
                r#"
                INSERT INTO knowledge (id, title, content, knowledge_type, access_level, created_by, created_at)
                VALUES (?1, ?2, ?3, 'best_practice', 'team_visible', ?4, datetime('now'))
                "#,
                knowledge_id.to_string(),
                format!("Test Knowledge {}", i),
                format!("This is test knowledge content number {} for automated testing", i),
                created_by.to_string()
            )
            .execute(self.pool.as_ref())
            .await?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_helper_creation() {
        let helper = DatabaseTestHelper::new().await.unwrap();
        
        // Verify core tables exist
        assert!(helper.table_exists("agents").await.unwrap());
        assert!(helper.table_exists("issues").await.unwrap());
        assert!(helper.table_exists("messages").await.unwrap());
        assert!(helper.table_exists("knowledge").await.unwrap());
    }

    #[tokio::test]
    async fn test_seed_test_data() {
        let helper = DatabaseTestHelper::new().await.unwrap();
        helper.seed_test_data().await.unwrap();
        
        // Verify data was created
        assert_eq!(helper.count_rows("agents").await.unwrap(), 5);
        assert_eq!(helper.count_rows("issues").await.unwrap(), 10);
        assert_eq!(helper.count_rows("messages").await.unwrap(), 20);
        assert_eq!(helper.count_rows("knowledge").await.unwrap(), 15);
    }

    #[tokio::test]
    async fn test_transaction_isolation() {
        let helper = DatabaseTestHelper::new().await.unwrap();
        
        // Begin transaction and insert data
        let mut tx = helper.begin_transaction().await.unwrap();
        sqlx::query!("INSERT INTO agents (id, name, status, created_at, last_seen) VALUES ('test', 'test', 'active', datetime('now'), datetime('now'))")
            .execute(&mut *tx)
            .await
            .unwrap();
        
        // Data should not be visible outside transaction
        assert_eq!(helper.count_rows("agents").await.unwrap(), 0);
        
        // Commit and verify data is visible
        tx.commit().await.unwrap();
        assert_eq!(helper.count_rows("agents").await.unwrap(), 1);
    }
}