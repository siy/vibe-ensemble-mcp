//! Web server for the Vibe Ensemble dashboard

use crate::{handlers, link_validator, middleware, Result};
use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
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
            port: 8081,
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

    /// Build the application router (public for testing)
    pub fn build_router(&self) -> Router {
        self.build_router_internal()
    }

    /// Build the application router (internal)
    fn build_router_internal(&self) -> Router {
        Router::new()
            // Dashboard routes
            .route("/", get(handlers::dashboard))
            .route("/dashboard", get(handlers::dashboard))
            .route("/link-health", get(handlers::link_health))
            // API routes
            .route("/api/health", get(handlers::health))
            .route("/api/stats", get(handlers::system_stats))
            // Agent API routes
            .route("/api/agents", get(handlers::agents_list))
            .route("/api/agents/:id", get(handlers::agent_get))
            // Issue API routes
            .route("/api/issues", get(handlers::issues_list))
            .route("/api/issues", post(handlers::issues_create))
            .route("/api/issues/:id", get(handlers::issue_get))
            .route("/api/issues/:id", put(handlers::issue_update))
            .route("/api/issues/:id", delete(handlers::issue_delete))
            // Link validation routes
            .merge(link_validator::create_router())
            // Add shared state
            .with_state(self.storage.clone())
            // Add middleware layers
            .layer(
                ServiceBuilder::new()
                    .layer(axum_middleware::from_fn_with_state(
                        self.storage.clone(),
                        middleware::navigation_analytics_middleware,
                    ))
                    .layer(axum_middleware::from_fn(middleware::logging_middleware))
                    .layer(axum_middleware::from_fn(
                        middleware::security_headers_middleware,
                    ))
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
    }

    /// Run the web server
    pub async fn run(self) -> Result<()> {
        let app = self.build_router_internal();
        let addr = format!("{}:{}", self.config.host, self.config.port);

        tracing::info!("Web dashboard starting on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
