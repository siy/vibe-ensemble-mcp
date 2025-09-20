# Bidirectional MCP Communication Implementation Plan for Vibe Ensemble

## Executive Summary

Based on analysis of the Vibe Ensemble MCP Server codebase and the claudecode.nvim protocol documentation, this plan outlines implementing bidirectional MCP communication support. The current Vibe Ensemble implementation already has sophisticated unidirectional communication (server-to-client events via SSE) and standard MCP tools. The enhancement will add true bidirectional capabilities allowing the server to initiate tool calls to connected clients.

## Current Architecture Analysis

### Existing Implementation
**Vibe Ensemble MCP Server** (v0.9.1):
- **Transport**: HTTP + SSE dual-endpoint architecture
- **Protocol**: MCP 2024-11-05 compliant with JSON-RPC 2.0
- **Current Capabilities**:
  - 25 MCP tools across 5 categories
  - Real-time SSE event broadcasting
  - Multi-agent coordination system
  - Permission management with 3 modes

**Current Endpoints**:
- `/mcp` (POST) - Standard MCP over HTTP
- `/sse` (GET) - Server-to-client event streaming
- `/messages` (POST) - Client-to-server requests (SSE transport)

### claudecode.nvim Protocol Insights
**Key Bidirectional Features**:
- WebSocket-based transport with authentication
- Dynamic tool registration with JSON schema validation
- Tool discovery mechanism (`get_tool_list()`)
- Request-response pattern with unique IDs
- Error handling with `pcall()` patterns

## Proposed Bidirectional Architecture

### 1. Enhanced Transport Layer

#### WebSocket Integration
Add WebSocket support alongside existing HTTP/SSE:

```rust
// New file: src/mcp/websocket.rs
pub struct WebSocketHandler {
    clients: Arc<DashMap<String, WebSocketSender>>,
    tool_registry: Arc<ClientToolRegistry>,
}

pub struct ClientToolRegistry {
    tools: Arc<DashMap<String, ClientToolDefinition>>,
    client_capabilities: Arc<DashMap<String, ClientCapabilities>>,
}
```

#### Connection Management
- **Client Registration**: Store client capabilities and available tools
- **Authentication**: Token-based auth with lock file pattern (similar to claudecode.nvim)
- **Heartbeat**: Bidirectional ping/pong for connection health

### 2. Bidirectional Protocol Extensions

#### Server-Initiated Tool Calls
Extend JSON-RPC to support server-initiated requests:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "client_tool_name",
    "arguments": { /* tool arguments */ }
  },
  "id": "server-request-123"
}
```

#### Client Tool Registration
Allow clients to register their available tools:

```json
{
  "jsonrpc": "2.0",
  "method": "tools/register",
  "params": {
    "name": "file_edit",
    "description": "Edit a file in the client editor",
    "input_schema": {
      "type": "object",
      "properties": {
        "path": {"type": "string"},
        "content": {"type": "string"}
      }
    }
  }
}
```

### 3. Enhanced MCP Types

#### New Protocol Types
```rust
// src/mcp/types.rs additions

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub client_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInitiatedRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BidirectionalCapabilities {
    pub server_initiated_requests: bool,
    pub client_tool_registration: bool,
    pub tool_discovery: bool,
}
```

## Detailed Implementation Plan

### Phase 1: WebSocket Foundation (Week 1)

#### 1.1 Dependencies
Add to `Cargo.toml`:
```toml
# WebSocket support
tokio-tungstenite = "0.20"
uuid = { version = "1.0", features = ["v4", "serde"] }
dashmap = "5.5" # Already present
```

#### 1.2 WebSocket Server Implementation
**File**: `src/mcp/websocket.rs`
- Basic WebSocket server setup
- Client connection management
- Message routing between HTTP and WebSocket transports
- Authentication middleware

#### 1.3 Enhanced Server Configuration
**File**: `src/server.rs`
```rust
// Add WebSocket route
.route("/ws", get(websocket_handler))
```

#### 1.4 Connection State Management
**File**: `src/mcp/connection_manager.rs`
- Track active WebSocket connections
- Maintain client capability maps
- Handle connection lifecycle events

### Phase 2: Client Tool Registry (Week 2)

#### 2.1 Tool Registration System
**File**: `src/mcp/client_tools.rs`
- `ClientToolRegistry` implementation
- Tool validation and schema checking
- Client capability negotiation

#### 2.2 Enhanced Protocol Handling
**File**: `src/mcp/types.rs`
- New message types for bidirectional communication
- Enhanced capability negotiation
- Tool registration request/response types

#### 2.3 MCP Server Extensions
**File**: `src/mcp/server.rs`
- Handle `tools/register` method
- Enhanced `initialize` with bidirectional capabilities
- New `client_tools/list` method

### Phase 3: Server-Initiated Requests (Week 3)

#### 3.1 Request Orchestration
**File**: `src/mcp/server_requests.rs`
```rust
pub struct ServerRequestManager {
    pending_requests: Arc<DashMap<String, PendingRequest>>,
    client_connections: Arc<ClientConnectionManager>,
}

impl ServerRequestManager {
    pub async fn call_client_tool(
        &self,
        client_id: &str,
        tool_name: &str,
        arguments: Value
    ) -> Result<Value, McpError> {
        // Implementation
    }
}
```

#### 3.2 Enhanced Worker Integration
**File**: `src/workers/mod.rs`
- Enable workers to make server-initiated requests
- Tool calling abstractions for worker processes
- Integration with existing worker spawning system

#### 3.3 Timeout and Error Handling
- Request timeout management (30-second default)
- Error propagation from client to server
- Retry mechanisms for failed requests

### Phase 4: Enhanced Tools and Coordination (Week 4)

#### 4.1 New Bidirectional MCP Tools
```rust
// File: src/mcp/bidirectional_tools.rs

pub struct CallClientToolTool;
pub struct ListClientToolsTool;
pub struct RegisterServerCapabilitiesTool;
```

#### 4.2 Worker Process Enhancement
**File**: `src/workers/domain.rs`
- Add client tool calling capabilities to workers
- Enhanced worker process initialization with bidirectional support
- Integration with permission system

#### 4.3 Enhanced Event System
**File**: `src/events/mod.rs`
- New event types for bidirectional communication
- Client connection/disconnection events
- Tool registration/unregistration events

### Phase 5: Testing and Integration (Week 5)

#### 5.1 Unit Tests
- WebSocket connection handling
- Tool registration validation
- Server-initiated request flow
- Error handling scenarios

#### 5.2 Integration Tests
- End-to-end bidirectional communication
- Multi-client scenarios
- Worker process integration
- Permission system integration

#### 5.3 claudecode.nvim Compatibility Testing
- Verify compatibility with existing MCP tools
- Test WebSocket transport alongside HTTP/SSE
- Validate authentication flow

## Technical Specifications

### WebSocket Protocol Flow

#### 1. Connection Establishment
```
Client → Server: WebSocket handshake with auth token
Server → Client: Connection accepted, send capabilities
Client → Server: Tool registration
Server → Client: Registration acknowledgment
```

#### 2. Bidirectional Communication
```
// Server-initiated tool call
Server → Client: {"jsonrpc":"2.0","method":"tools/call","params":{...},"id":"req-123"}
Client → Server: {"jsonrpc":"2.0","id":"req-123","result":{...}}

// Client-initiated request (existing flow)
Client → Server: {"jsonrpc":"2.0","method":"tools/call","params":{...},"id":"client-456"}
Server → Client: {"jsonrpc":"2.0","id":"client-456","result":{...}}
```

### Enhanced Configuration

#### New Configuration Options
```rust
// src/config.rs additions
pub struct Config {
    // ... existing fields
    pub enable_websocket: bool,
    pub websocket_auth_required: bool,
    pub client_tool_timeout: Duration,
    pub max_concurrent_client_requests: usize,
}
```

#### Auto-Configuration Updates
**File**: `src/configure.rs`
```rust
// Enhanced .mcp.json generation
let config = json!({
    "mcpServers": {
        "vibe-ensemble-mcp": {
            "type": "http",
            "url": format!("http://{}:{}/mcp", host, port)
        },
        "vibe-ensemble-sse": {
            "type": "sse",
            "url": format!("http://{}:{}/sse", host, port)
        },
        "vibe-ensemble-ws": {
            "type": "websocket",
            "url": format!("ws://{}:{}/ws", host, port),
            "bidirectional": true
        }
    }
});
```

## Integration with Existing Systems

### 1. Worker Process Enhancement
Current workers can be enhanced to use client tools:
- File editing through client editor
- Real-time status updates in client UI
- Interactive debugging capabilities

### 2. Permission System Integration
Extend existing permission modes to cover client tool access:
- `client_tools` permission category
- Fine-grained control over which client tools workers can access
- Audit logging for client tool usage

### 3. Event Broadcasting Enhancement
Integrate bidirectional events with existing SSE system:
- Client connection events
- Tool registration events
- Server-initiated request events

## Security Considerations

### 1. Authentication
- Token-based authentication similar to claudecode.nvim
- Lock file mechanism for secure token exchange
- Token rotation capabilities

### 2. Tool Access Control
- Client tool permissions integrated with existing permission system
- Validation of client tool schemas
- Rate limiting for server-initiated requests

### 3. Input Validation
- Strict validation of client tool responses
- Schema validation for tool registration
- Sanitization of client-provided data

## Migration Strategy

### 1. Backward Compatibility
- Existing HTTP/SSE transport remains fully functional
- New WebSocket transport is additive
- Gradual migration path for clients

### 2. Feature Flags
```rust
// src/feature_flags.rs
pub struct FeatureFlags {
    pub bidirectional_mcp: bool,
    pub websocket_transport: bool,
    pub client_tool_registry: bool,
}
```

### 3. Deployment Strategy
- Deploy with bidirectional features disabled by default
- Enable via configuration flags
- Monitor performance and stability

## Success Metrics

### 1. Functional Metrics
- [ ] WebSocket connections successfully established
- [ ] Client tools registered and discoverable
- [ ] Server-initiated tool calls complete successfully
- [ ] Error handling works correctly
- [ ] Authentication prevents unauthorized access

### 2. Performance Metrics
- WebSocket connection latency < 100ms
- Tool call round-trip time < 2 seconds
- Support for 50+ concurrent WebSocket connections
- Memory usage increase < 20% with bidirectional features

### 3. Compatibility Metrics
- [ ] Full backward compatibility with existing HTTP/SSE clients
- [ ] claudecode.nvim integration works without modifications
- [ ] Existing worker processes function unchanged

## Conclusion

This implementation plan provides a comprehensive approach to adding bidirectional MCP communication to Vibe Ensemble while maintaining full backward compatibility. The phased approach allows for incremental development and testing, ensuring stability throughout the implementation process.

The resulting system will enable powerful new use cases such as:
- Workers directly editing files in connected IDEs
- Real-time UI updates during worker execution
- Interactive debugging and approval workflows
- Enhanced coordination between multiple connected clients