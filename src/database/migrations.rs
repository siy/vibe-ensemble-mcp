use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use std::fs;
use tracing::{debug, info, warn};

pub struct MigrationRunner {
    pool: SqlitePool,
}

impl MigrationRunner {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Run all pending migrations from the migrations/ directory
    pub async fn run_migrations(&self) -> Result<()> {
        info!("Running database migrations");

        // Ensure migration tracking table exists
        self.ensure_migration_table().await?;

        // Get applied migrations
        let applied_versions = self.get_applied_migrations().await?;
        debug!("Applied migrations: {:?}", applied_versions);

        // Discover migration files
        let mut migration_files = self.discover_migrations()?;
        migration_files.sort_by(|a, b| a.version.cmp(&b.version));

        // Apply pending migrations
        let mut applied_count = 0;
        for migration in migration_files {
            if !applied_versions.contains(&migration.version) {
                info!(
                    "Applying migration {}: {}",
                    migration.version, migration.name
                );
                self.apply_migration(&migration).await?;
                applied_count += 1;
            }
        }

        if applied_count == 0 {
            info!("No pending migrations found");
        } else {
            info!("Applied {} migrations successfully", applied_count);
        }

        Ok(())
    }

    async fn ensure_migration_table(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_applied_migrations(&self) -> Result<Vec<i64>> {
        let rows =
            sqlx::query_as::<_, (i64,)>("SELECT version FROM schema_migrations ORDER BY version")
                .fetch_all(&self.pool)
                .await?;

        Ok(rows.into_iter().map(|(version,)| version).collect())
    }

    fn discover_migrations(&self) -> Result<Vec<Migration>> {
        let migrations_dir = "migrations";
        let mut migrations = Vec::new();

        if !std::path::Path::new(migrations_dir).exists() {
            warn!("Migrations directory '{}' does not exist", migrations_dir);
            return Ok(migrations);
        }

        let entries = fs::read_dir(migrations_dir)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".sql") {
                    if let Some(migration) = self.parse_migration_filename(filename)? {
                        migration.validate_file_exists(&path)?;
                        migrations.push(migration);
                    }
                }
            }
        }

        Ok(migrations)
    }

    fn parse_migration_filename(&self, filename: &str) -> Result<Option<Migration>> {
        // Expected format: 001_initial_schema.sql
        let stem = filename.strip_suffix(".sql").unwrap_or(filename);
        let parts: Vec<&str> = stem.splitn(2, '_').collect();

        if parts.len() != 2 {
            debug!("Skipping invalid migration filename: {}", filename);
            return Ok(None);
        }

        let version = parts[0]
            .parse::<i64>()
            .map_err(|_| anyhow::anyhow!("Invalid migration version in filename: {}", filename))?;

        let name = parts[1].replace('_', " ");

        Ok(Some(Migration {
            version,
            name,
            filename: filename.to_string(),
        }))
    }

    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        let migration_path = format!("migrations/{}", migration.filename);
        let sql_content = fs::read_to_string(&migration_path)?;

        debug!("Executing migration SQL: {}", migration_path);

        // Execute the migration SQL
        sqlx::query(&sql_content)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute migration {}: {}", migration_path, e)
            })?;

        // Record successful application (unless already recorded by the migration itself)
        sqlx::query("INSERT OR IGNORE INTO schema_migrations (version) VALUES (?)")
            .bind(migration.version)
            .execute(&self.pool)
            .await?;

        info!(
            "Successfully applied migration {}: {}",
            migration.version, migration.name
        );
        Ok(())
    }
}

#[derive(Debug)]
struct Migration {
    version: i64,
    name: String,
    filename: String,
}

impl Migration {
    fn validate_file_exists(&self, path: &std::path::Path) -> Result<()> {
        if !path.exists() {
            return Err(anyhow::anyhow!("Migration file does not exist: {:?}", path));
        }
        Ok(())
    }
}
