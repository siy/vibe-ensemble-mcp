//! Storage manager for coordinating database operations

use crate::{repositories::*, Error, Result};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::sync::Arc;
use tracing::{info, warn};

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: Option<u32>,
    pub migrate_on_startup: bool,
}

/// Main storage manager coordinating all repositories
pub struct StorageManager {
    pool: Pool<Sqlite>,
    agents: Arc<AgentRepository>,
    issues: Arc<IssueRepository>,
    messages: Arc<MessageRepository>,
    knowledge: Arc<KnowledgeRepository>,
    prompts: Arc<PromptRepository>,
}

impl StorageManager {
    /// Create a new storage manager
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database: {}", config.url);
        
        let pool = SqlitePool::connect(&config.url).await?;
        
        info!("Database connection established");

        // Create repositories
        let agents = Arc::new(AgentRepository::new(pool.clone()));
        let issues = Arc::new(IssueRepository::new(pool.clone()));
        let messages = Arc::new(MessageRepository::new(pool.clone()));
        let knowledge = Arc::new(KnowledgeRepository::new(pool.clone()));
        let prompts = Arc::new(PromptRepository::new(pool.clone()));

        Ok(Self {
            pool,
            agents,
            issues,
            messages,
            knowledge,
            prompts,
        })
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        info!("Running database migrations");
        
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| Error::Migration(e.to_string()))?;
        
        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get agent repository
    pub fn agents(&self) -> Arc<AgentRepository> {
        self.agents.clone()
    }

    /// Get issue repository
    pub fn issues(&self) -> Arc<IssueRepository> {
        self.issues.clone()
    }

    /// Get message repository
    pub fn messages(&self) -> Arc<MessageRepository> {
        self.messages.clone()
    }

    /// Get knowledge repository
    pub fn knowledge(&self) -> Arc<KnowledgeRepository> {
        self.knowledge.clone()
    }

    /// Get prompt repository
    pub fn prompts(&self) -> Arc<PromptRepository> {
        self.prompts.clone()
    }

    /// Check database health
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;
        Ok(())
    }

    /// Get database statistics
    pub async fn stats(&self) -> Result<DatabaseStats> {
        let agents_count = self.agents.count().await?;
        let issues_count = self.issues.count().await?;
        let messages_count = self.messages.count().await?;
        let knowledge_count = self.knowledge.count().await?;
        let prompts_count = self.prompts.count().await?;

        Ok(DatabaseStats {
            agents_count,
            issues_count,
            messages_count,
            knowledge_count,
            prompts_count,
        })
    }
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub agents_count: i64,
    pub issues_count: i64,
    pub messages_count: i64,
    pub knowledge_count: i64,
    pub prompts_count: i64,
}