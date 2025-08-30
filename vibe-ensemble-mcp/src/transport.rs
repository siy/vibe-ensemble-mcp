//! Transport layer for MCP communication
//!
//! This module provides different transport mechanisms for MCP protocol
//! communication, including WebSocket and in-memory transports.

use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter, Stdin, Stdout};
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};
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
                                    warn!("Received binary message that couldn't be converted to Unicode: {}", e);
                                    return Err(Error::Transport(
                                        "Received non-Unicode binary message".to_string(),
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

/// Enhanced stdio transport implementation for MCP protocol with full Claude Code compatibility
///
/// This implementation is optimized for Claude Code compatibility and includes:
/// - JSON-RPC 2.0 message validation with strict MCP compliance
/// - Newline-delimited message framing per MCP specification
/// - Unicode string handling (all Rust strings are UTF-8)
/// - Signal handling for graceful shutdown (SIGINT/SIGTERM)
/// - Performance-optimized buffering with configurable sizes
/// - Robust error handling and connection recovery
/// - Initialization state management with request/response correlation
pub struct StdioTransport {
    stdin_reader: BufReader<Stdin>,
    stdout_writer: BufWriter<Stdout>,
    connection_state: ConnectionState,
    last_init_id: Option<Value>,
    read_timeout: Duration,
    write_timeout: Duration,
    #[cfg_attr(not(test), allow(dead_code))]
    // Used for configuration tracking and potential future features
    buffer_size: usize,
    /// Keep track of message IDs for heartbeat/ping handling
    last_ping_id: Option<String>,
    /// Statistics for connection monitoring
    messages_sent: u64,
    messages_received: u64,
    errors_encountered: u64,
}

impl StdioTransport {
    /// Default read timeout for stdio operations (30 seconds)
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(30);

    /// Default write timeout for stdio operations (10 seconds)
    pub const DEFAULT_WRITE_TIMEOUT: Duration = Duration::from_secs(10);

    /// Default buffer size for stdio operations (64KB)
    pub const DEFAULT_BUFFER_SIZE: usize = 64 * 1024;

    /// Create a new stdio transport with default settings
    pub fn new() -> Self {
        info!("Creating stdio transport with default settings (buffer: {}KB, read timeout: {}s, write timeout: {}s)",
               Self::DEFAULT_BUFFER_SIZE / 1024,
               Self::DEFAULT_READ_TIMEOUT.as_secs(),
               Self::DEFAULT_WRITE_TIMEOUT.as_secs());
        Self {
            stdin_reader: BufReader::with_capacity(Self::DEFAULT_BUFFER_SIZE, tokio::io::stdin()),
            stdout_writer: BufWriter::with_capacity(Self::DEFAULT_BUFFER_SIZE, tokio::io::stdout()),
            connection_state: ConnectionState::Uninitialized,
            last_init_id: None,
            read_timeout: Self::DEFAULT_READ_TIMEOUT,
            write_timeout: Self::DEFAULT_WRITE_TIMEOUT,
            buffer_size: Self::DEFAULT_BUFFER_SIZE,
            last_ping_id: None,
            messages_sent: 0,
            messages_received: 0,
            errors_encountered: 0,
        }
    }

    /// Create a new stdio transport with custom settings
    pub fn with_config(
        read_timeout: Duration,
        write_timeout: Duration,
        buffer_size: usize,
    ) -> Self {
        // Ensure minimum buffer size of 4KB for reasonable performance
        let clamped_buffer_size = buffer_size.max(4096);
        info!("Creating stdio transport with custom settings (buffer: {}KB, read timeout: {}s, write timeout: {}s)",
               clamped_buffer_size / 1024,
               read_timeout.as_secs(),
               write_timeout.as_secs());
        Self {
            stdin_reader: BufReader::with_capacity(clamped_buffer_size, tokio::io::stdin()),
            stdout_writer: BufWriter::with_capacity(clamped_buffer_size, tokio::io::stdout()),
            connection_state: ConnectionState::Uninitialized,
            last_init_id: None,
            read_timeout,
            write_timeout,
            buffer_size: clamped_buffer_size,
            last_ping_id: None,
            messages_sent: 0,
            messages_received: 0,
            errors_encountered: 0,
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
                            warn!("Initialize request failed: {}", error);
                            return Ok(Some(false)); // Error
                        }
                    }
                }
            }
        }
        Ok(None) // No matching response
    }

    /// Update connection state based on initialization progress
    async fn update_initialization_state(
        &mut self,
        message: &str,
        is_outgoing: bool,
    ) -> Result<()> {
        if is_outgoing {
            // Check if we're sending an initialize request
            if let Some(init_id) = Self::is_initialize_request(message)? {
                match self.connection_state {
                    ConnectionState::Uninitialized => {
                        debug!(
                            "Transitioning to Initializing state with request ID: {:?}",
                            init_id
                        );
                        self.connection_state = ConnectionState::Initializing;
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Initializing => {
                        warn!(
                            "Received initialize request while already initializing - updating ID"
                        );
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Initialized => {
                        warn!("Received initialize request after initialization complete - reinitializing");
                        self.connection_state = ConnectionState::Initializing;
                        self.last_init_id = Some(init_id);
                    }
                    ConnectionState::Closed => {
                        return Err(Error::Transport(
                            "Cannot initialize a closed connection".to_string(),
                        ));
                    }
                }
            }
        } else {
            // Check if we're receiving an initialize response
            if let Some(success) = self.is_initialize_response(message)? {
                match &self.connection_state {
                    ConnectionState::Initializing => {
                        if success {
                            info!("Initialize response received - connection now initialized");
                            self.connection_state = ConnectionState::Initialized;
                            self.last_init_id = None;
                        } else {
                            error!("Initialize failed - closing connection");
                            self.connection_state = ConnectionState::Closed;
                            self.last_init_id = None;
                        }
                    }
                    other_state => {
                        warn!(
                            "Received initialize response in unexpected state: {:?}",
                            other_state
                        );
                    }
                }
            }
        }
        Ok(())
    }

    /// Detect if a message contains ping for heartbeat handling
    pub fn analyze_message(&mut self, message: &str) -> Result<()> {
        // Parse message to check for ping
        let parsed: Value = serde_json::from_str(message)
            .map_err(|e| Error::Transport(format!("Invalid JSON in message analysis: {}", e)))?;

        if let Value::Object(obj) = &parsed {
            // Check for ping method
            if let Some(method) = obj.get("method").and_then(|v| v.as_str()) {
                if method == "ping" {
                    if let Some(id_val) = obj.get("id") {
                        let id_str = match id_val {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Null => "null".to_string(),
                            other => other.to_string(),
                        };
                        debug!("Detected ping message with id: {}", id_str);
                        self.last_ping_id = Some(id_str);
                    }
                }
            }
        }

        Ok(())
    }

    /// Create a ping message for connection health checking
    pub fn create_ping_message(&mut self) -> String {
        use uuid::Uuid;
        let ping_id = Uuid::new_v4().to_string();
        self.last_ping_id = Some(ping_id.clone());

        format!(r#"{{"jsonrpc":"2.0","method":"ping","id":"{}"}}"#, ping_id)
    }

    /// Create a pong response message for responding to pings
    pub fn create_pong_message(&self, ping_id: &Value) -> String {
        format!(r#"{{"jsonrpc":"2.0","result":"pong","id":{}}}"#, ping_id)
    }

    /// Validate that a message is proper JSON-RPC and doesn't contain embedded newlines
    /// Strict per JSON-RPC 2.0: root must be Object or Array; every object must have "jsonrpc":"2.0".
    #[doc(hidden)]
    pub fn validate_message(message: &str) -> Result<()> {
        // Check for embedded newlines (MCP requirement)
        if message.contains('\n') || message.contains('\r') {
            return Err(Error::Transport(
                "Message contains embedded newlines, which violates MCP stdio transport requirements".to_string()
            ));
        }

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

        debug!("Message validation passed: JSON-RPC 2.0, no embedded newlines, valid Unicode");
        Ok(())
    }

    /// Wait until a shutdown signal is received (SIGINT/SIGTERM)
    async fn check_shutdown_signal() {
        tokio::select! {
            _ = signal::ctrl_c() => {
                info!("Received SIGINT (Ctrl+C), initiating graceful shutdown");
            }
            _ = Self::wait_for_sigterm() => {
                info!("Received SIGTERM, initiating graceful shutdown");
            }
        }
    }

    /// Wait for SIGTERM signal (Unix-like systems)
    #[cfg(any(unix, test))]
    #[doc(hidden)]
    pub async fn wait_for_sigterm() {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            sigterm.recv().await;
        }
    }

    /// Wait for SIGTERM signal (Windows - no-op as SIGTERM doesn't exist)
    #[cfg(all(not(unix), not(test)))]
    #[doc(hidden)]
    pub async fn wait_for_sigterm() {
        // On Windows, we only handle Ctrl+C
        std::future::pending::<()>().await;
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
        if self.is_closed() {
            self.errors_encountered += 1;
            return Err(Error::Transport("Stdio transport is closed".to_string()));
        }

        // Validate message before sending (Claude Code compatibility)
        Self::validate_message(message)?;

        // Update initialization state based on outgoing message
        self.update_initialization_state(message, true).await?;

        // Create write operation with timeout
        let write_operation = async {
            // Write message as UTF-8 bytes
            self.stdout_writer
                .write_all(message.as_bytes())
                .await
                .map_err(|e| {
                    error!("Failed to write message to stdout: {}", e);
                    self.connection_state = ConnectionState::Closed;
                    Error::Transport(format!("Failed to write to stdout: {}", e))
                })?;

            // Add newline delimiter (MCP requirement)
            self.stdout_writer.write_all(b"\n").await.map_err(|e| {
                error!("Failed to write newline delimiter to stdout: {}", e);
                self.connection_state = ConnectionState::Closed;
                Error::Transport(format!("Failed to write newline to stdout: {}", e))
            })?;

            // Ensure data is written to the underlying stream
            self.stdout_writer.flush().await.map_err(|e| {
                error!("Failed to flush stdout buffer: {}", e);
                Error::Transport(format!("Failed to flush stdout: {}", e))
            })?;

            Ok::<(), Error>(())
        };

        // Apply write timeout
        match timeout(self.write_timeout, write_operation).await {
            Ok(Ok(())) => {
                self.messages_sent += 1;
                debug!(
                    "Successfully sent message via stdio: {} bytes (total sent: {})",
                    message.len(),
                    self.messages_sent
                );
                Ok(())
            }
            Ok(Err(e)) => {
                self.errors_encountered += 1;
                error!("Write operation failed: {}", e);
                Err(e)
            }
            Err(_) => {
                self.errors_encountered += 1;
                error!("Write operation timed out after {:?}", self.write_timeout);
                self.connection_state = ConnectionState::Closed;
                Err(Error::Timeout(format!(
                    "Write timeout after {:?}",
                    self.write_timeout
                )))
            }
        }
    }

    async fn receive(&mut self) -> Result<String> {
        if self.is_closed() {
            self.errors_encountered += 1;
            return Err(Error::Transport("Stdio transport is closed".to_string()));
        }

        let read_timeout = self.read_timeout;

        // Create read operation with signal handling
        let mut line = String::new(); // Reuse buffer across iterations for better performance
        let read_operation = async move {
            loop {
                line.clear(); // Clear but keep allocated capacity

                tokio::select! {
                    result = self.stdin_reader.read_line(&mut line) => {
                        match result {
                            Ok(0) => {
                                info!("Stdin reached EOF - client disconnected, closing transport");
                                self.connection_state = ConnectionState::Closed;
                                self.errors_encountered += 1;
                                return Err(Error::Connection("Stdin reached EOF".to_string()));
                            }
                            Ok(bytes_read) => {
                                debug!("Read {} bytes from stdin", bytes_read);

                                // Remove newline delimiter (MCP requirement)
                                if line.ends_with('\n') {
                                    line.pop();
                                    if line.ends_with('\r') {
                                        line.pop(); // Handle Windows CRLF
                                    }
                                }

                                // Skip empty lines (keep-alive or malformed)
                                if line.trim().is_empty() {
                                    debug!("Received empty line, continuing to read");
                                    continue;
                                }

                                // Validate received message
                                if let Err(e) = Self::validate_message(&line) {
                                    warn!("Received invalid message: {}, continuing to read", e);
                                    self.errors_encountered += 1;
                                    // Don't fail hard on invalid messages, just log and continue
                                    // This provides better resilience against malformed input
                                    continue;
                                }

                                // Update initialization state based on incoming message
                                if let Err(e) = self.update_initialization_state(&line, false).await {
                                    warn!("Error updating initialization state: {}", e);
                                    // Continue processing the message even if state update fails
                                }

                                self.messages_received += 1;
                                debug!("Successfully received valid message: {} bytes (total received: {})",
                                       line.len(), self.messages_received);
                                return Ok(line);
                            }
                            Err(e) => {
                                error!("Failed to read from stdin: {}", e);

                                // Check if it's a recoverable error
                                match e.kind() {
                                    std::io::ErrorKind::Interrupted => {
                                        debug!("Read interrupted, retrying");
                                        continue;
                                    }
                                    std::io::ErrorKind::UnexpectedEof => {
                                        info!("Unexpected EOF on stdin, closing transport");
                                        self.connection_state = ConnectionState::Closed;
                                        self.errors_encountered += 1;
                                        return Err(Error::Connection("Unexpected EOF".to_string()));
                                    }
                                    _ => {
                                        return Err(Error::Transport(format!("Failed to read from stdin: {}", e)));
                                    }
                                }
                            }
                        }
                    }
                    _ = Self::check_shutdown_signal() => {
                        info!("Graceful shutdown initiated via signal");
                        self.connection_state = ConnectionState::Closed;
                        return Err(Error::Connection("Shutdown signal received".to_string()));
                    }
                }
            }
        };

        // Apply read timeout
        timeout(read_timeout, read_operation).await.map_err(|_| {
            debug!("Read operation timed out after {:?}", read_timeout);
            Error::Timeout(format!("Read timeout after {:?}", read_timeout))
        })?
    }

    async fn close(&mut self) -> Result<()> {
        if !self.is_closed() {
            info!(
                "Closing stdio transport - flushing buffers (stats: sent={}, received={}, errors={})",
                self.messages_sent, self.messages_received, self.errors_encountered
            );

            // Ensure all buffered data is written before closing
            if let Err(e) = self.stdout_writer.flush().await {
                warn!("Error flushing stdout buffer during close: {}", e);
                self.errors_encountered += 1;
            }

            // Shutdown stdout to signal end of communication
            if let Err(e) = self.stdout_writer.shutdown().await {
                warn!("Error shutting down stdout during close: {}", e);
                self.errors_encountered += 1;
            }

            self.connection_state = ConnectionState::Closed;
            self.last_init_id = None;
            info!("Stdio transport closed gracefully");
        }
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

    /// Create an SSE client transport.
    ///
    /// Note: Connection/session is established lazily on first `send()`. You may
    /// call `connect()` if you need the session_id eagerly, but it's not required.
    /// Call `connect()` on the transport to establish the SSE connection if needed.
    pub fn sse_client(base_url: &str) -> Box<dyn Transport> {
        Box::new(SseTransport::new(base_url))
    }

    /// Create a stdio transport with default settings
    pub fn stdio() -> Box<dyn Transport> {
        Box::new(StdioTransport::new())
    }

    /// Create a stdio transport with custom configuration for performance tuning
    pub fn stdio_with_config(
        read_timeout: Duration,
        write_timeout: Duration,
        buffer_size: usize,
    ) -> Box<dyn Transport> {
        Box::new(StdioTransport::with_config(
            read_timeout,
            write_timeout,
            buffer_size,
        ))
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

    #[tokio::test]
    async fn test_stdio_transport_message_validation() {
        // Test message validation function directly

        // Valid JSON-RPC 2.0 message
        let valid_message = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        assert!(StdioTransport::validate_message(valid_message).is_ok());

        // Invalid JSON
        let invalid_json = "not json";
        assert!(StdioTransport::validate_message(invalid_json).is_err());

        // Message with embedded newline
        let newline_message = "{\"jsonrpc\":\"2.0\",\n\"id\":1}";
        assert!(StdioTransport::validate_message(newline_message).is_err());

        // Message with carriage return
        let cr_message = "{\"jsonrpc\":\"2.0\",\r\"id\":1}";
        assert!(StdioTransport::validate_message(cr_message).is_err());

        // Wrong JSON-RPC version
        let wrong_version = r#"{"jsonrpc":"1.0","id":1,"method":"test"}"#;
        assert!(StdioTransport::validate_message(wrong_version).is_err());

        // Message without explicit JSON-RPC version (should fail with strict validation)
        let no_version = r#"{"id":1,"method":"test","params":{}}"#;
        assert!(StdioTransport::validate_message(no_version).is_err());

        // Valid batch request
        let valid_batch = r#"[{"jsonrpc":"2.0","id":1,"method":"test1"},{"jsonrpc":"2.0","id":2,"method":"test2"}]"#;
        assert!(StdioTransport::validate_message(valid_batch).is_ok());

        // Empty batch (should fail)
        let empty_batch = "[]";
        assert!(StdioTransport::validate_message(empty_batch).is_err());

        // Batch with invalid item (should fail)
        let invalid_batch = r#"[{"jsonrpc":"2.0","id":1},"not an object"]"#;
        assert!(StdioTransport::validate_message(invalid_batch).is_err());

        // Non-object/array root (should fail)
        let primitive_root = "\"just a string\"";
        assert!(StdioTransport::validate_message(primitive_root).is_err());
    }

    #[tokio::test]
    async fn test_stdio_transport_custom_config() {
        use tokio::time::Duration;

        let custom_transport = StdioTransport::with_config(
            Duration::from_secs(60), // 60s read timeout
            Duration::from_secs(20), // 20s write timeout
            128 * 1024,              // 128KB buffer
        );

        assert_eq!(custom_transport.read_timeout, Duration::from_secs(60));
        assert_eq!(custom_transport.write_timeout, Duration::from_secs(20));
        assert_eq!(custom_transport.buffer_size, 128 * 1024);
    }

    #[tokio::test]
    async fn test_stdio_transport_constants() {
        // Verify default constants are reasonable
        assert_eq!(
            StdioTransport::DEFAULT_READ_TIMEOUT,
            Duration::from_secs(30)
        );
        assert_eq!(
            StdioTransport::DEFAULT_WRITE_TIMEOUT,
            Duration::from_secs(10)
        );
        assert_eq!(StdioTransport::DEFAULT_BUFFER_SIZE, 64 * 1024);
    }

    #[tokio::test]
    async fn test_transport_factory_stdio_variants() {
        use tokio::time::Duration;

        // Test default stdio transport factory
        let _default_transport = TransportFactory::stdio();

        // Test custom stdio transport factory
        let _custom_transport = TransportFactory::stdio_with_config(
            Duration::from_secs(45),
            Duration::from_secs(15),
            32 * 1024,
        );
    }

    #[tokio::test]
    async fn test_stdio_transport_closed_state() {
        let mut transport = StdioTransport::new();

        // Transport should start as uninitialized
        assert_eq!(transport.connection_state(), ConnectionState::Uninitialized);
        assert!(!transport.is_closed());

        // Close the transport
        transport.close().await.unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Closed);
        assert!(transport.is_closed());

        // Operations on closed transport should fail
        let send_result = transport.send(r#"{"jsonrpc":"2.0","id":1}"#).await;
        assert!(send_result.is_err());
        assert!(matches!(send_result.unwrap_err(), Error::Transport(_)));

        let receive_result = transport.receive().await;
        assert!(receive_result.is_err());
        assert!(matches!(receive_result.unwrap_err(), Error::Transport(_)));
    }

    #[test]
    fn test_stdio_transport_default_implementation() {
        let transport1 = StdioTransport::new();
        let transport2 = StdioTransport::default();

        // Both should have the same configuration
        assert_eq!(transport1.read_timeout, transport2.read_timeout);
        assert_eq!(transport1.write_timeout, transport2.write_timeout);
        assert_eq!(transport1.buffer_size, transport2.buffer_size);
        assert_eq!(transport1.connection_state, transport2.connection_state);
        assert_eq!(transport1.messages_sent, transport2.messages_sent);
        assert_eq!(transport1.messages_received, transport2.messages_received);
        assert_eq!(transport1.errors_encountered, transport2.errors_encountered);
    }

    #[tokio::test]
    async fn test_stdio_transport_utf8_validation() {
        // Test with valid Unicode content including non-ASCII characters
        let utf8_message =
            r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"Hello 世界 🌍"}}"#;
        assert!(StdioTransport::validate_message(utf8_message).is_ok());

        // Test with ASCII-only content
        let ascii_message =
            r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"message":"Hello World"}}"#;
        assert!(StdioTransport::validate_message(ascii_message).is_ok());
    }

    #[tokio::test]
    async fn test_stdio_transport_json_rpc_compliance() {
        // Test various JSON-RPC 2.0 message formats

        // Request with positional parameters
        let request_positional = r#"{"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":1}"#;
        assert!(StdioTransport::validate_message(request_positional).is_ok());

        // Request with named parameters
        let request_named = r#"{"jsonrpc":"2.0","method":"subtract","params":{"subtrahend":23,"minuend":42},"id":2}"#;
        assert!(StdioTransport::validate_message(request_named).is_ok());

        // Notification (no id)
        let notification = r#"{"jsonrpc":"2.0","method":"update","params":[1,2,3,4,5]}"#;
        assert!(StdioTransport::validate_message(notification).is_ok());

        // Response with result
        let response_result = r#"{"jsonrpc":"2.0","result":19,"id":1}"#;
        assert!(StdioTransport::validate_message(response_result).is_ok());

        // Response with error
        let response_error =
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":1}"#;
        assert!(StdioTransport::validate_message(response_error).is_ok());

        // Batch request
        let batch = r#"[{"jsonrpc":"2.0","method":"sum","params":[1,2,4],"id":"1"},{"jsonrpc":"2.0","method":"notify_hello","params":[7]}]"#;
        assert!(StdioTransport::validate_message(batch).is_ok());
    }

    #[tokio::test]
    async fn test_initialization_state_management() {
        let mut transport = StdioTransport::new();
        assert_eq!(transport.connection_state(), ConnectionState::Uninitialized);
        assert!(transport.last_init_id.is_none());

        // Test initialization request detection
        let init_request = r#"{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":"init-123"}"#;

        // Simulate sending an initialize request
        transport
            .update_initialization_state(init_request, true)
            .await
            .unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Initializing);
        assert_eq!(
            transport.last_init_id,
            Some(serde_json::Value::String("init-123".to_string()))
        );

        // Test initialization response detection
        let init_response = r#"{"jsonrpc":"2.0","result":{"protocolVersion":"2024-11-05","serverInfo":{"name":"test-server","version":"1.0"},"capabilities":{}},"id":"init-123"}"#;

        // Simulate receiving an initialize response
        transport
            .update_initialization_state(init_response, false)
            .await
            .unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Initialized);
        assert!(transport.last_init_id.is_none());
    }

    #[tokio::test]
    async fn test_stdio_transport_ping_handling() {
        let mut transport = StdioTransport::new();

        // Create ping message
        let ping_message = transport.create_ping_message();
        assert!(ping_message.contains(r#""method":"ping""#));
        assert!(ping_message.contains(r#""jsonrpc":"2.0""#));
        assert!(ping_message.contains(r#""id":"#));

        // Analyze ping message
        transport.analyze_message(&ping_message).unwrap();
        assert!(transport.last_ping_id.is_some());
    }

    #[tokio::test]
    async fn test_stdio_transport_statistics_tracking() {
        let mut transport = StdioTransport::new();

        // Initial statistics
        let (sent, received, errors) = transport.get_stats();
        assert_eq!(sent, 0);
        assert_eq!(received, 0);
        assert_eq!(errors, 0);

        // Simulate some errors
        transport.errors_encountered = 5;
        let (_, _, errors) = transport.get_stats();
        assert_eq!(errors, 5);
    }

    #[tokio::test]
    async fn test_initialization_error_handling() {
        let mut transport = StdioTransport::new();

        // Send initialize request
        let init_request = r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":42}"#;
        transport
            .update_initialization_state(init_request, true)
            .await
            .unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Initializing);
        assert_eq!(
            transport.last_init_id,
            Some(serde_json::Value::Number(serde_json::Number::from(42)))
        );

        // Receive error response
        let error_response =
            r#"{"jsonrpc":"2.0","error":{"code":-32602,"message":"Invalid params"},"id":42}"#;
        transport
            .update_initialization_state(error_response, false)
            .await
            .unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Closed); // Error should close the connection
        assert!(transport.last_init_id.is_none());
    }

    #[tokio::test]
    async fn test_initialize_request_detection() {
        // Test various initialize request formats
        let request_with_string_id =
            r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":"test"}"#;
        let id = StdioTransport::is_initialize_request(request_with_string_id).unwrap();
        assert_eq!(id, Some(serde_json::Value::String("test".to_string())));

        let request_with_number_id =
            r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":123}"#;
        let id = StdioTransport::is_initialize_request(request_with_number_id).unwrap();
        assert_eq!(
            id,
            Some(serde_json::Value::Number(serde_json::Number::from(123)))
        );

        // Test non-initialize request
        let ping_request = r#"{"jsonrpc":"2.0","method":"ping","id":1}"#;
        let id = StdioTransport::is_initialize_request(ping_request).unwrap();
        assert!(id.is_none());
    }

    #[tokio::test]
    async fn test_connection_state_transitions() {
        let mut transport = StdioTransport::new();

        // Test closed connection prevents initialization
        transport.connection_state = ConnectionState::Closed;
        let init_request = r#"{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}"#;
        let result = transport
            .update_initialization_state(init_request, true)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Cannot initialize a closed connection"));

        // Test re-initialization from initialized state
        transport.connection_state = ConnectionState::Initialized;
        transport
            .update_initialization_state(init_request, true)
            .await
            .unwrap();
        assert_eq!(transport.connection_state(), ConnectionState::Initializing);
    }

    #[tokio::test]
    async fn test_stdio_transport_ping_analysis() {
        let mut transport = StdioTransport::new();

        // Test ping message with string ID
        let ping_request = r#"{"jsonrpc":"2.0","method":"ping","id":"test-ping"}"#;
        transport.analyze_message(ping_request).unwrap();
        assert_eq!(transport.last_ping_id, Some("test-ping".to_string()));

        // Test ping message with number ID
        let ping_number = r#"{"jsonrpc":"2.0","method":"ping","id":42}"#;
        transport.analyze_message(ping_number).unwrap();
        assert_eq!(transport.last_ping_id, Some("42".to_string()));

        // Test non-ping message
        let other_request = r#"{"jsonrpc":"2.0","method":"list_tools","id":2}"#;
        transport.analyze_message(other_request).unwrap();
        // Should keep the last ping ID
        assert_eq!(transport.last_ping_id, Some("42".to_string()));
    }
}
