use axum::extract::WebSocketUpgrade;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, Method},
    response::{Json, Response},
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
    database::DbPool,
    error::Result,
    mcp::{
        server::{mcp_handler, McpServer},
        websocket::{WebSocketManager, WebSocketQuery},
    },
    sse::{sse_handler, sse_message_handler, EventBroadcaster},
    workers::queue::QueueManager,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub queue_manager: Arc<QueueManager>,
    pub event_broadcaster: EventBroadcaster,
    pub mcp_server: Arc<McpServer>,
    pub websocket_manager: Arc<WebSocketManager>,
}

impl AppState {
    /// Get an event emitter instance for centralized event emission
    pub fn event_emitter(&self) -> crate::events::emitter::EventEmitter<'_> {
        crate::events::emitter::EventEmitter::new(&self.db, &self.event_broadcaster)
    }
}

pub async fn run_server(config: Config) -> Result<()> {
    // Initialize database
    let db = crate::database::create_pool(&config.database_url()).await?;

    // Initialize event broadcaster
    let event_broadcaster = EventBroadcaster::new();

    // Initialize queue manager (spawns completion event processor internally)
    let queue_manager = QueueManager::new(db.clone(), config.clone(), event_broadcaster.clone());

    // Initialize single MCP server instance with config-based tool registration
    let mcp_server = Arc::new(McpServer::new(&config));

    // Initialize WebSocket manager (conditionally based on config)
    let websocket_manager = if config.enable_websocket {
        Arc::new(WebSocketManager::with_concurrency_limit(
            config.max_concurrent_client_requests,
        ))
    } else {
        Arc::new(WebSocketManager::disabled())
    };

    let state = AppState {
        config: config.clone(),
        db,
        queue_manager,
        event_broadcaster,
        mcp_server,
        websocket_manager,
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
            axum::http::header::HeaderName::from_static("x-claude-code-ide-authorization"),
            axum::http::header::HeaderName::from_static("last-event-id"),
            axum::http::header::HeaderName::from_static("mcp-protocol-version"),
        ])
        .allow_origin(axum::http::header::HeaderValue::from_static("*"));

    let mut app = Router::new()
        .route("/health", get(health_check))
        .route("/mcp", post(mcp_handler))
        .route("/sse", get(sse_handler))
        .route("/messages", post(sse_message_handler));

    // Conditionally add WebSocket route if enabled
    if config.enable_websocket {
        app = app.route("/ws", get(websocket_handler));
        info!("WebSocket support enabled at /ws");
    } else {
        info!("WebSocket support disabled");
    }

    let app = app
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
    info!("Starting enhanced ticket recovery system...");

    // Step 1: Find all unprocessed tickets including claimed ones that may be stalled
    let unprocessed_tickets = sqlx::query(
        r#"
        SELECT ticket_id, project_id, current_stage, state, processing_worker_id,
               datetime('now') AS current_time, updated_at,
               (julianday('now') - julianday(updated_at)) * 24 * 60 AS minutes_since_update
        FROM tickets
        WHERE dependency_status = 'ready'
          AND (
            -- Case 1: Open tickets not being processed
            (state = 'open' AND processing_worker_id IS NULL)
            OR
            -- Case 2: Open tickets claimed but stalled (no update for >5 minutes)
            (state = 'open' AND processing_worker_id IS NOT NULL
             AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 5)
            OR
            -- Case 3: On-hold tickets that may be recoverable
            (state = 'on_hold')
          )
        ORDER BY project_id, current_stage, priority DESC, created_at ASC
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    if unprocessed_tickets.is_empty() {
        info!("No unprocessed tickets found for recovery");
        return Ok(());
    }

    let mut tickets_recovered = 0;
    let mut claimed_tickets_released = 0;
    let mut on_hold_tickets_recovered = 0;

    // Step 2: Process each ticket based on its current state
    for ticket_row in unprocessed_tickets {
        let ticket_id: String = ticket_row.get("ticket_id");
        let project_id: String = ticket_row.get("project_id");
        let current_stage: String = ticket_row.get("current_stage");
        let state_str: String = ticket_row.get("state");
        let processing_worker_id: Option<String> = ticket_row.get("processing_worker_id");
        let minutes_since_update: f64 = ticket_row.get("minutes_since_update");

        // Handle different recovery scenarios
        if state_str == "open" && processing_worker_id.is_some() {
            // Stalled claimed ticket - release claim first
            warn!(
                "Releasing stalled claim for ticket {} (worker: {}, stalled for {:.1} minutes)",
                ticket_id,
                processing_worker_id.unwrap(),
                minutes_since_update
            );

            // Release the claim
            let release_result = sqlx::query(
                r#"
                UPDATE tickets 
                SET processing_worker_id = NULL, updated_at = datetime('now')
                WHERE ticket_id = ?1 AND processing_worker_id IS NOT NULL
                "#,
            )
            .bind(&ticket_id)
            .execute(&state.db)
            .await?;

            if release_result.rows_affected() > 0 {
                claimed_tickets_released += 1;
                info!("Released stalled claim for ticket {}", ticket_id);
            }
        } else if state_str == "on_hold" {
            // On-hold ticket - attempt to bring back to open state
            info!(
                "Recovering on-hold ticket {} (on hold for {:.1} minutes)",
                ticket_id, minutes_since_update
            );

            // Move from on_hold back to open
            let recover_result = sqlx::query(
                r#"
                UPDATE tickets 
                SET state = 'open', processing_worker_id = NULL, updated_at = datetime('now')
                WHERE ticket_id = ?1 AND state = 'on_hold'
                "#,
            )
            .bind(&ticket_id)
            .execute(&state.db)
            .await?;

            if recover_result.rows_affected() > 0 {
                on_hold_tickets_recovered += 1;
                info!("Recovered on-hold ticket {} back to open state", ticket_id);
            }
        }

        // Step 3: Submit all tickets (now unclaimed and open) to queues
        if let Err(e) = state
            .queue_manager
            .submit_task(&project_id, &current_stage, &ticket_id)
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
    }

    info!(
        "Enhanced ticket recovery completed: {} tickets recovered, {} stalled claims released, {} on-hold tickets recovered",
        tickets_recovered, claimed_tickets_released, on_hold_tickets_recovered
    );

    Ok(())
}

/// WebSocket handler for bidirectional MCP communication
async fn websocket_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(query): Query<WebSocketQuery>,
    State(state): State<AppState>,
) -> Response {
    state
        .websocket_manager
        .handle_connection(ws, headers, Query(query), State(state.clone()))
        .await
}
