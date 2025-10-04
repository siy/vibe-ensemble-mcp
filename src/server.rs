use axum::extract::WebSocketUpgrade;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, Method},
    response::{IntoResponse, Json, Response},
    routing::{any, get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};
use tracing::{error, info};

use crate::{
    auth::AuthTokenManager,
    config::Config,
    database::{recovery::TicketRecovery, DbPool},
    error::Result,
    lockfile::LockFileManager,
    mcp::{
        server::{mcp_handler, McpServer},
        websocket::{WebSocketManager, WebSocketQuery},
    },
    sse::{sse_handler, sse_message_handler, EventBroadcaster},
    workers::queue::QueueManager,
};
use dashmap::DashMap;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: DbPool,
    pub queue_manager: Arc<QueueManager>,
    pub event_broadcaster: EventBroadcaster,
    pub mcp_server: Arc<McpServer>,
    pub websocket_manager: Arc<WebSocketManager>,
    pub websocket_token: Option<String>,
    pub auth_manager: Arc<AuthTokenManager>,
    pub coordinator_directories: Arc<dashmap::DashMap<String, String>>,
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

    // Initialize coordinator directories (shared across components)
    let coordinator_directories = Arc::new(DashMap::new());

    // Initialize queue manager (spawns completion event processor internally)
    let queue_manager = QueueManager::new(
        db.clone(),
        config.clone(),
        event_broadcaster.clone(),
        coordinator_directories.clone(),
    );

    // Initialize single MCP server instance with config-based tool registration
    let mcp_server = Arc::new(McpServer::new(&config));

    // Initialize WebSocket manager with concurrency limits and event broadcasting
    let websocket_manager = Arc::new(WebSocketManager::with_event_broadcasting(
        config.max_concurrent_client_requests,
        event_broadcaster.clone(),
    ));

    // Create auth token manager (we'll add the websocket token after binding to the port)
    let auth_manager = Arc::new(AuthTokenManager::new());

    let state = AppState {
        config: config.clone(),
        db,
        queue_manager,
        event_broadcaster,
        mcp_server,
        websocket_manager,
        websocket_token: None, // Will be set after binding to port
        auth_manager: Arc::clone(&auth_manager),
        coordinator_directories,
    };

    // Respawn workers for unfinished tasks if enabled
    if !config.no_respawn {
        respawn_workers_for_unfinished_tasks(&state).await?;
    }

    // Start update checking service if enabled
    if !config.disable_update_checks {
        info!(
            "Starting update check service (interval: {} hours)",
            config.update_check_interval_hours
        );
        let update_service = crate::updates::UpdateService::new(config.update_check_interval_hours);
        let _update_task =
            update_service.start_periodic_checks(state.db.clone(), state.event_broadcaster.clone());
        // Note: We don't need to keep the JoinHandle as the task will run until server shutdown
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

    // Add root route that handles both WebSocket upgrades and regular HTTP requests
    app = app.route("/", any(root_handler));
    info!("WebSocket support enabled at / (root path)");

    let app = app
        .layer(RequestBodyLimitLayer::new(1024 * 1024)) // 1 MiB
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    let address = config.server_address();
    info!("Server listening on {}", address);

    let listener = tokio::net::TcpListener::bind(&address).await?;

    // Now that we're successfully bound to the port, create/update the Claude IDE lock file
    let _websocket_token = {
        let lock_manager = LockFileManager::new(config.host.clone(), config.port);
        match lock_manager.create_or_update_claude_lock_file() {
            Ok(token) => {
                info!("Created/updated Claude IDE lock file with WebSocket token");
                auth_manager.add_token(token.clone());
                Some(token)
            }
            Err(e) => {
                error!("Failed to create Claude IDE lock file: {}", e);
                None
            }
        }
    };

    // Update the state with the websocket token (this is a bit tricky since state is immutable)
    // For now, the token is added to the auth_manager which is what matters for authentication

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
    // Process recovery using the dedicated recovery module
    let _stats = TicketRecovery::process_recovery(&state.db).await?;

    // Resubmit all ready tickets to queues
    let tickets_for_resubmission = TicketRecovery::get_tickets_for_resubmission(&state.db).await?;

    let mut resubmitted_count = 0;
    for (ticket_id, project_id, current_stage) in tickets_for_resubmission {
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
        resubmitted_count += 1;
    }

    if resubmitted_count > 0 {
        info!("Resubmitted {} tickets to queues", resubmitted_count);
    }

    Ok(())
}

/// Root handler that handles both WebSocket upgrades and regular HTTP requests
async fn root_handler(
    ws_upgrade: Option<WebSocketUpgrade>,
    headers: HeaderMap,
    Query(query): Query<WebSocketQuery>,
    State(state): State<AppState>,
) -> Response {
    // Check if this is a WebSocket upgrade request
    if let Some(upgrade_header) = headers.get("upgrade") {
        if let Ok(upgrade_value) = upgrade_header.to_str() {
            if upgrade_value.to_lowercase() == "websocket" {
                if let Some(ws) = ws_upgrade {
                    tracing::info!("WebSocket connection request received at / endpoint");
                    tracing::trace!("WebSocket upgrade request headers: {:?}", headers);
                    tracing::trace!("WebSocket query parameters: {:?}", query);

                    let response = state
                        .websocket_manager
                        .handle_connection(ws, headers, Query(query), State(state.clone()))
                        .await;

                    tracing::trace!("WebSocket handler returning response");
                    return response;
                }
            }
        }
    }

    // Regular HTTP request - return basic server information
    tracing::info!("HTTP request received at / endpoint");
    Json(json!({
        "service": "vibe-ensemble-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Multi-agent coordination MCP server for Claude Code",
        "endpoints": {
            "/": "WebSocket MCP connection (with Upgrade: websocket header)",
            "/health": "Health check endpoint",
            "/mcp": "HTTP MCP endpoint",
            "/sse": "Server-Sent Events endpoint",
            "/messages": "SSE message endpoint"
        },
        "websocket": {
            "protocol": "mcp",
            "authentication": "Required via query parameter 'token' or header 'x-claude-code-ide-authorization'"
        }
    })).into_response()
}
