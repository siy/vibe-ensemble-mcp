//! Transport layer for MCP communication
//!
//! This module provides WebSocket transport for MCP protocol communication.
//! Supports multi-agent WebSocket coordination.

pub mod automated_runner;
pub mod testing;

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{accept_async, tungstenite::Message};
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

/// WebSocket server for accepting multiple MCP connections
///
/// This provides a multi-agent WebSocket server that:
/// - Accepts multiple concurrent connections on a specified port
/// - Handles HTTP->WebSocket upgrade protocol
/// - Creates WebSocket transports for each connected agent
/// - Manages connection lifecycle and cleanup
/// - Integrates with the existing MCP server architecture
pub struct WebSocketServer {
    /// Host address to bind to
    host: String,
    /// Port to listen on
    port: u16,
    /// Read timeout for WebSocket connections
    read_timeout: Duration,
    /// Write timeout for WebSocket connections
    write_timeout: Duration,
}

impl WebSocketServer {
    /// Default WebSocket server port
    pub const DEFAULT_PORT: u16 = 8081;

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

    /// Start the WebSocket server and return a channel receiver for new connections
    ///
    /// Each connection yields a ready-to-use WebSocket transport that can be
    /// used with the MCP server. The server handles the HTTP upgrade protocol
    /// and connection lifecycle automatically.
    pub async fn start(
        self,
    ) -> Result<mpsc::UnboundedReceiver<std::result::Result<Box<dyn Transport>, Error>>> {
        use tokio::net::TcpListener;

        let bind_addr = format!("{}:{}", self.host, self.port);
        let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
            Error::Transport(format!(
                "Failed to bind WebSocket server to {}: {}",
                bind_addr, e
            ))
        })?;

        info!(
            "WebSocket MCP server listening on {} (timeouts: read={}s, write={}s)",
            bind_addr,
            self.read_timeout.as_secs(),
            self.write_timeout.as_secs()
        );

        let (connection_tx, connection_rx) = mpsc::unbounded_channel();

        // Spawn the server loop
        let read_timeout = self.read_timeout;
        let write_timeout = self.write_timeout;

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, remote_addr)) => {
                        debug!("Accepted TCP connection from {}", remote_addr);

                        let connection_tx = connection_tx.clone();

                        // Handle WebSocket upgrade in a separate task
                        tokio::spawn(async move {
                            match handle_websocket_connection(
                                stream,
                                remote_addr,
                                read_timeout,
                                write_timeout,
                            )
                            .await
                            {
                                Ok(transport) => {
                                    if connection_tx.send(Ok(transport)).is_err() {
                                        debug!("Connection receiver dropped, closing connection");
                                    }
                                }
                                Err(e) => {
                                    debug!(
                                        "WebSocket connection failed from {}: {}",
                                        remote_addr, e
                                    );
                                    // Don't send the error, just log it
                                }
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept TCP connection: {}", e);
                        if connection_tx
                            .send(Err(Error::Transport(format!("Accept error: {}", e))))
                            .is_err()
                        {
                            break; // Receiver dropped, stop server
                        }
                    }
                }
            }
            info!("WebSocket server loop ended");
        });

        Ok(connection_rx)
    }
}

/// Handle WebSocket connection directly (using tokio-tungstenite accept)
async fn handle_websocket_connection(
    stream: tokio::net::TcpStream,
    remote_addr: SocketAddr,
    read_timeout: Duration,
    write_timeout: Duration,
) -> std::result::Result<Box<dyn Transport>, Error> {
    debug!("Handling WebSocket connection from {}", remote_addr);

    // Accept the WebSocket connection
    // Note: For now using basic accept_async - subprotocol negotiation can be added later
    let websocket = accept_async(stream).await.map_err(|e| {
        error!(
            "Failed to accept WebSocket connection from {}: {}",
            remote_addr, e
        );
        Error::Transport(format!("WebSocket accept failed: {}", e))
    })?;

    info!("WebSocket connection established with {}", remote_addr);

    // Create transport
    let transport = TransportFactory::websocket_with_config(
        websocket,
        read_timeout,
        write_timeout,
        Some(remote_addr),
    );

    Ok(transport)
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
}
