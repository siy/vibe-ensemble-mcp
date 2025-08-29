//! Web server for the Vibe Ensemble dashboard

use crate::{handlers, middleware, websocket, Result};
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
    ws_manager: Arc<websocket::WebSocketManager>,
}

impl WebServer {
    /// Create a new web server
    pub async fn new(config: WebConfig, storage: Arc<StorageManager>) -> Result<Self> {
        let ws_manager = Arc::new(websocket::WebSocketManager::new());

        // Start background tasks for periodic updates
        ws_manager.start_stats_broadcaster(storage.clone()).await;
        ws_manager.start_ping_sender().await;

        // Start message event bridge
        let server = Self {
            config,
            storage,
            ws_manager,
        };
        server.start_message_event_bridge().await?;

        Ok(server)
    }

    /// Get the WebSocket manager for external access
    pub fn websocket_manager(&self) -> Arc<websocket::WebSocketManager> {
        self.ws_manager.clone()
    }

    /// Start the message event bridge to forward message service events to WebSocket
    async fn start_message_event_bridge(&self) -> Result<()> {
        let ws_manager = self.ws_manager.clone();
        let storage = self.storage.clone();

        // Subscribe to message service events
        let mut message_receiver = storage.message_service().subscribe().await;

        tokio::spawn(async move {
            while let Ok(message_event) = message_receiver.recv().await {
                match message_event.event_type {
                    vibe_ensemble_storage::services::message::MessageEventType::Sent => {
                        if let Err(e) = ws_manager.broadcast_message_sent(
                            message_event.message.id,
                            message_event.message.sender_id,
                            message_event.message.recipient_id,
                            format!("{:?}", message_event.message.message_type),
                            format!("{:?}", message_event.message.metadata.priority),
                            message_event.message.content.clone(),
                            message_event.message.metadata.correlation_id,
                        ) {
                            tracing::warn!("WS broadcast sent failed: {e}");
                        }
                    }
                    vibe_ensemble_storage::services::message::MessageEventType::Delivered => {
                        if let Err(e) = ws_manager.broadcast_message_delivered(
                            message_event.message.id,
                            message_event.message.sender_id,
                            message_event.message.recipient_id,
                            message_event
                                .message
                                .delivered_at
                                .unwrap_or(chrono::Utc::now()),
                        ) {
                            tracing::warn!("WS broadcast delivered failed: {e}");
                        }
                    }
                    vibe_ensemble_storage::services::message::MessageEventType::Failed => {
                        if let Err(e) = ws_manager.broadcast_message_failed(
                            message_event.message.id,
                            "Message delivery failed".to_string(),
                        ) {
                            tracing::warn!("WS broadcast failed failed: {e}");
                        }
                    }
                }
            }
            tracing::info!("Message event bridge stopped (subscribe channel closed)");
        });

        Ok(())
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
            .route("/messages", get(handlers::messages_page))
            .route("/link-health", get(handlers::links::link_health_page))
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
            // Add shared state
            .with_state(self.storage.clone())
            // WebSocket route needs separate router with different state
            .merge(
                Router::new()
                    .route("/ws", get(websocket::websocket_handler))
                    .with_state(self.ws_manager.clone()),
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
