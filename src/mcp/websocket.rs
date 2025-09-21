use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::HeaderMap;
use axum::response::Response;
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::types::JsonRpcRequest;
use crate::{error::AppError, server::AppState};

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
        }
    }

    /// Create a WebSocket manager with concurrency limits
    pub fn with_concurrency_limit(max_concurrent: usize) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            tool_registry: Arc::new(ClientToolRegistry::new()),
            pending_requests: Arc::new(DashMap::new()),
            concurrency_semaphore: Some(Arc::new(Semaphore::new(max_concurrent))),
        }
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
        let manager = self.clone();

        ws_upgrade
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
        info!("New WebSocket connection attempt: {}", client_id);

        // Authenticate connection
        if let Err(e) = self.authenticate_connection(&headers, &query, &state).await {
            warn!("WebSocket authentication failed for {}: {}", client_id, e);
            return;
        }

        let (mut sender, mut receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();

        // Spawn task to handle outgoing messages
        let client_id_clone = client_id.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sender.send(msg).await.is_err() {
                    debug!("Failed to send message to client {}", client_id_clone);
                    break;
                }
            }
        });

        // Initial capability negotiation
        let capabilities = self.negotiate_capabilities(&tx).await;

        // Register client connection
        let connection = ClientConnection {
            client_id: client_id.clone(),
            sender: tx.clone(),
            capabilities: capabilities.clone(),
            connected_at: chrono::Utc::now(),
        };

        self.clients.insert(client_id.clone(), connection);
        info!(
            "Client {} connected with capabilities: {:?}",
            client_id, capabilities
        );

        // Handle incoming messages
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Err(e) = self.handle_message(&client_id, &text, &state).await {
                        error!("Error handling message from {}: {}", client_id, e);
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Client {} disconnected", client_id);
                    break;
                }
                Ok(Message::Ping(data)) => {
                    if tx.send(Message::Pong(data)).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    error!("WebSocket error for client {}: {}", client_id, e);
                    break;
                }
                _ => {}
            }
        }

        // Cleanup on disconnect
        self.clients.remove(&client_id);
        self.tool_registry.remove_client_tools(&client_id);
        info!("Cleaned up client {}", client_id);
    }

    /// Authenticate WebSocket connection
    async fn authenticate_connection(
        &self,
        headers: &HeaderMap,
        query: &WebSocketQuery,
        state: &AppState,
    ) -> Result<()> {
        // If authentication is not required, allow connection
        if !state.config.websocket_auth_required {
            return Ok(());
        }

        // Check for token in query parameters
        if let Some(token) = &query.token {
            if self.validate_token(token, state).await {
                return Ok(());
            }
        }

        // Check for token in headers (Claude Code style)
        if let Some(auth_header) = headers.get("x-claude-code-ide-authorization") {
            if let Ok(token) = auth_header.to_str() {
                if self.validate_token(token, state).await {
                    return Ok(());
                }
            }
        }

        // Check for token in x-api-key header (alternative auth method)
        if let Some(api_key_header) = headers.get("x-api-key") {
            if let Ok(token) = api_key_header.to_str() {
                if self.validate_token(token, state).await {
                    return Ok(());
                }
            }
        }

        Err(AppError::BadRequest(
            "Invalid or missing authentication token".to_string(),
        ))
    }

    /// Validate authentication token
    async fn validate_token(&self, token: &str, state: &AppState) -> bool {
        // Basic token format validation
        if token.is_empty() || token.len() < 8 || token.len() > 256 {
            return false;
        }

        // Check for valid characters (alphanumeric, hyphens, underscores)
        if !token
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return false;
        }

        // If authentication is required, validate against server token
        if state.config.websocket_auth_required {
            // Check against server-generated token from AppState
            if let Some(expected_token) = &state.websocket_token {
                return self.constant_time_compare(token, expected_token);
            }

            // Fall back to environment variable
            if let Ok(expected_token) = std::env::var("WEBSOCKET_AUTH_TOKEN") {
                return self.constant_time_compare(token, &expected_token);
            }

            // If no configured token found, reject
            warn!("WebSocket authentication required but no token configured");
            return false;
        }

        // If authentication is optional, any non-empty token is valid
        true
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
        }
    }

    /// Handle incoming JSON-RPC message
    async fn handle_message(&self, client_id: &str, message: &str, state: &AppState) -> Result<()> {
        debug!("Received message from {}: {}", client_id, message);

        let request: JsonRpcRequest = serde_json::from_str(message)?;

        match request.method.as_str() {
            // WebSocket-specific methods handled locally
            "tools/register" => self.handle_tool_registration(client_id, &request).await,
            "notifications/initialized" => self.handle_initialized(client_id).await,

            // Standard MCP methods forwarded to unified handler
            "initialize" | "tools/list" | "tools/call" | "prompts/list" | "prompts/get" => {
                let response = state.mcp_server.handle_request(state, request).await;
                let response_value = serde_json::to_value(&response)?;
                self.send_message(client_id, &response_value).await
            }

            _ => {
                // Check if this is a response to a server-initiated request
                if request.id.is_some() {
                    self.handle_response(client_id, message).await
                } else {
                    warn!(
                        "Unknown method from client {}: {}",
                        client_id, request.method
                    );
                    Ok(())
                }
            }
        }
    }

    /// Handle tool registration from client
    async fn handle_tool_registration(
        &self,
        client_id: &str,
        request: &JsonRpcRequest,
    ) -> Result<()> {
        if let Some(params) = &request.params {
            if let Ok(tool_def) = serde_json::from_value::<ClientToolDefinition>(params.clone()) {
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
                return self.send_message(client_id, &response).await;
            }
        }

        let error_response = json!({
            "jsonrpc": "2.0",
            "id": request.id,
            "error": {
                "code": -32602,
                "message": "Invalid tool registration parameters"
            }
        });

        self.send_message(client_id, &error_response).await
    }

    /// Handle initialized notification
    async fn handle_initialized(&self, client_id: &str) -> Result<()> {
        info!("Client {} completed initialization", client_id);
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
        if let Some(client) = self.clients.get(client_id) {
            let text = serde_json::to_string(message)?;
            if client.sender.send(Message::Text(text)).is_err() {
                return Err(AppError::BadRequest(
                    "Failed to send message to client".to_string(),
                ));
            }
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
        // Acquire semaphore permit if concurrency limiting is enabled
        let _permit = if let Some(semaphore) = &self.concurrency_semaphore {
            Some(semaphore.acquire().await.map_err(|_| {
                AppError::BadRequest("Concurrency limit reached for client tool calls".to_string())
            })?)
        } else {
            None
        };
        let request_id = Uuid::new_v4().to_string();

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

        self.pending_requests.insert(request_id.clone(), pending);

        // Send request to client
        self.send_message(client_id, &request).await?;

        // Wait for response with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AppError::BadRequest("Request cancelled".to_string())),
            Err(_) => {
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
}

impl Clone for WebSocketManager {
    fn clone(&self) -> Self {
        Self {
            clients: Arc::clone(&self.clients),
            tool_registry: Arc::clone(&self.tool_registry),
            pending_requests: Arc::clone(&self.pending_requests),
            concurrency_semaphore: self.concurrency_semaphore.clone(),
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
