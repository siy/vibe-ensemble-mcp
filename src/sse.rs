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
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;
use tracing::{debug, info};

use crate::{
    mcp::{server::McpServer, types::JsonRpcRequest},
    server::AppState,
};

const MCP_PROTOCOL_VERSION: &str = "2024-11-05";

/// SSE event broadcaster for notifying clients about database changes
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: Arc<broadcast::Sender<String>>,
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self {
            sender: Arc::new(sender),
        }
    }

    /// Broadcast an event to all connected SSE clients
    pub fn broadcast_event(&self, event_type: &str, data: serde_json::Value) {
        let event_data = json!({
            "type": event_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": data
        });

        let _ = self.sender.send(event_data.to_string());
    }

    /// Broadcast a raw string event to all connected SSE clients
    pub fn broadcast(
        &self,
        event_data: String,
    ) -> Result<usize, tokio::sync::broadcast::error::SendError<String>> {
        self.sender.send(event_data)
    }

    /// Create a new receiver for SSE connections
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}

/// SSE endpoint handler that streams MCP-compliant notifications to Claude Code
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let broadcaster = &state.event_broadcaster;

    // Send MCP protocol initialization notification
    let init_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "serverInfo": {
                "name": "vibe-ensemble-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {},
                "notifications": {
                    "events": true,
                    "tickets": true,
                    "workers": true,
                    "queues": true
                }
            }
        }
    });

    // Send endpoint event for Claude Code SSE transport compatibility
    // Use configured host instead of hardcoded localhost
    let host = &state.server_info.host;
    let port = state.server_info.port;
    let endpoint_event = json!({
        "jsonrpc": "2.0",
        "method": "notifications/message",
        "params": {
            "level": "info",
            "logger": "vibe-ensemble-sse",
            "data": json!({
                "type": "endpoint",
                "uri": format!("http://{}:{}/messages", host, port)
            })
        }
    });

    // Create receiver BEFORE broadcasting to ensure new clients receive all events
    let mut receiver = broadcaster.subscribe();

    // Broadcast events for other existing clients (they won't see these as their receivers are already created)
    broadcaster.broadcast_event("mcp_notification", init_notification.clone());
    broadcaster.broadcast_event("endpoint", endpoint_event.clone());

    let stream = async_stream::stream! {
        // Send initialization message immediately to new clients
        yield Ok(Event::default()
            .event("message")
            .data(init_notification.to_string()));

        // Send endpoint discovery event immediately to new clients
        yield Ok(Event::default()
            .event("message")
            .data(endpoint_event.to_string()));

        loop {
            match receiver.recv().await {
                Ok(data) => {
                    // Wrap events in MCP notification format
                    let mcp_event = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                        // If it's already a proper MCP message, send as-is
                        if parsed.get("jsonrpc").is_some() {
                            data
                        } else {
                            // Wrap non-MCP events in MCP notification format
                            json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/resources/updated",
                                "params": {
                                    "uri": "vibe-ensemble://events",
                                    "event": parsed
                                }
                            }).to_string()
                        }
                    } else {
                        // Fallback for malformed JSON
                        json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/message",
                            "params": {
                                "level": "info",
                                "logger": "vibe-ensemble-sse",
                                "data": data
                            }
                        }).to_string()
                    };

                    yield Ok(Event::default()
                        .event("message")
                        .data(mcp_event));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Send MCP-compliant heartbeat
                    let heartbeat = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/ping",
                        "params": {
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    });
                    yield Ok(Event::default()
                        .event("ping")
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

/// Notify about event queue changes
pub async fn notify_event_change(
    broadcaster: &EventBroadcaster,
    event_type: &str,
    event_data: serde_json::Value,
) {
    let mcp_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": {
            "uri": "vibe-ensemble://events",
            "event": {
                "type": event_type,
                "data": event_data,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        }
    });

    broadcaster.broadcast_event("mcp_notification", mcp_notification);
}

/// Notify about ticket changes
pub async fn notify_ticket_change(
    broadcaster: &EventBroadcaster,
    ticket_id: &str,
    change_type: &str,
) {
    let mcp_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": {
            "uri": format!("vibe-ensemble://tickets/{}", ticket_id),
            "event": {
                "type": "ticket_changed",
                "change_type": change_type,
                "ticket_id": ticket_id,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        }
    });

    broadcaster.broadcast_event("mcp_notification", mcp_notification);
}

/// Notify about worker changes
pub async fn notify_worker_change(broadcaster: &EventBroadcaster, worker_id: &str, status: &str) {
    let mcp_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": {
            "uri": format!("vibe-ensemble://workers/{}", worker_id),
            "event": {
                "type": "worker_changed",
                "worker_id": worker_id,
                "status": status,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        }
    });

    broadcaster.broadcast_event("mcp_notification", mcp_notification);
}

/// Notify about queue changes
pub async fn notify_queue_change(
    broadcaster: &EventBroadcaster,
    queue_name: &str,
    change_type: &str,
) {
    let mcp_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/resources/updated",
        "params": {
            "uri": format!("vibe-ensemble://queues/{}", queue_name),
            "event": {
                "type": "queue_changed",
                "queue_name": queue_name,
                "change_type": change_type,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        }
    });

    broadcaster.broadcast_event("mcp_notification", mcp_notification);
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
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e)
                },
                "id": payload.get("id")
            });
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    };

    // Create MCP server and handle the request
    let mcp_server = McpServer::new();
    let response = mcp_server.handle_request(&state, request).await;

    info!("SSE message processed successfully");

    // Convert the response to JSON
    let response_value = match serde_json::to_value(&response) {
        Ok(val) => val,
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": format!("Internal error: {}", e)
                },
                "id": response.id
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };

    // If this is a successful MCP response, we may want to broadcast it
    if let Some(result) = response.result {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/message",
            "params": {
                "level": "info",
                "logger": "vibe-ensemble-sse",
                "data": result
            }
        });

        if let Err(e) = state.event_broadcaster.broadcast(notification.to_string()) {
            tracing::warn!("Failed to broadcast SSE response: {}", e);
        }
    }

    Ok(Json(response_value))
}
