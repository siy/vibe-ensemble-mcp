# Stage 2: Database Layer

**Duration**: 2-3 hours  
**Goal**: Complete SQLite schema and operations

## Overview

This stage implements the complete SQLite database layer with schema creation, migrations, and CRUD operations for all entities. The database will support projects, worker types, tickets, comments, workers, and events with proper relationships and constraints.

## Objectives

1. Design and implement complete database schema
2. Create database connection and migration system
3. Implement CRUD operations for all entities
4. Add connection pooling and transaction support
5. Create database initialization and seeding functions
6. Integrate database into the HTTP server state

## Database Schema

### Entity Relationship Diagram
```
Projects (1) -----> (*) WorkerTypes
    |                      |
    |                      |
    v                      v
Tickets (1) -----> (*) Comments
    |
    |
    v
Workers (*) -----> (1) Events
```

### Schema Implementation

#### 1. Projects Table
```sql
CREATE TABLE IF NOT EXISTS projects (
    repository_name TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    short_description TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

#### 2. Worker Types Table
```sql
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
);
```

#### 3. Tickets Table
```sql
CREATE TABLE IF NOT EXISTS tickets (
    ticket_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    execution_plan TEXT NOT NULL, -- JSON array of worker types
    last_completed_stage TEXT NOT NULL DEFAULT 'Planned',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    closed_at TEXT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
);
```

#### 4. Comments Table
```sql
CREATE TABLE IF NOT EXISTS comments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ticket_id TEXT NOT NULL,
    worker_type TEXT,
    worker_id TEXT,
    stage_number INTEGER,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (ticket_id) REFERENCES tickets(ticket_id) ON DELETE CASCADE
);
```

#### 5. Workers Table
```sql
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
);
```

#### 6. Events Table
```sql
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('ticket_stage_completed', 'worker_stopped')),
    ticket_id TEXT,
    worker_id TEXT,
    stage TEXT,
    reason TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    processed BOOLEAN NOT NULL DEFAULT 0
);
```

## Implementation

### 1. Database Module Structure

Create `src/database/mod.rs`:
```rust
pub mod schema;
pub mod projects;
pub mod worker_types;
pub mod tickets;
pub mod comments;
pub mod workers;
pub mod events;

use anyhow::Result;
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};
use tracing::{info, error};

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
```

### 2. Schema and Migrations (`src/database/schema.rs`)

```rust
use sqlx::{sqlite::SqlitePool, Row};
use anyhow::Result;
use tracing::{info, debug};

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
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS projects (
            repository_name TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            short_description TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
    "#)
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_worker_types_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating worker_types table");
    sqlx::query(r#"
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
    "#)
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_tickets_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating tickets table");
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS tickets (
            ticket_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            title TEXT NOT NULL,
            execution_plan TEXT NOT NULL,
            last_completed_stage TEXT NOT NULL DEFAULT 'Planned',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            closed_at TEXT NULL,
            FOREIGN KEY (project_id) REFERENCES projects(repository_name) ON DELETE CASCADE
        )
    "#)
    .execute(pool)
    .await?;
    Ok(())
}

async fn create_comments_table(pool: &SqlitePool) -> Result<()> {
    debug!("Creating comments table");
    sqlx::query(r#"
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
    "#)
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
            event_type TEXT NOT NULL CHECK (event_type IN ('ticket_stage_completed', 'worker_stopped')),
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
```

### 3. Entity Models and Operations

#### Projects (`src/database/projects.rs`)

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use anyhow::Result;

use super::DbPool;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub repository_name: String,
    pub path: String,
    pub short_description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub repository_name: String,
    pub path: String,
    pub short_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub path: Option<String>,
    pub short_description: Option<String>,
}

impl Project {
    pub async fn create(pool: &DbPool, req: CreateProjectRequest) -> Result<Project> {
        let project = sqlx::query_as::<_, Project>(r#"
            INSERT INTO projects (repository_name, path, short_description)
            VALUES (?1, ?2, ?3)
            RETURNING repository_name, path, short_description, created_at, updated_at
        "#)
        .bind(&req.repository_name)
        .bind(&req.path)
        .bind(&req.short_description)
        .fetch_one(pool)
        .await?;

        Ok(project)
    }

    pub async fn get_by_name(pool: &DbPool, repository_name: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>(r#"
            SELECT repository_name, path, short_description, created_at, updated_at
            FROM projects
            WHERE repository_name = ?1
        "#)
        .bind(repository_name)
        .fetch_optional(pool)
        .await?;

        Ok(project)
    }

    pub async fn list_all(pool: &DbPool) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>(r#"
            SELECT repository_name, path, short_description, created_at, updated_at
            FROM projects
            ORDER BY created_at DESC
        "#)
        .fetch_all(pool)
        .await?;

        Ok(projects)
    }

    pub async fn update(
        pool: &DbPool,
        repository_name: &str,
        req: UpdateProjectRequest,
    ) -> Result<Option<Project>> {
        // Build update query dynamically
        let mut set_clauses = Vec::new();
        let mut params: Vec<&(dyn sqlx::Encode<sqlx::Sqlite> + sqlx::types::Type<sqlx::Sqlite> + std::marker::Sync)> = Vec::new();
        
        if req.path.is_some() {
            set_clauses.push("path = ?");
        }
        if req.short_description.is_some() {
            set_clauses.push("short_description = ?");
        }
        
        if set_clauses.is_empty() {
            return Self::get_by_name(pool, repository_name).await;
        }

        set_clauses.push("updated_at = datetime('now')");
        
        let query = format!(
            "UPDATE projects SET {} WHERE repository_name = ? RETURNING repository_name, path, short_description, created_at, updated_at",
            set_clauses.join(", ")
        );

        let mut query_builder = sqlx::query_as::<_, Project>(&query);
        
        if let Some(path) = &req.path {
            query_builder = query_builder.bind(path);
        }
        if let Some(desc) = &req.short_description {
            query_builder = query_builder.bind(desc);
        }
        query_builder = query_builder.bind(repository_name);

        let project = query_builder.fetch_optional(pool).await?;
        Ok(project)
    }

    pub async fn delete(pool: &DbPool, repository_name: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM projects WHERE repository_name = ?1")
            .bind(repository_name)
            .execute(pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }
}
```

### 4. Integration with Server State

Update `src/server.rs`:
```rust
use crate::{config::Config, error::Result, database::DbPool};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
}

pub async fn run_server(config: Config) -> Result<()> {
    // Initialize database
    let db = crate::database::create_pool(&config.database_url()).await?;
    
    let state = AppState {
        config: config.clone(),
        db,
    };

    // ... rest of server setup
}
```

## Testing

### 1. Database Connection Test
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_database_connection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite:{}", db_path.display());
        
        let pool = create_pool(&db_url).await.unwrap();
        close_pool(pool).await;
    }

    #[tokio::test]
    async fn test_project_crud() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let db_url = format!("sqlite:{}", db_path.display());
        
        let pool = create_pool(&db_url).await.unwrap();
        
        // Create project
        let create_req = CreateProjectRequest {
            repository_name: "test/project".to_string(),
            path: "/tmp/test".to_string(),
            short_description: Some("Test project".to_string()),
        };
        
        let project = Project::create(&pool, create_req).await.unwrap();
        assert_eq!(project.repository_name, "test/project");
        
        // Get project
        let found = Project::get_by_name(&pool, "test/project").await.unwrap();
        assert!(found.is_some());
        
        // List projects
        let projects = Project::list_all(&pool).await.unwrap();
        assert_eq!(projects.len(), 1);
        
        // Delete project
        let deleted = Project::delete(&pool, "test/project").await.unwrap();
        assert!(deleted);
        
        close_pool(pool).await;
    }
}
```

### 2. Manual Testing

```bash
# Start server
cargo run

# Check health with database info
curl http://localhost:3000/health
```

## Validation Checklist

- [ ] Database file is created on first run
- [ ] All tables are created with correct schema
- [ ] Foreign key constraints work properly
- [ ] CRUD operations for all entities work
- [ ] Transactions are properly handled
- [ ] Connection pooling is functioning
- [ ] Server starts with database integration
- [ ] Tests pass for all database operations

## Files Created

- `src/database/mod.rs` - Database module root
- `src/database/schema.rs` - Schema creation and migrations
- `src/database/projects.rs` - Project CRUD operations
- `src/database/worker_types.rs` - Worker type operations
- `src/database/tickets.rs` - Ticket operations
- `src/database/comments.rs` - Comment operations
- `src/database/workers.rs` - Worker status operations
- `src/database/events.rs` - Event operations

## Performance Considerations

1. **Connection Pooling**: SQLite with connection pool for concurrent access
2. **Indexes**: Add indexes for frequently queried columns
3. **Transactions**: Use transactions for multi-table operations
4. **Prepared Statements**: All queries use prepared statements via sqlx

## Next Steps

After completing Stage 2:
1. Verify all database tests pass
2. Test database integration with server
3. Update progress in [TODO.md](../TODO.md)
4. Proceed to [Stage 3: MCP Protocol Implementation](STAGE_3_MCP_PROTOCOL.md)