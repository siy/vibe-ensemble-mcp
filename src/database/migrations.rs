use anyhow::Result;
use sqlx::sqlite::SqlitePool;
use tracing::{debug, info};

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
        // Embed migration files directly in binary to avoid path issues
        let migrations = vec![
            Migration {
                version: 1,
                name: "initial schema".to_string(),
                content: include_str!("../../migrations/001_initial_schema.sql").to_string(),
            },
            Migration {
                version: 2,
                name: "expand event types".to_string(),
                content: include_str!("../../migrations/002_expand_event_types.sql").to_string(),
            },
            Migration {
                version: 3,
                name: "add project rules and patterns".to_string(),
                content: include_str!("../../migrations/003_add_project_rules_patterns.sql")
                    .to_string(),
            },
            Migration {
                version: 4,
                name: "add event resolution".to_string(),
                content: include_str!("../../migrations/004_add_event_resolution.sql").to_string(),
            },
        ];

        Ok(migrations)
    }

    async fn apply_migration(&self, migration: &Migration) -> Result<()> {
        debug!(
            "Executing migration {}: {}",
            migration.version, migration.name
        );

        // Execute the migration SQL
        sqlx::query(&migration.content)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                anyhow::anyhow!("Failed to execute migration {}: {}", migration.version, e)
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
    content: String,
}
