//! Main server implementation

use crate::{config::Config, Result};
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::WebServer;

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
                static_files_path: config.web.static_files_path.clone(),
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

    /// Run the server
    pub async fn run(mut self) -> Result<()> {
        info!("Starting Vibe Ensemble MCP Server");
        info!("MCP Server listening on {}", self.config.server_addr());
        
        if let Some(web_server) = &self.web_server {
            info!("Web interface available at http://{}", self.config.web_addr());
        }

        // Start web server in the background if enabled
        let web_handle = if let Some(web_server) = self.web_server.take() {
            Some(tokio::spawn(async move {
                if let Err(e) = web_server.run().await {
                    warn!("Web server error: {}", e);
                }
            }))
        } else {
            None
        };

        // Start MCP server (this would be the main event loop)
        // For now, just wait for shutdown signal
        self.wait_for_shutdown().await;

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

        info!("Shutdown signal received");
    }
}