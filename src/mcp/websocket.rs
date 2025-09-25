use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{error, info, trace, warn};
use uuid::Uuid;

use super::types::JsonRpcRequest;
use crate::{error::AppError, server::AppState, sse::EventBroadcaster};

type Result<T> = std::result::Result<T, AppError>;

/// WebSocket connection manager
pub struct WebSocketManager {
    /// Active client connections
    pub clients: Arc<DashMap<String, ClientConnection>>,
    /// Client tool registry
    tool_registry: Arc<ClientToolRegistry>,
    /// Pending server-initiated requests
    pending_requests: Arc<DashMap<String, PendingRequest>>,
    /// Semaphore to limit concurrent client tool calls
    concurrency_semaphore: Option<Arc<Semaphore>>,
    /// Event broadcaster subscription (optional for independent operation)
    event_broadcaster: Option<EventBroadcaster>,
}

/// Individual client connection
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub client_id: String,
    pub sender: mpsc::UnboundedSender<Message>,
    pub capabilities: ClientCapabilities,
    pub connected_at: chrono::DateTime<chrono::Utc>,
}

/// Client capabilities negotiated during handshake
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCapabilities {
    pub bidirectional: bool,
    pub tools: Vec<String>,
    pub client_info: ClientInfo,
    // MCP client capabilities from initialize request
    #[serde(default)]
    pub mcp_capabilities: Option<super::types::ClientCapabilities>,
}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
    pub environment: String,
}

/// Client tool registry for bidirectional communication
pub struct ClientToolRegistry {
    /// Tools available from clients
    tools: Arc<DashMap<String, ClientToolDefinition>>,
    /// Client capabilities by client ID
    client_capabilities: Arc<DashMap<String, ClientCapabilities>>,
}

/// Client tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub client_id: String,
    pub registered_at: chrono::DateTime<chrono::Utc>,
}

/// Pending server-initiated request
#[derive(Debug)]
pub struct PendingRequest {
    pub request_id: String,
    pub client_id: String,
    pub tool_name: String,
    pub response_sender: tokio::sync::oneshot::Sender<Result<Value>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// WebSocket query parameters for authentication
#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    token: Option<String>,
}

impl Default for WebSocketManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            tool_registry: Arc::new(ClientToolRegistry::new()),
            pending_requests: Arc::new(DashMap::new()),
            concurrency_semaphore: None,
            event_broadcaster: None,
        }
    }

    /// Create a WebSocket manager with concurrency limits
    pub fn with_concurrency_limit(max_concurrent: usize) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            tool_registry: Arc::new(ClientToolRegistry::new()),
            pending_requests: Arc::new(DashMap::new()),
            concurrency_semaphore: Some(Arc::new(Semaphore::new(max_concurrent))),
            event_broadcaster: None,
        }
    }

    /// Create a WebSocket manager with concurrency limits and event broadcasting
    pub fn with_event_broadcasting(
        max_concurrent: usize,
        event_broadcaster: EventBroadcaster,
    ) -> Self {
        let manager = Self {
            clients: Arc::new(DashMap::new()),
            tool_registry: Arc::new(ClientToolRegistry::new()),
            pending_requests: Arc::new(DashMap::new()),
            concurrency_semaphore: Some(Arc::new(Semaphore::new(max_concurrent))),
            event_broadcaster: Some(event_broadcaster.clone()),
        };

        // Start event broadcasting task
        let manager_clone = manager.clone();
        tokio::spawn(async move {
            manager_clone.event_broadcasting_loop().await;
        });

        manager
    }

    /// Create a disabled WebSocket manager (when WebSocket is disabled in config)
    pub fn disabled() -> Self {
        // Same structure but will return errors for most operations
        Self::new()
    }

    /// Get client tool registry
    pub fn tool_registry(&self) -> &ClientToolRegistry {
        &self.tool_registry
    }

    /// Get pending requests
    pub fn pending_requests(&self) -> &DashMap<String, PendingRequest> {
        &self.pending_requests
    }

    /// Handle new WebSocket connection
    pub async fn handle_connection(
        &self,
        ws_upgrade: WebSocketUpgrade,
        headers: HeaderMap,
        query: Query<WebSocketQuery>,
        state: State<AppState>,
    ) -> Response {
        trace!("WebSocket upgrade request received");
        trace!("Headers: {:?}", headers);
        trace!("Query parameters: {:?}", query);

        // Validate MCP subprotocol as required by Claude Code IDE integration
        if let Err(error) = self.validate_mcp_subprotocol(&headers).await {
            warn!("WebSocket connection rejected: MCP subprotocol validation failed");
            return error.into_response();
        }

        let manager = self.clone();

        ws_upgrade
            .protocols(["mcp"]) // Explicitly accept only the "mcp" subprotocol
            .on_upgrade(move |socket| manager.handle_socket(socket, headers, query.0, state.0))
    }

    /// Handle WebSocket communication for a client
    async fn handle_socket(
        self,
        socket: WebSocket,
        headers: HeaderMap,
        query: WebSocketQuery,
        state: AppState,
    ) {
        let client_id = Uuid::new_v4().to_string();
        info!("New WebSocket connection attempt: client_id={}", client_id);
        trace!("Socket split starting for client: {}", client_id);

        // Authenticate connection
        trace!("Starting authentication for client: {}", client_id);
        if let Err(e) = self.authenticate_connection(&headers, &query, &state).await {
            warn!(
                "WebSocket authentication failed for client_id={}: error={}",
                client_id, e
            );
            trace!("Closing socket due to authentication failure");
            return;
        }
        info!(
            "WebSocket authentication successful for client_id={}",
            client_id
        );

        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();
        trace!(
            "WebSocket streams and channels created for client: {}",
            client_id
        );

        // Spawn task to handle outgoing messages
        let client_id_clone = client_id.clone();
        trace!(
            "Spawning outgoing message handler for client: {}",
            client_id
        );
        tokio::spawn(async move {
            trace!(
                "Outgoing message handler started for client: {}",
                client_id_clone
            );
            while let Some(msg) = rx.recv().await {
                trace!("Sending message to client {}: {:?}", client_id_clone, msg);
                if sender.send(msg).await.is_err() {
                    warn!(
                        "Failed to send message to client {}, connection broken",
                        client_id_clone
                    );
                    break;
                }
                trace!("Message sent successfully to client: {}", client_id_clone);
            }
            trace!(
                "Outgoing message handler ended for client: {}",
                client_id_clone
            );
        });

        // Initial capability negotiation
        trace!("Starting capability negotiation for client: {}", client_id);
        let capabilities = self.negotiate_capabilities(&tx).await;
        trace!(
            "Capability negotiation completed for client {}: {:?}",
            client_id,
            capabilities
        );

        // Register client connection
        let connection = ClientConnection {
            client_id: client_id.clone(),
            sender: tx.clone(),
            capabilities: capabilities.clone(),
            connected_at: chrono::Utc::now(),
        };

        self.clients.insert(client_id.clone(), connection);
        info!(
            "WebSocket client connected successfully: client_id={}, capabilities={:?}, client_info={:?}",
            client_id,
            capabilities,
            capabilities.client_info
        );
        trace!("Client {} registered in client registry", client_id);

        // Handle incoming messages
        trace!("Starting message reception loop for client: {}", client_id);
        while let Some(msg) = receiver.next().await {
            trace!(
                "Received WebSocket message from client {}: {:?}",
                client_id,
                msg
            );
            match msg {
                Ok(Message::Text(text)) => {
                    trace!(
                        "Processing text message from client {}: (message logged in handle_message)",
                        client_id
                    );
                    if let Err(e) = self.handle_message(&client_id, &text, &state).await {
                        error!(
                            "Error handling message from client_id={}: error={}, full_message={}",
                            client_id, e, text
                        );
                    }
                }
                Ok(Message::Close(close_frame)) => {
                    info!(
                        "Client {} disconnected with close frame: {:?}",
                        client_id, close_frame
                    );
                    break;
                }
                Ok(Message::Ping(data)) => {
                    trace!("Received ping from client {}, sending pong", client_id);
                    if tx.send(Message::Pong(data.clone())).is_err() {
                        warn!("Failed to send pong to client {}", client_id);
                        break;
                    }
                    trace!("Pong sent to client {}", client_id);
                }
                Ok(Message::Pong(data)) => {
                    trace!("Received pong from client {}: {:?}", client_id, data);
                }
                Ok(Message::Binary(data)) => {
                    warn!(
                        "Received unexpected binary message from client {}: {} bytes",
                        client_id,
                        data.len()
                    );
                }
                Err(e) => {
                    error!("WebSocket error for client_id={}: error={}", client_id, e);
                    trace!("WebSocket error details: {:?}", e);
                    break;
                }
            }
        }

        // Cleanup on disconnect
        trace!("Starting cleanup for disconnected client: {}", client_id);
        self.clients.remove(&client_id);
        self.tool_registry.remove_client_tools(&client_id);
        info!("Cleaned up client {}", client_id);
        trace!("Client {} fully removed from all registries", client_id);
    }

    /// Validate MCP subprotocol as required by Claude Code IDE integration
    async fn validate_mcp_subprotocol(&self, headers: &HeaderMap) -> Result<()> {
        trace!("Starting MCP subprotocol validation");

        // Check for Sec-WebSocket-Protocol header
        if let Some(protocol_header) = headers.get("sec-websocket-protocol") {
            trace!("Found Sec-WebSocket-Protocol header");
            if let Ok(protocol_str) = protocol_header.to_str() {
                trace!("Protocol header value: {}", protocol_str);

                // Check if "mcp" is among the requested protocols
                // The header can contain multiple protocols separated by commas
                let protocols: Vec<&str> = protocol_str.split(',').map(|s| s.trim()).collect();

                if protocols.contains(&"mcp") {
                    trace!("MCP subprotocol found in requested protocols");
                    return Ok(());
                } else {
                    warn!(
                        "MCP subprotocol not found in requested protocols: {:?}",
                        protocols
                    );
                }
            } else {
                warn!("Failed to parse Sec-WebSocket-Protocol header as string");
            }
        } else {
            warn!("No Sec-WebSocket-Protocol header found");
        }

        // Return proper HTTP error response for protocol validation failure
        Err(AppError::WebSocketProtocolError(
            "WebSocket connection requires 'mcp' subprotocol".to_string(),
        ))
    }

    /// Authenticate WebSocket connection
    async fn authenticate_connection(
        &self,
        headers: &HeaderMap,
        query: &WebSocketQuery,
        state: &AppState,
    ) -> Result<()> {
        trace!("Starting WebSocket authentication");
        trace!(
            "Available headers: {:?}",
            headers.keys().collect::<Vec<_>>()
        );
        trace!(
            "Query parameters: token={:?}",
            query
                .token
                .as_ref()
                .map(|t| format!("{}...", &t[..t.len().min(8)]))
        );

        // Authentication is always required

        // Check for token in query parameters
        if let Some(token) = &query.token {
            trace!("Found token in query parameters, validating...");
            if self.validate_token(token, state).await {
                info!("WebSocket authentication successful via query parameters");
                return Ok(());
            }
            trace!("Query token validation failed");
        } else {
            trace!("No token found in query parameters");
        }

        // Check for token in headers (Claude Code style)
        if let Some(auth_header) = headers.get("x-claude-code-ide-authorization") {
            trace!("Found x-claude-code-ide-authorization header");
            if let Ok(token) = auth_header.to_str() {
                trace!("Successfully parsed authorization header, validating token...");
                if self.validate_token(token, state).await {
                    info!("WebSocket authentication successful via Claude Code IDE authorization header");
                    return Ok(());
                }
                trace!("Claude Code authorization header validation failed");
            } else {
                warn!("Failed to parse x-claude-code-ide-authorization header as string");
            }
        } else {
            trace!("No x-claude-code-ide-authorization header found");
        }

        // Check for token in x-api-key header (alternative auth method)
        if let Some(api_key_header) = headers.get("x-api-key") {
            trace!("Found x-api-key header");
            if let Ok(token) = api_key_header.to_str() {
                trace!("Successfully parsed x-api-key header, validating token...");
                if self.validate_token(token, state).await {
                    info!("WebSocket authentication successful via API key header");
                    return Ok(());
                }
                trace!("API key header validation failed");
            } else {
                warn!("Failed to parse x-api-key header as string");
            }
        } else {
            trace!("No x-api-key header found");
        }

        warn!("WebSocket authentication failed: All authentication methods rejected");
        Err(AppError::BadRequest(
            "Invalid or missing authentication token".to_string(),
        ))
    }

    /// Validate authentication token
    async fn validate_token(&self, token: &str, state: &AppState) -> bool {
        trace!(
            "Starting token validation for token: {}...",
            &token[..token.len().min(8)]
        );

        // Basic token format validation
        if token.is_empty() || token.len() < 8 || token.len() > 256 {
            trace!("Token failed format validation: length={}", token.len());
            return false;
        }

        // Check for valid characters (alphanumeric, hyphens, underscores)
        if !token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            trace!("Token failed character validation: contains invalid characters");
            return false;
        }
        trace!("Token passed format validation");

        // Check against auth manager (primary method)
        trace!("Checking token against auth manager");
        if state.auth_manager.validate_token(token) {
            trace!("Token validation successful via auth manager");
            return true;
        }
        trace!("Token validation failed via auth manager");

        // Legacy fallback: Check against websocket_token field in state
        if let Some(expected_token) = &state.websocket_token {
            trace!("Checking token against state.websocket_token");
            let result = self.constant_time_compare(token, expected_token);
            if result {
                trace!("Token validation successful via state.websocket_token");
                return true;
            }
            trace!("Token validation failed via state.websocket_token");
        } else {
            trace!("No state.websocket_token configured");
        }

        // Fall back to environment variable
        if let Ok(expected_token) = std::env::var("WEBSOCKET_AUTH_TOKEN") {
            trace!("Checking token against WEBSOCKET_AUTH_TOKEN environment variable");
            let result = self.constant_time_compare(token, &expected_token);
            if result {
                trace!("Token validation successful via environment variable");
                return true;
            }
            trace!("Token validation failed via environment variable");
        } else {
            trace!("No WEBSOCKET_AUTH_TOKEN environment variable set");
        }

        // If no configured token found, reject
        warn!("WebSocket authentication required but no valid token found in any source");
        false
    }

    /// Constant-time string comparison to prevent timing attacks
    fn constant_time_compare(&self, a: &str, b: &str) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let mut result = 0u8;
        for (byte_a, byte_b) in a.bytes().zip(b.bytes()) {
            result |= byte_a ^ byte_b;
        }
        result == 0
    }

    /// Negotiate client capabilities
    async fn negotiate_capabilities(
        &self,
        _tx: &mpsc::UnboundedSender<Message>,
    ) -> ClientCapabilities {
        // Default capabilities - in practice this would be negotiated
        ClientCapabilities {
            bidirectional: true,
            tools: vec![],
            client_info: ClientInfo {
                name: "Unknown Client".to_string(),
                version: "1.0.0".to_string(),
                environment: "unknown".to_string(),
            },
            mcp_capabilities: None, // Will be set during initialize handshake
        }
    }

    /// Handle incoming JSON-RPC message
    async fn handle_message(&self, client_id: &str, message: &str, state: &AppState) -> Result<()> {
        // Log all incoming messages at INFO level with full content
        info!(
            "WebSocket message received from client_id={}, full_message={}",
            client_id, message
        );

        let request: JsonRpcRequest = match serde_json::from_str::<JsonRpcRequest>(message) {
            Ok(req) => {
                trace!(
                    "Successfully parsed JSON-RPC request: method={}, id={:?}",
                    req.method,
                    req.id
                );
                req
            }
            Err(e) => {
                error!(
                    "Failed to parse JSON-RPC request from client_id={}: error={}, full_message={}",
                    client_id, e, message
                );
                return Err(e.into());
            }
        };

        trace!(
            "Routing message: method={}, client_id={}",
            request.method,
            client_id
        );

        match request.method.as_str() {
            // WebSocket-specific methods that need special handling
            "tools/register" => {
                trace!("Handling tools/register for client_id={}", client_id);
                self.handle_tool_registration(client_id, &request).await
            }
            "initialize" => {
                trace!("Handling initialize for client_id={}", client_id);

                // Store client capabilities from initialize request before handling
                if let Some(params) = &request.params {
                    if let Ok(init_request) =
                        serde_json::from_value::<super::types::InitializeRequest>(params.clone())
                    {
                        if let Some(mut client) = self.clients.get_mut(client_id) {
                            client.capabilities.mcp_capabilities = Some(init_request.capabilities);
                            trace!("Stored MCP capabilities for client_id={}", client_id);
                        }
                    }
                }

                let response = state.mcp_server.handle_request(state, request).await;
                let response_value = serde_json::to_value(&response)?;
                self.send_message(client_id, &response_value).await
            }
            "notifications/initialized" => {
                trace!(
                    "Handling notifications/initialized for client_id={}",
                    client_id
                );
                self.handle_initialized(client_id).await
            }
            "getDiagnostics" => {
                trace!("Handling getDiagnostics for client_id={}", client_id);
                self.handle_get_diagnostics(client_id, &request, state)
                    .await
            }

            // Check if this is a response to a server-initiated request
            _ if request.id.is_some() => {
                if let Some(id) = &request.id {
                    let id_str = id.to_string();
                    if self.pending_requests.contains_key(&id_str) {
                        trace!(
                            "Found pending request with id={}, handling as response",
                            id_str
                        );
                        return self.handle_response(client_id, message).await;
                    }
                    trace!(
                        "No pending request found for id={}, treating as regular request",
                        id_str
                    );
                }
                // Fall through to unified handler for regular requests
                trace!(
                    "Forwarding request to MCP server: method={}",
                    request.method
                );
                let response = state.mcp_server.handle_request(state, request).await;
                let response_value = serde_json::to_value(&response)?;
                trace!(
                    "Sending MCP response to client_id={}: {:?}",
                    client_id,
                    response_value
                );
                self.send_message(client_id, &response_value).await
            }

            // All other methods (including standard MCP) forwarded to unified handler
            _ => {
                trace!(
                    "Forwarding request to MCP server: method={}",
                    request.method
                );
                let response = state.mcp_server.handle_request(state, request).await;
                let response_value = serde_json::to_value(&response)?;
                trace!(
                    "Sending MCP response to client_id={}: {:?}",
                    client_id,
                    response_value
                );
                self.send_message(client_id, &response_value).await
            }
        }
    }

    /// Handle tool registration from client
    async fn handle_tool_registration(
        &self,
        client_id: &str,
        request: &JsonRpcRequest,
    ) -> Result<()> {
        trace!("Processing tool registration from client_id={}", client_id);

        if let Some(params) = &request.params {
            trace!("Tool registration params: {:?}", params);
            if let Ok(tool_def) = serde_json::from_value::<ClientToolDefinition>(params.clone()) {
                trace!(
                    "Successfully parsed tool definition: name={}, description={}",
                    tool_def.name,
                    tool_def.description
                );

                let mut tool = tool_def;
                tool.client_id = client_id.to_string();
                tool.registered_at = chrono::Utc::now();

                self.tool_registry.register_tool(tool.clone());

                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.id,
                    "result": {
                        "registered": true,
                        "tool_name": tool.name
                    }
                });

                info!("Registered tool '{}' from client {}", tool.name, client_id);
                trace!(
                    "Sending registration success response to client_id={}",
                    client_id
                );
                return self.send_message(client_id, &response).await;
            } else {
                error!(
                    "Failed to parse tool definition from client_id={}: invalid format",
                    client_id
                );
            }
        } else {
            error!(
                "Tool registration from client_id={} missing params",
                client_id
            );
        }

        let error_response = json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "error": {
                "code": -32602,
                "message": "Invalid tool registration parameters"
            }
        });

        warn!(
            "Sending tool registration error response to client_id={}",
            client_id
        );
        self.send_message(client_id, &error_response).await
    }

    /// Handle initialized notification
    async fn handle_initialized(&self, client_id: &str) -> Result<()> {
        info!("Client {} completed initialization", client_id);
        Ok(())
    }

    /// Handle getDiagnostics request to return unprocessed events
    async fn handle_get_diagnostics(
        &self,
        client_id: &str,
        request: &super::types::JsonRpcRequest,
        state: &AppState,
    ) -> Result<()> {
        trace!("Handling getDiagnostics for client {}", client_id);

        // Get unprocessed events from the database
        let unprocessed_events =
            match crate::database::events::Event::get_unprocessed(&state.db).await {
                Ok(events) => events,
                Err(e) => {
                    error!("Failed to fetch unprocessed events: {}", e);
                    vec![]
                }
            };

        let event_count = unprocessed_events.len();
        let summary_text = if event_count == 0 {
            "No unprocessed events".to_string()
        } else {
            format!("{} unprocessed events requiring attention", event_count)
        };

        // Convert events to structured format
        let structured_events: Vec<serde_json::Value> = unprocessed_events
            .into_iter()
            .map(|event| {
                serde_json::json!({
                    "id": event.id,
                    "event_type": event.event_type,
                    "ticket_id": event.ticket_id,
                    "worker_id": event.worker_id,
                    "stage": event.stage,
                    "reason": event.reason,
                    "processed": event.processed,
                    "created_at": event.created_at,
                    "resolution_summary": event.resolution_summary
                })
            })
            .collect();

        // Create the response in the requested format
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": summary_text
                    }
                ],
                "structuredContent": {
                    "events": structured_events
                }
            }
        });

        let response_text = response.to_string();
        trace!("Sending getDiagnostics response: {}", response_text);

        if let Err(e) = self
            .clients
            .get(client_id)
            .ok_or_else(|| AppError::BadRequest(format!("Client {} not found", client_id)))?
            .sender
            .send(Message::Text(response_text))
        {
            error!(
                "Failed to send getDiagnostics response to {}: {}",
                client_id, e
            );
            return Err(AppError::Internal(anyhow::anyhow!(
                "Failed to send message: {}",
                e
            )));
        }

        info!(
            "Sent getDiagnostics response with {} events to client {}",
            event_count, client_id
        );
        Ok(())
    }

    /// Handle response to server-initiated request
    async fn handle_response(&self, _client_id: &str, message: &str) -> Result<()> {
        // Parse as a JSON-RPC response
        if let Ok(response) = serde_json::from_str::<Value>(message) {
            if let Some(id) = response.get("id").and_then(|v| v.as_str()) {
                if let Some((_, pending)) = self.pending_requests.remove(id) {
                    let result = if let Some(result) = response.get("result") {
                        Ok(result.clone())
                    } else if let Some(error) = response.get("error") {
                        Err(AppError::BadRequest(format!("Client error: {}", error)))
                    } else {
                        Err(AppError::BadRequest("Invalid response format".to_string()))
                    };

                    if pending.response_sender.send(result).is_err() {
                        warn!("Failed to send response for request {}", id);
                    }
                }
            }
        }
        Ok(())
    }

    /// Send message to client (public method for orchestration tools)
    pub async fn send_message(&self, client_id: &str, message: &Value) -> Result<()> {
        trace!(
            "Attempting to send message to client_id={}: {:?}",
            client_id,
            message
        );

        if let Some(client) = self.clients.get(client_id) {
            let text = match serde_json::to_string(message) {
                Ok(text) => {
                    trace!(
                        "Successfully serialized message for client_id={}: {}",
                        client_id,
                        text
                    );
                    text
                }
                Err(e) => {
                    error!(
                        "Failed to serialize message for client_id={}: error={}",
                        client_id, e
                    );
                    return Err(e.into());
                }
            };

            if client.sender.send(Message::Text(text)).is_err() {
                error!(
                    "Failed to send message to client_id={}: channel closed",
                    client_id
                );
                return Err(AppError::BadRequest(
                    "Failed to send message to client".to_string(),
                ));
            }
            trace!("Message sent successfully to client_id={}", client_id);
        } else {
            warn!(
                "Attempted to send message to non-existent client_id={}",
                client_id
            );
        }
        Ok(())
    }

    /// Call a tool on a client (server-initiated request)
    pub async fn call_client_tool(
        &self,
        client_id: &str,
        tool_name: &str,
        arguments: Value,
        timeout_secs: u64,
    ) -> Result<Value> {
        info!(
            "Initiating client tool call: client_id={}, tool_name={}, timeout={}s",
            client_id, tool_name, timeout_secs
        );
        trace!("Tool call arguments: {:?}", arguments);

        // Acquire semaphore permit if concurrency limiting is enabled
        let _permit = if let Some(semaphore) = &self.concurrency_semaphore {
            trace!("Acquiring concurrency semaphore permit");
            Some(semaphore.acquire().await.map_err(|_| {
                error!("Concurrency limit reached for client tool calls");
                AppError::BadRequest("Concurrency limit reached for client tool calls".to_string())
            })?)
        } else {
            trace!("No concurrency limit configured");
            None
        };

        let request_id = Uuid::new_v4().to_string();
        trace!("Generated request_id={} for tool call", request_id);

        let request = json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            },
            "id": request_id
        });

        let (tx, rx) = tokio::sync::oneshot::channel();

        let pending = PendingRequest {
            request_id: request_id.clone(),
            client_id: client_id.to_string(),
            tool_name: tool_name.to_string(),
            response_sender: tx,
            created_at: chrono::Utc::now(),
        };

        trace!("Registering pending request: request_id={}", request_id);
        self.pending_requests.insert(request_id.clone(), pending);

        // Send request to client
        trace!("Sending tool call request to client: {:?}", request);
        self.send_message(client_id, &request).await?;
        trace!("Tool call request sent successfully");

        // Wait for response with timeout
        trace!("Waiting for response with timeout: {}s", timeout_secs);
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(result)) => {
                trace!(
                    "Received successful response for request_id={}: {:?}",
                    request_id,
                    result
                );
                result
            }
            Ok(Err(_)) => {
                warn!("Request cancelled for request_id={}", request_id);
                Err(AppError::BadRequest("Request cancelled".to_string()))
            }
            Err(_) => {
                warn!(
                    "Request timeout for request_id={} after {}s",
                    request_id, timeout_secs
                );
                self.pending_requests.remove(&request_id);
                Err(AppError::BadRequest("Request timeout".to_string()))
            }
        }
    }

    /// List connected clients
    pub fn list_clients(&self) -> Vec<String> {
        self.clients
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Send MCP notifications for an event to a specific client
    async fn send_mcp_notifications(
        &self,
        client_id: &str,
        client: &ClientConnection,
        event_payload: &crate::events::EventPayload,
    ) {
        use super::types::*;

        // Check if client supports the necessary MCP capabilities
        let _has_sampling = client
            .capabilities
            .mcp_capabilities
            .as_ref()
            .and_then(|caps| caps.sampling.as_ref())
            .map(|sampling| sampling.enabled)
            .unwrap_or(false);

        let has_logging = client
            .capabilities
            .mcp_capabilities
            .as_ref()
            .and_then(|caps| caps.logging.as_ref())
            .map(|logging| logging.enabled)
            .unwrap_or(false);

        let has_resources = client
            .capabilities
            .mcp_capabilities
            .as_ref()
            .and_then(|caps| caps.resources.as_ref())
            .map(|resources| resources.subscribe)
            .unwrap_or(false);

        // 1. Send notifications/message for user-friendly event description
        if has_logging {
            let level = match &event_payload.event_type {
                crate::events::EventType::WorkerSpawned => "info",
                crate::events::EventType::WorkerFinished => "info",
                crate::events::EventType::WorkerFailed => "error",
                crate::events::EventType::TicketCreated => "info",
                crate::events::EventType::TicketClosed => "info",
                crate::events::EventType::TicketUpdated => "info",
                crate::events::EventType::TicketStageChanged => "info",
                crate::events::EventType::TicketUnblocked => "info",
                crate::events::EventType::QueueUpdated => "info",
                crate::events::EventType::SystemInit => "info",
                crate::events::EventType::SystemMessage => "info",
                crate::events::EventType::EndpointDiscovery => "info",
            };

            let user_friendly_data = self.format_user_friendly_event(event_payload);

            let notification_message = JsonRpcNotification::new(
                "notifications/message",
                Some(
                    serde_json::to_value(NotificationMessage {
                        level: level.to_string(),
                        logger: Some("vibe-ensemble".to_string()),
                        data: user_friendly_data,
                        _meta: Some(serde_json::json!({
                            "event_type": format!("{:?}", event_payload.event_type),
                            "timestamp": event_payload.timestamp
                        })),
                    })
                    .unwrap_or(serde_json::Value::Null),
                ),
            );

            let message_text = notification_message.to_string();
            if let Err(e) = client.sender.send(Message::Text(message_text.clone())) {
                error!(
                    "Failed to send notifications/message to client {}: {}",
                    client_id, e
                );
            } else {
                trace!(
                    "Sent notifications/message to client {}: {}",
                    client_id,
                    message_text
                );
            }
        }

        // 2. Send notifications/resources/updated with stable URI
        if has_resources {
            let resource_updated = JsonRpcNotification::new(
                "notifications/resources/updated",
                Some(
                    serde_json::to_value(ResourceUpdated {
                        uri: "ide://events".to_string(),
                        _meta: Some(serde_json::json!({
                            "event_type": format!("{:?}", event_payload.event_type),
                            "timestamp": event_payload.timestamp
                        })),
                    })
                    .unwrap_or(serde_json::Value::Null),
                ),
            );

            let message_text = resource_updated.to_string();
            if let Err(e) = client.sender.send(Message::Text(message_text.clone())) {
                error!(
                    "Failed to send notifications/resources/updated to client {}: {}",
                    client_id, e
                );
            } else {
                trace!(
                    "Sent notifications/resources/updated to client {}: {}",
                    client_id,
                    message_text
                );
            }
        }

        // 3. Send sampling/createMessage for supported clients
        // TEMPORARILY DISABLED per user request
        /*
        if has_sampling {
            let sampling_message = JsonRpcNotification::new(
                "sampling/createMessage",
                Some(serde_json::to_value(SamplingCreateMessage {
                    messages: vec![SamplingMessage {
                        role: "user".to_string(),
                        content: SamplingContent {
                            content_type: "text".to_string(),
                            text: "New IDE events available. Call list_events now and summarize the key changes.".to_string(),
                        },
                    }],
                    include_context: "thisServer".to_string(),
                    max_tokens: 200,
                }).unwrap_or(serde_json::Value::Null))
            );

            let message_text = sampling_message.to_string();
            if let Err(e) = client.sender.send(Message::Text(message_text.clone())) {
                error!(
                    "Failed to send sampling/createMessage to client {}: {}",
                    client_id, e
                );
            } else {
                trace!(
                    "Sent sampling/createMessage to client {}: {}",
                    client_id,
                    message_text
                );
            }
        }
        */
    }

    /// Format event data in a user-friendly way for notifications/message
    fn format_user_friendly_event(
        &self,
        event_payload: &crate::events::EventPayload,
    ) -> serde_json::Value {
        use crate::events::{EventData, EventType};

        match (&event_payload.event_type, &event_payload.data) {
            (EventType::WorkerSpawned, EventData::Worker(worker_data)) => {
                serde_json::json!({
                    "kind": "worker_spawned",
                    "message": format!("Spawned worker {} ({}) for project '{}'", worker_data.worker_id, worker_data.worker_type, worker_data.project_id),
                    "project_id": worker_data.project_id,
                    "worker_type": worker_data.worker_type,
                    "worker_id": worker_data.worker_id
                })
            }
            (EventType::WorkerFinished, EventData::Worker(worker_data)) => {
                serde_json::json!({
                    "kind": "worker_finished",
                    "message": format!("Completed worker {} ({}) for project '{}'", worker_data.worker_id, worker_data.worker_type, worker_data.project_id),
                    "project_id": worker_data.project_id,
                    "worker_type": worker_data.worker_type,
                    "worker_id": worker_data.worker_id
                })
            }
            (EventType::WorkerFailed, EventData::Worker(worker_data)) => {
                serde_json::json!({
                    "kind": "worker_failed",
                    "message": format!("Failed worker {} ({}) for project '{}'", worker_data.worker_id, worker_data.worker_type, worker_data.project_id),
                    "project_id": worker_data.project_id,
                    "worker_type": worker_data.worker_type,
                    "worker_id": worker_data.worker_id
                })
            }
            (EventType::TicketCreated, EventData::Ticket(ticket_data)) => {
                serde_json::json!({
                    "kind": "ticket_created",
                    "message": format!("Created ticket #{} in project '{}'", ticket_data.ticket_id, ticket_data.project_id),
                    "project_id": ticket_data.project_id,
                    "ticket_id": ticket_data.ticket_id,
                    "stage": ticket_data.stage
                })
            }
            (EventType::TicketClosed, EventData::Ticket(ticket_data)) => {
                serde_json::json!({
                    "kind": "ticket_closed",
                    "message": format!("Closed ticket #{} in project '{}'", ticket_data.ticket_id, ticket_data.project_id),
                    "project_id": ticket_data.project_id,
                    "ticket_id": ticket_data.ticket_id
                })
            }
            (EventType::TicketStageChanged, EventData::Ticket(ticket_data)) => {
                serde_json::json!({
                    "kind": "ticket_stage_changed",
                    "message": format!("Changed stage for ticket #{} in project '{}': {}", ticket_data.ticket_id, ticket_data.project_id, ticket_data.change_type),
                    "project_id": ticket_data.project_id,
                    "ticket_id": ticket_data.ticket_id,
                    "stage": ticket_data.stage,
                    "change": ticket_data.change_type
                })
            }
            (EventType::TicketUnblocked, EventData::Ticket(ticket_data)) => {
                serde_json::json!({
                    "kind": "ticket_unblocked",
                    "message": format!("Unblocked ticket #{} in project '{}'", ticket_data.ticket_id, ticket_data.project_id),
                    "project_id": ticket_data.project_id,
                    "ticket_id": ticket_data.ticket_id
                })
            }
            (EventType::QueueUpdated, EventData::Queue(queue_data)) => {
                serde_json::json!({
                    "kind": "queue_updated",
                    "message": format!("Queue '{}' updated: {} tasks for {} workers in project '{}'", queue_data.queue_name, queue_data.task_count, queue_data.worker_type, queue_data.project_id),
                    "project_id": queue_data.project_id,
                    "queue_name": queue_data.queue_name,
                    "task_count": queue_data.task_count,
                    "worker_type": queue_data.worker_type
                })
            }
            (EventType::SystemInit, EventData::System(system_data)) => {
                serde_json::json!({
                    "kind": "system_init",
                    "message": format!("{}: {}", system_data.component, system_data.message),
                    "component": system_data.component
                })
            }
            (EventType::SystemMessage, EventData::System(system_data)) => {
                serde_json::json!({
                    "kind": "system_message",
                    "message": format!("{}: {}", system_data.component, system_data.message),
                    "component": system_data.component
                })
            }
            _ => {
                serde_json::json!({
                    "kind": "event",
                    "message": format!("Event: {:?}", event_payload.event_type)
                })
            }
        }
    }

    /// Event broadcasting loop that forwards events to all connected WebSocket clients
    async fn event_broadcasting_loop(&self) {
        if let Some(event_broadcaster) = &self.event_broadcaster {
            let mut receiver = event_broadcaster.subscribe_websocket();

            loop {
                match receiver.recv().await {
                    Ok(event_payload) => {
                        // Convert event to JSON-RPC notification format
                        // TEMPORARILY DISABLED per user request - sampling/createMessage disabled
                        // let notification = event_payload.to_jsonrpc_notification();
                        let client_count = self.clients.len();

                        info!(
                            "WebSocket delivering event: type={}, clients={}",
                            serde_json::to_string(&event_payload.event_type)
                                .unwrap_or_else(|_| "unknown".to_string()),
                            client_count
                        );

                        // Log the complete JSON-RPC message being sent to WebSocket clients
                        // TEMPORARILY DISABLED per user request - sampling/createMessage disabled
                        /*
                        trace!(
                            "WebSocket JSON-RPC message: {}",
                            serde_json::to_string_pretty(&notification).unwrap_or_else(|_| {
                                "Failed to serialize JSON-RPC message".to_string()
                            })
                        );
                        */

                        // Broadcast to all connected WebSocket clients
                        let clients_to_remove =
                            Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
                        let mut successful_deliveries = 0;

                        for entry in self.clients.iter() {
                            let client_id = entry.key().clone();
                            let client = entry.value().clone();

                            // Send event to client - TEMPORARILY DISABLED (sampling/createMessage disabled)
                            // Only send MCP notifications now

                            // Count as successful for MCP notifications
                            successful_deliveries += 1;

                            // Send MCP notifications for each event
                            self.send_mcp_notifications(&client_id, &client, &event_payload)
                                .await;
                        }

                        // Remove broken client connections
                        let to_remove = clients_to_remove.lock().unwrap();
                        for client_id in to_remove.iter() {
                            self.clients.remove(client_id);
                            self.tool_registry.remove_client_tools(client_id);
                            info!("Removed broken WebSocket client: {}", client_id);
                        }

                        info!(
                            "WebSocket event delivery completed: {}/{} clients successful",
                            successful_deliveries, client_count
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(
                            "WebSocket event broadcaster lagged, skipped {} events",
                            skipped
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        info!("WebSocket event broadcaster closed, stopping event loop");
                        break;
                    }
                }
            }
        }
    }
}

impl Clone for WebSocketManager {
    fn clone(&self) -> Self {
        Self {
            clients: Arc::clone(&self.clients),
            tool_registry: Arc::clone(&self.tool_registry),
            pending_requests: Arc::clone(&self.pending_requests),
            concurrency_semaphore: self.concurrency_semaphore.clone(),
            event_broadcaster: self.event_broadcaster.clone(),
        }
    }
}

impl ClientToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: Arc::new(DashMap::new()),
            client_capabilities: Arc::new(DashMap::new()),
        }
    }

    /// Register a tool from a client
    pub fn register_tool(&self, tool: ClientToolDefinition) {
        let key = format!("{}:{}", tool.client_id, tool.name);
        self.tools.insert(key, tool);
    }

    /// Remove all tools from a client
    pub fn remove_client_tools(&self, client_id: &str) {
        self.tools
            .retain(|key, _| !key.starts_with(&format!("{}:", client_id)));
        self.client_capabilities.remove(client_id);
    }

    /// List all available client tools
    pub fn list_tools(&self) -> Vec<ClientToolDefinition> {
        self.tools
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get tool by name and client
    pub fn get_tool(&self, client_id: &str, tool_name: &str) -> Option<ClientToolDefinition> {
        let key = format!("{}:{}", client_id, tool_name);
        self.tools.get(&key).map(|entry| entry.value().clone())
    }
}

impl Default for ClientToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
