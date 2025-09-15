use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Row};

pub async fn get_database_info(pool: &SqlitePool) -> Result<String> {
    let row = sqlx::query("SELECT sqlite_version() as version")
        .fetch_one(pool)
        .await?;

    let version: String = row.get("version");
    Ok(version)
}
