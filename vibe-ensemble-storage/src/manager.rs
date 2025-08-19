//! Storage manager for coordinating database operations

use crate::{migrations::Migrations, repositories::*, services::*, Error, Result};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::sync::Arc;
use tracing::info;

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
    templates: Arc<TemplateRepository>,
    agent_service: Arc<AgentService>,
    issue_service: Arc<IssueService>,
    message_service: Arc<MessageService>,
    knowledge_service: Arc<KnowledgeService>,
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
        let templates = Arc::new(TemplateRepository::new(pool.clone()));

        // Create services
        let agent_service = Arc::new(AgentService::new(agents.clone()));
        let issue_service = Arc::new(IssueService::new(issues.clone()));
        let message_service = Arc::new(MessageService::new(messages.clone()));
        let knowledge_service = Arc::new(KnowledgeService::new((*knowledge).clone()));

        let manager = Self {
            pool,
            agents,
            issues,
            messages,
            knowledge,
            prompts,
            templates,
            agent_service,
            issue_service,
            message_service,
            knowledge_service,
        };

        // Run migrations if configured to do so
        if config.migrate_on_startup {
            manager.migrate().await?;
        }

        Ok(manager)
    }

    /// Run database migrations
    pub async fn migrate(&self) -> Result<()> {
        Migrations::run(&self.pool).await
    }

    /// Check if migrations are needed
    pub async fn needs_migration(&self) -> Result<bool> {
        Migrations::needs_migration(&self.pool).await
    }

    /// Verify database schema integrity
    pub async fn verify_schema(&self) -> Result<()> {
        Migrations::verify_schema(&self.pool).await
    }

    /// Initialize empty database (useful for testing)
    pub async fn initialize_empty_db(&self) -> Result<()> {
        Migrations::initialize_empty_db(&self.pool).await
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

    /// Get template repository
    pub fn templates(&self) -> Arc<TemplateRepository> {
        self.templates.clone()
    }

    /// Get agent service
    pub fn agent_service(&self) -> Arc<AgentService> {
        self.agent_service.clone()
    }

    /// Get issue service
    pub fn issue_service(&self) -> Arc<IssueService> {
        self.issue_service.clone()
    }

    /// Get message service
    pub fn message_service(&self) -> Arc<MessageService> {
        self.message_service.clone()
    }

    /// Get knowledge service
    pub fn knowledge_service(&self) -> Arc<KnowledgeService> {
        self.knowledge_service.clone()
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
        let templates_count = self.templates.count().await?;

        Ok(DatabaseStats {
            agents_count,
            issues_count,
            messages_count,
            knowledge_count,
            prompts_count,
            templates_count,
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
    pub templates_count: i64,
}
