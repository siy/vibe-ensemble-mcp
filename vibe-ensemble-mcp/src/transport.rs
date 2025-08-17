//! Transport layer for MCP communication
//!
//! This module provides different transport mechanisms for MCP protocol
//! communication, including WebSocket and in-memory transports.

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, accept_async, WebSocketStream};
use tokio_tungstenite::tungstenite::protocol::Message;
use tracing::{debug, error, info, warn};

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

/// WebSocket transport implementation for real-time MCP communication
pub struct WebSocketTransport<S> {
    websocket: WebSocketStream<S>,
    is_closed: bool,
}

impl<S> WebSocketTransport<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    /// Create a new WebSocket transport from an established connection
    pub fn new(websocket: WebSocketStream<S>) -> Self {
        Self {
            websocket,
            is_closed: false,
        }
    }

    /// Create a WebSocket transport by connecting to a URL (client)
    pub async fn connect(url: &str) -> Result<WebSocketTransport<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
        let (websocket, _) = connect_async(url).await.map_err(|e| {
            error!("Failed to connect to WebSocket: {}", e);
            Error::Transport(format!("WebSocket connection failed: {}", e))
        })?;

        info!("WebSocket client connected to: {}", url);
        Ok(WebSocketTransport::new(websocket))
    }

    /// Create a WebSocket transport from an incoming connection (server)
    pub async fn accept(stream: S) -> Result<Self> {
        let websocket = accept_async(stream).await.map_err(|e| {
            error!("Failed to accept WebSocket connection: {}", e);
            Error::Transport(format!("WebSocket accept failed: {}", e))
        })?;

        info!("WebSocket server accepted connection");
        Ok(WebSocketTransport::new(websocket))
    }
}

#[async_trait::async_trait]
impl<S> Transport for WebSocketTransport<S>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + Sync,
{
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed {
            return Err(Error::Transport("WebSocket connection is closed".to_string()));
        }

        debug!("Sending WebSocket message: {}", message);
        
        self.websocket
            .send(Message::Text(message.to_string()))
            .await
            .map_err(|e| {
                error!("Failed to send WebSocket message: {}", e);
                Error::Transport(format!("Failed to send WebSocket message: {}", e))
            })?;

        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport("WebSocket connection is closed".to_string()));
        }

        loop {
            match self.websocket.next().await {
                Some(Ok(message)) => {
                    match message {
                        Message::Text(text) => {
                            debug!("Received WebSocket text message");
                            return Ok(text);
                        }
                        Message::Binary(data) => {
                            // Try to convert binary to text
                            match String::from_utf8(data) {
                                Ok(text) => {
                                    debug!("Received WebSocket binary message (converted to text)");
                                    return Ok(text);
                                }
                                Err(e) => {
                                    warn!("Received binary message that couldn't be converted to UTF-8: {}", e);
                                    return Err(Error::Transport("Received non-UTF-8 binary message".to_string()));
                                }
                            }
                        }
                        Message::Close(frame) => {
                            info!("WebSocket connection closed by peer: {:?}", frame);
                            self.is_closed = true;
                            return Err(Error::Connection("Connection closed by peer".to_string()));
                        }
                        Message::Ping(payload) => {
                            debug!("Received WebSocket ping, sending pong");
                            self.websocket.send(Message::Pong(payload)).await.map_err(|e| {
                                Error::Transport(format!("Failed to send pong: {}", e))
                            })?;
                            // Continue loop to get the next message
                        }
                        Message::Pong(_) => {
                            debug!("Received WebSocket pong");
                            // Continue loop to get the next message
                        }
                        Message::Frame(_) => {
                            // Raw frames should be handled by the underlying library
                            warn!("Received unexpected raw WebSocket frame");
                            // Continue loop to get the next message
                        }
                    }
                }
                Some(Err(e)) => {
                    error!("WebSocket error: {}", e);
                    self.is_closed = true;
                    return Err(Error::Transport(format!("WebSocket error: {}", e)));
                }
                None => {
                    info!("WebSocket stream ended");
                    self.is_closed = true;
                    return Err(Error::Connection("WebSocket stream ended".to_string()));
                }
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        if !self.is_closed {
            info!("Closing WebSocket connection");
            if let Err(e) = self.websocket.send(Message::Close(None)).await {
                warn!("Error sending close frame: {}", e);
            }
            if let Err(e) = self.websocket.close(None).await {
                warn!("Error closing WebSocket: {}", e);
            }
            self.is_closed = true;
        }
        Ok(())
    }
}

/// In-memory transport for testing and local communication
pub struct InMemoryTransport {
    sender: mpsc::UnboundedSender<String>,
    receiver: mpsc::UnboundedReceiver<String>,
    is_closed: bool,
}

impl InMemoryTransport {
    /// Create a pair of connected in-memory transports
    pub fn pair() -> (Self, Self) {
        let (tx1, rx1) = mpsc::unbounded_channel();
        let (tx2, rx2) = mpsc::unbounded_channel();
        
        (
            Self {
                sender: tx1,
                receiver: rx2,
                is_closed: false,
            },
            Self {
                sender: tx2,
                receiver: rx1,
                is_closed: false,
            },
        )
    }
}

#[async_trait::async_trait]
impl Transport for InMemoryTransport {
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed {
            return Err(Error::Transport("In-memory transport is closed".to_string()));
        }

        self.sender
            .send(message.to_string())
            .map_err(|_| Error::Transport("Failed to send message through in-memory transport".to_string()))?;
        
        debug!("Sent message through in-memory transport");
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport("In-memory transport is closed".to_string()));
        }

        self.receiver
            .recv()
            .await
            .ok_or_else(|| {
                self.is_closed = true;
                Error::Connection("In-memory transport connection closed".to_string())
            })
    }

    async fn close(&mut self) -> Result<()> {
        self.is_closed = true;
        debug!("In-memory transport closed");
        Ok(())
    }
}

/// Transport factory for creating different transport types
pub struct TransportFactory;

impl TransportFactory {
    /// Create a WebSocket client transport
    pub async fn websocket_client(url: &str) -> Result<Box<dyn Transport>> {
        let transport = WebSocketTransport::<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>::connect(url).await?;
        Ok(Box::new(transport))
    }

    /// Create an in-memory transport pair for testing
    pub fn in_memory_pair() -> (Box<dyn Transport>, Box<dyn Transport>) {
        let (transport1, transport2) = InMemoryTransport::pair();
        (Box::new(transport1), Box::new(transport2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_transport() {
        let (mut transport1, mut transport2) = InMemoryTransport::pair();

        // Test sending from transport1 to transport2
        transport1.send("Hello").await.unwrap();
        let received = transport2.receive().await.unwrap();
        assert_eq!(received, "Hello");

        // Test sending from transport2 to transport1
        transport2.send("World").await.unwrap();
        let received = transport1.receive().await.unwrap();
        assert_eq!(received, "World");

        // Test closing
        transport1.close().await.unwrap();
        assert!(transport1.send("Should fail").await.is_err());
    }

    #[tokio::test]
    async fn test_transport_factory() {
        let (mut transport1, mut transport2) = TransportFactory::in_memory_pair();

        transport1.send("Factory test").await.unwrap();
        let received = transport2.receive().await.unwrap();
        assert_eq!(received, "Factory test");
    }
}