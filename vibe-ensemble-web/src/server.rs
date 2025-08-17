//! Web server implementation

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
    pub static_files_path: Option<String>,
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
            .route("/", get(handlers::dashboard::index))
            .route("/dashboard", get(handlers::dashboard::index))
            
            // Agent management routes
            .route("/agents", get(handlers::agents::list))
            .route("/agents/:id", get(handlers::agents::detail))
            
            // Issue management routes
            .route("/issues", get(handlers::issues::list))
            .route("/issues/new", get(handlers::issues::new_form))
            .route("/issues", post(handlers::issues::create))
            .route("/issues/:id", get(handlers::issues::detail))
            .route("/issues/:id/edit", get(handlers::issues::edit_form))
            .route("/issues/:id", post(handlers::issues::update))
            
            // Knowledge management routes
            .route("/knowledge", get(handlers::knowledge::list))
            .route("/knowledge/:id", get(handlers::knowledge::detail))
            
            // API routes
            .route("/api/health", get(handlers::api::health))
            .route("/api/stats", get(handlers::api::stats))
            
            // Add shared state
            .with_state(self.storage.clone())
            
            // Add middleware
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive())
            )
    }

    /// Run the web server
    pub async fn run(self) -> Result<()> {
        let app = self.build_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);
        
        tracing::info!("Web server starting on {}", addr);
        
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}