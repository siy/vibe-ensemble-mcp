use axum::{
    extract::State,
    http::Method,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing::{error, info};

use crate::{
    config::Config, 
    database::{DbPool, workers::Worker}, 
    error::Result, 
    mcp::server::mcp_handler, 
    sse::sse_handler,
    workers::{queue::QueueManager, process::ProcessManager, types::SpawnWorkerRequest},
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
    info!("Checking for workers to respawn...");
    
    // Get all workers that should be active but might have died
    let workers = Worker::list_by_project(&state.db, None).await?;
    let mut workers_respawned = 0;
    
    for worker in workers {
        // Only check workers that were marked as active but might have died
        if worker.status == "active" || worker.status == "spawning" {
            if let Some(pid) = worker.pid {
                // Check if the process is still running
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);
                
                if !is_running {
                    info!(
                        "Worker '{}' (PID: {}) for queue '{}' was marked as active but process died. Attempting respawn...", 
                        worker.worker_id, pid, worker.queue_name
                    );
                    
                    // Update the dead worker's status in database
                    Worker::update_status(&state.db, &worker.worker_id, "failed", None).await?;
                    
                    // Create event for dead worker
                    crate::database::events::Event::create_worker_stopped(
                        &state.db,
                        &worker.worker_id,
                        "process died, respawning on startup",
                    ).await?;
                    
                    // Generate a new worker ID for the replacement
                    let new_worker_id = format!("worker_{}_{}_{}", 
                        worker.worker_type, 
                        chrono::Utc::now().timestamp(), 
                        workers_respawned + 1
                    );
                    
                    let spawn_request = SpawnWorkerRequest {
                        worker_id: new_worker_id.clone(),
                        project_id: worker.project_id.clone(),
                        worker_type: worker.worker_type.clone(),
                        queue_name: worker.queue_name.clone(),
                    };
                    
                    // Create the queue if it doesn't exist
                    if let Err(e) = state.queue_manager.create_queue(&worker.queue_name).await {
                        error!("Failed to create queue '{}': {}", worker.queue_name, e);
                        continue;
                    }
                    
                    match ProcessManager::spawn_worker(state, spawn_request).await {
                        Ok(_) => {
                            info!("Successfully respawned worker '{}' to replace '{}' for queue '{}'", 
                                new_worker_id, worker.worker_id, worker.queue_name);
                            workers_respawned += 1;
                        }
                        Err(e) => {
                            error!("Failed to respawn worker for queue '{}': {}", worker.queue_name, e);
                        }
                    }
                }
            } else {
                // Worker has no PID, probably failed to start
                info!(
                    "Worker '{}' for queue '{}' has no PID, marking as failed", 
                    worker.worker_id, worker.queue_name
                );
                Worker::update_status(&state.db, &worker.worker_id, "failed", None).await?;
            }
        }
    }
    
    if workers_respawned > 0 {
        info!("Respawned {} workers that had died", workers_respawned);
    } else {
        info!("No dead workers found to respawn");
    }
    
    Ok(())
}
