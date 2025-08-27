//! Transport layer for MCP communication
//!
//! This module provides different transport mechanisms for MCP protocol
//! communication, including WebSocket and in-memory transports.

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin, Stdout};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{accept_async, connect_async, WebSocketStream};
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
    pub async fn connect(
        url: &str,
    ) -> Result<WebSocketTransport<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
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
            return Err(Error::Transport(
                "WebSocket connection is closed".to_string(),
            ));
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
            return Err(Error::Transport(
                "WebSocket connection is closed".to_string(),
            ));
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
                                    return Err(Error::Transport(
                                        "Received non-UTF-8 binary message".to_string(),
                                    ));
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
                            self.websocket
                                .send(Message::Pong(payload))
                                .await
                                .map_err(|e| {
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
            return Err(Error::Transport(
                "In-memory transport is closed".to_string(),
            ));
        }

        self.sender.send(message.to_string()).map_err(|_| {
            Error::Transport("Failed to send message through in-memory transport".to_string())
        })?;

        debug!("Sent message through in-memory transport");
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport(
                "In-memory transport is closed".to_string(),
            ));
        }

        self.receiver.recv().await.ok_or_else(|| {
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

/// Stdio transport implementation for MCP protocol communication over stdin/stdout
pub struct StdioTransport {
    stdin_reader: BufReader<Stdin>,
    stdout: Stdout,
    is_closed: bool,
}

impl StdioTransport {
    /// Create a new stdio transport
    pub fn new() -> Self {
        Self {
            stdin_reader: BufReader::new(tokio::io::stdin()),
            stdout: tokio::io::stdout(),
            is_closed: false,
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed {
            return Err(Error::Transport("Stdio transport is closed".to_string()));
        }

        // Write message followed by newline
        self.stdout
            .write_all(message.as_bytes())
            .await
            .map_err(|e| {
                error!("Failed to write to stdout: {}", e);
                Error::Transport(format!("Failed to write to stdout: {}", e))
            })?;

        self.stdout.write_all(b"\n").await.map_err(|e| {
            error!("Failed to write newline to stdout: {}", e);
            Error::Transport(format!("Failed to write newline to stdout: {}", e))
        })?;

        self.stdout.flush().await.map_err(|e| {
            error!("Failed to flush stdout: {}", e);
            Error::Transport(format!("Failed to flush stdout: {}", e))
        })?;

        debug!("Sent message via stdio: {}", message);
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport("Stdio transport is closed".to_string()));
        }

        let mut line = String::new();
        match self.stdin_reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Stdin reached EOF");
                Err(Error::Connection("Stdin reached EOF".to_string()))
            }
            Ok(_) => {
                // Remove trailing newline
                if line.ends_with('\n') {
                    line.pop();
                    if line.ends_with('\r') {
                        line.pop();
                    }
                }
                debug!("Received message via stdio: {}", line);
                Ok(line)
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                Err(Error::Transport(format!(
                    "Failed to read from stdin: {}",
                    e
                )))
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        self.is_closed = true;
        debug!("Stdio transport closed");
        Ok(())
    }
}

/// SSE transport implementation for MCP protocol communication over Server-Sent Events + HTTP POST
///
/// This transport enables bidirectional MCP communication using:
/// - Server-Sent Events (SSE) for server-to-client messaging
/// - HTTP POST requests for client-to-server messaging
///
/// The transport automatically manages session IDs and provides session recovery
/// capabilities for robust communication in network-unstable environments.
pub struct SseTransport {
    base_url: String,
    session_id: Option<String>,
    client: reqwest::Client,
    is_closed: bool,
}

impl SseTransport {
    /// Create a new SSE transport
    ///
    /// # Arguments
    /// * `base_url` - The base URL of the server to connect to (e.g., "http://localhost:8080")
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            session_id: None,
            client: reqwest::Client::new(),
            is_closed: false,
        }
    }

    /// Initialize connection by generating a unique session ID for communication
    ///
    /// # Returns
    /// The generated session ID that will be used for all subsequent communications
    ///
    /// # Errors
    /// Returns an error if the transport is already closed
    pub async fn connect(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport("SSE transport is closed".to_string()));
        }

        // For now, we will generate a session ID that will be sent in the initial message
        // In a real implementation, this would come from the SSE session_init event
        use uuid::Uuid;
        let session_id = format!("sse-{}", Uuid::new_v4());

        self.session_id = Some(session_id.clone());
        info!("SSE transport initialized with session ID: {}", session_id);

        Ok(session_id)
    }

    /// Helper method to send POST requests, reducing code duplication
    async fn send_post(
        &self,
        session_id: &str,
        json_payload: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let post_url = format!("{}/mcp/sse/{}", self.base_url, session_id);
        debug!("Sending SSE POST message to: {}", post_url);

        self.client
            .post(&post_url)
            .json(json_payload)
            .send()
            .await
            .map_err(|e| Error::Transport(format!("HTTP POST failed: {}", e)))
    }
}

#[async_trait::async_trait]
impl Transport for SseTransport {
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed {
            return Err(Error::Transport("SSE transport is closed".to_string()));
        }

        // For the simplified version, we will connect on first send if needed
        if self.session_id.is_none() {
            self.connect().await?;
        }

        let session_id = self
            .session_id
            .as_ref()
            .ok_or_else(|| Error::Transport("Session ID not set".to_string()))?;

        // Parse message as JSON to send as structured data
        let json_payload: serde_json::Value = serde_json::from_str(message)
            .map_err(|e| Error::Transport(format!("Invalid JSON message: {}", e)))?;

        let response = self.send_post(session_id, &json_payload).await?;

        if !response.status().is_success() {
            // Retry once on 404/410 to self-heal lost sessions
            if (response.status() == reqwest::StatusCode::NOT_FOUND
                || response.status() == reqwest::StatusCode::GONE)
                && self.session_id.is_some()
            {
                warn!("Session {} lost, attempting to reconnect", session_id);
                self.session_id = None;
                self.connect().await?;

                // Retry with new session
                let new_session_id = self
                    .session_id
                    .as_ref()
                    .ok_or_else(|| Error::Transport("Failed to get new session ID".to_string()))?;
                let retry_response = self.send_post(new_session_id, &json_payload).await?;

                if !retry_response.status().is_success() {
                    return Err(Error::Transport(format!(
                        "Retry HTTP POST failed with status: {}",
                        retry_response.status()
                    )));
                }
            } else {
                return Err(Error::Transport(format!(
                    "HTTP POST failed with status: {}",
                    response.status()
                )));
            }
        }

        debug!("SSE POST message sent successfully");
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        // In SSE transport, messages from server are received via the SSE event stream
        // established separately, not through this polling-based receive method.
        // The actual SSE events should be handled by an event listener on the SSE connection.
        Err(Error::Transport(
            "SSE transport does not support synchronous receive - server messages are delivered via SSE event stream".to_string(),
        ))
    }

    async fn close(&mut self) -> Result<()> {
        if !self.is_closed {
            info!("Closing SSE transport");
            self.is_closed = true;
        }
        Ok(())
    }
}

/// Transport factory for creating different transport types
pub struct TransportFactory;

impl TransportFactory {
    /// Create a WebSocket client transport
    pub async fn websocket_client(url: &str) -> Result<Box<dyn Transport>> {
        let transport = WebSocketTransport::<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >::connect(url)
        .await?;
        Ok(Box::new(transport))
    }

    /// Create an SSE client transport
    ///
    /// Note: Connection/session is established lazily on first `send()`. You may
    /// call `connect()` if you need the session_id eagerly, but it's not required.
    pub fn sse_client(base_url: &str) -> Box<dyn Transport> {
        Box::new(SseTransport::new(base_url))
    }

    /// Create a stdio transport
    pub fn stdio() -> Box<dyn Transport> {
        Box::new(StdioTransport::new())
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
