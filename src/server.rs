use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing::{error, info, warn};

use crate::{
    config::Config,
    database::{workers::Worker, DbPool},
    error::Result,
    mcp::server::mcp_handler,
    sse::sse_handler,
    workers::{process::ProcessManager, queue::QueueManager, types::SpawnWorkerRequest},
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub queue_manager: Arc<QueueManager>,
}

pub async fn run_server(config: Config) -> Result<()> {
    // Initialize database
    let db = crate::database::create_pool(&config.database_url()).await?;

    // Initialize queue manager
    let queue_manager = Arc::new(QueueManager::new());

    let state = AppState {
        config: config.clone(),
        db,
        queue_manager,
    };

    // Respawn workers for unfinished tasks if enabled
    if !config.no_respawn {
        respawn_workers_for_unfinished_tasks(&state).await?;
    }

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::ACCEPT,
            axum::http::header::CACHE_CONTROL,
        ])
        .allow_origin(axum::http::header::HeaderValue::from_static("*"));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(mcp_handler))
        .route("/sse", get(sse_handler))
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1 MiB
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let address = config.server_address();
    info!("Server listening on {}", address);

    let listener = tokio::net::TcpListener::bind(&address).await?;

    match axum::serve(listener, app).await {
        Ok(_) => info!("Server stopped gracefully"),
        Err(e) => error!("Server error: {}", e),
    }

    Ok(())
}

async fn health_check(State(state): State<AppState>) -> Result<Json<Value>> {
    // Test database connection
    let db_version = match crate::database::schema::get_database_info(&state.db).await {
        Ok(version) => version,
        Err(e) => {
            error!("Database health check failed: {}", e);
            return Ok(Json(json!({
                "status": "unhealthy",
                "service": "vibe-ensemble-mcp",
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "error": "Database connection failed"
            })));
        }
    };

    Ok(Json(json!({
        "status": "healthy",
        "service": "vibe-ensemble-mcp",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "database": {
            "version": db_version,
            "status": "connected"
        }
    })))
}

async fn respawn_workers_for_unfinished_tasks(state: &AppState) -> Result<()> {
    info!("Checking for stage-based workers to respawn...");

    // First, clean up any dead workers
    let workers = Worker::list_by_project(&state.db, None).await?;
    let mut dead_workers_cleaned = 0;

    for worker in workers {
        // Only check workers that were marked as active but might have died
        if worker.status == "active" || worker.status == "spawning" {
            if let Some(pid) = worker.pid {
                // Check if process is still running using kill -0
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);

                if !is_running {
                    info!(
                        "Worker '{}' (PID: {}) for stage '{}' was marked as active but process died. Cleaning up...", 
                        worker.worker_id, pid, worker.worker_type
                    );

                    // Update the dead worker's status in database
                    Worker::update_status(&state.db, &worker.worker_id, "failed", None).await?;

                    // Create event for dead worker
                    crate::database::events::Event::create_worker_stopped(
                        &state.db,
                        &worker.worker_id,
                        "process died, cleaned up on startup",
                    )
                    .await?;

                    dead_workers_cleaned += 1;
                }
            } else {
                // Worker has no PID, probably failed to start
                warn!(
                    "Worker '{}' has no PID, marking as failed",
                    worker.worker_id
                );
                Worker::update_status(&state.db, &worker.worker_id, "failed", None).await?;
                dead_workers_cleaned += 1;
            }
        }
    }

    if dead_workers_cleaned > 0 {
        info!("Cleaned up {} dead workers", dead_workers_cleaned);
    }

    // Now check for stages that need workers
    // Get all unique stages that have open tickets
    let stages_with_tickets = sqlx::query(
        r#"
        SELECT DISTINCT current_stage, project_id
        FROM tickets 
        WHERE state = 'open'
        ORDER BY project_id, current_stage
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let mut workers_spawned = 0;

    for stage_row in stages_with_tickets {
        let stage: String = stage_row.get("current_stage");
        let project_id: String = stage_row.get("project_id");

        // Check if there's an active worker for this stage/project
        let active_workers = sqlx::query(
            r#"
            SELECT worker_id, pid, status
            FROM workers 
            WHERE project_id = ?1 AND worker_type = ?2 AND status IN ('spawning', 'active', 'idle')
            "#,
        )
        .bind(&project_id)
        .bind(&stage)
        .fetch_all(&state.db)
        .await?;

        let has_active_worker = active_workers.iter().any(|w| {
            let pid_val: Option<i64> = w.try_get("pid").ok();
            let status: String = w.get("status");

            if let Some(pid) = pid_val {
                // Double-check the process is actually running
                std::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .map(|status| status.success())
                    .unwrap_or(false)
            } else {
                status == "spawning" // Allow spawning workers without PID yet
            }
        });

        if !has_active_worker {
            info!(
                "Stage '{}' in project '{}' has open tickets but no active worker. Spawning...",
                stage, project_id
            );

            // Generate a unique worker ID
            let worker_id = format!("{}-{}", stage, chrono::Utc::now().timestamp());

            let spawn_request = SpawnWorkerRequest {
                worker_id: worker_id.clone(),
                project_id: project_id.clone(),
                worker_type: stage.clone(),
                queue_name: format!("{}-queue", stage), // Keep queue for internal implementation
            };

            match ProcessManager::spawn_worker(state, spawn_request).await {
                Ok(_) => {
                    info!(
                        "Successfully spawned worker '{}' for stage '{}' in project '{}'",
                        worker_id, stage, project_id
                    );
                    workers_spawned += 1;
                }
                Err(e) => {
                    error!(
                        "Failed to spawn worker for stage '{}' in project '{}': {}",
                        stage, project_id, e
                    );
                }
            }
        }
    }

    if workers_spawned > 0 {
        info!(
            "Spawned {} new workers for stages with open tickets",
            workers_spawned
        );
    } else {
        info!("No new workers needed - all stages with open tickets have active workers");
    }

    Ok(())
}
