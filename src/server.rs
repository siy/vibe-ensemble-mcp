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
use tracing::{error, info};

use crate::{
    config::Config,
    database::DbPool,
    error::Result,
    mcp::server::mcp_handler,
    sse::{sse_handler, sse_message_handler, EventBroadcaster},
    workers::queue::QueueManager,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub queue_manager: Arc<QueueManager>,
    pub server_info: ServerInfo,
    pub event_broadcaster: EventBroadcaster,
}

#[derive(Clone)]
pub struct ServerInfo {
    pub port: u16,
}

pub async fn run_server(config: Config) -> Result<()> {
    // Initialize database
    let db = crate::database::create_pool(&config.database_url()).await?;

    // Initialize event broadcaster
    let event_broadcaster = EventBroadcaster::new();

    // Initialize queue manager
    let (queue_manager, worker_output_receiver) =
        QueueManager::new(db.clone(), config.clone(), event_broadcaster.clone());
    let queue_manager = Arc::new(queue_manager);

    // Start the WorkerOutput processor now that QueueManager is in an Arc
    let queue_manager_clone = queue_manager.clone();
    let db_clone = db.clone();
    tokio::spawn(async move {
        queue_manager_clone
            .start_worker_output_processor(db_clone, worker_output_receiver)
            .await;
    });

    let state = AppState {
        config: config.clone(),
        db,
        queue_manager,
        server_info: ServerInfo { port: config.port },
        event_broadcaster,
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
            axum::http::header::AUTHORIZATION,
            axum::http::header::HeaderName::from_static("x-api-key"),
            axum::http::header::HeaderName::from_static("last-event-id"),
            axum::http::header::HeaderName::from_static("mcp-protocol-version"),
        ])
        .allow_origin(axum::http::header::HeaderValue::from_static("*"));

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(mcp_handler))
        .route("/sse", get(sse_handler))
        .route("/messages", post(sse_message_handler))
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
    info!("Starting queue-based ticket recovery system...");

    // Step 1: Find all open tickets and group them by project/stage
    let open_tickets = sqlx::query(
        r#"
        SELECT ticket_id, project_id, current_stage
        FROM tickets 
        WHERE state = 'open' AND processing_worker_id IS NULL
        ORDER BY project_id, current_stage, priority DESC, created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    if open_tickets.is_empty() {
        info!("No open tickets found for recovery");
        return Ok(());
    }

    let mut tickets_recovered = 0;

    // Step 2: Submit tickets to their appropriate queues and start consumer threads
    for ticket_row in open_tickets {
        let ticket_id: String = ticket_row.get("ticket_id");
        let project_id: String = ticket_row.get("project_id");
        let current_stage: String = ticket_row.get("current_stage");

        // Submit ticket to queue - creates queue and consumer if needed
        if let Err(e) = state
            .queue_manager
            .submit_task(&project_id, &current_stage, &ticket_id, &state.db)
            .await
        {
            error!("Failed to submit ticket {} to queue: {}", ticket_id, e);
            continue;
        }

        info!(
            "Submitted ticket {} to queue for project={}, stage={}",
            ticket_id, project_id, current_stage
        );
        tickets_recovered += 1;

        // Consumer thread is automatically created by submit_task if needed
    }

    info!(
        "Ticket recovery completed: {} tickets recovered",
        tickets_recovered
    );

    Ok(())
}
