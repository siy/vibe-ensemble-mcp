# API Overview

The Vibe Ensemble MCP Server provides comprehensive APIs for coordinating multiple Claude Code instances. This document provides an overview of the API structure, conventions, and capabilities.

## API Structure

The server exposes two primary API interfaces:

### 1. REST API
- **Base URL**: `http://localhost:8080/api` (development)
- **Protocol**: HTTP/1.1 and HTTP/2
- **Format**: JSON
- **Authentication**: JWT Bearer tokens
- **Documentation**: [OpenAPI Specification](openapi.yaml)

### 2. MCP Protocol API
- **Transport**: WebSocket and HTTP
- **Protocol**: Model Context Protocol (MCP)
- **Format**: JSON-RPC 2.0
- **Authentication**: Token-based
- **Documentation**: [MCP Protocol Reference](mcp-protocol.md)

## API Conventions

### Request/Response Format
All API endpoints follow consistent patterns:

**Request Headers**:
```
Content-Type: application/json
Authorization: Bearer <jwt-token>
User-Agent: vibe-ensemble-client/0.1.0
```

**Response Format**:
```json
{
  "data": { /* actual data */ },
  "timestamp": "2025-08-19T12:00:00Z",
  "meta": {
    "total": 100,
    "page": 1,
    "per_page": 20
  }
}
```

**Error Format**:
```json
{
  "error": "ValidationError",
  "message": "Invalid request parameters",
  "details": {
    "field": "priority",
    "reason": "Invalid enum value"
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

### HTTP Status Codes
- `200 OK` - Successful operation
- `201 Created` - Resource created successfully
- `204 No Content` - Successful operation with no response body
- `400 Bad Request` - Invalid request parameters
- `401 Unauthorized` - Authentication required
- `403 Forbidden` - Insufficient permissions
- `404 Not Found` - Resource not found
- `409 Conflict` - Resource conflict
- `429 Too Many Requests` - Rate limit exceeded
- `500 Internal Server Error` - Server error
- `503 Service Unavailable` - Service temporarily unavailable

### Pagination
List endpoints support pagination with query parameters:
- `limit` - Maximum number of items to return (1-1000, default: 100)
- `offset` - Number of items to skip (default: 0)

### Filtering and Search
Most list endpoints support filtering:
- Query parameters for common fields
- `search` parameter for text search
- Date range filtering with `from` and `to` parameters

### Rate Limiting
- **Default Limits**: 1000 requests per hour per IP
- **Authenticated**: 5000 requests per hour per user
- **Headers**: `X-RateLimit-*` headers in responses
- **Exceeded**: HTTP 429 with retry information

## Core Resources

### System Resources
- **Health Check**: `GET /api/health`
- **System Statistics**: `GET /api/stats`
- **Configuration**: `GET /api/config`
- **Metrics**: `GET /api/metrics`

### Agent Management
- **List Agents**: `GET /api/agents`
- **Agent Details**: `GET /api/agents/{id}`
- **Agent Registration**: `POST /api/agents`
- **Update Agent**: `PUT /api/agents/{id}`
- **Agent Status**: `GET /api/agents/{id}/status`

### Issue Tracking
- **List Issues**: `GET /api/issues`
- **Create Issue**: `POST /api/issues`
- **Issue Details**: `GET /api/issues/{id}`
- **Update Issue**: `PUT /api/issues/{id}`
- **Delete Issue**: `DELETE /api/issues/{id}`
- **Issue Comments**: `GET /api/issues/{id}/comments`

### Knowledge Management
- **List Knowledge**: `GET /api/knowledge`
- **Knowledge Details**: `GET /api/knowledge/{id}`
- **Search Knowledge**: `GET /api/knowledge/search`
- **Knowledge Categories**: `GET /api/knowledge/categories`
- **Knowledge Tags**: `GET /api/knowledge/tags`

### Messaging
- **List Messages**: `GET /api/messages`
- **Send Message**: `POST /api/messages`
- **Message History**: `GET /api/messages/history`
- **Broadcast Message**: `POST /api/messages/broadcast`

## Authentication

### JWT Token Authentication
Most endpoints require authentication via JWT tokens:

1. **Obtain Token**: `POST /auth/login`
2. **Include in Header**: `Authorization: Bearer <token>`
3. **Token Refresh**: `POST /auth/refresh`
4. **Logout**: `POST /auth/logout`

### Token Structure
```json
{
  "sub": "user-id",
  "roles": ["coordinator", "admin"],
  "exp": 1692364800,
  "iat": 1692278400,
  "agent_id": "agent-uuid"
}
```

## WebSocket Connections

### Real-time Events
Connect to WebSocket endpoints for real-time updates:
- **URL**: `ws://localhost:8080/ws`
- **Authentication**: Include JWT token in connection
- **Events**: Issue updates, agent status, system notifications

### Event Types
```json
{
  "type": "issue_created",
  "data": { /* issue data */ },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

## Error Handling

### Client Error Handling
Implement robust error handling for:
- Network timeouts
- Rate limiting
- Authentication expiry
- Service unavailability

### Retry Strategy
- **Exponential Backoff**: For 5xx errors
- **Rate Limit Respect**: Honor `Retry-After` header
- **Circuit Breaker**: For persistent failures

## SDK and Client Libraries

### Official SDKs
- **Rust**: `vibe-ensemble-client` crate
- **Python**: `vibe-ensemble-python` package
- **JavaScript**: `@vibe-ensemble/client` npm package

### Example Usage
```rust
use vibe_ensemble_client::{Client, Config};

let config = Config::new()
    .base_url("http://localhost:8080")
    .token("your-jwt-token");

let client = Client::new(config)?;
let agents = client.agents().list().await?;
```

## API Versioning

### Version Strategy
- **Current Version**: v1 (default)
- **Header**: `API-Version: v1`
- **URL Path**: `/api/v1/` (optional)
- **Deprecation**: 6-month notice for breaking changes

### Compatibility
- Backward compatible changes in minor versions
- Breaking changes require major version bump
- Legacy version support for 2 major versions

## Testing and Development

### API Testing
- **Postman Collection**: Available in `docs/api/`
- **OpenAPI Tools**: Generate clients and tests
- **Mock Server**: Available for development

### Development Tools
- **API Explorer**: Swagger UI at `/docs`
- **Health Dashboard**: Available at `/health`
- **Metrics**: Prometheus format at `/metrics`

## Security Considerations

### API Security
- **HTTPS**: Required in production
- **CORS**: Configurable origin restrictions
- **Input Validation**: All inputs validated and sanitized
- **Rate Limiting**: Protection against abuse
- **Audit Logging**: All API calls logged

### Best Practices
- Use HTTPS in production
- Implement proper error handling
- Respect rate limits
- Use appropriate timeout values
- Validate all responses
- Implement circuit breakers for resilience

## Support and Resources

### Documentation
- [REST API Reference](rest-api.md)
- [MCP Protocol Reference](mcp-protocol.md)
- [OpenAPI Specification](openapi.yaml)
- [Client SDK Documentation](../developer/sdk.md)

### Getting Help
- **Issues**: GitHub Issues for bugs and feature requests
- **Discussions**: GitHub Discussions for questions
- **API Questions**: Use `api` label on GitHub issues

---

*For detailed endpoint documentation, see the [OpenAPI Specification](openapi.yaml) or use the interactive API explorer at `/docs`.*