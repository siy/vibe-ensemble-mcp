//! Transport layer for MCP communication
//!
//! This module provides WebSocket transport for MCP protocol communication.
//! Supports multi-agent WebSocket coordination.

pub mod automated_runner;
pub mod testing;

use crate::{server::McpServer, Error, Result};
use axum::{
    extract::ws::WebSocketUpgrade,
    extract::{Json as JsonExtract, Path},
    http::StatusCode,
    response::Sse,
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// Validate that a message is proper JSON-RPC 2.0 (free function for testing)
pub fn validate_websocket_message(message: &str) -> Result<()> {
    // Validate JSON structure
    let parsed: Value = serde_json::from_str(message)
        .map_err(|e| Error::Transport(format!("Invalid JSON in message: {}", e)))?;

    // Strict JSON-RPC 2.0 validation
    fn ensure_v2(obj: &serde_json::Map<String, Value>) -> Result<()> {
        match obj.get("jsonrpc").and_then(|v| v.as_str()) {
            Some("2.0") => Ok(()),
            _ => Err(Error::Transport(
                "Message must use JSON-RPC 2.0 protocol".to_string(),
            )),
        }
    }

    match &parsed {
        Value::Object(obj) => ensure_v2(obj)?,
        Value::Array(items) => {
            if items.is_empty() {
                return Err(Error::Transport("Batch must not be empty".to_string()));
            }
            for item in items {
                if let Value::Object(obj) = item {
                    ensure_v2(obj)?
                } else {
                    return Err(Error::Transport(
                        "Batch items must be JSON objects".to_string(),
                    ));
                }
            }
        }
        _ => {
            return Err(Error::Transport(
                "JSON-RPC message must be an object or non-empty array".to_string(),
            ));
        }
    }

    debug!("Message validation passed: JSON-RPC 2.0, valid Unicode");
    Ok(())
}

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

/// WebSocket transport for MCP protocol communication
///
/// This implementation provides WebSocket-based transport for multi-agent coordination:
/// - Full JSON-RPC 2.0 compliance over WebSocket frames
/// - Connection lifecycle management with proper error handling
/// - Support for multiple concurrent connections
/// - Automatic reconnection support for clients
/// - MCP protocol initialization state tracking
pub struct WebSocketTransport<S>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + futures_util::Stream<
            Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        > + Send
        + Sync
        + Unpin,
{
    /// WebSocket stream for sending and receiving messages
    websocket: S,
    /// Connection state for MCP initialization sequencing
    connection_state: ConnectionState,
    /// Last initialization request ID for correlation
    last_init_id: Option<Value>,
    /// Read timeout for WebSocket operations
    read_timeout: Duration,
    /// Write timeout for WebSocket operations
    write_timeout: Duration,
    /// Remote address for logging and debugging
    remote_addr: Option<SocketAddr>,
    /// Statistics for connection monitoring
    messages_sent: u64,
    messages_received: u64,
    errors_encountered: u64,
    /// Keep track of message IDs for heartbeat/ping handling
    last_ping_id: Option<Value>,
}

impl<S> WebSocketTransport<S>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + futures_util::Stream<
            Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        > + Send
        + Sync
        + Unpin,
{
    /// Default read timeout for WebSocket operations (30 seconds)
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default write timeout for WebSocket operations (10 seconds)  
    pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

    /// Create a new WebSocket transport with default settings
    pub fn new(websocket: S) -> Self {
        info!(
            "Creating WebSocket transport with default settings (read timeout: {}s, write timeout: {}s)",
            Self::DEFAULT_READ_TIMEOUT.as_secs(),
            Self::DEFAULT_WRITE_TIMEOUT.as_secs()
        );
        Self {
            websocket,
            connection_state: ConnectionState::Uninitialized,
            last_init_id: None,
            read_timeout: Self::DEFAULT_READ_TIMEOUT,
            write_timeout: Self::DEFAULT_WRITE_TIMEOUT,
            remote_addr: None,
            messages_sent: 0,
            messages_received: 0,
            errors_encountered: 0,
            last_ping_id: None,
        }
    }

    /// Create a new WebSocket transport with custom settings and remote address
    pub fn with_config(
        websocket: S,
        read_timeout: Duration,
        write_timeout: Duration,
        remote_addr: Option<SocketAddr>,
    ) -> Self {
        info!(
            "Creating WebSocket transport with custom settings (read timeout: {}s, write timeout: {}s, remote: {:?})",
            read_timeout.as_secs(),
            write_timeout.as_secs(),
            remote_addr
        );
        Self {
            websocket,
            connection_state: ConnectionState::Uninitialized,
            last_init_id: None,
            read_timeout,
            write_timeout,
            remote_addr,
            messages_sent: 0,
            messages_received: 0,
            errors_encountered: 0,
            last_ping_id: None,
        }
    }

    /// Get the current connection state
    pub fn connection_state(&self) -> ConnectionState {
        self.connection_state
    }

    /// Check if transport is ready for MCP protocol operations
    pub fn is_initialized(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Initialized)
    }

    /// Check if transport is closed
    pub fn is_closed(&self) -> bool {
        matches!(self.connection_state, ConnectionState::Closed)
    }

    /// Get connection statistics for monitoring
    pub fn get_stats(&self) -> (u64, u64, u64) {
        (
            self.messages_sent,
            self.messages_received,
            self.errors_encountered,
        )
    }

    /// Get remote address if available
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Check if a message is an MCP initialize request
    fn is_initialize_request(message: &str) -> Result<Option<Value>> {
        let parsed: Value = serde_json::from_str(message)
            .map_err(|e| Error::Transport(format!("Invalid JSON in message: {}", e)))?;

        if let Value::Object(obj) = &parsed {
            if obj.get("method").and_then(|v| v.as_str()) == Some("initialize") {
                return Ok(obj.get("id").cloned());
            }
        }
        Ok(None)
    }

    /// Update connection state based on initialization progress
    async fn update_initialization_state(
        &mut self,
        message: &str,
        is_outgoing: bool,
    ) -> Result<()> {
        // Reuse the initialization logic
        if !is_outgoing {
            if let Some(init_id) = Self::is_initialize_request(message)? {
                match self.connection_state {
                    ConnectionState::Uninitialized | ConnectionState::Initialized => {
                        debug!(
                            "WebSocket incoming initialize request - transitioning to Initializing with ID: {:?}",
                            init_id
                        );
                        self.connection_state = ConnectionState::Initializing;
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Initializing => {
                        warn!("WebSocket incoming initialize while already initializing - updating ID");
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Closed => {
                        return Err(Error::Transport(
                            "Cannot initialize a closed WebSocket connection".to_string(),
                        ));
                    }
                }
                return Ok(());
            }
        }

        if is_outgoing {
            // Check if we're sending an initialize request
            if let Some(init_id) = Self::is_initialize_request(message)? {
                match self.connection_state {
                    ConnectionState::Uninitialized => {
                        debug!(
                            "WebSocket transitioning to Initializing state with request ID: {:?}",
                            init_id
                        );
                        self.connection_state = ConnectionState::Initializing;
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Initializing => {
                        warn!(
                            "WebSocket initialize request while already initializing - updating ID"
                        );
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Initialized => {
                        warn!("WebSocket initialize request after initialization complete - reinitializing");
                        self.connection_state = ConnectionState::Initializing;
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Closed => {
                        return Err(Error::Transport(
                            "Cannot initialize a closed WebSocket connection".to_string(),
                        ));
                    }
                }
            }
            // If we're sending a response to a recorded initialize, finalize state now
            if let Some(success) = self.is_initialize_response(message)? {
                match &self.connection_state {
                    ConnectionState::Initializing => {
                        if success {
                            info!(
                                "WebSocket initialize response sent - connection now initialized"
                            );
                            self.connection_state = ConnectionState::Initialized;
                            self.last_init_id = None;
                        } else {
                            error!("WebSocket initialize error response sent - closing connection");
                            self.connection_state = ConnectionState::Closed;
                            self.last_init_id = None;
                            self.errors_encountered += 1;
                        }
                    }
                    other_state => {
                        warn!(
                            "WebSocket sending initialize response in unexpected state: {:?}",
                            other_state
                        );
                    }
                }
                return Ok(());
            }
        } else {
            // Check if we're receiving an initialize response (client role)
            if let Some(success) = self.is_initialize_response(message)? {
                match &self.connection_state {
                    ConnectionState::Initializing => {
                        if success {
                            info!("WebSocket initialize response received - connection now initialized");
                            self.connection_state = ConnectionState::Initialized;
                            self.last_init_id = None;
                        } else {
                            error!("WebSocket initialize failed - closing connection");
                            self.connection_state = ConnectionState::Closed;
                            self.last_init_id = None;
                        }
                    }
                    other_state => {
                        warn!(
                            "WebSocket received initialize response in unexpected state: {:?}",
                            other_state
                        );
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a message is an MCP initialize response correlating to our request
    /// Returns Ok(Some(true)) for success, Ok(Some(false)) for error, Ok(None) for no match
    fn is_initialize_response(&self, message: &str) -> Result<Option<bool>> {
        if let Some(expected_id) = &self.last_init_id {
            let parsed: Value = serde_json::from_str(message)
                .map_err(|e| Error::Transport(format!("Invalid JSON in message: {}", e)))?;

            if let Value::Object(obj) = &parsed {
                // Check if this is a response with the expected ID
                if let Some(response_id) = obj.get("id") {
                    if response_id == expected_id {
                        // Check if it's a successful initialize response
                        if obj.get("result").is_some() {
                            return Ok(Some(true)); // Success
                        }
                        // Check if it's an initialize error response
                        if let Some(error) = obj.get("error") {
                            warn!("WebSocket initialize request failed: {}", error);
                            return Ok(Some(false)); // Error
                        }
                    }
                }
            }
        }
        Ok(None) // No matching response
    }

    /// Analyze message for ping handling and other metadata
    pub fn analyze_message(&mut self, message: &str) -> Result<()> {
        // Parse message to check for ping - don't fail on malformed JSON
        let parsed: Value = match serde_json::from_str(message) {
            Ok(value) => value,
            Err(e) => {
                debug!(
                    "WebSocket failed to parse JSON in message analysis: {}, continuing",
                    e
                );
                return Ok(());
            }
        };

        if let Value::Object(obj) = &parsed {
            // Check for ping method
            if let Some(method) = obj.get("method").and_then(|v| v.as_str()) {
                if method == "ping" {
                    if let Some(id_val) = obj.get("id") {
                        debug!("WebSocket detected ping message with id: {}", id_val);
                        self.last_ping_id = Some(id_val.clone());
                    }
                }
            }
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl<S> Transport for WebSocketTransport<S>
where
    S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + futures_util::Stream<
            Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
        > + Send
        + Sync
        + Unpin,
{
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed() {
            self.errors_encountered += 1;
            return Err(Error::Transport(
                "WebSocket transport is closed".to_string(),
            ));
        }

        // Validate message before sending (MCP compliance)
        validate_websocket_message(message)?;

        // Update initialization state based on outgoing message
        self.update_initialization_state(message, true).await?;

        // Create send operation with timeout
        let send_operation = async {
            let ws_message = Message::Text(message.to_string());
            self.websocket.send(ws_message).await.map_err(|e| {
                error!("Failed to send WebSocket message: {}", e);
                self.connection_state = ConnectionState::Closed;
                Error::Transport(format!("Failed to send WebSocket message: {}", e))
            })?;

            Ok::<(), Error>(())
        };

        // Apply write timeout
        match timeout(self.write_timeout, send_operation).await {
            Ok(Ok(())) => {
                self.messages_sent += 1;
                debug!(
                    "Successfully sent WebSocket message: {} bytes (total sent: {}, remote: {:?})",
                    message.len(),
                    self.messages_sent,
                    self.remote_addr
                );
                Ok(())
            }
            Ok(Err(e)) => {
                self.errors_encountered += 1;
                error!("WebSocket send operation failed: {}", e);
                Err(e)
            }
            Err(_) => {
                self.errors_encountered += 1;
                error!(
                    "WebSocket send operation timed out after {:?}",
                    self.write_timeout
                );
                self.connection_state = ConnectionState::Closed;
                Err(Error::Timeout(format!(
                    "WebSocket send timeout after {:?}",
                    self.write_timeout
                )))
            }
        }
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed() {
            self.errors_encountered += 1;
            return Err(Error::Transport(
                "WebSocket transport is closed".to_string(),
            ));
        }

        let read_timeout = self.read_timeout;

        // Create receive operation with proper message handling
        let receive_operation = async move {
            loop {
                match self.websocket.next().await {
                    Some(Ok(message)) => {
                        match message {
                            Message::Text(text) => {
                                debug!(
                                    "Received WebSocket text message: {} bytes (remote: {:?})",
                                    text.len(),
                                    self.remote_addr
                                );

                                // Skip empty messages
                                if text.trim().is_empty() {
                                    debug!("Received empty WebSocket message, continuing to read");
                                    continue;
                                }

                                // Validate received message
                                if let Err(e) = validate_websocket_message(&text) {
                                    warn!("WebSocket received invalid message: {}, continuing to read", e);
                                    self.errors_encountered += 1;
                                    // Don't fail hard on invalid messages, just log and continue
                                    continue;
                                }

                                // Update initialization state based on incoming message
                                if let Err(e) = self.update_initialization_state(&text, false).await
                                {
                                    warn!("WebSocket error updating initialization state: {}", e);
                                    // Continue processing the message even if state update fails
                                }

                                self.messages_received += 1;
                                debug!(
                                "Successfully received valid WebSocket message: {} bytes (total received: {})",
                                text.len(), self.messages_received
                            );
                                return Ok(text);
                            }
                            Message::Binary(data) => {
                                debug!(
                                "Received WebSocket binary message: {} bytes, converting to text",
                                data.len()
                            );

                                // Try to convert binary data to UTF-8 string
                                match String::from_utf8(data) {
                                    Ok(text) => {
                                        // Skip empty messages
                                        if text.trim().is_empty() {
                                            debug!("Received empty WebSocket binary message, continuing to read");
                                            continue;
                                        }

                                        // Validate received message
                                        if let Err(e) = validate_websocket_message(&text) {
                                            warn!("WebSocket received invalid binary message: {}, continuing to read", e);
                                            self.errors_encountered += 1;
                                            continue;
                                        }

                                        // Update initialization state
                                        if let Err(e) =
                                            self.update_initialization_state(&text, false).await
                                        {
                                            warn!(
                                                "WebSocket error updating initialization state: {}",
                                                e
                                            );
                                        }

                                        self.messages_received += 1;
                                        debug!(
                                        "Successfully received valid WebSocket binary message: {} bytes (total received: {})",
                                        text.len(), self.messages_received
                                    );
                                        return Ok(text);
                                    }
                                    Err(e) => {
                                        warn!("WebSocket binary message is not valid UTF-8: {}, continuing to read", e);
                                        self.errors_encountered += 1;
                                        continue;
                                    }
                                }
                            }
                            Message::Ping(data) => {
                                debug!("Received WebSocket ping, responding with pong");
                                if let Err(e) = self.websocket.send(Message::Pong(data)).await {
                                    error!("Failed to send WebSocket pong response: {}", e);
                                    self.errors_encountered += 1;
                                }
                                continue;
                            }
                            Message::Pong(_) => {
                                debug!("Received WebSocket pong");
                                continue;
                            }
                            Message::Close(frame) => {
                                if let Some(frame) = frame {
                                    info!(
                                        "WebSocket connection closed by remote: {} - {}",
                                        frame.code, frame.reason
                                    );
                                } else {
                                    info!("WebSocket connection closed by remote");
                                }
                                self.connection_state = ConnectionState::Closed;
                                return Err(Error::Connection(
                                    "WebSocket closed by remote".to_string(),
                                ));
                            }
                            Message::Frame(_) => {
                                debug!("Received raw WebSocket frame, ignoring");
                                continue;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket receive error: {}", e);
                        self.connection_state = ConnectionState::Closed;
                        self.errors_encountered += 1;
                        return Err(Error::Transport(format!("WebSocket receive error: {}", e)));
                    }
                    None => {
                        info!("WebSocket stream ended - connection closed");
                        self.connection_state = ConnectionState::Closed;
                        return Err(Error::Connection("WebSocket stream ended".to_string()));
                    }
                }
            }
        };

        // Apply read timeout
        timeout(read_timeout, receive_operation)
            .await
            .map_err(|_| {
                debug!(
                    "WebSocket read operation timed out after {:?}",
                    read_timeout
                );
                Error::Timeout(format!("WebSocket read timeout after {:?}", read_timeout))
            })?
    }

    async fn close(&mut self) -> Result<()> {
        if !self.is_closed() {
            info!(
                "Closing WebSocket transport (stats: sent={}, received={}, errors={}, remote: {:?})",
                self.messages_sent, self.messages_received, self.errors_encountered, self.remote_addr
            );

            // Send close frame
            if let Err(e) = self.websocket.send(Message::Close(None)).await {
                warn!("Error sending WebSocket close frame: {}", e);
                self.errors_encountered += 1;
            }

            self.connection_state = ConnectionState::Closed;
            self.last_init_id = None;
            info!("WebSocket transport closed gracefully");
        }
        Ok(())
    }
}

/// Connection state for MCP initialization sequencing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection is uninitialized
    Uninitialized,
    /// Connection is in the process of being initialized
    Initializing,
    /// Connection has been successfully initialized
    Initialized,
    /// Connection has been closed
    Closed,
}

/// Transport factory for creating WebSocket and testing transports
/// Simplified for WebSocket-only architecture
pub struct TransportFactory;

impl TransportFactory {
    /// Create a WebSocket transport with default settings
    pub fn websocket<S>(websocket: S) -> Box<dyn Transport>
    where
        S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
            + futures_util::Stream<
                Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
            > + Send
            + Sync
            + Unpin
            + 'static,
    {
        Box::new(WebSocketTransport::new(websocket))
    }

    /// Create a WebSocket transport with custom configuration
    pub fn websocket_with_config<S>(
        websocket: S,
        read_timeout: Duration,
        write_timeout: Duration,
        remote_addr: Option<SocketAddr>,
    ) -> Box<dyn Transport>
    where
        S: futures_util::Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
            + futures_util::Stream<
                Item = std::result::Result<Message, tokio_tungstenite::tungstenite::Error>,
            > + Send
            + Sync
            + Unpin
            + 'static,
    {
        Box::new(WebSocketTransport::with_config(
            websocket,
            read_timeout,
            write_timeout,
            remote_addr,
        ))
    }

    /// Create an in-memory transport pair for testing
    pub fn in_memory_pair() -> (Box<dyn Transport>, Box<dyn Transport>) {
        let (transport1, transport2) = InMemoryTransport::pair();
        (Box::new(transport1), Box::new(transport2))
    }
}

/// SSE session information for MCP protocol
#[derive(Debug, Clone)]
pub struct SseSession {
    /// Session ID
    pub id: String,
    /// Channel sender for sending messages to SSE client
    pub sender: mpsc::UnboundedSender<String>,
}

/// Global SSE session manager for MCP protocol
type SseSessionManager = Arc<RwLock<HashMap<String, SseSession>>>;

/// Multi-transport MCP server supporting WebSocket, HTTP, and SSE
///
/// This provides a multi-agent HTTP server with multiple MCP transports:
/// - WebSocket upgrade at /ws endpoint for persistent connections
/// - HTTP POST endpoint at /mcp for direct JSON-RPC 2.0 requests
/// - Server-Sent Events at /events endpoint for streaming transport
/// - HTTP POST endpoint at /messages/<session_id> for SSE client requests
/// - Health check endpoint at /health for service discovery
/// - Implements auto-discovery port fallback (22360, 22361, 22362, 9090, 8081)
/// - Creates appropriate transports for each connected agent
/// - Manages connection lifecycle and cleanup
/// - Integrates with the existing MCP server architecture
pub struct MultiTransportServer {
    /// Host address to bind to
    host: String,
    /// Preferred port (will fallback if unavailable)
    port: u16,
    /// Read timeout for WebSocket connections
    read_timeout: Duration,
    /// Write timeout for WebSocket connections
    write_timeout: Duration,
}

impl MultiTransportServer {
    /// Port fallback sequence for auto-discovery (in priority order)
    pub const PORT_FALLBACK_SEQUENCE: &'static [u16] = &[22360, 22361, 22362, 9090, 8081];

    /// Default WebSocket server port (first in fallback sequence)
    pub const DEFAULT_PORT: u16 = 22360;

    /// Create a new WebSocket server with default settings
    pub fn new(host: String, port: u16) -> Self {
        Self {
            host,
            port,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
        }
    }

    /// Create a WebSocket server with custom timeout settings
    pub fn with_timeouts(
        host: String,
        port: u16,
        read_timeout: Duration,
        write_timeout: Duration,
    ) -> Self {
        Self {
            host,
            port,
            read_timeout,
            write_timeout,
        }
    }

    /// Get the server's bind address
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Start the HTTP server with WebSocket upgrade and return a channel receiver for new connections
    ///
    /// This implements port fallback (22360, 22361, 22362, 9090, 8081) and creates an HTTP server
    /// with a /ws endpoint that handles WebSocket upgrade requests. Each connection yields a
    /// ready-to-use WebSocket transport that can be used with the MCP server.
    pub async fn start(
        self,
        mcp_server: McpServer,
    ) -> Result<(
        u16,
        mpsc::UnboundedReceiver<std::result::Result<Box<dyn Transport>, Error>>,
    )> {
        use tokio::net::TcpListener;
        use tower::ServiceBuilder;
        use tower_http::cors::CorsLayer;

        let (connection_tx, connection_rx) = mpsc::unbounded_channel();

        // Try ports in fallback sequence
        let mut bound_port = None;
        let mut listener = None;
        let mut bind_attempts = Vec::new();

        // Start with provided port, then try fallback sequence
        let mut ports_to_try = vec![self.port];
        if !Self::PORT_FALLBACK_SEQUENCE.contains(&self.port) {
            ports_to_try.extend_from_slice(Self::PORT_FALLBACK_SEQUENCE);
        } else {
            // If provided port is in sequence, try the full sequence
            ports_to_try = Self::PORT_FALLBACK_SEQUENCE.to_vec();
        }

        for port in ports_to_try {
            let bind_addr = format!("{}:{}", self.host, port);
            match TcpListener::bind(&bind_addr).await {
                Ok(l) => {
                    bound_port = Some(port);
                    listener = Some(l);
                    info!("Successfully bound HTTP server to {}", bind_addr);
                    break;
                }
                Err(e) => {
                    debug!("Failed to bind to {}: {:?}", bind_addr, e.kind());
                    bind_attempts.push((port, e));
                }
            }
        }

        let listener = listener.ok_or_else(|| {
            let attempts_str = bind_attempts
                .iter()
                .map(|(port, err)| format!("{}:{} ({})", self.host, port, err))
                .collect::<Vec<_>>()
                .join(", ");

            warn!(
                "All preferred ports are unavailable. Tried: {}",
                attempts_str
            );
            Error::Transport(format!(
                "Failed to bind HTTP server to any port. Tried: {}",
                attempts_str
            ))
        })?;

        let bound_port = bound_port.unwrap();

        info!(
            "HTTP MCP server with WebSocket upgrade listening on {}:{} (timeouts: read={}s, write={}s)",
            self.host,
            bound_port,
            self.read_timeout.as_secs(),
            self.write_timeout.as_secs()
        );

        // Create the router with WebSocket upgrade endpoint
        let read_timeout = self.read_timeout;
        let write_timeout = self.write_timeout;
        let connection_tx_clone = connection_tx.clone();
        let mcp_server_http = mcp_server.clone();
        let mcp_server_sse = mcp_server.clone();
        let mcp_server_messages = mcp_server.clone();

        // Create SSE session manager
        let sse_sessions: SseSessionManager = Arc::new(RwLock::new(HashMap::new()));
        let sse_sessions_clone = sse_sessions.clone();
        let sse_sessions_messages = sse_sessions.clone();

        let host_port = format!("{}:{}", self.host, self.port);

        let app = Router::new()
            .route(
                "/ws",
                get(move |ws: WebSocketUpgrade| async move {
                    handle_websocket_upgrade(ws, connection_tx_clone, read_timeout, write_timeout)
                        .await
                }),
            )
            .route(
                "/mcp",
                post(move |payload| handle_mcp_http_request(payload, mcp_server_http.clone())),
            )
            .route(
                "/events",
                get(move || handle_sse_connection(mcp_server_sse.clone(), sse_sessions_clone.clone(), host_port.clone())),
            )
            .route(
                "/messages/:session_id",
                post(move |session_id, payload| handle_sse_message_request(session_id, payload, mcp_server_messages.clone(), sse_sessions_messages.clone())),
            )
            .route("/health", get(handle_health_check))
            .layer(
                ServiceBuilder::new()
                    .layer(CorsLayer::permissive()) // Allow cross-origin WebSocket connections
                    .into_inner(),
            );

        // Spawn the HTTP server
        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("HTTP server error: {}", e);
                if connection_tx
                    .send(Err(Error::Transport(format!("HTTP server error: {}", e))))
                    .is_err()
                {
                    debug!("Connection receiver dropped");
                }
            }
            info!("HTTP server loop ended");
        });

        Ok((bound_port, connection_rx))
    }
}

/// Handle WebSocket upgrade from Axum
async fn handle_websocket_upgrade(
    ws: axum::extract::ws::WebSocketUpgrade,
    connection_tx: mpsc::UnboundedSender<std::result::Result<Box<dyn Transport>, Error>>,
    read_timeout: Duration,
    write_timeout: Duration,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| async move {
        debug!("WebSocket upgrade completed for connection");

        // Create a channel-based transport that wraps the Axum WebSocket
        let transport = ChannelWrappedTransport::new(socket, read_timeout, write_timeout);

        if connection_tx.send(Ok(Box::new(transport))).is_err() {
            debug!("Connection receiver dropped, closing connection");
        } else {
            info!("WebSocket connection established and sent to handler");
        }
    })
}

/// Channel-wrapped transport that bridges Axum WebSocket to our Transport trait
/// This avoids Sync trait issues by using channels and spawning a background task
pub struct ChannelWrappedTransport {
    send_tx: mpsc::UnboundedSender<String>,
    recv_rx: mpsc::UnboundedReceiver<std::result::Result<String, Error>>,
    close_tx: Option<mpsc::UnboundedSender<()>>,
    is_closed: bool,
}

impl ChannelWrappedTransport {
    pub fn new(
        socket: axum::extract::ws::WebSocket,
        read_timeout: Duration,
        write_timeout: Duration,
    ) -> Self {
        let (send_tx, send_rx) = mpsc::unbounded_channel::<String>();
        let (recv_tx, recv_rx) = mpsc::unbounded_channel::<std::result::Result<String, Error>>();
        let (close_tx, close_rx) = mpsc::unbounded_channel::<()>();

        // Spawn background task to handle WebSocket operations
        tokio::spawn(handle_axum_websocket_task(
            socket,
            send_rx,
            recv_tx,
            close_rx,
            read_timeout,
            write_timeout,
        ));

        Self {
            send_tx,
            recv_rx,
            close_tx: Some(close_tx),
            is_closed: false,
        }
    }
}

#[async_trait::async_trait]
impl Transport for ChannelWrappedTransport {
    async fn send(&mut self, message: &str) -> Result<()> {
        if self.is_closed {
            return Err(Error::Transport(
                "Channel-wrapped transport is closed".to_string(),
            ));
        }

        // Validate message before sending (MCP compliance)
        validate_websocket_message(message)?;

        self.send_tx
            .send(message.to_string())
            .map_err(|_| Error::Transport("Failed to send message through channel".to_string()))?;

        debug!(
            "Successfully queued message for sending: {} bytes",
            message.len()
        );
        Ok(())
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed {
            return Err(Error::Transport(
                "Channel-wrapped transport is closed".to_string(),
            ));
        }

        match self.recv_rx.recv().await {
            Some(Ok(message)) => {
                debug!(
                    "Successfully received message from channel: {} bytes",
                    message.len()
                );
                Ok(message)
            }
            Some(Err(e)) => {
                self.is_closed = true;
                Err(e)
            }
            None => {
                self.is_closed = true;
                Err(Error::Connection(
                    "WebSocket background task ended".to_string(),
                ))
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        if !self.is_closed {
            info!("Closing channel-wrapped transport");

            if let Some(close_tx) = self.close_tx.take() {
                let _ = close_tx.send(());
            }

            self.is_closed = true;
            info!("Channel-wrapped transport closed");
        }
        Ok(())
    }
}

/// Background task to handle Axum WebSocket operations
async fn handle_axum_websocket_task(
    mut socket: axum::extract::ws::WebSocket,
    mut send_rx: mpsc::UnboundedReceiver<String>,
    recv_tx: mpsc::UnboundedSender<std::result::Result<String, Error>>,
    mut close_rx: mpsc::UnboundedReceiver<()>,
    _read_timeout: Duration,
    _write_timeout: Duration,
) {
    use futures_util::StreamExt;

    loop {
        tokio::select! {
            // Handle outgoing messages
            msg = send_rx.recv() => {
                if let Some(message) = msg {
                    let ws_message = axum::extract::ws::Message::Text(message.clone());
                    if let Err(e) = socket.send(ws_message).await {
                        error!("Failed to send WebSocket message: {}", e);
                        let _ = recv_tx.send(Err(Error::Transport(format!("Send failed: {}", e))));
                        break;
                    }
                    debug!("Sent WebSocket message: {} bytes", message.len());
                } else {
                    debug!("Send channel closed");
                    break;
                }
            }

            // Handle incoming messages
            msg = socket.next() => {
                match msg {
                    Some(Ok(message)) => {
                        match message {
                            axum::extract::ws::Message::Text(text) => {
                                if !text.trim().is_empty() {
                                    if validate_websocket_message(&text).is_ok() {
                                        if recv_tx.send(Ok(text)).is_err() {
                                            debug!("Receive channel closed");
                                            break;
                                        }
                                    } else {
                                        warn!("Received invalid WebSocket message, skipping");
                                    }
                                }
                            }
                            axum::extract::ws::Message::Binary(data) => {
                                if let Ok(text) = String::from_utf8(data) {
                                    if !text.trim().is_empty() && validate_websocket_message(&text).is_ok() && recv_tx.send(Ok(text)).is_err() {
                                        debug!("Receive channel closed");
                                        break;
                                    }
                                }
                            }
                            axum::extract::ws::Message::Ping(data) => {
                                if let Err(e) = socket.send(axum::extract::ws::Message::Pong(data)).await {
                                    error!("Failed to send pong: {}", e);
                                    break;
                                }
                            }
                            axum::extract::ws::Message::Pong(_) => {
                                debug!("Received pong");
                            }
                            axum::extract::ws::Message::Close(frame) => {
                                if let Some(frame) = frame {
                                    info!("WebSocket closed by remote: {} - {}", frame.code, frame.reason);
                                } else {
                                    info!("WebSocket closed by remote");
                                }
                                let _ = recv_tx.send(Err(Error::Connection("WebSocket closed by remote".to_string())));
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {}", e);
                        let _ = recv_tx.send(Err(Error::Transport(format!("WebSocket error: {}", e))));
                        break;
                    }
                    None => {
                        info!("WebSocket stream ended");
                        let _ = recv_tx.send(Err(Error::Connection("WebSocket stream ended".to_string())));
                        break;
                    }
                }
            }

            // Handle close signal
            _ = close_rx.recv() => {
                info!("Received close signal for WebSocket task");
                let _ = socket.send(axum::extract::ws::Message::Close(None)).await;
                break;
            }
        }
    }

    info!("WebSocket background task ended");
}

/// Handle HTTP MCP request (POST /mcp)
async fn handle_mcp_http_request(
    JsonExtract(payload): JsonExtract<Value>,
    mcp_server: McpServer,
) -> std::result::Result<Json<Value>, StatusCode> {
    // Validate JSON-RPC 2.0 format
    if let Err(_e) = validate_mcp_request(&payload) {
        debug!("Invalid MCP HTTP request: {:?}", payload);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Convert payload to string for MCP server processing
    let message = match serde_json::to_string(&payload) {
        Ok(msg) => msg,
        Err(e) => {
            debug!("Failed to serialize MCP request: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Process request through MCP server
    match mcp_server.handle_message(&message).await {
        Ok(Some(response_str)) => {
            // Parse response back to JSON for return
            match serde_json::from_str::<Value>(&response_str) {
                Ok(response_json) => {
                    debug!("HTTP MCP response: {:?}", response_json);
                    Ok(Json(response_json))
                }
                Err(e) => {
                    error!("Failed to parse MCP server response: {}", e);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Ok(None) => {
            // No response needed (notification)
            Ok(Json(serde_json::json!({"result": null})))
        }
        Err(e) => {
            error!("MCP server error: {}", e);
            let error_response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.get("id").cloned().unwrap_or(Value::Null),
                "error": {
                    "code": -32603,
                    "message": "Internal error",
                    "data": e.to_string()
                }
            });
            Ok(Json(error_response))
        }
    }
}

/// Handle SSE connection (GET /events)
/// Implements MCP SSE transport protocol: sends 'endpoint' event first, then 'message' events
async fn handle_sse_connection(
    mcp_server: McpServer,
    sse_sessions: SseSessionManager,
    host_port: String,
) -> Sse<impl futures_util::Stream<Item = std::result::Result<axum::response::sse::Event, std::convert::Infallible>>> {
    use axum::response::sse::Event;
    use futures_util::stream;
    use uuid::Uuid;

    debug!("SSE connection established for MCP protocol");

    // Generate unique session ID for this SSE connection
    let session_id = Uuid::new_v4().to_string();
    let (sender, receiver) = mpsc::unbounded_channel::<String>();

    // Register session in the global manager
    {
        let mut sessions = sse_sessions.write().await;
        sessions.insert(session_id.clone(), SseSession {
            id: session_id.clone(),
            sender: sender.clone(),
        });
    }

    debug!("Created SSE session: {}", session_id);

    // Create stream that follows MCP SSE protocol
    let host_port_clone = host_port.clone();
    let stream = stream::unfold((0, session_id.clone(), mcp_server, receiver, sse_sessions.clone(), host_port_clone), 
        move |(counter, session_id, server, mut receiver, sessions, host_port)| async move {
            if counter == 0 {
                // First event: Send 'endpoint' event with message URL according to MCP SSE spec
                let endpoint_url = format!("http://{}/messages/{}", host_port, session_id);
                let event = Event::default()
                    .event("endpoint")
                    .data(endpoint_url);

                debug!("SSE sending 'endpoint' event for session: {}", session_id);
                Some((Ok(event), (counter + 1, session_id, server, receiver, sessions, host_port)))
            } else {
                // Subsequent events: Wait for messages from the session channel
                // This allows HTTP POST requests to /messages/:session_id to send responses back via SSE
                match receiver.recv().await {
                    Some(message_data) => {
                        let event = Event::default()
                            .event("message")
                            .data(message_data);

                        debug!("SSE sending 'message' event for session: {}", session_id);
                        Some((Ok(event), (counter + 1, session_id, server, receiver, sessions, host_port)))
                    }
                    None => {
                        // Channel closed, clean up session
                        debug!("SSE channel closed for session: {}", session_id);
                        let mut session_map = sessions.write().await;
                        session_map.remove(&session_id);
                        None // End stream
                    }
                }
            }
        });

    Sse::new(stream)
}

/// Handle SSE message request (POST /messages/:session_id)
/// Processes MCP requests from SSE clients and sends responses back via the SSE channel
async fn handle_sse_message_request(
    Path(session_id): Path<String>,
    JsonExtract(payload): JsonExtract<Value>,
    mcp_server: McpServer,
    sse_sessions: SseSessionManager,
) -> std::result::Result<Json<Value>, StatusCode> {
    debug!("SSE message request for session: {}", session_id);

    // Validate JSON-RPC 2.0 format
    if let Err(_e) = validate_mcp_request(&payload) {
        debug!("Invalid MCP SSE request: {:?}", payload);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Convert payload to string for MCP server processing
    let message = match serde_json::to_string(&payload) {
        Ok(msg) => msg,
        Err(e) => {
            debug!("Failed to serialize MCP SSE request: {}", e);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Process request through MCP server
    match mcp_server.handle_message(&message).await {
        Ok(Some(response_str)) => {
            // Send response back through SSE channel
            {
                let sessions = sse_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    if session.sender.send(response_str.clone()).is_err() {
                        debug!("Failed to send response to SSE session: {}", session_id);
                        return Err(StatusCode::GONE); // Session closed
                    }
                } else {
                    debug!("SSE session not found: {}", session_id);
                    return Err(StatusCode::NOT_FOUND);
                }
            }

            // Return acknowledgment to HTTP POST client
            let ack_response = serde_json::json!({
                "status": "sent",
                "session_id": session_id
            });
            debug!("SSE message sent for session: {}", session_id);
            Ok(Json(ack_response))
        }
        Ok(None) => {
            // No response needed (notification)
            let ack_response = serde_json::json!({
                "status": "processed",
                "session_id": session_id
            });
            Ok(Json(ack_response))
        }
        Err(e) => {
            error!("MCP server error for SSE session {}: {}", session_id, e);
            let error_response = serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.get("id").cloned().unwrap_or(Value::Null),
                "error": {
                    "code": -32603,
                    "message": "Internal error",
                    "data": e.to_string()
                }
            });

            // Send error response back through SSE channel
            {
                let sessions = sse_sessions.read().await;
                if let Some(session) = sessions.get(&session_id) {
                    let error_str = serde_json::to_string(&error_response).unwrap_or_else(|_| "{}".to_string());
                    let _ = session.sender.send(error_str);
                }
            }

            Ok(Json(error_response))
        }
    }
}

/// Handle health check (GET /health)
async fn handle_health_check() -> Json<Value> {
    let health_response = serde_json::json!({
        "status": "healthy",
        "service": "vibe-ensemble-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "transports": ["websocket", "http", "sse"],
        "endpoints": {
            "websocket": "/ws",
            "http": "/mcp",
            "sse": "/events",
            "health": "/health"
        }
    });

    debug!("Health check requested");
    Json(health_response)
}

/// Validate MCP JSON-RPC 2.0 request format
fn validate_mcp_request(payload: &Value) -> Result<()> {
    let obj = payload
        .as_object()
        .ok_or_else(|| Error::Transport("Request must be a JSON object".to_string()))?;

    // Validate JSON-RPC 2.0 protocol version
    match obj.get("jsonrpc").and_then(|v| v.as_str()) {
        Some("2.0") => {}
        _ => {
            return Err(Error::Transport(
                "Must use JSON-RPC 2.0 protocol".to_string(),
            ))
        }
    }

    // Validate method exists
    if obj.get("method").is_none() {
        return Err(Error::Transport(
            "Request must include method field".to_string(),
        ));
    }

    // Note: 'id' field is required for requests but MUST NOT be present for notifications
    // Both are valid according to JSON-RPC 2.0 specification

    Ok(())
}

// Type alias for backwards compatibility
pub type WebSocketServer = MultiTransportServer;

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

    #[tokio::test]
    async fn test_message_validation() {
        // Test free function for message validation
        // Valid JSON-RPC 2.0 message
        let valid_message = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        assert!(validate_websocket_message(valid_message).is_ok());

        // Invalid JSON
        let invalid_json = "not json";
        assert!(validate_websocket_message(invalid_json).is_err());

        // Wrong JSON-RPC version
        let wrong_version = r#"{"jsonrpc":"1.0","id":1,"method":"test"}"#;
        assert!(validate_websocket_message(wrong_version).is_err());

        // Message without explicit JSON-RPC version (should fail with strict validation)
        let no_version = r#"{"id":1,"method":"test","params":{}}"#;
        assert!(validate_websocket_message(no_version).is_err());

        // Valid batch request
        let valid_batch = r#"[{"jsonrpc":"2.0","id":1,"method":"test1"},{"jsonrpc":"2.0","id":2,"method":"test2"}]"#;
        assert!(validate_websocket_message(valid_batch).is_ok());

        // Empty batch (should fail)
        let empty_batch = "[]";
        assert!(validate_websocket_message(empty_batch).is_err());

        // Batch with invalid item (should fail)
        let invalid_batch = r#"[{"jsonrpc":"2.0","id":1},"not an object"]"#;
        assert!(validate_websocket_message(invalid_batch).is_err());

        // Non-object/array root (should fail)
        let primitive_root = "\"just a string\"";
        assert!(validate_websocket_message(primitive_root).is_err());
    }

    #[tokio::test]
    async fn test_websocket_constants() {
        // Test some basic WebSocket transport constants
        use std::time::Duration;

        // These are the expected timeout values for the WebSocket transport
        let expected_read_timeout = Duration::from_secs(30);
        let expected_write_timeout = Duration::from_secs(10);

        // Verify they're reasonable durations
        assert!(expected_read_timeout > Duration::from_secs(1));
        assert!(expected_write_timeout > Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_mcp_request_validation() {
        use serde_json::json;

        // Valid MCP request
        let valid_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        });
        assert!(validate_mcp_request(&valid_request).is_ok());

        // Missing jsonrpc
        let no_jsonrpc = json!({
            "id": 1,
            "method": "initialize"
        });
        assert!(validate_mcp_request(&no_jsonrpc).is_err());

        // Wrong jsonrpc version
        let wrong_version = json!({
            "jsonrpc": "1.0",
            "id": 1,
            "method": "initialize"
        });
        assert!(validate_mcp_request(&wrong_version).is_err());

        // Missing method
        let no_method = json!({
            "jsonrpc": "2.0",
            "id": 1
        });
        assert!(validate_mcp_request(&no_method).is_err());

        // Missing id
        let no_id = json!({
            "jsonrpc": "2.0",
            "method": "initialize"
        });
        assert!(validate_mcp_request(&no_id).is_err());

        // Not an object
        let not_object = json!("invalid");
        assert!(validate_mcp_request(&not_object).is_err());
    }

    #[tokio::test]
    async fn test_multi_transport_server_creation() {
        let server = MultiTransportServer::new("127.0.0.1".to_string(), 22360);

        assert_eq!(server.host, "127.0.0.1");
        assert_eq!(server.port, 22360);
        assert_eq!(server.bind_address(), "127.0.0.1:22360");
    }

    #[tokio::test]
    async fn test_transport_server_port_constants() {
        assert_eq!(MultiTransportServer::DEFAULT_PORT, 22360);
        assert_eq!(
            MultiTransportServer::PORT_FALLBACK_SEQUENCE,
            &[22360, 22361, 22362, 9090, 8081]
        );
    }
}
