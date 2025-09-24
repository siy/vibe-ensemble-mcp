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
use tracing::debug;

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
    pub fn new() -> Self {
        let (sse_sender, _) = broadcast::channel::<EventPayload>(512);
        let (websocket_sender, _) = broadcast::channel::<EventPayload>(512);
        Self {
            sse_sender: Arc::new(sse_sender),
            websocket_sender: Arc::new(websocket_sender),
        }
    }

    /// Broadcast a typed event to all connected SSE and WebSocket clients
    pub fn broadcast(&self, event: EventPayload) {
        // Broadcast to SSE clients
        if let Err(e) = self.sse_sender.send(event.clone()) {
            debug!("SSE broadcast failed: {}", e);
        }

        // Broadcast to WebSocket clients
        if let Err(e) = self.websocket_sender.send(event) {
            debug!("WebSocket broadcast failed: {}", e);
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

    // Extract tool name for security check if this is a tool call
    let tool_name = if request.method == "tools/call" {
        request
            .params
            .as_ref()
            .and_then(|params| params.get("name"))
            .and_then(|name| name.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

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

    // If this is a successful MCP response, check if it should be broadcast over SSE
    if let Some(result) = response.result {
        let should_broadcast = match &tool_name {
            Some(name) => state.config.sse_echo_allowlist.contains(name),
            None => true, // Allow non-tool requests (like list_tools, initialize, etc.)
        };

        if should_broadcast {
            use crate::events::EventPayload;

            let event =
                EventPayload::system_message("mcp_response", "MCP request processed", Some(result));

            state.event_broadcaster.broadcast(event);
        } else {
            debug!(
                "Skipping SSE broadcast for tool '{}' (not in allowlist)",
                tool_name.unwrap_or_default()
            );
        }
    }

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
        assert!(websocket_result.is_ok(), "WebSocket receiver should receive the event");

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
        assert!(result.is_ok(), "WebSocket receiver should work independently");

        let received_event = result.unwrap().unwrap();
        assert_eq!(received_event.event_type, test_event.event_type);
    }
}
