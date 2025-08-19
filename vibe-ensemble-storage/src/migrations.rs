//! Database migration utilities

use crate::{Error, Result};
use sqlx::{Pool, Sqlite};
use tracing::info;

/// Migration utilities and helpers
pub struct Migrations;

impl Migrations {
    /// Run database migrations using sqlx migrate
    pub async fn run(pool: &Pool<Sqlite>) -> Result<()> {
        info!("Running database migrations");

        sqlx::migrate!("./migrations")
            .run(pool)
            .await
            .map_err(|e| Error::Migration(e.to_string()))?;

        info!("Database migrations completed successfully");
        Ok(())
    }

    /// Get the current schema version from sqlx migration records
    pub async fn current_version(pool: &Pool<Sqlite>) -> Result<Option<i64>> {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM _sqlx_migrations ORDER BY version DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;

        Ok(row)
    }

    /// Check if migrations are needed by verifying core tables exist
    pub async fn needs_migration(pool: &Pool<Sqlite>) -> Result<bool> {
        // Check if core tables exist
        let table_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('agents', 'issues', 'messages', 'knowledge', 'system_prompts', 'agent_templates')"
        )
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        // Should have 6 core tables
        Ok(table_exists < 6)
    }

    /// Verify database schema integrity
    pub async fn verify_schema(pool: &Pool<Sqlite>) -> Result<()> {
        // Check that all expected tables exist
        let tables = [
            "agents",
            "issues",
            "messages",
            "knowledge",
            "system_prompts",
            "agent_templates",
        ];

        for table in &tables {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?",
            )
            .bind(table)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

            if exists == 0 {
                return Err(Error::Migration(format!(
                    "Required table '{}' does not exist",
                    table
                )));
            }
        }

        info!("Database schema verification completed successfully");
        Ok(())
    }

    /// Check if we can rollback migrations (useful for development)
    pub async fn can_rollback(pool: &Pool<Sqlite>) -> Result<bool> {
        let migration_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _sqlx_migrations")
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

        Ok(migration_count.unwrap_or(0) > 0)
    }

    /// Initialize an empty database with basic tables
    pub async fn initialize_empty_db(pool: &Pool<Sqlite>) -> Result<()> {
        info!("Initializing empty database");

        // For development, we can run the migration to set up the schema
        Self::run(pool).await?;

        info!("Empty database initialized successfully");
        Ok(())
    }
}

/// Convenience function for running migrations
pub async fn run_migrations(pool: &Pool<Sqlite>) -> Result<()> {
    Migrations::run(pool).await
}
