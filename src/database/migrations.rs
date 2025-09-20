use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tracing::info;

/// Run database migrations using sqlx::migrate!() macro
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    info!("Running database migrations using sqlx::migrate!()");

    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("Migration failed: {}", e))?;

    info!("Database migrations completed successfully");
    Ok(())
}
