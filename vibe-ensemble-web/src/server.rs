//! Web server for the Vibe Ensemble dashboard

use crate::{csrf::CsrfStore, handlers, middleware, Result};
use axum::{
    middleware as axum_middleware,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use vibe_ensemble_storage::StorageManager;

/// Shared state for web handlers
#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<StorageManager>,
    pub csrf_store: Arc<CsrfStore>,
}

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
    csrf_store: Arc<CsrfStore>,
}

impl WebServer {
    /// Create a new web server
    pub async fn new(config: WebConfig, storage: Arc<StorageManager>) -> Result<Self> {
        let csrf_store = Arc::new(CsrfStore::new());

        Ok(Self {
            config,
            storage,
            csrf_store,
        })
    }

    /// Build the application router (public for testing)
    pub fn build_router(&self) -> Router {
        self.build_router_internal()
    }

    /// Build the application router (internal)
    fn build_router_internal(&self) -> Router {
        let app_state = AppState {
            storage: self.storage.clone(),
            csrf_store: self.csrf_store.clone(),
        };

        Router::new()
            // Dashboard routes
            .route("/", get(handlers::dashboard))
            .route("/dashboard", get(handlers::dashboard))
            .route("/messages", get(handlers::messages_page))
            .route("/link-health", get(handlers::links::link_health_page))
            // Web UI routes
            .route("/agents", get(handlers::agents::list))
            .route("/agents/:id", get(handlers::agents::detail))
            .route("/issues", get(handlers::issues::list))
            .route("/issues/new", get(handlers::issues::new_form))
            .route("/issues", post(handlers::issues::create))
            .route("/issues/:id", get(handlers::issues::detail))
            .route("/issues/:id/edit", get(handlers::issues::edit_form))
            .route("/issues/:id", post(handlers::issues::update))
            .route("/issues/:id/delete", post(handlers::issues::delete))
            .route("/knowledge", get(handlers::knowledge::list))
            .route("/knowledge/search", get(handlers::knowledge::search))
            .route("/knowledge/:id", get(handlers::knowledge::detail))
            // Prompt management routes
            // TODO: Prompt management routes will be implemented in a future update
            // API routes
            .route("/api/health", get(handlers::health))
            .route("/api/stats", get(handlers::system_stats))
            // Agent API routes
            .route("/api/agents", get(handlers::agents_list))
            .route("/api/agents/:id", get(handlers::agent_get))
            .route(
                "/api/agents/:id/terminate",
                post(handlers::agents::terminate),
            )
            // Issue API routes
            .route("/api/issues", get(handlers::issues_list))
            .route("/api/issues", post(handlers::issues_create))
            .route("/api/issues/:id", get(handlers::issue_get))
            .route("/api/issues/:id", put(handlers::issue_update))
            .route("/api/issues/:id", delete(handlers::issue_delete))
            // Message API routes
            .route("/api/messages", get(handlers::messages_list))
            .route(
                "/api/messages/conversations",
                get(handlers::messages_conversations),
            )
            .route("/api/messages/search", get(handlers::messages_search))
            .route("/api/messages/analytics", get(handlers::messages_analytics))
            .route("/api/messages/:id", get(handlers::message_get))
            .route(
                "/api/messages/thread/:correlation_id",
                get(handlers::messages_by_correlation),
            )
            // TODO: Prompt API routes will be implemented in a future update
            // Link validation API routes
            .route(
                "/api/links/health",
                get(handlers::links::link_health_summary),
            )
            .route(
                "/api/links/status",
                get(handlers::links::link_status_details),
            )
            .route("/api/links/validate", get(handlers::links::validate_links))
            .route("/api/links/analytics", get(handlers::links::link_analytics))
            .route("/api/links/auto-repair", get(handlers::links::auto_repair))
            .route(
                "/api/links/:url/repair-suggestions",
                get(handlers::links::repair_suggestions),
            )
            // Add shared state
            .with_state(self.storage.clone())
            // CSRF-protected routes with AppState
            .merge(
                Router::new()
                    .route("/knowledge/new", get(handlers::knowledge::new_form))
                    .route("/knowledge", post(handlers::knowledge::create))
                    .with_state(app_state.clone()),
            )
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
        let app = self.build_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);

        tracing::info!("Web dashboard starting on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
