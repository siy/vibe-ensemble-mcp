use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Json,
    },
    Json as JsonExtractor,
};
use futures::Stream;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::{events::EventPayload, mcp::types::JsonRpcRequest, server::AppState};

/// SSE and WebSocket event broadcaster for notifying clients about database changes
#[derive(Clone)]
pub struct EventBroadcaster {
    sse_sender: Arc<broadcast::Sender<EventPayload>>,
    websocket_sender: Arc<broadcast::Sender<EventPayload>>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBroadcaster {
    // Increased channel capacity from 512 to 2048
    const BROADCAST_CHANNEL_SIZE: usize = 2048;
    const HEALTH_CHECK_INTERVAL_SECS: u64 = 30;

    pub fn new() -> Self {
        let (sse_sender, _) = broadcast::channel::<EventPayload>(Self::BROADCAST_CHANNEL_SIZE);
        let (websocket_sender, _) =
            broadcast::channel::<EventPayload>(Self::BROADCAST_CHANNEL_SIZE);

        let broadcaster = Self {
            sse_sender: Arc::new(sse_sender),
            websocket_sender: Arc::new(websocket_sender),
        };

        // Spawn health monitoring task
        broadcaster.spawn_health_monitor();

        broadcaster
    }

    /// Spawn a background task to monitor broadcaster health
    fn spawn_health_monitor(&self) {
        let sse_sender = Arc::clone(&self.sse_sender);
        let websocket_sender = Arc::clone(&self.websocket_sender);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(Self::HEALTH_CHECK_INTERVAL_SECS));
            loop {
                interval.tick().await;

                let sse_receivers = sse_sender.receiver_count();
                let websocket_receivers = websocket_sender.receiver_count();

                info!(
                    "EventBroadcaster health: SSE receivers={}, WebSocket receivers={}",
                    sse_receivers, websocket_receivers
                );

                // Warn if approaching capacity
                if sse_receivers > Self::BROADCAST_CHANNEL_SIZE / 2 {
                    warn!(
                        "High SSE receiver count: {}/{} ({}%)",
                        sse_receivers,
                        Self::BROADCAST_CHANNEL_SIZE,
                        (sse_receivers * 100) / Self::BROADCAST_CHANNEL_SIZE
                    );
                }
                if websocket_receivers > Self::BROADCAST_CHANNEL_SIZE / 2 {
                    warn!(
                        "High WebSocket receiver count: {}/{} ({}%)",
                        websocket_receivers,
                        Self::BROADCAST_CHANNEL_SIZE,
                        (websocket_receivers * 100) / Self::BROADCAST_CHANNEL_SIZE
                    );
                }
            }
        });
    }

    /// Broadcast a typed event to all connected SSE and WebSocket clients
    pub fn broadcast(&self, event: EventPayload) {
        use tracing::{info, trace};

        // Log the event being broadcast
        info!(
            "Broadcasting event: type={}, timestamp={}, data={}",
            serde_json::to_string(&event.event_type).unwrap_or_else(|_| "unknown".to_string()),
            event.timestamp,
            serde_json::to_string(&event.data).unwrap_or_else(|_| "{}".to_string())
        );

        // Generate and log the complete JSON-RPC message that will be sent
        let jsonrpc_message = event.to_jsonrpc_notification();
        trace!(
            "Complete JSON-RPC message for event broadcast: {}",
            serde_json::to_string_pretty(&jsonrpc_message)
                .unwrap_or_else(|_| "Failed to serialize JSON-RPC message".to_string())
        );

        // Broadcast to SSE clients
        let sse_result = self.sse_sender.send(event.clone());
        let sse_receiver_count = self.sse_sender.receiver_count();

        if let Err(e) = sse_result {
            debug!("SSE broadcast failed: {}", e);
        } else {
            info!(
                "SSE broadcast successful to {} receivers",
                sse_receiver_count
            );
        }

        // Broadcast to WebSocket clients
        let websocket_result = self.websocket_sender.send(event);
        let websocket_receiver_count = self.websocket_sender.receiver_count();

        if let Err(e) = websocket_result {
            debug!("WebSocket broadcast failed: {}", e);
        } else {
            info!(
                "WebSocket broadcast successful to {} receivers",
                websocket_receiver_count
            );
        }
    }

    /// Create a new receiver for SSE connections
    pub fn subscribe_sse(&self) -> broadcast::Receiver<EventPayload> {
        self.sse_sender.subscribe()
    }

    /// Create a new receiver for WebSocket connections
    pub fn subscribe_websocket(&self) -> broadcast::Receiver<EventPayload> {
        self.websocket_sender.subscribe()
    }

    /// Legacy method for backward compatibility
    pub fn subscribe(&self) -> broadcast::Receiver<EventPayload> {
        self.subscribe_sse()
    }
}

/// SSE endpoint handler that streams MCP-compliant notifications to Claude Code
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let broadcaster = &state.event_broadcaster;

    // Create typed events for initialization
    let host = &state.config.host;
    let port = state.config.port;
    let system_init_event = EventPayload::system_init();
    let endpoint_discovery_event = EventPayload::endpoint_discovery(
        &format!("http://{}:{}/messages", host, port),
        &format!("http://{}:{}/sse", host, port),
    );

    // Create receiver for this SSE connection
    let mut receiver = broadcaster.subscribe_sse();

    let stream = async_stream::stream! {
        // Send initialization events immediately to new clients
        let init_json = system_init_event.to_jsonrpc_notification();
        yield Ok(Event::default()
            .event("message")
            .data(init_json.to_string()));

        let endpoint_json = endpoint_discovery_event.to_jsonrpc_notification();
        yield Ok(Event::default()
            .event("message")
            .data(endpoint_json.to_string()));

        loop {
            match receiver.recv().await {
                Ok(event_payload) => {
                    // Serialize typed event to JSON-RPC at the boundary
                    let mcp_event = event_payload.to_jsonrpc_notification();
                    yield Ok(Event::default()
                        .event("message")
                        .data(mcp_event.to_string()));
                }
                Err(broadcast::error::RecvError::Lagged(skipped_messages)) => {
                    debug!("SSE client lagged, skipped {} messages", skipped_messages);
                    // Send MCP-compliant heartbeat using JSON-RPC envelope helper
                    use crate::mcp::constants::JsonRpcEnvelopes;
                    let heartbeat = JsonRpcEnvelopes::ping();
                    yield Ok(Event::default()
                        .event("message")
                        .data(heartbeat.to_string()));
                }
                Err(_) => break, // Channel closed
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive-mcp"),
    )
}

/// HTTP POST endpoint for receiving messages from Claude Code SSE transport
pub async fn sse_message_handler(
    State(state): State<AppState>,
    JsonExtractor(payload): JsonExtractor<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    debug!("Received SSE message: {}", payload);

    // Parse the JSON as an MCP JsonRpcRequest
    let request: JsonRpcRequest = match serde_json::from_value(payload.clone()) {
        Ok(req) => req,
        Err(e) => {
            use crate::mcp::constants::JsonRpcEnvelopes;
            let error_response = JsonRpcEnvelopes::error_response(
                -32700,
                &format!("Parse error: {}", e),
                payload.get("id").cloned(),
            );
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    };

    // Tool name extraction removed (was only used for SSE echo filtering)

    // Use stored MCP server and handle the request
    let response = state.mcp_server.handle_request(&state, request).await;

    debug!("SSE message processed successfully");

    // Convert the response to JSON
    let response_value = match serde_json::to_value(&response) {
        Ok(val) => val,
        Err(e) => {
            use crate::mcp::constants::JsonRpcEnvelopes;
            let error_response = JsonRpcEnvelopes::error_response(
                -32603,
                &format!("Internal error: {}", e),
                response.id,
            );
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    // MCP response echo removed (redundant infrastructure event) - no processing needed

    Ok(Json(response_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventPayload;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_independent_sse_websocket_broadcasting() {
        // Create broadcaster
        let broadcaster = EventBroadcaster::new();

        // Create receivers for both SSE and WebSocket
        let mut sse_receiver = broadcaster.subscribe_sse();
        let mut websocket_receiver = broadcaster.subscribe_websocket();

        // Create test event
        let test_event = EventPayload::system_init();

        // Broadcast event
        broadcaster.broadcast(test_event.clone());

        // Verify both receivers get the event
        let sse_result = timeout(Duration::from_millis(100), sse_receiver.recv()).await;
        let websocket_result = timeout(Duration::from_millis(100), websocket_receiver.recv()).await;

        assert!(sse_result.is_ok(), "SSE receiver should receive the event");
        assert!(
            websocket_result.is_ok(),
            "WebSocket receiver should receive the event"
        );

        let sse_event = sse_result.unwrap().unwrap();
        let websocket_event = websocket_result.unwrap().unwrap();

        // Events should be identical
        assert_eq!(sse_event.event_type, test_event.event_type);
        assert_eq!(websocket_event.event_type, test_event.event_type);
    }

    #[tokio::test]
    async fn test_sse_only_operation() {
        // Create broadcaster
        let broadcaster = EventBroadcaster::new();

        // Only subscribe to SSE
        let mut sse_receiver = broadcaster.subscribe_sse();

        // Create and broadcast event
        let test_event = EventPayload::system_message("test", "SSE only test", None);
        broadcaster.broadcast(test_event.clone());

        // SSE should receive event
        let result = timeout(Duration::from_millis(100), sse_receiver.recv()).await;
        assert!(result.is_ok(), "SSE receiver should work independently");

        let received_event = result.unwrap().unwrap();
        assert_eq!(received_event.event_type, test_event.event_type);
    }

    #[tokio::test]
    async fn test_websocket_only_operation() {
        // Create broadcaster
        let broadcaster = EventBroadcaster::new();

        // Only subscribe to WebSocket
        let mut websocket_receiver = broadcaster.subscribe_websocket();

        // Create and broadcast event
        let test_event = EventPayload::system_message("test", "WebSocket only test", None);
        broadcaster.broadcast(test_event.clone());

        // WebSocket should receive event
        let result = timeout(Duration::from_millis(100), websocket_receiver.recv()).await;
        assert!(
            result.is_ok(),
            "WebSocket receiver should work independently"
        );

        let received_event = result.unwrap().unwrap();
        assert_eq!(received_event.event_type, test_event.event_type);
    }
}
