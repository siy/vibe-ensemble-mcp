use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::Stream;
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::sync::broadcast;

use crate::server::AppState;

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
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "vibe-ensemble-mcp",
                "version": "0.8.0"
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

    broadcaster.broadcast_event("mcp_notification", init_notification);

    let mut receiver = broadcaster.subscribe();

    let stream = async_stream::stream! {
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
            "event_type": event_type,
            "event_data": event_data,
            "timestamp": chrono::Utc::now().to_rfc3339()
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
            "change_type": change_type,
            "timestamp": chrono::Utc::now().to_rfc3339()
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
            "status": status,
            "timestamp": chrono::Utc::now().to_rfc3339()
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
            "change_type": change_type,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    });
    
    broadcaster.broadcast_event("mcp_notification", mcp_notification);
}
