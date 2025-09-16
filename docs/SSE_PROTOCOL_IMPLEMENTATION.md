# SSE Protocol Implementation Documentation

## Overview

This document provides a comprehensive description of the Server-Sent Events (SSE) protocol implementation in the Vibe-Ensemble MCP project. This implementation enables real-time communication between the MCP server and Claude Code, providing reliable event streaming and dual-endpoint architecture for maximum compatibility.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Implementation Details](#implementation-details)
3. [Protocol Specification](#protocol-specification)
4. [API Endpoints](#api-endpoints)
5. [Event Broadcasting System](#event-broadcasting-system)
6. [Claude Code Integration](#claude-code-integration)
7. [Configuration](#configuration)
8. [Testing and Verification](#testing-and-verification)
9. [Troubleshooting](#troubleshooting)

## Architecture Overview

### Dual-Endpoint Architecture

The SSE implementation follows Claude Code's extended MCP specification with a dual-endpoint design:

- **`/sse` (GET)**: Server-to-client event streaming endpoint
- **`/messages` (POST)**: Client-to-server HTTP request endpoint

This architecture provides:
- **Unidirectional streaming** for real-time events via SSE
- **Bidirectional communication** via HTTP POST for client requests
- **Full compatibility** with Claude Code's SSE transport
- **Backward compatibility** with standard MCP over HTTP

### Component Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Claude Code   │    │  MCP Server     │    │  Event System   │
│                 │    │                 │    │                 │
│  SSE Client ────┼───→│  /sse endpoint  │    │ EventBroadcaster│
│                 │    │                 │    │                 │
│  HTTP Client────┼───→│ /messages       │───→│ Tool Execution  │
│                 │    │  endpoint       │    │                 │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Implementation Details

### Core Files and Components

#### 1. Server Configuration (`src/server.rs`)

**Key Routes:**
```rust
let app = Router::new()
    .route("/health", get(health_check))
    .route("/mcp", post(mcp_handler))
    .route("/sse", get(sse_handler))           // SSE streaming endpoint
    .route("/messages", post(sse_message_handler)) // HTTP POST endpoint
    .layer(RequestBodyLimitLayer::new(1024 * 1024))
    .layer(TraceLayer::new_for_http())
    .layer(cors)
    .with_state(state);
```

**CORS Configuration:**
```rust
let cors = CorsLayer::new()
    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
    .allow_headers([
        axum::http::header::CONTENT_TYPE,
        axum::http::header::ACCEPT,
        axum::http::header::CACHE_CONTROL,
        axum::http::header::AUTHORIZATION,
        axum::http::header::HeaderName::from_static("x-api-key"),
        axum::http::header::HeaderName::from_static("last-event-id"),      // SSE-specific
        axum::http::header::HeaderName::from_static("mcp-protocol-version"), // MCP-specific
    ])
    .allow_origin(axum::http::header::HeaderValue::from_static("*"));
```

#### 2. SSE Implementation (`src/sse.rs`)

**EventBroadcaster Structure:**
```rust
#[derive(Clone)]
pub struct EventBroadcaster {
    sender: Arc<broadcast::Sender<String>>,
}

impl EventBroadcaster {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100); // 100 message buffer
        Self {
            sender: Arc::new(sender),
        }
    }

    pub fn broadcast_event(&self, event_type: &str, data: serde_json::Value) {
        let event_data = json!({
            "type": event_type,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "data": data
        });
        let _ = self.sender.send(event_data.to_string());
    }

    pub fn broadcast(&self, event_data: String) -> Result<usize, broadcast::error::SendError<String>> {
        self.sender.send(event_data)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.sender.subscribe()
    }
}
```

**SSE Handler Implementation:**
```rust
pub async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let broadcaster = &state.event_broadcaster;

    // Send MCP protocol initialization notification
    let init_notification = json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "serverInfo": {
                "name": "vibe-ensemble-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {},
                "notifications": {
                    "events": true,
                    "tickets": true,
                    "workers": true,
                    "queues": true
                }
            }
        }
    });

    broadcaster.broadcast_event("mcp_notification", init_notification.clone());

    // Send endpoint event for Claude Code SSE transport compatibility
    let port = state.server_info.port;
    let endpoint_event = json!({
        "jsonrpc": "2.0",
        "method": "notifications/message",
        "params": {
            "level": "info",
            "logger": "vibe-ensemble-sse",
            "data": json!({
                "type": "endpoint",
                "uri": format!("http://localhost:{}/messages", port)
            })
        }
    });
    
    broadcaster.broadcast_event("endpoint", endpoint_event);
    
    let mut receiver = broadcaster.subscribe();

    let stream = async_stream::stream! {
        // Send initialization message immediately to new clients
        yield Ok(Event::default()
            .event("message")
            .data(init_notification.to_string()));

        loop {
            match receiver.recv().await {
                Ok(data) => {
                    // Wrap events in MCP notification format
                    let mcp_event = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                        if parsed.get("jsonrpc").is_some() {
                            data // Already MCP-compliant
                        } else {
                            // Wrap non-MCP events in MCP notification format
                            json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/resources/updated",
                                "params": {
                                    "uri": "vibe-ensemble://events",
                                    "event": parsed
                                }
                            }).to_string()
                        }
                    } else {
                        // Fallback for malformed JSON
                        json!({
                            "jsonrpc": "2.0",
                            "method": "notifications/message",
                            "params": {
                                "level": "info",
                                "logger": "vibe-ensemble-sse",
                                "data": data
                            }
                        }).to_string()
                    };

                    yield Ok(Event::default()
                        .event("message")
                        .data(mcp_event));
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Send MCP-compliant heartbeat
                    let heartbeat = json!({
                        "jsonrpc": "2.0",
                        "method": "notifications/ping",
                        "params": {
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    });
                    yield Ok(Event::default()
                        .event("ping")
                        .data(heartbeat.to_string()));
                }
                Err(_) => break, // Channel closed
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keep-alive-mcp"),
    )
}
```

**HTTP POST Message Handler:**
```rust
pub async fn sse_message_handler(
    State(state): State<AppState>,
    JsonExtractor(payload): JsonExtractor<Value>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    debug!("Received SSE message: {}", payload);
    
    // Parse the JSON as an MCP JsonRpcRequest
    let request: JsonRpcRequest = match serde_json::from_value(payload.clone()) {
        Ok(req) => req,
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e)
                },
                "id": payload.get("id")
            });
            return Err((StatusCode::BAD_REQUEST, Json(error_response)));
        }
    };
    
    // Create MCP server and handle the request
    let mcp_server = McpServer::new();
    let response = mcp_server.handle_request(&state, request).await;
    
    info!("SSE message processed successfully");
    
    // Convert the response to JSON
    let response_value = match serde_json::to_value(&response) {
        Ok(val) => val,
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": format!("Internal error: {}", e)
                },
                "id": response.id
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
        }
    };
    
    // If this is a successful MCP response, broadcast it to SSE clients
    if let Some(result) = response.result {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/message",
            "params": {
                "level": "info",
                "logger": "vibe-ensemble-sse",
                "data": result
            }
        });
        
        if let Err(e) = state.event_broadcaster.broadcast(notification.to_string()) {
            tracing::warn!("Failed to broadcast SSE response: {}", e);
        }
    }
    
    Ok(Json(response_value))
}
```

#### 3. Event Integration in MCP Tools

**Example: Project Tools Integration (`src/mcp/project_tools.rs`)**
```rust
// After successful project creation
let event = json!({
    "jsonrpc": "2.0",
    "method": "notifications/resources/updated",
    "params": {
        "uri": "vibe-ensemble://projects",
        "event": {
            "type": "project_created",
            "project": {
                "repository_name": project.repository_name,
                "path": project.path,
                "description": project.short_description,
                "created_at": project.created_at
            },
            "timestamp": chrono::Utc::now().to_rfc3339()
        }
    }
});

if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
    tracing::warn!("Failed to broadcast project_created event: {}", e);
}
```

## Protocol Specification

### MCP Protocol Version
- **Version**: `2024-11-05`
- **Compliance**: Full JSON-RPC 2.0 compatibility
- **Extensions**: Claude Code SSE transport support

### Message Format

#### SSE Event Format
```
event: message
data: {JSON-RPC 2.0 message}
```

#### MCP Notification Format
```json
{
  "jsonrpc": "2.0",
  "method": "notifications/{type}",
  "params": {
    // Type-specific parameters
  }
}
```

### Supported Notification Types

1. **Initialization**: `notifications/initialized`
2. **Resource Updates**: `notifications/resources/updated`
3. **Messages**: `notifications/message`
4. **Ping**: `notifications/ping`

## API Endpoints

### 1. SSE Streaming Endpoint

**Endpoint**: `GET /sse`

**Headers**:
- `Accept: text/event-stream`
- `Cache-Control: no-cache`

**Response**:
- Content-Type: `text/event-stream`
- Connection: `keep-alive`

**Event Flow**:
1. **Initialization Event**: Immediate MCP protocol handshake
2. **Endpoint Event**: Claude Code compatibility announcement
3. **Resource Events**: Real-time updates from tool execution
4. **Keep-Alive Events**: Periodic heartbeat (every 30 seconds)

### 2. HTTP POST Message Endpoint

**Endpoint**: `POST /messages`

**Headers**:
- `Content-Type: application/json`

**Request Body**: MCP JSON-RPC request
```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "method": "tools/call",
  "params": {
    "name": "tool_name",
    "arguments": { /* tool arguments */ }
  }
}
```

**Response**: MCP JSON-RPC response
```json
{
  "jsonrpc": "2.0",
  "id": "request-id",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Operation result"
      }
    ]
  }
}
```

### 3. Health Check Endpoint

**Endpoint**: `GET /health`

**Response**:
```json
{
  "status": "healthy",
  "service": "vibe-ensemble-mcp",
  "timestamp": "2025-09-16T23:23:42.135158+00:00",
  "database": {
    "version": "4",
    "status": "connected"
  }
}
```

## Event Broadcasting System

### Event Types and Sources

#### 1. Project Events
- **Source**: `src/mcp/project_tools.rs`
- **Events**: `project_created`, `project_updated`, `project_deleted`
- **URI**: `vibe-ensemble://projects`

#### 2. Ticket Events
- **Source**: `src/mcp/ticket_tools.rs`
- **Events**: `ticket_created`, `ticket_updated`, `ticket_closed`, `ticket_claimed`, `ticket_released`
- **URI**: `vibe-ensemble://tickets/{ticket_id}`

#### 3. Worker Events
- **Source**: `src/mcp/worker_type_tools.rs`
- **Events**: `worker_type_created`, `worker_type_updated`, `worker_type_deleted`
- **URI**: `vibe-ensemble://workers/{worker_id}`

#### 4. Queue Events
- **Source**: Various worker operations
- **Events**: `queue_created`, `task_assigned`, `worker_spawned`, `worker_stopped`
- **URI**: `vibe-ensemble://queues/{queue_name}`

### Event Broadcasting Helpers

**Notification Functions** (`src/sse.rs`):
```rust
pub async fn notify_event_change(
    broadcaster: &EventBroadcaster,
    event_type: &str,
    event_data: serde_json::Value,
)

pub async fn notify_ticket_change(
    broadcaster: &EventBroadcaster,
    ticket_id: &str,
    change_type: &str,
)

pub async fn notify_worker_change(
    broadcaster: &EventBroadcaster, 
    worker_id: &str, 
    status: &str
)

pub async fn notify_queue_change(
    broadcaster: &EventBroadcaster,
    queue_name: &str,
    change_type: &str,
)
```

## Claude Code Integration

### Configuration Generation

The MCP configuration for Claude Code includes dual server setup:

**File**: `src/configure.rs`
```rust
let config = json!({
    "mcpServers": {
        "vibe-ensemble-mcp": {
            "type": "http",
            "url": format!("http://{}:{}/mcp", host, port),
            "protocol_version": "2024-11-05"
        },
        "vibe-ensemble-sse": {
            "type": "sse",
            "url": format!("http://{}:{}/sse", host, port),
            "protocol_version": "2024-11-05"
        }
    }
});
```

### Claude Code Compatibility Features

1. **Endpoint Discovery**: Automatic `/messages` endpoint announcement
2. **Keep-Alive**: 30-second interval heartbeat messages
3. **Error Handling**: Proper JSON-RPC error responses
4. **Auto-Broadcasting**: Tool responses automatically streamed to SSE clients

### Known Claude Code Issues

Based on source code analysis of Claude Code v1.0.108:

1. **Authentication Bugs**: Bearer token handling issues in SSE transport
2. **Timeout Problems**: Connection timeouts around 60 seconds
3. **Connection Limits**: Limited concurrent SSE connections
4. **Deprecation**: SSE transport deprecated in MCP 2025 specification

## Configuration

### Server Configuration

**Environment Variables**:
- `PORT`: Server port (default: 3000)
- `HOST`: Server host (default: 127.0.0.1)
- `LOG_LEVEL`: Logging level (debug, info, warn, error)

**Command Line**:
```bash
cargo run -- --port 3000 --log-level debug
```

### MCP Configuration

Generated automatically via:
```bash
cargo run -- --configure-claude-code --host localhost --port 3000
```

**Output Location**: `~/.claude/config.json`

## Testing and Verification

### Manual Testing

#### 1. SSE Connectivity Test
```bash
curl -N -H "Accept: text/event-stream" http://localhost:3000/sse
```

**Expected Output**:
```
event: message
data: {"jsonrpc":"2.0","method":"notifications/initialized",...}
```

#### 2. HTTP POST Test
```bash
curl -X POST http://localhost:3000/messages \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-1", 
    "method": "tools/list",
    "params": {}
  }'
```

#### 3. Event Broadcasting Test
```bash
curl -X POST http://localhost:3000/messages \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "id": "test-2",
    "method": "tools/call",
    "params": {
      "name": "create_project",
      "arguments": {
        "repository_name": "test/sse-integration-test",
        "path": "/tmp/sse-test",
        "description": "Testing SSE integration"
      }
    }
  }'
```

**Expected**: `project_created` event broadcast to SSE clients

### Automated Testing

Tests verify:
- SSE connection establishment
- MCP message processing
- Event broadcasting functionality
- Error handling
- CORS compliance

## Troubleshooting

### Common Issues

#### 1. SSE Connection Fails
**Symptoms**: Client cannot connect to `/sse` endpoint
**Solutions**:
- Verify CORS headers are properly configured
- Check firewall settings
- Ensure `Accept: text/event-stream` header is sent

#### 2. No Events Received
**Symptoms**: SSE connected but no events arrive
**Solutions**:
- Check EventBroadcaster initialization in server startup
- Verify tool integrations include event broadcasting calls
- Enable trace logging to debug event flow

#### 3. HTTP POST Errors
**Symptoms**: `/messages` endpoint returns errors
**Solutions**:
- Validate JSON-RPC request format
- Check request Content-Type header
- Verify MCP tool implementations

#### 4. Claude Code Integration Issues
**Symptoms**: Claude Code cannot connect or times out
**Solutions**:
- Update Claude Code to latest version
- Check for authentication configuration
- Verify endpoint URLs in MCP configuration
- Monitor for 60-second timeout issues

### Debug Logging

Enable trace-level logging for detailed debugging:
```bash
cargo run -- --port 3000 --log-level trace
```

**Key Log Messages**:
- `SSE message processed successfully`
- `Successfully broadcast {event_type} event`
- `Failed to broadcast {event_type} event`
- Request/response tracing with latency

### Performance Monitoring

Monitor these metrics:
- **SSE Connection Count**: Number of active connections
- **Event Broadcasting Rate**: Events per second
- **Message Processing Latency**: HTTP POST response times
- **Buffer Overflow**: Broadcast channel lagged errors

## Future Enhancements

### Planned Improvements

1. **Authentication**: Implement proper authentication for SSE transport
2. **Rate Limiting**: Add rate limiting for HTTP POST endpoint
3. **Connection Management**: Better handling of SSE connection lifecycle
4. **Event Filtering**: Allow clients to subscribe to specific event types
5. **Compression**: Support for event compression
6. **HTTP Stream Transport**: Migrate to MCP 2025 HTTP Stream Transport

### Migration Path

When MCP 2025 specification is adopted:
1. Implement HTTP Stream Transport alongside SSE
2. Deprecate SSE transport with migration notices
3. Update Claude Code configuration generation
4. Maintain backward compatibility during transition period

## Conclusion

This SSE implementation provides a robust, real-time communication layer between the Vibe-Ensemble MCP server and Claude Code. The dual-endpoint architecture ensures compatibility while the event broadcasting system enables comprehensive real-time monitoring of system operations.

The implementation has been thoroughly tested and verified to work with Claude Code's extended MCP SSE transport, providing a reliable foundation for real-time multi-agent coordination and monitoring.