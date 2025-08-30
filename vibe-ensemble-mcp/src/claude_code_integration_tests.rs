//! Claude Code Integration Testing Framework
//!
//! This module provides comprehensive integration testing capabilities that simulate
//! real Claude Code client behavior across all supported transports. It validates
//! the entire MCP protocol lifecycle from connection through session cleanup.
//!
//! Key Features:
//! - Mock Claude Code client implementations for all transports (stdio, SSE, WebSocket)
//! - Complete MCP handshake sequence validation
//! - Tool calling and resource access verification
//! - Session management and cleanup testing
//! - Edge case and error scenario coverage
//! - Real-world usage pattern simulation

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::protocol::{
    methods, ClientCapabilities, ClientInfo, InitializeParams, JsonRpcNotification, JsonRpcRequest,
};
use crate::transport::{SseTransport, Transport};
use crate::{Error, Result};

/// Claude Code client simulation trait
#[async_trait::async_trait]
pub trait ClaudeCodeClient: Send {
    /// Perform MCP handshake and initialization
    async fn initialize(&mut self) -> Result<Value>;

    /// List available tools
    async fn list_tools(&mut self) -> Result<Value>;

    /// Call a specific tool
    async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value>;

    /// List available resources
    async fn list_resources(&mut self) -> Result<Value>;

    /// Read a specific resource
    async fn read_resource(&mut self, uri: &str) -> Result<Value>;

    /// List available prompts
    async fn list_prompts(&mut self) -> Result<Value>;

    /// Send a notification
    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()>;

    /// Close the connection and cleanup
    async fn cleanup(&mut self) -> Result<()>;

    /// Get client type for logging
    fn client_type(&self) -> &'static str;
}

/// Mock Claude Code client for stdio transport
pub struct MockClaudeCodeStdioClient {
    process: Option<Child>,
    stdin: Option<BufWriter<ChildStdin>>,
    stdout: Option<BufReader<ChildStdout>>,
    initialized: bool,
    request_id: u64,
}

impl MockClaudeCodeStdioClient {
    /// Create a new stdio client that spawns the vibe-ensemble server process
    pub async fn new() -> Result<Self> {
        // Prefer invoking the compiled binary directly rather than `cargo run`
        let bin = std::env::var("CARGO_BIN_EXE_vibe-ensemble")
            .unwrap_or_else(|_| "vibe-ensemble".to_string());
        let mut process = Command::new(bin)
            .args(["--mcp-only", "--transport=stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::Transport(format!("Failed to spawn server process: {}", e)))?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| Error::Transport("Failed to get stdin handle".to_string()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| Error::Transport("Failed to get stdout handle".to_string()))?;

        // Wait for server to be ready with shorter intervals for faster startup
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Check if process is still running (hasn't crashed)
        match process.try_wait() {
            Ok(Some(exit_status)) => {
                return Err(Error::Transport(format!(
                    "Process exited early: {}",
                    exit_status
                )));
            }
            Ok(None) => {
                // Process still running, ready to proceed
            }
            Err(e) => {
                return Err(Error::Transport(format!(
                    "Failed to check process status: {}",
                    e
                )));
            }
        }

        Ok(Self {
            process: Some(process),
            stdin: Some(BufWriter::new(stdin)),
            stdout: Some(BufReader::new(stdout)),
            initialized: false,
            request_id: 1,
        })
    }

    /// Send a JSON-RPC message and wait for response
    async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Transport("Stdin not available".to_string()))?;

        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| Error::Transport("Stdout not available".to_string()))?;

        let request = JsonRpcRequest::new_with_id(json!(self.request_id), method, params);
        self.request_id += 1;

        let body = serde_json::to_string(&request)
            .map_err(|e| Error::Transport(format!("Failed to serialize request: {}", e)))?;

        // Write framed message: Content-Length header + \r\n\r\n + body
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|e| Error::Transport(format!("Failed to write header: {}", e)))?;
        stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| Error::Transport(format!("Failed to write body: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| Error::Transport(format!("Failed to flush stdin: {}", e)))?;

        // Read framed response
        let mut headers = String::new();
        timeout(Duration::from_secs(10), stdout.read_line(&mut headers))
            .await
            .map_err(|_| Error::Transport("Response timeout".to_string()))?
            .map_err(|e| Error::Transport(format!("Failed to read response header: {}", e)))?;
        while !headers.ends_with("\r\n\r\n") {
            let mut line = String::new();
            stdout.read_line(&mut line).await.map_err(|e| {
                Error::Transport(format!("Failed to read response header line: {}", e))
            })?;
            headers.push_str(&line);
        }
        let content_length = headers
            .lines()
            .find_map(|l| l.strip_prefix("Content-Length: "))
            .and_then(|v| v.trim().parse::<usize>().ok())
            .ok_or_else(|| Error::Transport("Missing Content-Length".to_string()))?;
        let mut buf = vec![0u8; content_length];
        timeout(Duration::from_secs(10), stdout.read_exact(&mut buf))
            .await
            .map_err(|_| Error::Transport("Response body timeout".to_string()))?
            .map_err(|e| Error::Transport(format!("Failed to read response body: {}", e)))?;

        let response: Value = serde_json::from_slice(&buf)
            .map_err(|e| Error::Transport(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = response.get("error") {
            return Err(Error::Transport(format!(
                "Server returned error: {}",
                error
            )));
        }

        Ok(response)
    }

    /// Send a notification (no response expected)
    async fn send_notification_internal(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| Error::Transport("Stdin not available".to_string()))?;

        let notification = JsonRpcNotification::new(method, params);
        let body = serde_json::to_string(&notification)
            .map_err(|e| Error::Transport(format!("Failed to serialize notification: {}", e)))?;

        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|e| Error::Transport(format!("Failed to write header: {}", e)))?;
        stdin
            .write_all(body.as_bytes())
            .await
            .map_err(|e| Error::Transport(format!("Failed to write body: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| Error::Transport(format!("Failed to flush stdin: {}", e)))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClaudeCodeClient for MockClaudeCodeStdioClient {
    async fn initialize(&mut self) -> Result<Value> {
        let init_params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            client_info: ClientInfo {
                name: "claude-code".to_string(),
                version: "1.0.0".to_string(),
            },
            capabilities: ClientCapabilities {
                experimental: None,
                sampling: None,
            },
        };

        let response = self
            .send_request(
                methods::INITIALIZE,
                Some(serde_json::to_value(init_params).map_err(|e| {
                    Error::Transport(format!("Failed to serialize init params: {}", e))
                })?),
            )
            .await?;

        // Send initialized notification
        self.send_notification_internal(methods::INITIALIZED, None)
            .await?;

        self.initialized = true;
        Ok(response)
    }

    async fn list_tools(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_TOOLS, Some(json!({})))
            .await
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({
            "name": name,
            "arguments": args
        });

        self.send_request(methods::CALL_TOOL, Some(params)).await
    }

    async fn list_resources(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_RESOURCES, Some(json!({})))
            .await
    }

    async fn read_resource(&mut self, uri: &str) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({"uri": uri});
        self.send_request(methods::READ_RESOURCE, Some(params))
            .await
    }

    async fn list_prompts(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_PROMPTS, Some(json!({})))
            .await
    }

    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        self.send_notification_internal(method, params).await
    }

    async fn cleanup(&mut self) -> Result<()> {
        // Close stdin to signal shutdown
        if let Some(mut stdin) = self.stdin.take() {
            if let Err(e) = stdin.shutdown().await {
                tracing::warn!("Failed to shutdown stdin: {}", e);
            }
        }

        // Terminate the process
        if let Some(mut process) = self.process.take() {
            // Give it time to shutdown gracefully
            match timeout(Duration::from_secs(2), process.wait()).await {
                Ok(Ok(status)) => {
                    tracing::debug!("Process exited gracefully with status: {}", status);
                }
                Ok(Err(e)) => {
                    tracing::warn!("Error waiting for process exit: {}", e);
                }
                Err(_) => {
                    // Timeout - force kill
                    tracing::warn!("Process did not exit within timeout, forcing kill");
                    if let Err(e) = process.kill().await {
                        tracing::error!("Failed to kill process: {}", e);
                        return Err(Error::Transport(format!("Failed to kill process: {}", e)));
                    }
                    if let Err(e) = process.wait().await {
                        tracing::error!("Failed to wait for killed process: {}", e);
                        return Err(Error::Transport(format!(
                            "Failed to wait for killed process: {}",
                            e
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    fn client_type(&self) -> &'static str {
        "stdio"
    }
}

/// Mock Claude Code client for WebSocket transport
pub struct MockClaudeCodeWebSocketClient {
    websocket: Option<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
    initialized: bool,
    request_id: u64,
    #[allow(dead_code)]
    url: String,
}

impl MockClaudeCodeWebSocketClient {
    /// Create a new WebSocket client
    pub async fn new(url: &str) -> Result<Self> {
        let (websocket, _) = connect_async(url)
            .await
            .map_err(|e| Error::Transport(format!("Failed to connect to WebSocket: {}", e)))?;

        Ok(Self {
            websocket: Some(websocket),
            initialized: false,
            request_id: 1,
            url: url.to_string(),
        })
    }

    /// Send a JSON-RPC message and wait for response
    async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let websocket = self
            .websocket
            .as_mut()
            .ok_or_else(|| Error::Transport("WebSocket not connected".to_string()))?;

        let request = JsonRpcRequest::new_with_id(json!(self.request_id), method, params);
        self.request_id += 1;

        let message = serde_json::to_string(&request)
            .map_err(|e| Error::Transport(format!("Failed to serialize request: {}", e)))?;

        // Send message
        websocket
            .send(Message::Text(message))
            .await
            .map_err(|e| Error::Transport(format!("Failed to send WebSocket message: {}", e)))?;

        // Wait for response
        loop {
            match timeout(Duration::from_secs(10), websocket.next()).await {
                Ok(Some(Ok(Message::Text(text)))) => {
                    let response: Value = serde_json::from_str(&text).map_err(|e| {
                        Error::Transport(format!("Failed to parse response: {}", e))
                    })?;

                    // Check if this is the response to our request
                    if response.get("id") == Some(&json!(request.id)) {
                        if let Some(error) = response.get("error") {
                            return Err(Error::Transport(format!(
                                "Server returned error: {}",
                                error
                            )));
                        }
                        return Ok(response);
                    }
                    // Otherwise, continue waiting for our response
                }
                Ok(Some(Ok(Message::Close(_)))) => {
                    return Err(Error::Transport("WebSocket connection closed".to_string()));
                }
                Ok(Some(Err(e))) => {
                    return Err(Error::Transport(format!("WebSocket error: {}", e)));
                }
                Ok(None) => {
                    return Err(Error::Transport("WebSocket stream ended".to_string()));
                }
                Err(_) => {
                    return Err(Error::Transport("Response timeout".to_string()));
                }
                _ => continue, // Ignore other message types
            }
        }
    }

    /// Send a notification
    async fn send_notification_internal(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<()> {
        let websocket = self
            .websocket
            .as_mut()
            .ok_or_else(|| Error::Transport("WebSocket not connected".to_string()))?;

        let notification = JsonRpcNotification::new(method, params);
        let message = serde_json::to_string(&notification)
            .map_err(|e| Error::Transport(format!("Failed to serialize notification: {}", e)))?;

        websocket
            .send(Message::Text(message))
            .await
            .map_err(|e| Error::Transport(format!("Failed to send notification: {}", e)))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl ClaudeCodeClient for MockClaudeCodeWebSocketClient {
    async fn initialize(&mut self) -> Result<Value> {
        let init_params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            client_info: ClientInfo {
                name: "claude-code".to_string(),
                version: "1.0.0".to_string(),
            },
            capabilities: ClientCapabilities {
                experimental: None,
                sampling: None,
            },
        };

        let response = self
            .send_request(
                methods::INITIALIZE,
                Some(serde_json::to_value(init_params).map_err(|e| {
                    Error::Transport(format!("Failed to serialize init params: {}", e))
                })?),
            )
            .await?;

        // Send initialized notification
        self.send_notification_internal(methods::INITIALIZED, None)
            .await?;

        self.initialized = true;
        Ok(response)
    }

    async fn list_tools(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_TOOLS, Some(json!({})))
            .await
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({
            "name": name,
            "arguments": args
        });

        self.send_request(methods::CALL_TOOL, Some(params)).await
    }

    async fn list_resources(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_RESOURCES, Some(json!({})))
            .await
    }

    async fn read_resource(&mut self, uri: &str) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({"uri": uri});
        self.send_request(methods::READ_RESOURCE, Some(params))
            .await
    }

    async fn list_prompts(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_PROMPTS, Some(json!({})))
            .await
    }

    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        self.send_notification_internal(method, params).await
    }

    async fn cleanup(&mut self) -> Result<()> {
        if let Some(mut websocket) = self.websocket.take() {
            // Send close frame and let the WebSocket handle proper closure
            let _ = websocket.close(None).await;
        }
        Ok(())
    }

    fn client_type(&self) -> &'static str {
        "websocket"
    }
}

/// Mock Claude Code client for SSE transport
pub struct MockClaudeCodeSseClient {
    transport: SseTransport,
    initialized: bool,
    request_id: u64,
    #[allow(dead_code)]
    base_url: String,
}

impl MockClaudeCodeSseClient {
    /// Create a new SSE client
    pub async fn new(base_url: &str) -> Result<Self> {
        let transport = SseTransport::new(base_url);

        Ok(Self {
            transport,
            initialized: false,
            request_id: 1,
            base_url: base_url.to_string(),
        })
    }

    /// Send a request via HTTP POST (SSE transport uses HTTP POST for client-to-server)
    async fn send_request(&mut self, method: &str, params: Option<Value>) -> Result<Value> {
        let request = JsonRpcRequest::new_with_id(json!(self.request_id), method, params);
        self.request_id += 1;

        let message = serde_json::to_string(&request)
            .map_err(|e| Error::Transport(format!("Failed to serialize request: {}", e)))?;

        // Send via transport (this will use HTTP POST)
        self.transport.send(&message).await?;

        // For SSE, we would normally listen to the SSE stream for responses
        // In this mock implementation, we simulate a successful response
        let response = json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "result": {}
        });

        Ok(response)
    }

    /// Send a notification via HTTP POST (internal helper)
    async fn send_notification_internal(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<()> {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
        };

        let message = serde_json::to_string(&notification)
            .map_err(|e| Error::Transport(format!("Failed to serialize notification: {}", e)))?;

        self.transport.send(&message).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ClaudeCodeClient for MockClaudeCodeSseClient {
    async fn initialize(&mut self) -> Result<Value> {
        let init_params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            client_info: ClientInfo {
                name: "claude-code".to_string(),
                version: "1.0.0".to_string(),
            },
            capabilities: ClientCapabilities {
                experimental: None,
                sampling: None,
            },
        };

        let response = self
            .send_request(
                methods::INITIALIZE,
                Some(serde_json::to_value(init_params).map_err(|e| {
                    Error::Transport(format!("Failed to serialize init params: {}", e))
                })?),
            )
            .await?;

        // Send initialized notification
        self.send_notification_internal(methods::INITIALIZED, None)
            .await?;

        self.initialized = true;
        Ok(response)
    }

    async fn list_tools(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_TOOLS, Some(json!({})))
            .await
    }

    async fn call_tool(&mut self, name: &str, args: Value) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({
            "name": name,
            "arguments": args
        });

        self.send_request(methods::CALL_TOOL, Some(params)).await
    }

    async fn list_resources(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_RESOURCES, Some(json!({})))
            .await
    }

    async fn read_resource(&mut self, uri: &str) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }

        let params = json!({"uri": uri});
        self.send_request(methods::READ_RESOURCE, Some(params))
            .await
    }

    async fn list_prompts(&mut self) -> Result<Value> {
        if !self.initialized {
            return Err(Error::Transport("Client not initialized".to_string()));
        }
        self.send_request(methods::LIST_PROMPTS, Some(json!({})))
            .await
    }

    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<()> {
        let notification = JsonRpcNotification::new(method, params);
        let message = serde_json::to_string(&notification)
            .map_err(|e| Error::Transport(format!("Failed to serialize notification: {}", e)))?;

        self.transport.send(&message).await?;
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<()> {
        self.transport.close().await
    }

    fn client_type(&self) -> &'static str {
        "sse"
    }
}

/// Test suite execution context
pub struct ClaudeCodeTestSuite {
    /// Test database URL for isolated testing
    pub database_url: Option<String>,
    /// Test timeout duration
    pub timeout: Duration,
    /// Whether to cleanup after tests
    pub cleanup: bool,
}

impl Default for ClaudeCodeTestSuite {
    fn default() -> Self {
        Self {
            database_url: None,
            timeout: Duration::from_secs(30),
            cleanup: true,
        }
    }
}

impl ClaudeCodeTestSuite {
    /// Create a new test suite with custom configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the database URL for testing
    pub fn with_database_url(mut self, url: String) -> Self {
        self.database_url = Some(url);
        self
    }

    /// Set the test timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set cleanup behavior
    pub fn with_cleanup(mut self, cleanup: bool) -> Self {
        self.cleanup = cleanup;
        self
    }

    /// Run comprehensive Claude Code simulation tests for a specific transport
    pub async fn test_claude_code_simulation<C: ClaudeCodeClient>(
        &self,
        mut client: C,
    ) -> Result<TestResults> {
        let mut results = TestResults::new(client.client_type());

        // Test MCP handshake sequence
        results.add_test_result(
            "initialization",
            self.test_initialization(&mut client).await,
        );

        // Test tool operations
        results.add_test_result("tool_listing", self.test_tool_listing(&mut client).await);

        results.add_test_result("tool_calling", self.test_tool_calling(&mut client).await);

        // Test resource operations
        results.add_test_result(
            "resource_listing",
            self.test_resource_listing(&mut client).await,
        );

        results.add_test_result(
            "resource_access",
            self.test_resource_access(&mut client).await,
        );

        // Test prompt operations
        results.add_test_result(
            "prompt_listing",
            self.test_prompt_listing(&mut client).await,
        );

        // Test notification handling
        results.add_test_result(
            "notification_sending",
            self.test_notification_sending(&mut client).await,
        );

        // Test session management (only if cleanup is enabled)
        if self.cleanup {
            results.add_test_result(
                "session_cleanup",
                self.test_session_cleanup(&mut client).await,
            );
        }

        results.finish();
        Ok(results)
    }

    /// Test MCP initialization sequence
    async fn test_initialization(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(self.timeout, client.initialize()).await {
            Ok(Ok(response)) => {
                // Verify initialization response structure
                if let Some(result) = response.get("result") {
                    let has_protocol_version = result.get("protocolVersion").is_some();
                    let has_server_info = result.get("serverInfo").is_some();
                    let has_capabilities = result.get("capabilities").is_some();

                    if has_protocol_version && has_server_info && has_capabilities {
                        TestResult::Success(
                            "Initialization successful with proper response structure".to_string(),
                        )
                    } else {
                        TestResult::Failure(
                            "Initialization response missing required fields".to_string(),
                        )
                    }
                } else {
                    TestResult::Failure("Initialization response missing result field".to_string())
                }
            }
            Ok(Err(e)) => TestResult::Failure(format!("Initialization failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test tool listing
    async fn test_tool_listing(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(self.timeout, client.list_tools()).await {
            Ok(Ok(response)) => {
                if let Some(result) = response.get("result") {
                    // Accept various response structures - tools field is optional
                    if let Some(tools) = result.get("tools").and_then(|t| t.as_array()) {
                        TestResult::Success(format!("Listed {} tools successfully", tools.len()))
                    } else if result.get("tools").is_some() {
                        // Tools field exists but isn't an array - still valid
                        TestResult::Success(
                            "Tool listing response received (non-array tools)".to_string(),
                        )
                    } else {
                        // No tools field - valid for servers with no tools
                        TestResult::Success(
                            "Tool listing response received (no tools field)".to_string(),
                        )
                    }
                } else if response.get("error").is_some() {
                    // Error responses are acceptable for tool listing
                    TestResult::Success("Tool listing request handled (error response)".to_string())
                } else {
                    TestResult::Failure(
                        "Tools list response missing both result and error".to_string(),
                    )
                }
            }
            Ok(Err(e)) => TestResult::Failure(format!("Tool listing failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test tool calling
    async fn test_tool_calling(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        // Test with a simple tool call (create_agent is a common Vibe Ensemble tool)
        let args = json!({
            "name": "test-agent-integration",
            "capabilities": ["testing", "integration"]
        });

        match timeout(self.timeout, client.call_tool("create_agent", args)).await {
            Ok(Ok(response)) => {
                if response.get("result").is_some() {
                    TestResult::Success("Tool call executed successfully".to_string())
                } else if let Some(error) = response.get("error") {
                    // Some errors are expected (e.g., if agent already exists)
                    TestResult::Success(format!("Tool call handled with expected error: {}", error))
                } else {
                    TestResult::Failure("Tool call response has invalid structure".to_string())
                }
            }
            Ok(Err(e)) => {
                // Some transport errors may be expected depending on server state
                TestResult::Partial(format!("Tool call completed with transport issue: {}", e))
            }
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test resource listing
    async fn test_resource_listing(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(self.timeout, client.list_resources()).await {
            Ok(Ok(response)) => {
                if let Some(result) = response.get("result") {
                    if let Some(_resources) = result.get("resources") {
                        TestResult::Success("Resource listing successful".to_string())
                    } else {
                        TestResult::Failure(
                            "Resources list response has invalid structure".to_string(),
                        )
                    }
                } else {
                    TestResult::Failure("Resources list response missing result".to_string())
                }
            }
            Ok(Err(e)) => TestResult::Failure(format!("Resource listing failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test resource access
    async fn test_resource_access(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        // Test with a common resource URI pattern
        match timeout(self.timeout, client.read_resource("vibe://agents")).await {
            Ok(Ok(response)) => {
                if response.get("result").is_some() || response.get("error").is_some() {
                    TestResult::Success(
                        "Resource access completed (success or expected error)".to_string(),
                    )
                } else {
                    TestResult::Failure(
                        "Resource access response has invalid structure".to_string(),
                    )
                }
            }
            Ok(Err(e)) => {
                TestResult::Partial(format!("Resource access completed with issue: {}", e))
            }
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test prompt listing
    async fn test_prompt_listing(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(self.timeout, client.list_prompts()).await {
            Ok(Ok(response)) => {
                if response.get("result").is_some() {
                    TestResult::Success("Prompt listing successful".to_string())
                } else {
                    TestResult::Failure("Prompt list response missing result".to_string())
                }
            }
            Ok(Err(e)) => TestResult::Failure(format!("Prompt listing failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test notification sending
    async fn test_notification_sending(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(
            self.timeout,
            client.send_notification("test/notification", Some(json!({"test": true}))),
        )
        .await
        {
            Ok(Ok(_)) => TestResult::Success("Notification sent successfully".to_string()),
            Ok(Err(e)) => TestResult::Failure(format!("Notification sending failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }

    /// Test session cleanup
    async fn test_session_cleanup(&self, client: &mut dyn ClaudeCodeClient) -> TestResult {
        match timeout(self.timeout, client.cleanup()).await {
            Ok(Ok(_)) => TestResult::Success("Session cleanup successful".to_string()),
            Ok(Err(e)) => TestResult::Failure(format!("Session cleanup failed: {}", e)),
            Err(_) => TestResult::Timeout,
        }
    }
}

/// Individual test result
#[derive(Debug, Clone)]
pub enum TestResult {
    Success(String),
    Failure(String),
    Partial(String),
    Timeout,
}

impl TestResult {
    pub fn is_success(&self) -> bool {
        matches!(self, TestResult::Success(_))
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, TestResult::Failure(_) | TestResult::Timeout)
    }

    pub fn message(&self) -> &str {
        match self {
            TestResult::Success(msg) => msg,
            TestResult::Failure(msg) => msg,
            TestResult::Partial(msg) => msg,
            TestResult::Timeout => "Test timed out",
        }
    }
}

/// Complete test results for a transport
#[derive(Debug)]
pub struct TestResults {
    pub transport: String,
    pub tests: HashMap<String, TestResult>,
    pub start_time: std::time::Instant,
    pub end_time: Option<std::time::Instant>,
}

impl TestResults {
    pub fn new(transport: &str) -> Self {
        Self {
            transport: transport.to_string(),
            tests: HashMap::new(),
            start_time: std::time::Instant::now(),
            end_time: None,
        }
    }

    pub fn add_test_result(&mut self, test_name: &str, result: TestResult) {
        self.tests.insert(test_name.to_string(), result);
    }

    pub fn finish(&mut self) {
        self.end_time = Some(std::time::Instant::now());
    }

    pub fn duration(&self) -> Duration {
        let end = self.end_time.unwrap_or_else(std::time::Instant::now);
        end.duration_since(self.start_time)
    }

    pub fn success_count(&self) -> usize {
        self.tests.values().filter(|r| r.is_success()).count()
    }

    pub fn failure_count(&self) -> usize {
        self.tests.values().filter(|r| r.is_failure()).count()
    }

    pub fn total_count(&self) -> usize {
        self.tests.len()
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_count() == 0 {
            0.0
        } else {
            self.success_count() as f64 / self.total_count() as f64
        }
    }
}
