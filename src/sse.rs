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

/// SSE endpoint handler that streams events to Claude Code
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let broadcaster = get_or_create_broadcaster(&state).await;
    let mut receiver = broadcaster.subscribe();
    
    let stream = async_stream::stream! {
        loop {
            match receiver.recv().await {
                Ok(data) => {
                    yield Ok(Event::default().data(data));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Client lagged behind, send a heartbeat
                    yield Ok(Event::default().event("heartbeat").data("ping"));
                }
                Err(_) => break, // Channel closed
            }
        }
    };

    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(30))
                .text("keep-alive-text")
        )
}

/// Get or create the event broadcaster singleton
async fn get_or_create_broadcaster(state: &AppState) -> EventBroadcaster {
    // For simplicity, create a new broadcaster each time
    // In production, you might want to store this in AppState
    let broadcaster = EventBroadcaster::new();
    
    // Send a welcome event
    broadcaster.broadcast_event("connection", json!({
        "message": "Connected to vibe-ensemble event stream",
        "server_info": {
            "host": state.config.host,
            "port": state.config.port
        }
    }));
    
    broadcaster
}

/// Notify about event queue changes
pub async fn notify_event_change(broadcaster: &EventBroadcaster, event_type: &str, event_data: serde_json::Value) {
    broadcaster.broadcast_event(event_type, json!({
        "event_queue_update": true,
        "event_details": event_data
    }));
}

/// Notify about ticket changes
pub async fn notify_ticket_change(broadcaster: &EventBroadcaster, ticket_id: &str, change_type: &str) {
    broadcaster.broadcast_event("ticket_update", json!({
        "ticket_id": ticket_id,
        "change_type": change_type
    }));
}

/// Notify about worker changes
pub async fn notify_worker_change(broadcaster: &EventBroadcaster, worker_id: &str, status: &str) {
    broadcaster.broadcast_event("worker_update", json!({
        "worker_id": worker_id,
        "status": status
    }));
}

/// Notify about queue changes  
pub async fn notify_queue_change(broadcaster: &EventBroadcaster, queue_name: &str, change_type: &str) {
    broadcaster.broadcast_event("queue_update", json!({
        "queue_name": queue_name,
        "change_type": change_type
    }));
}