pub mod comments;
pub mod events;
pub mod projects;
pub mod schema;
pub mod tickets;
pub mod worker_types;
pub mod workers;

use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use tracing::info;

pub type DbPool = Pool<Sqlite>;

pub async fn create_pool(database_url: &str) -> Result<DbPool> {
    info!("Connecting to database: {}", database_url);

    let pool = SqlitePool::connect(database_url).await?;

    info!("Running database migrations");
    schema::run_migrations(&pool).await?;

    Ok(pool)
}

pub async fn close_pool(pool: DbPool) {
    info!("Closing database connection pool");
    pool.close().await;
}
