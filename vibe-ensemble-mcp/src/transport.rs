//! Transport layer for MCP communication

use crate::{Error, Result};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc;

/// Transport trait for MCP communication
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send a message
    async fn send(&mut self, message: &str) -> Result<()>;
    
    /// Receive a message
    async fn receive(&mut self) -> Result<String>;
    
    /// Close the transport
    async fn close(&mut self) -> Result<()>;
}

/// WebSocket transport implementation placeholder
pub struct WebSocketTransport {
    sender: mpsc::UnboundedSender<String>,
    receiver: mpsc::UnboundedReceiver<String>,
}

impl WebSocketTransport {
    /// Create a new WebSocket transport
    pub fn new() -> (Self, mpsc::UnboundedSender<String>, mpsc::UnboundedReceiver<String>) {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();
        
        (
            Self {
                sender: tx1,
                receiver: rx2,
            },
            tx2,
            rx1,
        )
    }
}

#[async_trait::async_trait]
impl Transport for WebSocketTransport {
    async fn send(&mut self, message: &str) -> Result<()> {
        self.sender
            .send(message.to_string())
            .map_err(|_| Error::Transport("Failed to send message".to_string()))?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        self.receiver
            .recv()
            .await
            .ok_or_else(|| Error::Transport("Connection closed".to_string()))
    }

    async fn close(&mut self) -> Result<()> {
        // Close channels - they will be dropped automatically
        Ok(())
    }
}