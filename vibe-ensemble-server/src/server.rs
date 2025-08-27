//! Main server implementation

use crate::{config::Config, McpTransport, OperationMode, Result};
use axum::response::sse::Event;
use axum::{
    extract::{ws::WebSocket, Path, Query, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response, Sse},
    routing::{get, post},
    Router,
};
use futures_util::Stream;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration, Instant};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::WebServer;

/// SSE session information for managing Claude Code connections
///
/// Each session represents an active SSE connection and maintains:
/// - A message channel for server-to-client communication
/// - Activity tracking for automatic cleanup of stale connections
/// - Initialization state to track session lifecycle
#[derive(Debug)]
struct SseSession {
    /// Channel to send messages to SSE client (bounded to prevent OOM)
    sender: mpsc::Sender<String>,
    /// Last activity timestamp for session cleanup
    last_activity: Instant,
    /// Whether session has been initialized with session_init
    initialized: bool,
}

/// Query parameters for SSE endpoint
#[derive(serde::Deserialize)]
struct SseQuery {
    session_id: Option<String>,
}

/// SSE session manager for Claude Code integration
type SessionManager = Arc<RwLock<HashMap<String, SseSession>>>;

/// Shared application state for API handlers
#[derive(Clone)]
pub struct AppState {
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
    sse_sessions: SessionManager,
}

/// Main server orchestrating all components
pub struct Server {
    config: Config,
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
    web_server: Option<WebServer>,
    operation_mode: OperationMode,
    transport: McpTransport,
    sse_sessions: SessionManager,
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

            Arc::new(McpServer::builder()
                    .with_capabilities(vibe_ensemble_mcp::protocol::ServerCapabilities {
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
                    })
                    .with_agent_service(agent_service)
                    .with_issue_service(issue_service)
                    .with_message_service(message_service)
                    .with_knowledge_service(knowledge_service)
                    .build())
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

        let sse_sessions = Arc::new(RwLock::new(HashMap::new()));

        Ok(Self {
            config,
            storage,
            mcp_server,
            web_server,
            operation_mode,
            transport,
            sse_sessions,
        })
    }

    /// Create the main coordination API router
    fn create_api_router(&self) -> Router {
        let state = AppState {
            storage: self.storage.clone(),
            mcp_server: self.mcp_server.clone(),
            sse_sessions: self.sse_sessions.clone(),
        };

        // Note: Session cleanup task will be started in the run() method

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
                    info!("MCP SSE endpoint enabled at /mcp/events (GET) - for Claude Code integration");
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

        // Start SSE session cleanup task if SSE is enabled
        if matches!(self.transport, McpTransport::Sse | McpTransport::Both) {
            let session_cleanup = self.sse_sessions.clone();
            let session_timeout = self.config.mcp.session_timeout;
            tokio::spawn(async move {
                let mut cleanup_interval = interval(Duration::from_secs(30));
                loop {
                    cleanup_interval.tick().await;
                    // Add basic error recovery for cleanup failures
                    match tokio::time::timeout(
                        Duration::from_secs(10),
                        Self::cleanup_expired_sessions(&session_cleanup, session_timeout),
                    )
                    .await
                    {
                        Ok(()) => {}
                        Err(_) => {
                            error!("Session cleanup task timed out after 10 seconds");
                        }
                    }
                }
            });
        }

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
                            debug!("Received MCP message ({} bytes)", message.len());

                            match mcp_server.handle_message(&message).await {
                                Ok(Some(response)) => {
                                    debug!("Sending MCP response ({} bytes)", response.len());
                                    if let Err(e) = transport.send(&response).await {
                                        error!("Failed to send MCP response: {}", e);
                                        break;
                                    }
                                }
                                Ok(None) => {
                                    debug!("No MCP response required");
                                }
                                Err(e) => {
                                    error!("Error processing MCP message: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            debug!("MCP transport error: {}", e);
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

    /// Clean up expired SSE sessions
    async fn cleanup_expired_sessions(sessions: &SessionManager, session_timeout_secs: u64) {
        let mut sessions = sessions.write().await;
        let now = Instant::now();
        let timeout = Duration::from_secs(session_timeout_secs);

        let expired_sessions: Vec<String> = sessions
            .iter()
            .filter_map(|(session_id, session)| {
                if now.duration_since(session.last_activity) > timeout {
                    Some(session_id.clone())
                } else {
                    None
                }
            })
            .collect();

        for session_id in expired_sessions {
            debug!("Cleaning up expired SSE session: {}", session_id);
            sessions.remove(&session_id);
        }
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
    // Extract method and id for logging, avoid logging full payload to prevent PII leakage
    let method = payload
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");
    let id = payload.get("id").and_then(|i| i.as_str()).unwrap_or("none");
    debug!("Received MCP HTTP request - method: {}, id: {}", method, id);

    // Convert JSON to string for MCP server processing
    let message = serde_json::to_string(&payload).map_err(|e| {
        error!("Failed to serialize JSON payload: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Process through MCP server
    match state.mcp_server.handle_message(&message).await {
        Ok(Some(response)) => {
            debug!("Sending MCP HTTP response: {}", response);
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
            debug!("No response required for MCP message");
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
                    debug!("Received MCP WebSocket message ({} bytes)", text.len());

                    // Process the message through MCP server
                    match state.mcp_server.handle_message(text).await {
                        Ok(Some(response)) => {
                            debug!("Sending MCP WebSocket response ({} bytes)", response.len());
                            if let Err(e) = socket
                                .send(axum::extract::ws::Message::Text(response))
                                .await
                            {
                                error!("Failed to send WebSocket response: {}", e);
                                break;
                            }
                        }
                        Ok(None) => {
                            debug!("No response required for MCP message");
                        }
                        Err(e) => {
                            error!("Error processing MCP message: {}", e);
                            // Send error response
                            let error_response = json!({
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

/// MCP SSE handler for Claude Code integration - full MCP protocol over SSE
async fn mcp_sse_handler(
    State(state): State<AppState>,
    Query(params): Query<SseQuery>,
) -> Sse<impl Stream<Item = std::result::Result<Event, axum::BoxError>>> {
    // Generate session ID and check for duplicates
    let mut session_id = params
        .session_id
        .unwrap_or_else(|| format!("sse_{}", Uuid::new_v4()));

    // Create bounded channel for sending messages to this SSE connection (1024 message buffer)
    let (sender, mut receiver) = mpsc::channel::<String>(1024);

    // Register session, handling reconnects gracefully
    {
        let mut sessions = state.sse_sessions.write().await;
        // Check for duplicate session_id and generate a new one if needed
        while sessions.contains_key(&session_id) {
            warn!(
                "Session {} already exists; generating a new session id",
                session_id
            );
            session_id = format!("sse_{}", Uuid::new_v4());
        }
        sessions.insert(
            session_id.clone(),
            SseSession {
                sender: sender.clone(),
                last_activity: Instant::now(),
                initialized: false,
            },
        );
    }

    info!(
        "New MCP SSE connection established with session_id: {}",
        session_id
    );

    // For Claude Code, immediately send MCP initialization response
    // Claude Code expects this to happen automatically when connecting to /mcp/events
    let init_response = {
        use vibe_ensemble_mcp::protocol::{InitializeResult, ServerInfo, MCP_VERSION};

        let result = InitializeResult {
            protocol_version: MCP_VERSION.to_string(),
            server_info: ServerInfo {
                name: "vibe-ensemble".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: state.mcp_server.capabilities().clone(),
            instructions: Some(
                "Vibe Ensemble MCP Server - Coordinating multiple Claude Code instances via SSE"
                    .to_string(),
            ),
        };

        // Create proper MCP JSON-RPC response
        let mcp_response = vibe_ensemble_mcp::protocol::JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::String(session_id.clone()),
            result: Some(serde_json::to_value(result).unwrap()),
            error: None,
        };

        serde_json::to_string(&mcp_response).unwrap()
    };

    if let Err(e) = sender.try_send(init_response) {
        error!("Failed to send MCP initialization response: {}", e);
    } else {
        debug!(
            "Sent MCP initialization response for session: {}",
            session_id
        );
        // Mark session as initialized
        let mut sessions = state.sse_sessions.write().await;
        if let Some(s) = sessions.get_mut(&session_id) {
            s.initialized = true;
            s.last_activity = Instant::now();
        }
    }

    // Clone state for cleanup
    let cleanup_sessions = state.sse_sessions.clone();
    let cleanup_session_id = session_id.clone();

    let stream = async_stream::stream! {
        // Send heartbeat every 30 seconds and handle messages from POST endpoint
        let mut heartbeat = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Handle messages from POST endpoint
                msg = receiver.recv() => {
                    match msg {
                        Some(message) => {
                            debug!("Sending SSE message to session {}: {}", session_id, message);

                            // Try to parse as JSON for proper event formatting
                            let event = if let Ok(json_msg) = serde_json::from_str::<Value>(&message) {
                                Event::default().json_data(json_msg)
                            } else {
                                Ok(Event::default().data(message))
                            };

                            match event {
                                Ok(e) => {
                                    yield Ok(e);
                                    // Update activity on successful message delivery
                                    let mut sessions = cleanup_sessions.write().await;
                                    if let Some(s) = sessions.get_mut(&session_id) {
                                        s.last_activity = Instant::now();
                                    }
                                }
                                Err(err) => {
                                    error!("Failed to create SSE event: {}", err);
                                    // Continue stream instead of yielding error
                                    // to prevent stream termination
                                    continue;
                                }
                            }
                        }
                        None => {
                            debug!("SSE message channel closed for session: {}", session_id);
                            break;
                        }
                    }
                },
                // Send periodic heartbeat
                _ = heartbeat.tick() => {
                    let heartbeat_msg = json!({
                        "type": "heartbeat",
                        "session_id": session_id,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    });

                    let event = Event::default()
                        .json_data(heartbeat_msg)
                        .map_err(axum::BoxError::from);

                    yield event;
                    // Update activity on heartbeat
                    let mut sessions = cleanup_sessions.write().await;
                    if let Some(s) = sessions.get_mut(&session_id) {
                        s.last_activity = Instant::now();
                    }
                }
            }
        }

        // Cleanup session on stream end
        info!("SSE connection closed for session: {}", session_id);
        let mut sessions = cleanup_sessions.write().await;
        sessions.remove(&cleanup_session_id);
    };

    // We emit structured heartbeats; extra comment keepalives not needed.
    Sse::new(stream)
}

/// MCP SSE POST handler for Claude Code integration
async fn mcp_sse_post_handler(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(payload): Json<Value>,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Extract method and id for logging to avoid logging full payloads
    let method = payload
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("unknown");
    let id = payload.get("id").and_then(|i| i.as_str()).unwrap_or("none");
    debug!(
        "Received MCP SSE POST request for session {} - method: {}, id: {}",
        session_id, method, id
    );

    // Convert JSON to string for MCP server processing
    let message = serde_json::to_string(&payload).map_err(|e| {
        error!("Failed to serialize JSON payload: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    // Process through MCP server
    match state.mcp_server.handle_message(&message).await {
        Ok(Some(response)) => {
            // Avoid logging full response to prevent PII leakage
            debug!("MCP server produced response for session: {}", session_id);

            // Try to send response via SSE channel
            let mut sessions = state.sse_sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_id) {
                // Update last activity
                session.last_activity = Instant::now();
                session.initialized = true;

                // Send response via SSE channel
                use tokio::sync::mpsc::error::TrySendError;
                let send_result = session.sender.try_send(response.clone());
                if let Err(err) = send_result {
                    match err {
                        TrySendError::Full(_msg) => {
                            warn!(
                                "SSE channel full for session {}, signaling backpressure",
                                session_id
                            );
                            return Err(StatusCode::TOO_MANY_REQUESTS); // 429 Too Many Requests
                        }
                        TrySendError::Closed(_msg) => {
                            warn!(
                                "SSE channel closed for session {}, expiring session",
                                session_id
                            );
                            sessions.remove(&session_id);
                            return Err(StatusCode::GONE); // 410 Gone - session expired
                        }
                    }
                } else {
                    debug!("Response sent via SSE channel for session: {}", session_id);
                }
            } else {
                debug!(
                    "Session {} not found; signaling client to reconnect",
                    session_id
                );
                return Err(StatusCode::GONE);
            }

            // Also return HTTP response for non-SSE callers
            match serde_json::from_str::<Value>(&response) {
                Ok(json_response) => Ok(Json(json_response)),
                Err(e) => {
                    error!("Failed to parse MCP response as JSON: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Ok(None) => {
            debug!("No response required for MCP message");
            // For notifications (no response expected), return 204 No Content
            Err(StatusCode::NO_CONTENT)
        }
        Err(e) => {
            error!("Error processing MCP message: {}", e);
            // Return proper JSON-RPC error response instead of 500
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
