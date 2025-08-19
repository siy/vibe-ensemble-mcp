# Architecture Overview

This document provides a comprehensive overview of the Vibe Ensemble MCP Server architecture, including system design, component relationships, data flow, and architectural decisions.

## System Architecture

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   Client Layer                          │
├─────────────────────────────────────────────────────────┤
│  Claude Code    │  Web Interface  │  REST API Clients   │
│  Agents         │                 │                     │
└─────────────────────────────────────────────────────────┘
           │                   │                   │
           │ MCP Protocol      │ HTTP/WebSocket    │ REST API
           │                   │                   │
┌─────────────────────────────────────────────────────────┐
│                  API Gateway Layer                      │
├─────────────────────────────────────────────────────────┤
│  MCP Server     │  Web Server     │  WebSocket Manager  │
│                 │  (Axum)         │                     │
└─────────────────────────────────────────────────────────┘
           │                   │                   │
           │                   │                   │
┌─────────────────────────────────────────────────────────┐
│                  Service Layer                          │
├─────────────────────────────────────────────────────────┤
│  Agent Mgmt  │  Issue Tracking │  Knowledge Mgmt │ Msg  │
│  Service     │  Service        │  Service        │ Svc  │
└─────────────────────────────────────────────────────────┘
           │                   │                   │
           │                   │                   │
┌─────────────────────────────────────────────────────────┐
│                  Storage Layer                          │
├─────────────────────────────────────────────────────────┤
│  Repository Layer (SQLx)                                │
│  ├── Agent Repository                                   │
│  ├── Issue Repository                                   │
│  ├── Knowledge Repository                               │
│  └── Message Repository                                 │
└─────────────────────────────────────────────────────────┘
           │
           │
┌─────────────────────────────────────────────────────────┐
│                 Persistence Layer                       │
├─────────────────────────────────────────────────────────┤
│  SQLite (Development) / PostgreSQL (Production)        │
└─────────────────────────────────────────────────────────┘
```

### Core Components

#### 1. Agent Management System
**Purpose**: Handles registration, lifecycle, and capability tracking for all connected Claude Code agents.

**Key Features**:
- Agent registration and discovery
- Capability declaration and matching
- Health monitoring and status tracking
- Hierarchical agent relationships (coordinator vs workers)

**Components**:
- `Agent` domain model
- `AgentService` business logic
- `AgentRepository` data access
- Agent status monitoring

#### 2. Issue Tracking System  
**Purpose**: Provides persistent storage and workflow management for tasks and problems requiring resolution.

**Key Features**:
- Issue lifecycle management (Open → InProgress → Resolved → Closed)
- Priority-based task assignment
- Workflow automation
- Integration with agent assignments

**Components**:
- `Issue` domain model with status transitions
- `IssueService` workflow management
- `IssueRepository` persistence
- Web-based issue management interface

#### 3. Messaging System
**Purpose**: Enables real-time communication between agents using standardized protocols.

**Key Features**:
- Point-to-point and broadcast messaging
- Message persistence and delivery confirmation
- Protocol validation and routing
- WebSocket real-time notifications

**Components**:
- `Message` domain model
- `MessageService` routing and delivery
- `MessageRepository` persistence
- `WebSocketManager` real-time notifications

#### 4. Knowledge Management System
**Purpose**: Collects, organizes, and distributes development patterns, practices, and guidelines.

**Key Features**:
- Pattern and practice documentation
- Semantic search and categorization
- Version control and access management
- AI-powered knowledge intelligence

**Components**:
- `KnowledgeEntry` domain model
- `KnowledgeService` search and organization
- `KnowledgeRepository` storage
- `KnowledgeIntelligence` AI features

#### 5. Persistence Layer
**Purpose**: Ensures data consistency and recovery capabilities across all subsystems.

**Key Features**:
- ACID-compliant transactions
- Database migration management
- Connection pooling and optimization
- Multi-database support (SQLite/PostgreSQL)

## Component Architecture

### Domain-Driven Design

The system follows Domain-Driven Design principles:

```
vibe-ensemble-core/
├── agent.rs          # Agent domain model and behaviors
├── issue.rs          # Issue domain model and workflows  
├── knowledge.rs      # Knowledge domain model and operations
├── message.rs        # Message domain model and protocols
└── config.rs         # Configuration domain model
```

**Domain Models**:
- Rich domain objects with behavior
- Domain-specific validation rules
- Business logic encapsulation
- Event-driven state changes

### Service Layer Architecture

```rust
// Example service structure
pub trait AgentService: Send + Sync {
    async fn register_agent(&self, agent: Agent) -> Result<Agent>;
    async fn update_status(&self, id: Uuid, status: AgentStatus) -> Result<()>;
    async fn assign_task(&self, agent_id: Uuid, task_id: Uuid) -> Result<()>;
    async fn find_available_agents(&self, capability: &str) -> Result<Vec<Agent>>;
}
```

**Service Layer Benefits**:
- Business logic separation from infrastructure
- Transaction boundary management
- Cross-cutting concern handling (logging, metrics)
- Dependency inversion for testability

### Repository Pattern

```rust
#[async_trait]
pub trait AgentRepository: Send + Sync {
    async fn create(&self, agent: &Agent) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Agent>>;
    async fn update(&self, agent: &Agent) -> Result<()>;
    async fn list(&self) -> Result<Vec<Agent>>;
}
```

**Repository Benefits**:
- Data access abstraction
- Database-agnostic business logic
- Testing with mock implementations
- Query optimization centralization

## Data Architecture

### Database Schema

#### Agents Table
```sql
CREATE TABLE agents (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    agent_type TEXT NOT NULL, -- 'Coordinator', 'Worker', 'Monitor'
    status TEXT NOT NULL,     -- 'Active', 'Inactive', 'Disconnected', 'Error'
    capabilities TEXT[],      -- JSON array of capabilities
    metadata JSONB,           -- Additional agent metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMPTZ
);
```

#### Issues Table
```sql
CREATE TABLE issues (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    status TEXT NOT NULL,     -- 'Open', 'InProgress', 'Resolved', 'Closed'
    priority TEXT NOT NULL,   -- 'Low', 'Medium', 'High', 'Critical'
    assigned_agent_id UUID REFERENCES agents(id),
    reporter_id TEXT NOT NULL,
    labels TEXT[],
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_at TIMESTAMPTZ
);
```

#### Knowledge Entries Table
```sql
CREATE TABLE knowledge_entries (
    id UUID PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT NOT NULL,
    tags TEXT[],
    author_id TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

#### Messages Table
```sql
CREATE TABLE messages (
    id UUID PRIMARY KEY,
    from_agent TEXT NOT NULL,
    to_agent TEXT,            -- NULL for broadcast messages
    message_type TEXT NOT NULL,
    content JSONB NOT NULL,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);
```

### Data Flow Patterns

#### Agent Registration Flow
```
1. Agent connects via MCP protocol
2. AgentService validates capabilities
3. AgentRepository persists agent data
4. MessageService broadcasts agent availability
5. WebSocketManager notifies web clients
```

#### Issue Assignment Flow
```
1. Issue created via API or web interface
2. IssueService determines assignment strategy
3. AgentService finds available agents with required capabilities
4. MessageService sends task assignment to agent
5. Agent confirms task acceptance
6. IssueService updates issue status to InProgress
```

#### Knowledge Discovery Flow
```
1. Agent or user searches knowledge repository
2. KnowledgeService performs semantic search
3. KnowledgeIntelligence ranks results by relevance
4. Related patterns and practices are suggested
5. Usage statistics updated for improvement
```

## Communication Architecture

### MCP Protocol Implementation

The system implements the Model Context Protocol for agent communication:

```rust
pub enum MCPMessage {
    Initialize { 
        protocol_version: String,
        capabilities: AgentCapabilities 
    },
    ListResources { 
        cursor: Option<String> 
    },
    ReadResource { 
        uri: String 
    },
    Subscribe { 
        uri: String 
    },
    CallTool { 
        name: String, 
        arguments: Value 
    },
}
```

**Protocol Features**:
- Resource discovery and subscription
- Tool invocation and results
- Bidirectional streaming
- Connection lifecycle management

### REST API Design

RESTful API following OpenAPI 3.0 specification:

```
GET    /api/agents           # List agents
POST   /api/agents           # Register agent
GET    /api/agents/{id}      # Get agent details
PUT    /api/agents/{id}      # Update agent
DELETE /api/agents/{id}      # Deregister agent

GET    /api/issues           # List issues  
POST   /api/issues           # Create issue
GET    /api/issues/{id}      # Get issue details
PUT    /api/issues/{id}      # Update issue
DELETE /api/issues/{id}      # Delete issue
```

**API Design Principles**:
- Resource-oriented URIs
- Consistent HTTP status codes
- Comprehensive error responses
- Pagination and filtering support

### WebSocket Real-time Communication

Real-time event broadcasting:

```rust
pub enum WebSocketEvent {
    AgentStatusChanged { agent_id: Uuid, status: AgentStatus },
    IssueCreated { issue: Issue },
    IssueStatusChanged { issue_id: Uuid, status: IssueStatus },
    MessageReceived { message: Message },
    SystemNotification { message: String },
}
```

## Security Architecture

### Authentication & Authorization

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   JWT Token     │───▶│  Auth Middle-   │───▶│   Permission    │
│   Validation    │    │  ware           │    │   Check         │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Token         │    │   User Context  │    │   Resource      │
│   Refresh       │    │   Injection     │    │   Access        │
└─────────────────┘    └─────────────────┘    └─────────────────┘
```

**Security Features**:
- JWT-based authentication
- Role-based access control
- API rate limiting
- Request/response encryption
- Audit logging

### Data Protection

- **Encryption at Rest**: Sensitive data encrypted in database
- **Encryption in Transit**: TLS for all communications
- **Key Management**: Secure key generation and rotation
- **Data Anonymization**: PII scrubbing in logs and exports

## Scalability Architecture

### Horizontal Scaling

```
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  Load Balancer   │    │  Load Balancer   │    │  Load Balancer   │
│                  │    │                  │    │                  │
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐
│  MCP Server      │    │  MCP Server      │    │  MCP Server      │
│  Instance 1      │    │  Instance 2      │    │  Instance N      │
└────────┬─────────┘    └────────┬─────────┘    └────────┬─────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                                 ▼
                    ┌──────────────────┐
                    │  Shared Database │
                    │  (PostgreSQL)    │
                    └──────────────────┘
```

**Scaling Strategies**:
- Stateless service design
- Database connection pooling
- Caching layer (Redis)
- Message queue for async processing

### Performance Optimization

- **Connection Pooling**: Optimized database connections
- **Query Optimization**: Indexed queries and batch operations
- **Caching**: In-memory and distributed caching
- **Async Processing**: Non-blocking I/O operations

## Monitoring and Observability

### Metrics Collection

```rust
// Example metrics
counter!("agents.registered", &[("type", agent_type)]);
histogram!("api.request_duration", duration, &[("endpoint", endpoint)]);
gauge!("database.connections", active_connections);
```

**Key Metrics**:
- Agent registration/deregistration rates
- Issue creation and resolution times
- API response times and error rates
- Database connection pool utilization

### Distributed Tracing

```rust
#[tracing::instrument(skip(self))]
async fn create_issue(&self, issue: Issue) -> Result<Issue> {
    tracing::info!("Creating issue: {}", issue.title);
    // Implementation
}
```

**Tracing Features**:
- Request correlation across services
- Performance bottleneck identification
- Error propagation tracking
- Service dependency mapping

## Deployment Architecture

### Container Architecture

```dockerfile
# Multi-stage build
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates
COPY --from=builder /app/target/release/vibe-ensemble-server /usr/local/bin/
EXPOSE 8080
CMD ["vibe-ensemble-server"]
```

### Orchestration

```yaml
# Kubernetes deployment example
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vibe-ensemble-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: vibe-ensemble-server
  template:
    spec:
      containers:
      - name: server
        image: vibe-ensemble:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: database-secret
              key: url
```

## Testing Architecture

### Test Pyramid

```
                    ┌─────────────────┐
                    │   E2E Tests     │
                    │  (Integration)  │
                    └─────────────────┘
              ┌─────────────────────────────┐
              │     Integration Tests       │
              │   (Service Layer)          │
              └─────────────────────────────┘
        ┌───────────────────────────────────────┐
        │            Unit Tests                 │
        │     (Domain & Repository)            │
        └───────────────────────────────────────┘
```

**Test Categories**:
- **Unit Tests**: Domain logic and individual components
- **Integration Tests**: Service interactions and database operations
- **End-to-End Tests**: Complete user workflows
- **Performance Tests**: Load and stress testing
- **Property-Based Tests**: Generated input validation

## Future Architecture Considerations

### Microservices Evolution

Potential service boundaries for future decomposition:
- **Agent Management Service**: Dedicated agent lifecycle management
- **Knowledge Service**: Advanced AI-powered knowledge management
- **Workflow Service**: Complex issue workflow orchestration
- **Notification Service**: Multi-channel notification delivery

### Event-Driven Architecture

Migration to event-driven patterns:
- **Event Store**: Audit trail and state reconstruction
- **CQRS**: Command-Query Responsibility Segregation
- **Event Sourcing**: State as sequence of events
- **Saga Pattern**: Distributed transaction management

### Cloud-Native Features

- **Service Mesh**: Advanced traffic management and security
- **Serverless Functions**: Event-triggered processing
- **Managed Databases**: Cloud provider database services
- **Auto-scaling**: Dynamic resource allocation

---

*This architecture documentation is living and will evolve as the system grows. For implementation details, see the [Developer Setup Guide](setup.md) and [Contributing Guide](contributing.md).*