# Vibe Ensemble

A powerful multi-agent coordination system that enables Claude Code instances to collaborate seamlessly through dual-transport architecture (WebSocket + SSE), intelligent task orchestration, and automated worker management with real-time permission approval workflows.

## What is Vibe Ensemble?

Vibe Ensemble is an advanced coordination server that transforms how multiple Claude Code instances work together on complex projects. Built on a dual-transport architecture (WebSocket + SSE), it provides real-time communication, intelligent task distribution, automated worker spawning, and seamless coordinator approval workflows to create a true multi-agent development environment.

Think of it as an intelligent "mission control" that not only prevents conflicts but actively orchestrates teamwork across your AI development agents.

## Key Features

### Dual-Transport Architecture (WebSocket + SSE)
- **Real-time Communication**: WebSocket-based protocol for instant agent coordination
- **SSE Message Delivery**: Server-Sent Events for reliable coordinator-worker communication
- **Concurrent Connections**: Support for 10-50+ concurrent agents per instance
- **Automatic Reconnection**: Robust connection management with graceful failure handling
- **MCP 2.0 Compliance**: Full JSON-RPC 2.0 protocol implementation over WebSocket

### Intelligent Task Orchestration
- **Automated Worker Spawning**: Automatically creates specialized workers for specific tasks
- **Task-Worker Mapping**: Intelligent assignment based on capabilities and workload
- **Auto-Approval Workflow**: Seamless coordinator approval of worker permissions
- **Retry Logic**: Automatic retry with exponential backoff for failed operations
- **Lifecycle Management**: Complete worker lifecycle from spawn to cleanup

### Advanced Coordination Features
- **Conflict Prevention**: Proactive detection and resolution of agent conflicts
- **Knowledge Sharing**: Dynamic pattern recognition and insight distribution
- **Issue Tracking**: Comprehensive task management with priority handling
- **Cross-Project Learning**: Shared expertise across multiple project boundaries
- **Permission Monitoring**: Proactive monitoring and auto-approval of worker permissions
- **Real-time Updates**: SSE-powered live updates for coordinators and workers

### Production-Ready Infrastructure
- **Web Dashboard**: Real-time monitoring with system metrics and agent analytics
- **SQLite Storage**: Persistent coordination data with ACID guarantees
- **Security Hardening**: Process isolation, localhost-only binding, and data ownership
- **Cross-Platform**: Mac, Linux, and Windows support with automated releases
- **378+ Tests**: Comprehensive test suite ensuring reliability and stability

## Quick Start

### 1. Install

**macOS/Linux:**
```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash
```

**Windows:**
```bash
iwr https://vibeensemble.dev/install.ps1 -UseBasicParsing | iex
```

Note: Verify installer integrity (checksum/signature) before executing:
```bash
curl -fsSLO https://vibeensemble.dev/install.sh
shasum -a 256 install.sh  # or sha256sum
# Compare against the published checksum on the releases page
bash install.sh
```

### 2. Start the WebSocket Server

```bash
# Start the full system (WebSocket MCP + Web Dashboard)
vibe-ensemble

# Or start WebSocket MCP server only
vibe-ensemble --mcp-only --transport=websocket
```

This launches:
- **WebSocket MCP Server** on `ws://127.0.0.1:8081` (default port)
- **Web Dashboard** at `http://127.0.0.1:8080` (if not using --mcp-only)
- **SQLite Database** in project-local directory: `./.vibe-ensemble/data.db`

### 3. Connect Claude Code Agents

Configure Claude Code to connect via WebSocket MCP:

```json
{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "vibe-ensemble",
      "args": ["--mcp-only", "--transport=websocket", "--port=8081"],
      "transport": {
        "type": "websocket",
        "url": "ws://127.0.0.1:8081"
      }
    }
  }
}
```

**Alternative stdio transport** (legacy support):
```json
{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "vibe-ensemble --mcp-only --transport=stdio",
      "args": []
    }
  }
}
```

### 4. Verify Connection

Check agent coordination through the web dashboard or verify with:

```bash
# Check server status
curl http://127.0.0.1:8080/api/health

# List connected agents  
curl http://127.0.0.1:8080/api/agents
```

Your Claude Code instances now have access to powerful multi-agent coordination tools!

## How the Dual-Transport Architecture Works

### Multi-Agent Coordination Flow

```
┌─────────────┐    WebSocket     ┌─────────────────────┐
│ Claude Code │◄────────────────►│   Vibe Ensemble    │
│ Coordinator │   JSON-RPC       │   Dual Transport    │
└─────────────┘                  │  (WebSocket + SSE)  │
                                 └─────────────────────┘
                                           │
                                   Auto-Approval &
                                    SSE Messages
                                           ▼
┌─────────────┐    WebSocket     ┌─────────────────────┐
│ Claude Code │◄────────────────►│  Task Orchestrator  │
│  Worker 1   │   Real-time      │  & Worker Manager   │
└─────────────┘   Coordination   │                     │
                                 │  Permission         │
┌─────────────┐    WebSocket     │  Auto-Approval      │
│ Claude Code │◄─────────────────│  Workflow           │
│  Worker 2   │   Multi-agent    │                     │
└─────────────┘   Protocol       └─────────────────────┘
```

### Coordination Process

1. **WebSocket Connection**: Each Claude Code agent connects via WebSocket protocol
2. **Agent Registration**: Agents register capabilities and specializations with auto-replacement
3. **Task Distribution**: Coordinator creates tasks and spawns specialized workers
4. **Permission Auto-Approval**: Seamless approval of worker permissions via coordinator
5. **Real-time Coordination**: Agents communicate instantly through WebSocket + SSE channels
6. **Conflict Resolution**: Proactive detection and intelligent resolution protocols
7. **Knowledge Synthesis**: Shared learning across all connected agents
8. **Automated Cleanup**: Worker lifecycle management with graceful termination

## Use Cases

### Advanced Single Developer Workflows
- **Intelligent Task Distribution**: Coordinator agent creates tasks, spawns specialized workers automatically
- **Multi-Component Projects**: Frontend, backend, testing, and documentation agents working in harmony
- **Automated Code Review**: Review agents triggered automatically on code changes
- **Cross-Project Learning**: Knowledge patterns shared between different projects

### Small Team Multi-Agent Coordination
- **Distributed Agent Networks**: Each developer runs coordinated agent clusters
- **Centralized Task Management**: Shared task orchestration across team members
- **Real-time Conflict Prevention**: Instant notifications when agents might conflict
- **Knowledge Amplification**: Team-wide pattern recognition and best practice sharing

### Production Development Scenarios
- **CI/CD Integration**: Agents coordinate with build systems and deployment pipelines
- **Issue-Driven Development**: Automatic worker spawning based on GitHub issues or tickets
- **Quality Assurance Networks**: Multiple testing agents ensuring comprehensive coverage
- **Documentation Automation**: Specialized documentation agents maintaining up-to-date docs

## Dual-Transport Architecture

Vibe Ensemble is built for scalability and seamless real-time coordination:

- **WebSocket + SSE**: Real-time multi-agent communication with JSON-RPC 2.0 over WebSocket + Server-Sent Events
- **Auto-Approval Workflow**: Intelligent coordinator approval of worker permissions without human intervention
- **Local-First**: Complete privacy with all data stored locally on your machine
- **SQLite Storage**: High-performance persistent storage with ACID guarantees
- **Task Orchestration**: Intelligent worker spawning and lifecycle management with permission automation
- **Web Dashboard**: Production-ready monitoring with real-time metrics and agent analytics
- **Legacy Support**: WebSocket for modern coordination + stdio for backward compatibility

## Building from Source

If you prefer to build from source:

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

The binary will be at `target/release/vibe-ensemble`.

## Configuration

### Zero Configuration Setup
Vibe Ensemble works out of the box, but offers powerful customization:

```bash
# Default: WebSocket MCP + Web Dashboard + SQLite
vibe-ensemble

# WebSocket MCP server only (no web dashboard)
vibe-ensemble --mcp-only --transport=websocket

# Custom ports for WebSocket and web interfaces
vibe-ensemble --port=8081 --web-port=8080

# Custom database location
DATABASE_URL="sqlite://./my-project.db" vibe-ensemble

# Legacy stdio transport (backward compatibility)
vibe-ensemble --mcp-only --transport=stdio
```

### Advanced Configuration

```bash
# Production deployment with custom settings
vibe-ensemble \
  --port=8081 \
  --web-port=8080 \
  --host=127.0.0.1 \
  --max-workers=50 \
  --task-timeout=7200

# Multi-project coordination
DATABASE_URL="sqlite:///shared/coordination.db" vibe-ensemble --port=8081
```

### Environment Variables

```bash
# Database configuration
export DATABASE_URL="sqlite://./project-coordination.db"

# Network configuration  
export VIBE_ENSEMBLE_PORT=8081
export VIBE_WEB_PORT=8080
export VIBE_HOST="127.0.0.1"

# Worker management
export VIBE_MAX_WORKERS=25
export VIBE_TASK_TIMEOUT=3600

# Logging
export RUST_LOG=vibe_ensemble=info
```

## Documentation

- [Installation Guide](docs/installation.md) - WebSocket server setup and Claude Code configuration
- [Architecture Guide](docs/architecture.md) - WebSocket protocol, task orchestration, and system design
- [User Guide](docs/user-guide.md) - Multi-agent workflows and coordination patterns
- [Configuration Guide](docs/configuration.md) - Advanced settings and deployment options
- [Troubleshooting Guide](docs/troubleshooting.md) - WebSocket connection issues and debugging
- [Developer Guide](docs/developer-guide.md) - Contributing and extending the system

## Support

- [GitHub Issues](https://github.com/siy/vibe-ensemble-mcp/issues) - Bug reports and feature requests
- [Discussions](https://github.com/siy/vibe-ensemble-mcp/discussions) - Questions and community

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.