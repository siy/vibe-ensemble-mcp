# Architecture Overview

This document explains how Vibe Ensemble works internally and the design decisions behind its simplicity and reliability.

## Design Principles

### Local-First
Vibe Ensemble runs entirely on your local machine. No cloud services, no external dependencies, no data leaving your computer. This ensures:
- **Privacy**: Your code and coordination data stays private
- **Performance**: No network latency for coordination
- **Reliability**: Works offline and survives network issues
- **Control**: You own and control all your data

### SQLite-Only Storage
A single SQLite database file stores everything:
- **Zero Configuration**: No database server to install or configure
- **Portable**: Move your coordination data anywhere
- **Reliable**: ACID transactions and proven durability
- **Efficient**: Fast queries and minimal resource usage

### stdio Transport
Direct integration with Claude Code via MCP stdio protocol:
- **Simple**: No network configuration or port management
- **Secure**: Process-level isolation and communication
- **Reliable**: Direct process communication without network issues
- **Standard**: Uses the official MCP protocol specification

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Vibe Ensemble Process                    │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌──────────────┐    ┌─────────────┐  │
│  │   MCP Server    │    │ Web Server   │    │   SQLite    │  │
│  │   (stdio)       │    │ (dashboard)  │    │ (storage)   │  │
│  │                 │    │              │    │             │  │
│  │ • Agent Mgmt    │    │ • Monitoring │    │ • Agents    │  │
│  │ • Issue Track   │    │ • Analytics  │    │ • Issues    │  │
│  │ • Knowledge     │    │ • Health     │    │ • Messages  │  │
│  │ • Messaging     │    │ • Control    │    │ • Knowledge │  │
│  └─────────────────┘    └──────────────┘    └─────────────┘  │
└─────────────────────────────────────────────────────────────┘
           │                      │
           │ stdio/MCP            │ HTTP
           │                      │
┌──────────▼──────────┐          │
│   Claude Code #1    │          │
│                     │          │
│ • Frontend Agent    │          │
│ • React/TypeScript  │          │
└─────────────────────┘          │
                                 │
┌─────────────────────┐          │
│   Claude Code #2    │          │
│                     │          │
│ • Backend Agent     │          │
│ • Node.js/Database  │          │
└─────────────────────┘          │
                                 │
┌─────────────────────┐    ┌─────▼─────┐
│   Claude Code #3    │    │ Web       │
│                     │    │ Browser   │
│ • Testing Agent     │    │           │
│ • Jest/Cypress      │    │ Dashboard │
└─────────────────────┘    └───────────┘
```

## Core Components

### MCP Server
The heart of agent coordination:
- **Tool Registration**: Provides coordination tools to Claude Code agents
- **Request Handling**: Processes tool calls from agents
- **Data Management**: Stores and retrieves coordination data
- **Protocol Compliance**: Implements MCP 2024-11-05 specification

### Web Dashboard
Optional monitoring interface:
- **Agent Overview**: See active agents and their status
- **Issue Tracking**: Monitor shared tasks and assignments
- **Knowledge Base**: Browse shared insights and patterns
- **System Health**: Monitor performance and resource usage

### SQLite Database
Persistent storage for all coordination data:
- **Agents Table**: Registered agents and their capabilities
- **Issues Table**: Tasks, bugs, and coordination needs
- **Messages Table**: Inter-agent communication history  
- **Knowledge Table**: Shared patterns and insights

## Data Flow

### Agent Registration
1. Claude Code starts with Vibe Ensemble MCP server configured
2. Agent uses `vibe/agent/register` tool to register
3. Vibe Ensemble stores agent info in SQLite
4. Agent appears in web dashboard

### Issue Coordination
1. Agent A creates issue using `vibe/issue/create`
2. Issue stored in database with metadata
3. Agent B queries issues using `vibe/issue/list`
4. Agent B assigns itself using `vibe/issue/assign`
5. Both agents can track progress and share updates

### Knowledge Sharing
1. Agent discovers useful pattern during work
2. Agent stores insight using `vibe/knowledge/add`
3. Other agents can search knowledge using `vibe/knowledge/search`
4. Knowledge appears in web dashboard for browsing

### Conflict Prevention
1. Agent checks for potential conflicts using `vibe/conflict/detect`
2. System checks current assignments and file access patterns
3. Returns warnings if multiple agents working on same area
4. Agent can coordinate through messaging or issue assignment

## Security Model

### Process Isolation
- Each Claude Code instance runs as separate process
- MCP stdio provides process-level security boundary
- No network exposure of agent communication

### Local-Only Access
- Web dashboard binds to localhost only (127.0.0.1)
- No external network access required or provided
- SQLite database stored in user's local directory

### Data Ownership
- All data stored locally on user's machine
- No cloud services or external data transmission
- User has complete control over all coordination data

## Performance Characteristics

### Latency
- **Tool Calls**: ~1-5ms (local SQLite query)
- **Web Dashboard**: ~10-50ms (local HTTP)
- **Agent Registration**: ~5-10ms (database write)

### Throughput  
- **Concurrent Agents**: 10-50 agents per instance
- **Tool Calls**: 100+ calls per second
- **Database Size**: Handles millions of records efficiently

### Resource Usage
- **Memory**: 50-200MB typical usage
- **CPU**: Minimal when idle, <10% under load
- **Storage**: 10-100MB for typical coordination data

## Scalability Patterns

### Single Developer
- One Vibe Ensemble instance per developer
- Multiple projects can share same instance
- 5-10 agents typical for diverse project needs

### Small Team
- Each developer runs own Vibe Ensemble instance
- Agents can coordinate across instances (future feature)
- Shared knowledge via git or file sync

### Multiple Projects
- Same Vibe Ensemble instance handles multiple projects
- Agents can specialize per project or work across projects
- Knowledge and patterns shared between projects

## Extension Points

### New MCP Tools
Adding coordination capabilities:
1. Define tool in `vibe-ensemble-mcp` crate
2. Add database schema if needed
3. Implement tool logic and error handling
4. Add tests and documentation

### Storage Backends
Supporting different databases:
1. Implement storage trait in `vibe-ensemble-storage`
2. Add connection management and migrations
3. Update configuration system
4. Maintain compatibility with existing tools

### Transport Protocols
Supporting additional communication methods:
1. Implement transport in `vibe-ensemble-mcp`
2. Add protocol-specific message handling
3. Update server configuration
4. Ensure MCP compliance

## Monitoring and Observability

### Health Endpoints
- `/api/health` - Basic health check
- `/api/stats` - System statistics
- `/api/agents` - Active agent list

### Logging
- Structured logging with configurable levels
- Request/response logging for debugging
- Error logging with context and stack traces

### Metrics
- Agent connection count and duration
- Tool call frequency and latency
- Database query performance
- System resource utilization

## Future Considerations

### Multi-Instance Coordination
Potential for agents to coordinate across multiple Vibe Ensemble instances for larger teams.

### Enhanced Knowledge Management
Richer knowledge representation with tags, categories, and relationships.

### Advanced Conflict Detection
More sophisticated analysis of potential conflicts based on code analysis and change patterns.

### Integration APIs
Webhooks or APIs for integration with external development tools and CI/CD systems.

The architecture prioritizes simplicity, reliability, and local control while providing the essential coordination capabilities needed for effective multi-agent development workflows.