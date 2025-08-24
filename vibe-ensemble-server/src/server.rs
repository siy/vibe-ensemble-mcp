//! Main server implementation

use crate::{config::Config, Result};
use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::signal;
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
        info!("Starting Vibe Ensemble MCP Server");
        info!("Main API listening on {}", self.config.server_addr());

        if let Some(_web_server) = &self.web_server {
            info!(
                "Web interface available at http://{}",
                self.config.web_addr()
            );
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
        info!("MCP server ready for protocol handling");

        // Wait for shutdown signal while serving API
        tokio::select! {
            result = axum::serve(listener, app) => {
                if let Err(e) = result {
                    warn!("API server error: {}", e);
                }
            }
            _ = self.wait_for_shutdown() => {
                info!("Shutdown signal received");
            }
        }

        // Graceful shutdown
        info!("Shutting down server...");

        if let Some(handle) = web_handle {
            handle.abort();
        }

        info!("Server shutdown complete");
        Ok(())
    }

    /// Wait for shutdown signal
    async fn wait_for_shutdown(&self) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
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
            "version": env!("CARGO_PKG_VERSION")
        }))),
        Err(_) => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn server_status(
    State(state): State<AppState>,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Check both storage and MCP server status
    let storage_healthy = state.storage.health_check().await.is_ok();
    // MCP server is available if we can access it
    let mcp_available = state.mcp_server.capabilities().tools.is_some();

    Ok(Json(json!({
        "status": if storage_healthy { "operational" } else { "degraded" },
        "timestamp": chrono::Utc::now(),
        "version": env!("CARGO_PKG_VERSION"),
        "components": {
            "storage": if storage_healthy { "healthy" } else { "unhealthy" },
            "mcp_server": if mcp_available { "available" } else { "unavailable" }
        },
        "message": "Vibe Ensemble server is running"
    })))
}
