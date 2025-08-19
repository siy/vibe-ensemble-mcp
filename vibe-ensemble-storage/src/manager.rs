//! Storage manager for coordinating database operations

use crate::{migrations::Migrations, performance::*, repositories::*, services::*, Error, Result};
use sqlx::{Pool, Sqlite, SqlitePool};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: Option<u32>,
    pub migrate_on_startup: bool,
    pub performance_config: Option<PerformanceConfig>,
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
    performance_layer: Arc<PerformanceLayer>,
}

impl StorageManager {
    /// Create a new storage manager
    pub async fn new(config: &DatabaseConfig) -> Result<Self> {
        info!("Connecting to database: {}", config.url);

        // Create performance-optimized connection pool
        let mut connect_options = config
            .url
            .parse::<sqlx::sqlite::SqliteConnectOptions>()
            .map_err(|e| Error::Internal(anyhow::anyhow!("Invalid database URL: {}", e)))?;

        // Optimize connection settings
        connect_options = connect_options
            .busy_timeout(Duration::from_secs(30))
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .foreign_keys(true)
            .pragma("cache_size", "-64000") // 64MB cache
            .pragma("temp_store", "memory")
            .pragma("mmap_size", "268435456") // 256MB mmap
            .pragma("optimize", "1");

        let pool_builder = SqlitePool::connect_with(connect_options);
        let pool = if let Some(max_connections) = config.max_connections {
            info!("Using connection pool with {} connections", max_connections);
            SqlitePool::connect_with(connect_options.clone().create_if_missing(true)).await?
        } else {
            pool_builder.await?
        };

        info!("Database connection established with performance optimizations");

        // Initialize performance layer
        let performance_config = config.performance_config.clone().unwrap_or_default();
        let performance_layer = Arc::new(PerformanceLayer::new(performance_config));

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
            performance_layer,
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

    /// Get performance layer
    pub fn performance_layer(&self) -> Arc<PerformanceLayer> {
        self.performance_layer.clone()
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

    /// Get comprehensive performance report
    pub async fn performance_report(&self) -> Result<PerformanceReport> {
        Ok(self.performance_layer.performance_report().await)
    }

    /// Clear all caches
    pub async fn clear_cache(&self) -> Result<()> {
        self.performance_layer.cache_manager.clear().await;
        Ok(())
    }

    /// Optimize database (run VACUUM and ANALYZE)
    pub async fn optimize_database(&self) -> Result<()> {
        info!("Optimizing database...");

        // Run VACUUM to reclaim space and defragment
        sqlx::query("VACUUM")
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        // Run ANALYZE to update query planner statistics
        sqlx::query("ANALYZE")
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        info!("Database optimization complete");
        Ok(())
    }

    /// Execute operation with performance tracking
    pub async fn execute_with_tracking<F, R>(&self, operation_name: &str, operation: F) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        self.performance_layer
            .execute_with_tracking(operation_name, operation)
            .await
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
