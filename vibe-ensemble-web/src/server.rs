//! Web server implementation

use crate::{auth::AuthService, handlers, websocket::WebSocketManager, Result};
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, serve_dir::ServeDir, trace::TraceLayer};
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
    auth_service: Arc<AuthService>,
    ws_manager: Arc<WebSocketManager>,
}

impl WebServer {
    /// Create a new web server
    pub async fn new(config: WebConfig, storage: Arc<StorageManager>) -> Result<Self> {
        let auth_service = Arc::new(AuthService::new());
        let ws_manager = Arc::new(WebSocketManager::new());
        
        // Start WebSocket background tasks
        ws_manager.start_stats_broadcaster(storage.clone()).await;
        ws_manager.start_ping_sender().await;
        
        Ok(Self { 
            config, 
            storage,
            auth_service,
            ws_manager,
        })
    }

    /// Build the application router
    fn build_router(&self) -> Router {
        // Create public routes (no authentication required)
        let public_routes = Router::new()
            .route("/login", get(crate::auth::login_page))
            .route("/login", post(crate::auth::login_handler))
            .route("/logout", get(crate::auth::logout_handler));

        // Create protected routes (authentication required)
        let protected_routes = Router::new()
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
            .route("/issues/:id", axum::routing::delete(handlers::issues::delete))
            // Knowledge management routes
            .route("/knowledge", get(handlers::knowledge::list))
            .route("/knowledge/:id", get(handlers::knowledge::detail))
            .route("/knowledge/new", get(handlers::knowledge::new_form))
            .route("/knowledge", post(handlers::knowledge::create))
            // System administration routes
            .route("/admin", get(handlers::admin::index))
            .route("/admin/config", get(handlers::admin::config))
            .route("/admin/logs", get(handlers::admin::logs))
            .route("/admin/sessions", get(handlers::admin::sessions))
            // WebSocket route for real-time updates
            .route("/ws", get(crate::websocket::websocket_handler))
            // Authentication middleware for protected routes
            .layer(middleware::from_fn_with_state(
                self.auth_service.clone(),
                crate::auth::auth_middleware,
            ));

        // API routes (may or may not require auth depending on endpoint)
        let api_routes = Router::new()
            .route("/api/health", get(handlers::api::health))
            .route("/api/stats", get(handlers::api::stats))
            .route("/api/agents", get(handlers::api::agents_list))
            .route("/api/agents/:id", get(handlers::api::agent_detail))
            .route("/api/issues", get(handlers::api::issues_list))
            .route("/api/issues", post(handlers::api::issue_create))
            .route("/api/issues/:id", get(handlers::api::issue_detail))
            .route("/api/issues/:id", axum::routing::put(handlers::api::issue_update))
            .route("/api/issues/:id", axum::routing::delete(handlers::api::issue_delete))
            .route("/api/knowledge", get(handlers::api::knowledge_list))
            .route("/api/knowledge/:id", get(handlers::api::knowledge_detail))
            .route("/api/messages", get(handlers::api::messages_list));

        // Combine all routes
        let mut app = Router::new()
            .merge(public_routes)
            .merge(protected_routes)
            .merge(api_routes);

        // Serve static files if path is configured
        if let Some(static_path) = &self.config.static_files_path {
            app = app.nest_service("/static", ServeDir::new(static_path));
        }

        app
            // Add shared state
            .with_state(self.storage.clone())
            .with_state(self.auth_service.clone())
            .with_state(self.ws_manager.clone())
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

        tracing::info!("Web server starting on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}
