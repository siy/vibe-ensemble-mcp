use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Row};
use tracing::{debug, info};

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    info!("Running database migrations");

    // Create tables
    create_projects_table(pool).await?;
    create_worker_types_table(pool).await?;
    create_tickets_table(pool).await?;
    create_comments_table(pool).await?;
    create_workers_table(pool).await?;
    create_events_table(pool).await?;

    info!("Database migrations completed successfully");
    Ok(())
}

async fn create_projects_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating projects table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS projects (
            repository_name TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            short_description TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
    "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_worker_types_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating worker_types table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS worker_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            project_id TEXT NOT NULL,
            worker_type TEXT NOT NULL,
            short_description TEXT,
            system_prompt TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE,
            UNIQUE(project_id, worker_type)
        )
    "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_tickets_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating tickets table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS tickets (
            ticket_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            execution_plan TEXT NOT NULL,
            current_stage TEXT NOT NULL DEFAULT 'planning',
            state TEXT NOT NULL DEFAULT 'open' CHECK (state IN ('open', 'closed', 'on_hold')),
            priority TEXT NOT NULL DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            closed_at TEXT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
        )
    "#,
    )
    .execute(pool)
    .await?;

    // Migration: Add new columns to existing tickets table if they don't exist
    // This handles the case where the table already exists but with the old schema
    let _ = sqlx::query("ALTER TABLE tickets ADD COLUMN current_stage TEXT DEFAULT 'planning'")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE tickets ADD COLUMN state TEXT DEFAULT 'open' CHECK (state IN ('open', 'closed', 'on_hold'))")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE tickets ADD COLUMN priority TEXT DEFAULT 'medium' CHECK (priority IN ('low', 'medium', 'high', 'urgent'))")
        .execute(pool)
        .await;

    // Migration: Copy data from last_completed_stage to current_stage if needed
    let _ = sqlx::query("UPDATE tickets SET current_stage = last_completed_stage WHERE current_stage = 'planning' AND last_completed_stage != 'Planned'")
        .execute(pool)
        .await;

    // Migration: Add processing_worker_id column for ticket claiming functionality
    let _ = sqlx::query("ALTER TABLE tickets ADD COLUMN processing_worker_id TEXT")
        .execute(pool)
        .await;

    Ok(())
}

async fn create_comments_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating comments table");
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ticket_id TEXT NOT NULL,
            worker_type TEXT,
            worker_id TEXT,
            stage_number INTEGER,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (ticket_id) REFERENCES tickets(ticket_id) ON DELETE CASCADE
        )
    "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_workers_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating workers table");
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS workers (
            worker_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            worker_type TEXT NOT NULL,
            status TEXT NOT NULL CHECK (status IN ('spawning', 'active', 'idle', 'finished', 'failed')),
            pid INTEGER,
            queue_name TEXT NOT NULL,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            last_activity TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
        )
    "#)
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_events_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating events table");
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL CHECK (event_type IN ('ticket_stage_completed', 'worker_stopped', 'task_assigned')),
            ticket_id TEXT,
            worker_id TEXT,
            stage TEXT,
            reason TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            processed BOOLEAN NOT NULL DEFAULT 0
        )
    "#)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_database_info(pool: &SqlitePool) -> Result<String> {
    let row = sqlx::query("SELECT sqlite_version() as version")
        .fetch_one(pool)
        .await?;

    let version: String = row.get("version");
    Ok(version)
}
