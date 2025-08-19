# REST API Reference

Complete reference for the Vibe Ensemble MCP Server REST API. This document provides detailed information about all available endpoints, request/response formats, and usage examples.

## Base URL
- Development: `http://localhost:8080/api`
- Production: `https://your-domain.com/api`

## Authentication

All API endpoints (except health checks) require authentication via JWT Bearer tokens:

```bash
curl -H "Authorization: Bearer <your-token>" \
     -H "Content-Type: application/json" \
     https://api.example.com/api/agents
```

## System Endpoints

### Health Check

Check the health status of the server and database connectivity.

**Endpoint**: `GET /api/health`

**Response**:
```json
{
  "status": "healthy",
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
curl http://localhost:8080/api/health
```

### System Statistics

Get overall system statistics including counts of agents, issues, messages, and knowledge entries.

**Endpoint**: `GET /api/stats`

**Response**:
```json
{
  "agents": 5,
  "issues": 23,
  "messages": 156,
  "knowledge": 45,
  "prompts": 12,
  "timestamp": "2025-08-19T12:00:00Z"
}
```

## Agent Management

### List Agents

Retrieve a list of all registered agents with optional filtering.

**Endpoint**: `GET /api/agents`

**Query Parameters**:
- `limit` (integer, optional): Maximum number of agents to return (1-1000, default: 100)
- `offset` (integer, optional): Number of agents to skip for pagination (default: 0)
- `status` (string, optional): Filter by agent status (`Active`, `Inactive`, `Disconnected`, `Error`)
- `agent_type` (string, optional): Filter by agent type (`Coordinator`, `Worker`, `Monitor`)

**Response**:
```json
{
  "agents": [
    {
      "id": "123e4567-e89b-12d3-a456-426614174000",
      "name": "Claude-Coordinator-1",
      "agent_type": "Coordinator",
      "status": "Active",
      "capabilities": ["task_coordination", "issue_management"],
      "metadata": {
        "version": "0.1.0",
        "location": "primary"
      },
      "created_at": "2025-08-19T10:00:00Z",
      "updated_at": "2025-08-19T12:00:00Z",
      "last_seen": "2025-08-19T12:00:00Z"
    }
  ],
  "total": 1,
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
# List all active agents
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/agents?status=Active"

# Get first 10 coordinator agents
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/agents?agent_type=Coordinator&limit=10"
```

### Get Agent Details

Retrieve detailed information about a specific agent.

**Endpoint**: `GET /api/agents/{id}`

**Parameters**:
- `id` (UUID, required): Agent ID

**Response**:
```json
{
  "agent": {
    "id": "123e4567-e89b-12d3-a456-426614174000",
    "name": "Claude-Coordinator-1",
    "agent_type": "Coordinator",
    "status": "Active",
    "capabilities": ["task_coordination", "issue_management"],
    "metadata": {
      "version": "0.1.0",
      "location": "primary",
      "tasks_completed": 25,
      "uptime_seconds": 3600
    },
    "created_at": "2025-08-19T10:00:00Z",
    "updated_at": "2025-08-19T12:00:00Z",
    "last_seen": "2025-08-19T12:00:00Z"
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
curl -H "Authorization: Bearer <token>" \
     http://localhost:8080/api/agents/123e4567-e89b-12d3-a456-426614174000
```

## Issue Management

### List Issues

Retrieve a list of all issues with optional filtering.

**Endpoint**: `GET /api/issues`

**Query Parameters**:
- `limit` (integer, optional): Maximum number of issues to return (1-1000, default: 100)
- `offset` (integer, optional): Number of issues to skip for pagination (default: 0)
- `status` (string, optional): Filter by issue status (`Open`, `InProgress`, `Resolved`, `Closed`)
- `priority` (string, optional): Filter by priority (`Low`, `Medium`, `High`, `Critical`)
- `assigned_agent_id` (UUID, optional): Filter by assigned agent ID

**Response**:
```json
{
  "issues": [
    {
      "id": "456e7890-e89b-12d3-a456-426614174001",
      "title": "Implement user authentication",
      "description": "Add JWT-based authentication to the API endpoints",
      "status": "Open",
      "priority": "High",
      "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000",
      "reporter_id": "user-123",
      "labels": ["authentication", "security", "api"],
      "metadata": {
        "estimated_hours": 8,
        "components": ["auth", "middleware"]
      },
      "created_at": "2025-08-19T09:00:00Z",
      "updated_at": "2025-08-19T11:30:00Z",
      "resolved_at": null
    }
  ],
  "total": 1,
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
# List all high priority open issues
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/issues?status=Open&priority=High"

# Get issues assigned to specific agent
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/issues?assigned_agent_id=123e4567-e89b-12d3-a456-426614174000"
```

### Create Issue

Create a new issue.

**Endpoint**: `POST /api/issues`

**Request Body**:
```json
{
  "title": "Fix database connection pool",
  "description": "Database connections are not being properly returned to the pool, causing connection exhaustion under load.",
  "priority": "Critical",
  "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000"
}
```

**Response** (201 Created):
```json
{
  "issue": {
    "id": "789e1234-e89b-12d3-a456-426614174002",
    "title": "Fix database connection pool",
    "description": "Database connections are not being properly returned to the pool, causing connection exhaustion under load.",
    "status": "Open",
    "priority": "Critical",
    "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000",
    "reporter_id": "api-user",
    "labels": [],
    "metadata": {},
    "created_at": "2025-08-19T12:00:00Z",
    "updated_at": "2025-08-19T12:00:00Z",
    "resolved_at": null
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
curl -X POST \
     -H "Authorization: Bearer <token>" \
     -H "Content-Type: application/json" \
     -d '{
       "title": "Fix database connection pool",
       "description": "Database connections not properly returned to pool",
       "priority": "Critical",
       "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000"
     }' \
     http://localhost:8080/api/issues
```

### Get Issue Details

Retrieve detailed information about a specific issue.

**Endpoint**: `GET /api/issues/{id}`

**Parameters**:
- `id` (UUID, required): Issue ID

**Response**:
```json
{
  "issue": {
    "id": "456e7890-e89b-12d3-a456-426614174001",
    "title": "Implement user authentication",
    "description": "Add JWT-based authentication to the API endpoints",
    "status": "InProgress",
    "priority": "High",
    "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000",
    "reporter_id": "user-123",
    "labels": ["authentication", "security", "api"],
    "metadata": {
      "estimated_hours": 8,
      "components": ["auth", "middleware"],
      "progress": 30
    },
    "created_at": "2025-08-19T09:00:00Z",
    "updated_at": "2025-08-19T11:30:00Z",
    "resolved_at": null
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

### Update Issue

Update an existing issue.

**Endpoint**: `PUT /api/issues/{id}`

**Parameters**:
- `id` (UUID, required): Issue ID

**Request Body**:
```json
{
  "title": "Implement user authentication (Updated)",
  "description": "Add JWT-based authentication to API endpoints with rate limiting",
  "priority": "Critical",
  "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000"
}
```

**Response**:
```json
{
  "issue": {
    "id": "456e7890-e89b-12d3-a456-426614174001",
    "title": "Implement user authentication (Updated)",
    "description": "Add JWT-based authentication to API endpoints with rate limiting",
    "status": "InProgress",
    "priority": "Critical",
    "assigned_agent_id": "123e4567-e89b-12d3-a456-426614174000",
    "reporter_id": "user-123",
    "labels": ["authentication", "security", "api"],
    "metadata": {
      "estimated_hours": 12,
      "components": ["auth", "middleware", "rate-limiting"]
    },
    "created_at": "2025-08-19T09:00:00Z",
    "updated_at": "2025-08-19T12:15:00Z",
    "resolved_at": null
  },
  "timestamp": "2025-08-19T12:15:00Z"
}
```

### Delete Issue

Delete an existing issue.

**Endpoint**: `DELETE /api/issues/{id}`

**Parameters**:
- `id` (UUID, required): Issue ID

**Response**: 204 No Content

**Example**:
```bash
curl -X DELETE \
     -H "Authorization: Bearer <token>" \
     http://localhost:8080/api/issues/456e7890-e89b-12d3-a456-426614174001
```

## Knowledge Management

### List Knowledge Entries

Retrieve knowledge entries with optional filtering and search.

**Endpoint**: `GET /api/knowledge`

**Query Parameters**:
- `limit` (integer, optional): Maximum number of entries to return (default: 100)
- `offset` (integer, optional): Number of entries to skip (default: 0)
- `category` (string, optional): Filter by category
- `tag` (string, optional): Filter by tag
- `search` (string, optional): Search in title and content

**Response**:
```json
{
  "knowledge": [
    {
      "id": "abc12345-e89b-12d3-a456-426614174003",
      "title": "Database Connection Pooling Best Practices",
      "content": "When implementing database connection pooling...",
      "category": "database",
      "tags": ["database", "connection", "performance"],
      "author_id": "agent-123",
      "version": 1,
      "metadata": {
        "difficulty": "intermediate",
        "estimated_read_time": "5 minutes"
      },
      "created_at": "2025-08-19T08:00:00Z",
      "updated_at": "2025-08-19T08:00:00Z"
    }
  ],
  "total": 1,
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
# Search knowledge entries
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/knowledge?search=database&category=performance"

# Get entries with specific tag
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/knowledge?tag=security"
```

### Get Knowledge Entry

Retrieve a specific knowledge entry.

**Endpoint**: `GET /api/knowledge/{id}`

**Parameters**:
- `id` (UUID, required): Knowledge entry ID

**Response**:
```json
{
  "knowledge": {
    "id": "abc12345-e89b-12d3-a456-426614174003",
    "title": "Database Connection Pooling Best Practices",
    "content": "# Database Connection Pooling Best Practices\n\nWhen implementing database connection pooling, consider the following guidelines:\n\n## Pool Sizing\n- Start with pool size = number of CPU cores\n- Monitor connection utilization\n- Adjust based on application load patterns\n\n## Connection Lifecycle\n- Always return connections to pool\n- Set appropriate timeout values\n- Handle connection failures gracefully",
    "category": "database",
    "tags": ["database", "connection", "performance", "best-practices"],
    "author_id": "agent-123",
    "version": 2,
    "metadata": {
      "difficulty": "intermediate",
      "estimated_read_time": "5 minutes",
      "related_issues": ["789e1234-e89b-12d3-a456-426614174002"],
      "last_reviewed": "2025-08-19T10:00:00Z"
    },
    "created_at": "2025-08-19T08:00:00Z",
    "updated_at": "2025-08-19T10:30:00Z"
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

## Message Management

### List Messages

Retrieve messages with optional filtering.

**Endpoint**: `GET /api/messages`

**Query Parameters**:
- `limit` (integer, optional): Maximum number of messages to return (default: 100)
- `offset` (integer, optional): Number of messages to skip (default: 0)
- `from_agent` (string, optional): Filter by sender agent ID
- `to_agent` (string, optional): Filter by recipient agent ID
- `message_type` (string, optional): Filter by message type

**Response**:
```json
{
  "messages": [
    {
      "id": "def67890-e89b-12d3-a456-426614174004",
      "from_agent": "agent-coordinator-1",
      "to_agent": "agent-worker-2",
      "message_type": "TaskAssignment",
      "content": {
        "task_id": "456e7890-e89b-12d3-a456-426614174001",
        "task_type": "issue_resolution",
        "priority": "high",
        "deadline": "2025-08-20T18:00:00Z"
      },
      "metadata": {
        "retry_count": 0,
        "correlation_id": "req-123"
      },
      "created_at": "2025-08-19T12:00:00Z",
      "delivered_at": "2025-08-19T12:00:01Z"
    }
  ],
  "total": 1,
  "timestamp": "2025-08-19T12:00:00Z"
}
```

**Example**:
```bash
# Get messages from specific agent
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/messages?from_agent=agent-coordinator-1"

# Get task assignment messages
curl -H "Authorization: Bearer <token>" \
     "http://localhost:8080/api/messages?message_type=TaskAssignment"
```

## Error Responses

### Common Error Format

All API errors return a consistent format:

```json
{
  "error": "ValidationError",
  "message": "Invalid request parameters",
  "details": {
    "field": "priority",
    "reason": "Invalid enum value",
    "allowed_values": ["Low", "Medium", "High", "Critical"]
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

### Error Types

- `ValidationError`: Request validation failed
- `AuthenticationError`: Authentication required or failed
- `AuthorizationError`: Insufficient permissions
- `NotFoundError`: Resource not found
- `ConflictError`: Resource conflict
- `RateLimitError`: Rate limit exceeded
- `InternalError`: Server error

## Rate Limiting

API requests are subject to rate limiting:

**Rate Limit Headers**:
```
X-RateLimit-Limit: 5000
X-RateLimit-Remaining: 4999
X-RateLimit-Reset: 1692364800
```

**Rate Limit Exceeded Response** (429):
```json
{
  "error": "RateLimitError",
  "message": "Rate limit exceeded",
  "details": {
    "limit": 5000,
    "reset_at": "2025-08-19T13:00:00Z"
  },
  "timestamp": "2025-08-19T12:00:00Z"
}
```

## SDK Examples

### Rust SDK
```rust
use vibe_ensemble_client::{Client, Config, IssueRequest};

let config = Config::new()
    .base_url("http://localhost:8080")
    .token("your-jwt-token");

let client = Client::new(config)?;

// List agents
let agents = client.agents().list().await?;

// Create issue
let issue_req = IssueRequest {
    title: "Fix bug in authentication".to_string(),
    description: "Users cannot log in with special characters".to_string(),
    priority: "High".to_string(),
    assigned_agent_id: Some(agent_id),
};

let issue = client.issues().create(issue_req).await?;
```

### Python SDK
```python
from vibe_ensemble_client import Client, IssueRequest

client = Client(
    base_url="http://localhost:8080",
    token="your-jwt-token"
)

# List agents
agents = client.agents.list()

# Create issue
issue = client.issues.create(IssueRequest(
    title="Fix bug in authentication",
    description="Users cannot log in with special characters",
    priority="High",
    assigned_agent_id=agent_id
))
```

### JavaScript SDK
```javascript
import { Client } from '@vibe-ensemble/client';

const client = new Client({
  baseUrl: 'http://localhost:8080',
  token: 'your-jwt-token'
});

// List agents
const agents = await client.agents.list();

// Create issue
const issue = await client.issues.create({
  title: 'Fix bug in authentication',
  description: 'Users cannot log in with special characters',
  priority: 'High',
  assigned_agent_id: agentId
});
```

---

*For more examples and detailed usage patterns, see the [API Examples](../examples/api-examples.md) documentation.*