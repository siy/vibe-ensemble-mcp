//! Main server implementation

use crate::{config::Config, Result};
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};
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
}

impl Server {
    /// Create a new server instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing server components");

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

        // Initialize MCP server
        let mcp_server = Arc::new(McpServer::new());

        // Initialize web server if enabled
        let web_server = if config.web.enabled {
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
        })
    }

    /// Create the main coordination API router
    fn create_api_router(&self) -> Router {
        let state = AppState {
            storage: self.storage.clone(),
            mcp_server: self.mcp_server.clone(),
        };

        Router::new()
            // Health and status endpoints
            .route("/health", get(health_check))
            .route("/status", get(server_status))
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

        info!("Starting Vibe Ensemble MCP Server");

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
        info!("MCP server ready for protocol handling");

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
            status_response["message"] = json!("All systems operational - Vibe Ensemble server is ready");
        },
        "degraded" => {
            status_response["message"] = json!("Service is partially operational with some components unavailable");
            let mut suggestions = Vec::new();
            if !storage_healthy {
                suggestions.push("Check database connection and permissions");
            }
            if !mcp_available {
                suggestions.push("MCP server initialization may be incomplete");
            }
            status_response["suggestions"] = json!(suggestions);
        },
        "unhealthy" => {
            status_response["message"] = json!("Service is experiencing issues - multiple components unavailable");
            status_response["suggestions"] = json!([
                "Check database configuration and connectivity",
                "Verify MCP server initialization",
                "Review server logs for detailed error information",
                "Consider restarting the service if issues persist"
            ]);
        },
        _ => {}
    }

    Ok(Json(status_response))
}
