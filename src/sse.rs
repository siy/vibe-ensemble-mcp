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

/// SSE event broadcaster for notifying clients about database changes
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: Arc<broadcast::Sender<EventPayload>>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel::<EventPayload>(512);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Broadcast a typed event to all connected SSE clients
    pub fn broadcast(&self, event: EventPayload) {
        if let Err(e) = self.sender.send(event) {
            debug!("SSE broadcast failed: {}", e);
        }
    }

    /// Create a new receiver for SSE connections
    pub fn subscribe(&self) -> broadcast::Receiver<EventPayload> {
        self.sender.subscribe()
    }
}

/// SSE endpoint handler that streams MCP-compliant notifications to Claude Code
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let broadcaster = &state.event_broadcaster;

    // Create typed events for initialization
    let host = &state.server_info.host;
    let port = state.server_info.port;
    let system_init_event = EventPayload::system_init();
    let endpoint_discovery_event = EventPayload::endpoint_discovery(
        &format!("http://{}:{}/messages", host, port),
        &format!("http://{}:{}/sse", host, port),
    );

    // Create receiver for this connection
    let mut receiver = broadcaster.subscribe();

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

    // If this is a successful MCP response, we may want to broadcast it
    if let Some(result) = response.result {
        use crate::events::EventPayload;

        let event =
            EventPayload::system_message("mcp_response", "MCP request processed", Some(result));

        state.event_broadcaster.broadcast(event);
    }

    Ok(Json(response_value))
}
