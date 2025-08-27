//! Main server implementation

use crate::{config::Config, McpTransport, OperationMode, Result};
use axum::response::sse::{Event, KeepAlive};
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade, Path},
    http::StatusCode,
    response::{Json, Response, Sse},
    routing::{get, post},
    Router,
};
use futures_util::Stream;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
// use tokio::time::{interval, Duration}; // Removed - no longer needed for SSE monitoring
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn, debug};
use uuid::Uuid;
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::WebServer;

/// SSE session for MCP communication
#[derive(Clone)]
pub struct SseSession {
    pub session_id: String,
    pub sender: broadcast::Sender<String>,
}

/// Shared application state for API handlers
#[derive(Clone)]
pub struct AppState {
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
    sse_sessions: Arc<RwLock<HashMap<String, SseSession>>>,
}

/// Main server orchestrating all components
pub struct Server {
    config: Config,
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
    web_server: Option<WebServer>,
    operation_mode: OperationMode,
    transport: McpTransport,
}

impl Server {
    /// Create a new server instance
    pub async fn new(
        config: Config,
        operation_mode: OperationMode,
        transport: McpTransport,
    ) -> Result<Self> {
        info!(
            "Initializing server components for {:?} mode",
            operation_mode
        );

        // Initialize storage
        let db_config = vibe_ensemble_storage::manager::DatabaseConfig {
            url: config.database.url.clone(),
            max_connections: config.database.max_connections,
            migrate_on_startup: config.database.migrate_on_startup,
            performance_config: None,
        };
        let storage = Arc::new(StorageManager::new(&db_config).await?);

        if config.database.migrate_on_startup {
            storage.migrate().await?;
        }

        // Initialize MCP server with all services if MCP is enabled
        let mcp_server = if matches!(operation_mode, OperationMode::Full | OperationMode::McpOnly) {
            let agent_service = storage.agent_service();
            let issue_service = storage.issue_service();
            let message_service = storage.message_service();
            let knowledge_service = storage.knowledge_service();

            Arc::new(McpServer::new_with_capabilities_and_all_services(
                vibe_ensemble_mcp::protocol::ServerCapabilities {
                    experimental: None,
                    logging: None,
                    prompts: Some(vibe_ensemble_mcp::protocol::PromptsCapability {
                        list_changed: Some(true),
                    }),
                    resources: Some(vibe_ensemble_mcp::protocol::ResourcesCapability {
                        subscribe: Some(true),
                        list_changed: Some(true),
                    }),
                    tools: Some(vibe_ensemble_mcp::protocol::ToolsCapability {
                        list_changed: Some(true),
                    }),
                    vibe_agent_management: Some(true),
                    vibe_issue_tracking: Some(true),
                    vibe_messaging: Some(true),
                    vibe_knowledge_management: Some(true),
                },
                agent_service,
                issue_service,
                message_service,
                knowledge_service,
            ))
        } else {
            Arc::new(McpServer::new())
        };

        // Initialize web server if enabled
        let web_server = if config.web.enabled
            && matches!(
                operation_mode,
                OperationMode::Full | OperationMode::WebOnly | OperationMode::McpOnly
            ) {
            let web_config = vibe_ensemble_web::server::WebConfig {
                enabled: config.web.enabled,
                host: config.web.host.clone(),
                port: config.web.port,
            };
            Some(WebServer::new(web_config, storage.clone()).await?)
        } else {
            None
        };

        Ok(Self {
            config,
            storage,
            mcp_server,
            web_server,
            operation_mode,
            transport,
        })
    }

    /// Create the main coordination API router
    fn create_api_router(&self) -> Router {
        let state = AppState {
            storage: self.storage.clone(),
            mcp_server: self.mcp_server.clone(),
            sse_sessions: Arc::new(RwLock::new(HashMap::new())),
        };

        let mut router = Router::new()
            // Health and status endpoints
            .route("/health", get(health_check))
            .route("/status", get(server_status));

        // Add MCP endpoints if MCP is enabled
        if matches!(
            self.operation_mode,
            OperationMode::Full | OperationMode::McpOnly
        ) {
            match self.transport {
                McpTransport::Websocket => {
                    router = router.route("/mcp", get(mcp_websocket_handler));
                    info!("MCP WebSocket endpoint enabled at /mcp (GET)");
                }
                McpTransport::Stdio => {
                    // Stdio transport doesn't need HTTP endpoints - it's handled separately in main.rs
                }
                McpTransport::Sse => {
                    router = router
                        .route("/mcp/events", get(mcp_sse_handler))
                        .route("/mcp/sse/:session_id", post(mcp_sse_post_handler));
                    info!("MCP SSE endpoint enabled at /mcp/events (GET)");
                    info!("MCP SSE POST endpoint enabled at /mcp/sse/:session_id (POST)");
                }
                McpTransport::Both => {
                    router = router
                        .route("/mcp", get(mcp_websocket_handler))
                        .route("/mcp", post(mcp_http_handler))
                        .route("/mcp/events", get(mcp_sse_handler))
                        .route("/mcp/sse/:session_id", post(mcp_sse_post_handler));
                    info!("MCP WebSocket endpoint enabled at /mcp (GET)");
                    info!("MCP HTTP endpoint enabled at /mcp (POST)");
                    info!("MCP SSE endpoint enabled at /mcp/events (GET)");
                    info!("MCP SSE POST endpoint enabled at /mcp/sse/:session_id (POST)");
                }
            }
        }

        router
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
            .with_state(state)
    }

    /// Run the server
    pub async fn run(mut self) -> Result<()> {
        // Print configuration summary with security warnings
        self.config.print_startup_summary();

        match self.operation_mode {
            OperationMode::Full => {
                info!("Starting Vibe Ensemble Server in Full mode (API + Web + MCP)")
            }
            OperationMode::WebOnly => return self.run_web_only().await,
            OperationMode::ApiOnly => info!("Starting Vibe Ensemble Server in API-only mode"),
            OperationMode::McpOnly => {
                // MCP-only with stdio is handled in main.rs, this would be WebSocket only
                info!("Starting Vibe Ensemble Server in MCP-only mode (WebSocket)")
            }
        }

        // Create API router
        let app = self.create_api_router();

        // Start main API server
        let listener = tokio::net::TcpListener::bind(self.config.server_addr()).await?;

        // Start web server in the background if enabled
        let web_handle = self.web_server.take().map(|web_server| {
            tokio::spawn(async move {
                if let Err(e) = web_server.run().await {
                    warn!("Web server error: {}", e);
                }
            })
        });

        // Start MCP stdio handler in the background if needed
        let mcp_stdio_handle = if matches!(
            self.operation_mode,
            OperationMode::Full | OperationMode::McpOnly
        ) && matches!(self.transport, McpTransport::Stdio)
        {
            let mcp_server = self.mcp_server.clone();
            Some(tokio::spawn(async move {
                info!("Starting MCP stdio handler");
                let mut transport = vibe_ensemble_mcp::transport::TransportFactory::stdio();

                loop {
                    match transport.receive().await {
                        Ok(message) => {
                            tracing::debug!("Received MCP message: {}", message);

                            match mcp_server.handle_message(&message).await {
                                Ok(Some(response)) => {
                                    tracing::debug!("Sending MCP response: {}", response);
                                    if let Err(e) = transport.send(&response).await {
                                        error!("Failed to send MCP response: {}", e);
                                        break;
                                    }
                                }
                                Ok(None) => {
                                    tracing::debug!("No MCP response required");
                                }
                                Err(e) => {
                                    error!("Error processing MCP message: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::debug!("MCP transport error: {}", e);
                            break;
                        }
                    }
                }

                if let Err(e) = transport.close().await {
                    warn!("Error closing MCP transport: {}", e);
                }
                info!("MCP stdio handler stopped");
            }))
        } else {
            info!("MCP server ready for protocol handling");
            None
        };

        // Use axum's graceful shutdown for better connection draining
        let shutdown = async {
            let ctrl_c = async {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to install Ctrl+C handler");
            };

            #[cfg(unix)]
            let terminate = async {
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to install signal handler")
                    .recv()
                    .await;
            };

            #[cfg(not(unix))]
            let terminate = std::future::pending::<()>();

            tokio::select! {
                _ = ctrl_c => { info!("Shutdown signal received"); },
                _ = terminate => { info!("Shutdown signal received"); },
            }
        };

        let graceful = axum::serve(listener, app).with_graceful_shutdown(shutdown);

        if let Err(e) = graceful.await {
            warn!("API server error: {}", e);
        }

        // Graceful shutdown
        info!("Shutting down server...");

        if let Some(handle) = web_handle {
            handle.abort();
        }

        if let Some(handle) = mcp_stdio_handle {
            handle.abort();
        }

        info!("Server shutdown complete");
        Ok(())
    }

    /// Run server in web-only mode
    async fn run_web_only(mut self) -> Result<()> {
        info!("Starting Vibe Ensemble Server in Web-only mode");

        if let Some(web_server) = self.web_server.take() {
            web_server.run().await.map_err(|e| e.into())
        } else {
            Err(crate::Error::Configuration(
                "Web server not configured".to_string(),
            ))
        }
    }
}

// API Handler implementations

async fn health_check(
    State(state): State<AppState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Check database health
    match state.storage.health_check().await {
        Ok(_) => Ok(Json(json!({
            "status": "healthy",
            "timestamp": chrono::Utc::now(),
            "version": env!("CARGO_PKG_VERSION"),
            "message": "All systems operational"
        }))),
        Err(e) => {
            warn!("Health check failed: {}", e);
            Ok(Json(json!({
                "status": "unhealthy",
                "timestamp": chrono::Utc::now(),
                "version": env!("CARGO_PKG_VERSION"),
                "error": "Database connectivity issue",
                "message": "Service temporarily unavailable. Please check database connection and try again.",
                "suggestions": [
                    "Verify database file exists and has proper permissions",
                    "Check DATABASE_URL environment variable",
                    "Ensure sufficient disk space",
                    "Contact administrator if problem persists"
                ]
            })))
        }
    }
}

async fn server_status(
    State(state): State<AppState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Check both storage and MCP server status
    let storage_healthy = state.storage.health_check().await.is_ok();
    // MCP server is available if we can access it
    let mcp_available = state.mcp_server.capabilities().tools.is_some();

    let overall_status = if storage_healthy && mcp_available {
        "operational"
    } else if storage_healthy || mcp_available {
        "degraded"
    } else {
        "unhealthy"
    };

    let mut status_response = json!({
        "status": overall_status,
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "storage": if storage_healthy { "healthy" } else { "unhealthy" },
            "mcp_server": if mcp_available { "available" } else { "unavailable" }
        },
        "endpoints": {
            "health": "/health",
            "status": "/status",
            "web_dashboard": "http://127.0.0.1:8081/dashboard" // TODO: Make this configurable
        }
    });

    // Add appropriate message and suggestions based on status
    match overall_status {
        "operational" => {
            status_response["message"] =
                json!("All systems operational - Vibe Ensemble server is ready");
        }
        "degraded" => {
            status_response["message"] =
                json!("Service is partially operational with some components unavailable");
            let mut suggestions = Vec::new();
            if !storage_healthy {
                suggestions.push("Check database connection and permissions");
            }
            if !mcp_available {
                suggestions.push("MCP server initialization may be incomplete");
            }
            status_response["suggestions"] = json!(suggestions);
        }
        "unhealthy" => {
            status_response["message"] =
                json!("Service is experiencing issues - multiple components unavailable");
            status_response["suggestions"] = json!([
                "Check database configuration and connectivity",
                "Verify MCP server initialization",
                "Review server logs for detailed error information",
                "Consider restarting the service if issues persist"
            ]);
        }
        _ => {}
    }

    Ok(Json(status_response))
}

/// MCP WebSocket handler
async fn mcp_websocket_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_mcp_websocket(socket, state))
}

/// MCP HTTP handler for POST requests (Claude Code JSON-RPC 2.0)
async fn mcp_http_handler(
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> std::result::Result<Json<Value>, StatusCode> {
    tracing::debug!("Received MCP HTTP request: {}", payload);

    // Convert JSON to string for MCP server processing
    let message = serde_json::to_string(&payload).map_err(|e| {
        error!("Failed to serialize JSON payload: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Process through MCP server
    match state.mcp_server.handle_message(&message).await {
        Ok(Some(response)) => {
            tracing::debug!("Sending MCP HTTP response: {}", response);
            // Parse response back to JSON
            match serde_json::from_str::<Value>(&response) {
                Ok(json_response) => Ok(Json(json_response)),
                Err(e) => {
                    error!("Failed to parse MCP response as JSON: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Ok(None) => {
            tracing::debug!("No response required for MCP message");
            // For notifications (no response expected), return 204 No Content
            Err(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            error!("Error processing MCP message: {}", e);
            // Return JSON-RPC error response
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": "Internal error",
                    "data": e.to_string()
                },
                "id": payload.get("id")
            });
            Ok(Json(error_response))
        }
    }
}

/// Handle MCP WebSocket connection
async fn handle_mcp_websocket(mut socket: WebSocket, state: AppState) {
    info!("New MCP WebSocket connection established");

    // Handle MCP messages over WebSocket
    loop {
        match socket.recv().await {
            Some(Ok(msg)) => {
                if let Ok(text) = msg.to_text() {
                    tracing::debug!("Received MCP WebSocket message: {}", text);

                    // Process the message through MCP server
                    match state.mcp_server.handle_message(text).await {
                        Ok(Some(response)) => {
                            tracing::debug!("Sending MCP WebSocket response: {}", response);
                            if let Err(e) = socket
                                .send(axum::extract::ws::Message::Text(response))
                                .await
                            {
                                error!("Failed to send WebSocket response: {}", e);
                                break;
                            }
                        }
                        Ok(None) => {
                            tracing::debug!("No response required for MCP message");
                        }
                        Err(e) => {
                            error!("Error processing MCP message: {}", e);
                            // Send error response
                            let error_response = serde_json::json!({
                                "jsonrpc": "2.0",
                                "error": {
                                    "code": -32603,
                                    "message": "Internal error",
                                    "data": e.to_string()
                                }
                            });
                            if let Err(e) = socket
                                .send(axum::extract::ws::Message::Text(error_response.to_string()))
                                .await
                            {
                                error!("Failed to send error response: {}", e);
                                break;
                            }
                        }
                    }
                } else {
                    warn!("Received non-text WebSocket message, ignoring");
                }
            }
            Some(Err(e)) => {
                error!("WebSocket error: {}", e);
                break;
            }
            None => {
                info!("MCP WebSocket connection closed");
                break;
            }
        }
    }
}

/// MCP SSE handler for bidirectional MCP protocol communication
/// Creates an SSE stream for server-to-client messages
async fn mcp_sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = std::result::Result<Event, axum::BoxError>>> {
    // Create a new session ID for this SSE connection
    let session_id = Uuid::new_v4().to_string();
    info!("Creating new MCP SSE session: {}", session_id);

    // Create broadcast channel for this session
    let (sender, mut receiver) = broadcast::channel(1024);

    // Store the session
    {
        let mut sessions = state.sse_sessions.write().await;
        sessions.insert(session_id.clone(), SseSession {
            session_id: session_id.clone(),
            sender: sender.clone(),
        });
    }

    // Send session initialization message
    let init_message = json!({
        "type": "session_init",
        "session_id": session_id,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": "MCP SSE session established. Use POST /mcp/sse/{session_id} to send messages."
    });

    if let Err(e) = sender.send(serde_json::to_string(&init_message).unwrap_or_default()) {
        warn!("Failed to send session init message: {}", e);
    }

    let sessions_for_cleanup = state.sse_sessions.clone();
    let session_id_for_cleanup = session_id.clone();

    let stream = async_stream::stream! {
        // Send the session initialization event
        let init_event = Event::default()
            .json_data(init_message)
            .map_err(axum::BoxError::from);
        yield init_event;

        // Listen for messages from the broadcast channel
        loop {
            match receiver.recv().await {
                Ok(message) => {
                    debug!("Sending MCP SSE message: {}", message);
                    
                    // Try to parse as JSON to validate format
                    let json_message: Value = match serde_json::from_str(&message) {
                        Ok(json) => json,
                        Err(_) => {
                            // If not valid JSON, wrap it in a generic message structure
                            json!({
                                "type": "message",
                                "data": message,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            })
                        }
                    };

                    let event = Event::default()
                        .json_data(json_message)
                        .map_err(axum::BoxError::from);

                    yield event;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("SSE session broadcast channel closed: {}", session_id_for_cleanup);
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("SSE session {} lagged behind, skipped {} messages", session_id_for_cleanup, skipped);
                    // Continue trying to receive
                }
            }
        }

        // Cleanup session when stream ends
        let mut sessions = sessions_for_cleanup.write().await;
        sessions.remove(&session_id_for_cleanup);
        info!("Cleaned up MCP SSE session: {}", session_id_for_cleanup);
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// MCP SSE POST handler for client-to-server messages
/// Receives MCP messages from client and sends responses back via SSE
async fn mcp_sse_post_handler(
    Path(session_id): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<Value>,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("Received MCP SSE POST request for session {}: {}", session_id, payload);

    // Find the SSE session
    let session = {
        let sessions = state.sse_sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    let session = match session {
        Some(session) => session,
        None => {
            warn!("SSE session not found: {}", session_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Convert JSON to string for MCP server processing
    let message = serde_json::to_string(&payload).map_err(|e| {
        error!("Failed to serialize JSON payload: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Process through MCP server
    match state.mcp_server.handle_message(&message).await {
        Ok(Some(response)) => {
            debug!("Sending MCP SSE response via session {}: {}", session_id, response);
            
            // Send response back through SSE channel
            if let Err(e) = session.sender.send(response.clone()) {
                warn!("Failed to send response to SSE session {}: {}", session_id, e);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            // Also return the response directly for immediate feedback
            match serde_json::from_str::<Value>(&response) {
                Ok(json_response) => Ok(Json(json_response)),
                Err(e) => {
                    error!("Failed to parse MCP response as JSON: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Ok(None) => {
            debug!("No response required for MCP message in session {}", session_id);
            // For notifications (no response expected), return 204 No Content
            Err(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            error!("Error processing MCP message in session {}: {}", session_id, e);
            
            // Create JSON-RPC error response
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": "Internal error",
                    "data": e.to_string()
                },
                "id": payload.get("id")
            });

            // Send error through SSE channel as well
            if let Err(e) = session.sender.send(serde_json::to_string(&error_response).unwrap_or_default()) {
                warn!("Failed to send error to SSE session {}: {}", session_id, e);
            }

            Ok(Json(error_response))
        }
    }
}
