//! Main server implementation

use crate::{config::Config, McpTransport, OperationMode, Result};
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Json, Response},
    routing::{any, get},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info, warn};
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::WebServer;

/// Shared application state for API handlers
#[derive(Clone)]
pub struct AppState {
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
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
            && matches!(operation_mode, OperationMode::Full | OperationMode::WebOnly)
        {
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
        };

        let mut router = Router::new()
            // Health and status endpoints
            .route("/health", get(health_check))
            .route("/status", get(server_status));

        // Add MCP WebSocket endpoint if MCP is enabled and WebSocket transport is supported
        if matches!(
            self.operation_mode,
            OperationMode::Full | OperationMode::McpOnly
        ) && matches!(self.transport, McpTransport::Websocket | McpTransport::Both)
        {
            router = router.route("/mcp", any(mcp_websocket_handler));
            info!("MCP WebSocket endpoint enabled at /mcp");
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

        // MCP server is available for protocol handling via handle_message
        if matches!(
            self.operation_mode,
            OperationMode::Full | OperationMode::McpOnly
        ) {
            info!("MCP server ready for protocol handling");
        }

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
