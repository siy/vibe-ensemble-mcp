//! WebSocket support for real-time updates
//!
//! Provides real-time communication between the web interface and server
//! for live updates of system status, agent activity, and issue changes.

use crate::{auth::Session, Error, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Request, State, WebSocketUpgrade,
    },
    response::Response,
};
use futures_util::{stream::StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use tokio::sync::broadcast;
use uuid::Uuid;
use vibe_ensemble_storage::StorageManager;

/// WebSocket message types sent to clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WebSocketMessage {
    /// System statistics update
    StatsUpdate {
        agents_count: i64,
        issues_count: i64,
        messages_count: i64,
        knowledge_count: i64,
        prompts_count: i64,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Agent status change
    AgentStatusUpdate {
        agent_id: Uuid,
        name: String,
        status: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// New issue created
    IssueCreated {
        issue_id: Uuid,
        title: String,
        priority: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Issue status changed
    IssueStatusUpdate {
        issue_id: Uuid,
        status: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// New message received
    MessageReceived {
        message_id: Uuid,
        from_agent: String,
        to_agent: Option<String>,
        message_type: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Knowledge entry added
    KnowledgeAdded {
        entry_id: Uuid,
        title: String,
        category: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// System health update
    HealthUpdate {
        status: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Keep-alive ping
    Ping {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Pong response
    Pong {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Message sent event
    MessageSent {
        message_id: Uuid,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        message_type: String,
        priority: String,
        content: String,
        correlation_id: Option<Uuid>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Message delivered event
    MessageDelivered {
        message_id: Uuid,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        delivered_at: chrono::DateTime<chrono::Utc>,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    /// Message failed event
    MessageFailed {
        message_id: Uuid,
        error_message: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}

/// WebSocket client connection info
#[derive(Debug, Clone)]
pub struct WebSocketClient {
    pub id: Uuid,
    pub session: Session,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

/// WebSocket manager for handling real-time connections
#[derive(Debug, Clone)]
pub struct WebSocketManager {
    /// Broadcast channel for sending messages to all clients
    sender: broadcast::Sender<WebSocketMessage>,
    /// Connected clients
    clients: Arc<RwLock<HashMap<Uuid, WebSocketClient>>>,
}

impl WebSocketManager {
    /// Create a new WebSocket manager
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1000); // Buffer up to 1000 messages

        Self {
            sender,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get a receiver for WebSocket messages
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.sender.subscribe()
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast(&self, message: WebSocketMessage) -> Result<usize> {
        match self.sender.send(message) {
            Ok(count) => Ok(count),
            Err(broadcast::error::SendError(_)) => Ok(0), // No receivers
        }
    }

    /// Add a connected client
    pub fn add_client(&self, client: WebSocketClient) -> Result<()> {
        let mut clients = self.clients.write().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire clients lock: {}", e))
        })?;
        clients.insert(client.id, client);
        Ok(())
    }

    /// Remove a client
    pub fn remove_client(&self, client_id: Uuid) -> Result<()> {
        let mut clients = self.clients.write().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire clients lock: {}", e))
        })?;
        clients.remove(&client_id);
        Ok(())
    }

    /// Get count of connected clients
    pub fn client_count(&self) -> Result<usize> {
        let clients = self.clients.read().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire clients lock: {}", e))
        })?;
        Ok(clients.len())
    }

    /// Send periodic statistics updates
    pub async fn start_stats_broadcaster(&self, storage: Arc<StorageManager>) {
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

            loop {
                interval.tick().await;

                // Get current statistics
                if let Ok(stats) = storage.stats().await {
                    let message = WebSocketMessage::StatsUpdate {
                        agents_count: stats.agents_count,
                        issues_count: stats.issues_count,
                        messages_count: stats.messages_count,
                        knowledge_count: stats.knowledge_count,
                        prompts_count: stats.prompts_count,
                        timestamp: chrono::Utc::now(),
                    };

                    let _ = sender.send(message);
                }
            }
        });
    }

    /// Send periodic ping messages
    pub async fn start_ping_sender(&self) {
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                let message = WebSocketMessage::Ping {
                    timestamp: chrono::Utc::now(),
                };

                let _ = sender.send(message);
            }
        });
    }
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

/// WebSocket connection handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(ws_manager): State<Arc<WebSocketManager>>,
    request: Request,
) -> Response {
    // Extract session from request extensions (added by auth middleware)
    let session = request
        .extensions()
        .get::<Session>()
        .cloned()
        .unwrap_or_else(|| {
            // Fallback session for unauthenticated connections
            Session::new("anonymous".to_string(), "Anonymous".to_string(), false)
        });

    ws.on_upgrade(move |socket| handle_websocket(socket, ws_manager, session))
}

/// Handle individual WebSocket connection
async fn handle_websocket(socket: WebSocket, ws_manager: Arc<WebSocketManager>, session: Session) {
    let client_id = Uuid::new_v4();
    let client = WebSocketClient {
        id: client_id,
        session: session.clone(),
        connected_at: chrono::Utc::now(),
    };

    // Add client to manager
    if let Err(e) = ws_manager.add_client(client) {
        tracing::error!("Failed to add WebSocket client: {}", e);
        return;
    }

    tracing::info!(
        "WebSocket client connected: {} ({})",
        client_id,
        session.username
    );

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let mut message_receiver = ws_manager.subscribe();

    // Spawn task to forward broadcast messages to this client
    let client_id_copy = client_id;
    let forward_task = tokio::spawn(async move {
        loop {
            match message_receiver.recv().await {
                Ok(message) => {
                    let json_message = match serde_json::to_string(&message) {
                        Ok(json) => json,
                        Err(e) => {
                            tracing::error!("Failed to serialize WebSocket message: {}", e);
                            continue;
                        }
                    };
                    if ws_sender.send(Message::Text(json_message)).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        "WebSocket client {} lagged by {} messages; skipping.",
                        client_id_copy,
                        n
                    );
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Handle incoming messages from client
    let client_id_clone2 = client_id;
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = ws_receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle incoming text messages (e.g., pong responses)
                    if let Ok(message) = serde_json::from_str::<WebSocketMessage>(&text) {
                        match message {
                            WebSocketMessage::Pong { .. } => {
                                // Client responded to ping, connection is healthy
                                tracing::debug!("Received pong from client {}", client_id_clone2);
                            }
                            _ => {
                                tracing::debug!(
                                    "Received message from client {}: {:?}",
                                    client_id_clone2,
                                    message
                                );
                            }
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    tracing::info!("WebSocket client {} disconnected", client_id_clone2);
                    break;
                }
                Ok(Message::Pong(_)) => {
                    tracing::debug!("Received pong from client {}", client_id_clone2);
                }
                Ok(_) => {
                    // Handle other message types if needed
                }
                Err(e) => {
                    tracing::error!("WebSocket error for client {}: {}", client_id_clone2, e);
                    break;
                }
            }
        }
    });

    // Wait for either task to complete (connection closed or error)
    tokio::select! {
        _ = forward_task => {},
        _ = receive_task => {},
    }

    // Clean up: remove client from manager
    if let Err(e) = ws_manager.remove_client(client_id) {
        tracing::error!("Failed to remove WebSocket client {}: {}", client_id, e);
    }

    tracing::info!("WebSocket client {} connection closed", client_id);
}

/// Helper functions for broadcasting specific events
impl WebSocketManager {
    /// Broadcast agent status update
    pub fn broadcast_agent_status(
        &self,
        agent_id: Uuid,
        name: String,
        status: String,
    ) -> Result<()> {
        let message = WebSocketMessage::AgentStatusUpdate {
            agent_id,
            name,
            status,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast new issue created
    pub fn broadcast_issue_created(
        &self,
        issue_id: Uuid,
        title: String,
        priority: String,
    ) -> Result<()> {
        let message = WebSocketMessage::IssueCreated {
            issue_id,
            title,
            priority,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast issue status update
    pub fn broadcast_issue_status(&self, issue_id: Uuid, status: String) -> Result<()> {
        let message = WebSocketMessage::IssueStatusUpdate {
            issue_id,
            status,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast new message received
    pub fn broadcast_message_received(
        &self,
        message_id: Uuid,
        from_agent: String,
        to_agent: Option<String>,
        message_type: String,
    ) -> Result<()> {
        let message = WebSocketMessage::MessageReceived {
            message_id,
            from_agent,
            to_agent,
            message_type,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast new knowledge entry
    pub fn broadcast_knowledge_added(
        &self,
        entry_id: Uuid,
        title: String,
        category: String,
    ) -> Result<()> {
        let message = WebSocketMessage::KnowledgeAdded {
            entry_id,
            title,
            category,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast system health update
    pub fn broadcast_health_update(&self, status: String) -> Result<()> {
        let message = WebSocketMessage::HealthUpdate {
            status,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast message sent event
    #[allow(clippy::too_many_arguments)]
    pub fn broadcast_message_sent(
        &self,
        message_id: Uuid,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        message_type: impl Into<String>,
        priority: impl Into<String>,
        content: impl Into<String>,
        correlation_id: Option<Uuid>,
    ) -> Result<()> {
        let mut content = content.into();
        if content.len() > 4096 {
            content.truncate(4096);
        }
        let message = WebSocketMessage::MessageSent {
            message_id,
            sender_id,
            recipient_id,
            message_type: message_type.into(),
            priority: priority.into(),
            content,
            correlation_id,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast message delivered event
    pub fn broadcast_message_delivered(
        &self,
        message_id: Uuid,
        sender_id: Uuid,
        recipient_id: Option<Uuid>,
        delivered_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<()> {
        let message = WebSocketMessage::MessageDelivered {
            message_id,
            sender_id,
            recipient_id,
            delivered_at,
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }

    /// Broadcast message failed event
    pub fn broadcast_message_failed(
        &self,
        message_id: Uuid,
        error_message: impl Into<String>,
    ) -> Result<()> {
        let message = WebSocketMessage::MessageFailed {
            message_id,
            error_message: error_message.into(),
            timestamp: chrono::Utc::now(),
        };
        self.broadcast(message)?;
        Ok(())
    }
}
