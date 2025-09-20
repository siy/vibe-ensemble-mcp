pub mod comments;
pub mod dag;
pub mod events;
pub mod migrations;
pub mod projects;
pub mod schema;
pub mod tickets;
pub mod worker_types;
pub mod workers;

use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use std::{fs, path::Path};
use tracing::info;

pub type DbPool = Pool<Sqlite>;

/// Ensures the vibe-ensemble-mcp directory structure exists
pub fn ensure_directory_structure(database_path: &str) -> Result<()> {
    // Handle SQLite URL format (remove "sqlite:" prefix if present)
    let clean_path = database_path
        .strip_prefix("sqlite:")
        .unwrap_or(database_path);
    let db_path = Path::new(clean_path);

    // Create the parent directory for the database (.vibe-ensemble-mcp/)
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
        info!("Created directory: {}", parent.display());

        // Create the logs directory structure
        let logs_dir = parent.join("logs");
        fs::create_dir_all(&logs_dir)?;
        info!("Created logs directory: {}", logs_dir.display());
    }

    Ok(())
}

/// Gets the centralized logs directory path for a specific project
pub fn get_project_logs_dir(database_path: &str, project_name: &str) -> Result<String> {
    // Handle SQLite URL format (remove "sqlite:" prefix if present)
    let clean_path = database_path
        .strip_prefix("sqlite:")
        .unwrap_or(database_path);
    let db_path = Path::new(clean_path);

    if let Some(parent) = db_path.parent() {
        let project_logs_dir = parent.join("logs").join(project_name);
        fs::create_dir_all(&project_logs_dir)?;
        Ok(project_logs_dir.to_string_lossy().to_string())
    } else {
        // Fallback to current directory if no parent
        let project_logs_dir = Path::new("logs").join(project_name);
        fs::create_dir_all(&project_logs_dir)?;
        Ok(project_logs_dir.to_string_lossy().to_string())
    }
}

pub async fn create_pool(database_url: &str) -> Result<DbPool> {
    info!("Connecting to SQLite database");

    // Ensure directory structure exists
    ensure_directory_structure(database_url)?;

    let pool = SqlitePool::connect(database_url).await?;

    info!("Running database migrations");
    migrations::run_migrations(&pool).await?;

    Ok(pool)
}

pub async fn close_pool(pool: DbPool) {
    info!("Closing database connection pool");
    pool.close().await;
}
