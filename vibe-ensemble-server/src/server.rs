//! Main server implementation

use crate::{config::Config, Result};
use std::sync::Arc;
use tokio::signal;
use tracing::{info, warn};
use vibe_ensemble_mcp::server::McpServer;
use vibe_ensemble_monitoring::{
    MonitoringConfig as MonitoringCrateConfig, MonitoringServer, ObservabilityService,
};
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::WebServer;

/// Main server orchestrating all components
pub struct Server {
    config: Config,
    storage: Arc<StorageManager>,
    mcp_server: Arc<McpServer>,
    web_server: Option<WebServer>,
    observability: Option<Arc<ObservabilityService>>,
    monitoring_server: Option<MonitoringServer>,
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

        // Initialize monitoring if enabled
        let (observability, monitoring_server) = if config.monitoring.enabled {
            let monitoring_config = MonitoringCrateConfig {
                metrics: vibe_ensemble_monitoring::config::MetricsConfig {
                    enabled: true,
                    host: config.monitoring.metrics_host.clone(),
                    port: config.monitoring.metrics_port,
                    collection_interval: 15,
                    system_metrics: true,
                    business_metrics: true,
                },
                tracing: vibe_ensemble_monitoring::config::TracingConfig {
                    enabled: config.monitoring.tracing_enabled,
                    service_name: "vibe-ensemble-mcp".to_string(),
                    jaeger_endpoint: config.monitoring.jaeger_endpoint.clone(),
                    sampling_ratio: 1.0,
                    max_spans: 10000,
                    json_logs: config.logging.format == "json",
                },
                health: vibe_ensemble_monitoring::config::HealthConfig {
                    enabled: true,
                    host: config.monitoring.health_host.clone(),
                    port: config.monitoring.health_port,
                    timeout: 30,
                    readiness_enabled: true,
                    liveness_enabled: true,
                },
                alerting: vibe_ensemble_monitoring::config::AlertingConfig {
                    enabled: config.monitoring.alerting_enabled,
                    error_rate_threshold: 5.0,
                    response_time_threshold: 1000,
                    memory_threshold: 85.0,
                    cpu_threshold: 80.0,
                    check_interval: 60,
                },
            };

            let mut observability_service =
                ObservabilityService::new(monitoring_config).with_storage(storage.clone());

            observability_service.initialize().await?;
            let observability = Arc::new(observability_service);

            let monitoring_server = MonitoringServer::new(
                observability.clone(),
                config.monitoring.health_host.clone(),
                config.monitoring.health_port,
            );

            (Some(observability), Some(monitoring_server))
        } else {
            (None, None)
        };

        Ok(Self {
            config,
            storage,
            mcp_server,
            web_server,
            observability,
            monitoring_server,
        })
    }

    /// Run the server
    pub async fn run(mut self) -> Result<()> {
        info!("Starting Vibe Ensemble MCP Server");
        info!("MCP Server listening on {}", self.config.server_addr());

        if let Some(web_server) = &self.web_server {
            info!(
                "Web interface available at http://{}",
                self.config.web_addr()
            );
        }

        if let Some(_) = &self.observability {
            info!(
                "Monitoring available at http://{}:{}",
                self.config.monitoring.health_host, self.config.monitoring.health_port
            );
            info!(
                "Metrics available at http://{}:{}/metrics",
                self.config.monitoring.metrics_host, self.config.monitoring.metrics_port
            );
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

        // Start monitoring server in the background if enabled
        let monitoring_handle = if let Some(monitoring_server) = self.monitoring_server.take() {
            Some(tokio::spawn(async move {
                if let Err(e) = monitoring_server.run().await {
                    warn!("Monitoring server error: {}", e);
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

        if let Some(handle) = monitoring_handle {
            handle.abort();
        }

        // Shutdown observability service gracefully
        if let Some(observability) = &self.observability {
            observability.shutdown().await;
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
