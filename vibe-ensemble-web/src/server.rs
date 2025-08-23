//! Web server for the Vibe Ensemble dashboard

use crate::{handlers, Result};
use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use vibe_ensemble_storage::StorageManager;

/// Web server configuration
#[derive(Debug, Clone)]
pub struct WebConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 3000,
        }
    }
}

/// Web server instance
pub struct WebServer {
    config: WebConfig,
    storage: Arc<StorageManager>,
}

impl WebServer {
    /// Create a new web server
    pub async fn new(config: WebConfig, storage: Arc<StorageManager>) -> Result<Self> {
        Ok(Self { config, storage })
    }

    /// Build the application router
    fn build_router(&self) -> Router {
        Router::new()
            // Dashboard routes
            .route("/", get(handlers::dashboard))
            .route("/dashboard", get(handlers::dashboard))
            // API routes
            .route("/api/health", get(handlers::health))
            .route("/api/agents", get(handlers::agents_list))
            .route("/api/issues", get(handlers::issues_list))
            .route("/api/issues", post(handlers::issues_create))
            .route("/api/stats", get(handlers::system_stats))
            // Add shared state
            .with_state(self.storage.clone())
            // Add middleware
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
    }

    /// Run the web server
    pub async fn run(self) -> Result<()> {
        let app = self.build_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);

        tracing::info!("Web dashboard starting on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
