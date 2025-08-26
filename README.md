# Vibe Ensemble MCP Server

A comprehensive MCP (Model Context Protocol) server in Rust for coordinating multiple Claude Code instances with distributed task execution, unified management, communication, and issue tracking.

## Overview

Vibe Ensemble is an advanced MCP server that serves as the central coordination hub for multiple Claude Code instances, featuring intelligent coordination AI, enabling:

- **Intelligent Agent Coordination**: AI-powered dependency detection, conflict resolution, and escalation management
- **Distributed Task Execution**: Seamless work coordination across multiple AI agents with proactive monitoring
- **Cross-Project Integration**: Advanced tools for managing dependencies and coordination across project boundaries
- **Real-time Communication**: Sophisticated messaging with structured protocols and coordination awareness
- **Issue Tracking**: Persistent task and problem management with intelligent workflow automation
- **Knowledge-Driven Decisions**: Pattern recognition and organizational learning for continuous improvement
- **Dynamic Prompt Management**: Advanced AI behavior configuration with coordination specialists

## Architecture

The system is built around five core subsystems:

1. **Agent Management System** - Registration, lifecycle, and capability tracking
2. **Issue Tracking System** - Persistent storage and workflow management
3. **Messaging System** - Real-time communication with standardized protocols
4. **Knowledge Management System** - Development patterns and organizational knowledge
5. **Persistence Layer** - Data consistency and recovery across all subsystems

### Intelligent Agent Hierarchy

- **Coordinator Agent**: Enhanced with intelligent coordination prompts featuring dependency detection, conflict resolution protocols, and automated escalation management
- **Worker Agents**: Execute tasks with coordination awareness, proactive dependency detection, and intelligent escalation protocols  
- **Specialist Coordinators**: Cross-project coordinators, conflict resolvers, and escalation managers for complex multi-agent scenarios

All agents use MCP coordination tools: `vibe/dependency/analyze`, `vibe/conflict/detect`, `vibe/agent/message`, `vibe/coordination/escalate`.

## Features

### Core Foundation (✅ Implemented)
- **Agent Management**: Complete registration, lifecycle, and capability tracking
- **Message System**: Full messaging infrastructure with delivery confirmations
- **Issue Tracking**: Comprehensive workflow management and persistence
- **Knowledge Management**: Repository with search, tagging, and access control
- **Intelligent Prompt Management**: Advanced coordination specialists with A/B testing, metrics, and hot-swapping
- **Database Layer**: SQLx-based persistence with migrations and optimizations
- **MCP Protocol Server**: Full implementation with 42 coordination tools for cross-project collaboration

### Implementation Status
- **Production-Ready Phase**: ✅ **Complete** (316+ tests passing)
- **Active Crates**: 6 comprehensive libraries for production deployment
- **Coordination Tools**: 42+ MCP tools for seamless multi-agent collaboration  
- **Web Dashboard**: ✅ Complete with real-time system monitoring and metrics
- **Production Server**: ✅ Complete with HTTP API, configuration hardening, and security
- **System Monitoring**: ✅ Complete with performance logging and resource tracking
- **Test Coverage**: Comprehensive unit, integration, coordination, and security testing
- **CI/CD**: Robust workflow with security auditing, cross-platform builds, and release automation

### Advanced Features (✅ Implemented)
- **Configuration Hardening**: Security validation and production warnings
- **System Metrics**: Real-time CPU, memory, disk, and database monitoring  
- **Performance Logging**: Request timing and slow query detection
- **Security Headers**: CSRF protection, content type validation, and XSS prevention
- **Cross-Platform Support**: Native binaries for Linux, macOS, and Windows

## Installation

### Quick Install (Recommended)

**macOS/Linux:**
```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash
```

**Windows PowerShell:**
```bash
iwr https://vibeensemble.dev/install.ps1 -UseBasicParsing | iex
```


### Binary Download
Download the latest release for your platform from [GitHub Releases](https://github.com/siy/vibe-ensemble-mcp/releases/latest).

### Starting the Server

After installation, start the Vibe Ensemble server:

```bash
vibe-ensemble
```

The server will start with:
- **Web Dashboard**: http://127.0.0.1:8081 (system monitoring, agent management)
- **API Endpoints**: http://127.0.0.1:8080 (health, stats, coordination)
- **MCP Server**: Running on configured transport (WebSocket/stdio)

## Claude Code Setup

### 1. Configure Claude Code MCP Settings

Add the Vibe Ensemble MCP server to your Claude Code configuration:

**Option A: Using Claude Code Settings UI**
1. Open Claude Code settings (Cmd/Ctrl + ,)
2. Navigate to "MCP Servers"
3. Add a new server with:
   - **Name**: `vibe-ensemble`
   - **Command**: `vibe-ensemble --mcp-only --transport=stdio`
   - **Args**: `--transport=stdio`

**Option B: Direct Configuration File**
Add to your Claude Code MCP settings file (`~/.config/claude-code/mcp_settings.json`):

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

### 2. Available MCP Tools

Once configured, Claude Code will have access to 42+ coordination tools including:

**Agent Management:**
- `vibe/agent/register` - Register a new agent with capabilities
- `vibe/agent/list` - List all registered agents
- `vibe/agent/message` - Send messages between agents

**Task Coordination:**
- `vibe/task/create` - Create and assign tasks
- `vibe/task/update` - Update task status and progress
- `vibe/dependency/analyze` - Analyze task dependencies
- `vibe/conflict/detect` - Detect and resolve conflicts

**Knowledge Management:**
- `vibe/knowledge/store` - Store development patterns and insights
- `vibe/knowledge/search` - Search organizational knowledge base
- `vibe/guideline/enforce` - Apply organizational standards

### 3. Multi-Agent Coordination

For teams using multiple Claude Code instances:

1. **Each agent registers automatically** when first using vibe tools
2. **Coordinate work** using `vibe/coordination/escalate` for complex tasks
3. **Share knowledge** through the centralized knowledge base
4. **Monitor progress** via the web dashboard at http://127.0.0.1:8081

### Building from Source (Development)

If you prefer to build from source or contribute to development:

1. **Prerequisites**: Rust 1.80+, SQLite, Git
2. **Clone and build**:
   ```bash
   git clone https://github.com/siy/vibe-ensemble-mcp.git
   cd vibe-ensemble-mcp
   cargo build --release
   ```
3. **Run**: `cargo run --bin vibe-ensemble`

### Configuration

The server uses a hierarchical configuration system:

- **Global defaults**: Built-in sensible defaults
- **Environment files**: `.env` for local development
- **Environment variables**: Override for deployment
- **Runtime configuration**: Database-stored settings

Example configuration:

```toml
[server]
host = "127.0.0.1"
port = 8080
max_connections = 1000

[database]
url = "sqlite:vibe-ensemble.db"
max_connections = 10

[mcp]
transport = "websocket"
timeout_seconds = 30
```

## Development

### Project Structure

```
vibe-ensemble-mcp/
├── vibe-ensemble-core/         # ✅ Core domain models and business logic
├── vibe-ensemble-storage/      # ✅ SQLx persistence layer with migrations  
├── vibe-ensemble-prompts/      # ✅ Intelligent prompt management with coordination specialists
├── vibe-ensemble-mcp/          # ✅ MCP protocol server with 42+ coordination tools
├── vibe-ensemble-server/       # ✅ Production server with HTTP API and configuration hardening
├── vibe-ensemble-web/          # ✅ Web dashboard with real-time monitoring and system metrics
├── config/                     # ⚙️ Production-ready configuration templates
├── scripts/                    # 📦 Cross-platform installation scripts
├── docs/                       # 📚 Comprehensive documentation with troubleshooting
├── tests/                      # 🧪 Integration, E2E, security, and performance testing
└── .github/workflows/          # 🔄 CI/CD with cross-platform builds and releases
```

**Current Status**: Production-ready phase complete with 316+ passing tests across 6 comprehensive crates including web dashboard and production server.

### Common Development Tasks

```bash
# Format code
cargo fmt

# Run clippy linter
cargo clippy

# Run clippy with strict warnings
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Check compilation without building
cargo check

# Run a specific test
cargo test <test_name>

# Clean build artifacts
cargo clean
```

### Testing

The project includes comprehensive testing:

- **Unit tests**: Individual component testing
- **Integration tests**: MCP protocol compliance
- **End-to-end tests**: Multi-agent scenarios
- **Performance tests**: Load and scalability testing

Run all tests:
```bash
cargo test --all
```

## Documentation

- [High-Level Design](docs/high-level-design.md) - System architecture and design decisions
- [Implementation Plan](docs/implementation-plan.md) - Detailed development roadmap
- [Git Worktrees Guide](docs/git-worktrees.md) - Parallel development with multiple agents
- [Contributing Guide](CONTRIBUTING.md) - Development workflow and standards

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Workflow

1. **Fork the repository**
2. **Create a feature branch**: `git checkout -b feature/amazing-feature`
3. **Make your changes** with tests and documentation
4. **Run the test suite**: `cargo test --all`
5. **Submit a pull request**

### Code Standards

- Follow Rust idioms and best practices
- Maintain test coverage above 90%
- Include documentation for public APIs
- Use conventional commit messages
- Ensure all CI checks pass

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

- **GitHub Issues**: [Report bugs or request features](https://github.com/siy/vibe-ensemble-mcp/issues)
- **Discussions**: [Community discussions and questions](https://github.com/siy/vibe-ensemble-mcp/discussions)
- **Documentation**: [Comprehensive guides and API reference](docs/)

## Roadmap

See our [Implementation Plan](docs/implementation-plan.md) for detailed development phases:

- **Phase 1**: Foundation & Core Infrastructure ✅ **COMPLETE**
  - Core domain models and business logic
  - SQLx persistence layer with migrations
  - Intelligent prompt management with coordination specialists
  
- **Phase 2**: MCP Protocol Integration ✅ **COMPLETE**
  - Full MCP protocol server implementation
  - 42 coordination tools for multi-agent collaboration
  - Agent registration and lifecycle management
  
- **Phase 3**: Intelligent Coordination System ✅ **COMPLETE**  
  - Advanced prompt management with coordination specialists
  - Cross-project coordination, conflict resolution, escalation management
  - Knowledge-driven coordination with pattern recognition
  - Comprehensive testing (324 tests passing)
  - Robust CI/CD pipeline with coordination validation
  
- **Phase 4**: Web Interface & Dashboard 🚧 **NEXT**
- **Phase 5**: Security & Monitoring 🚧 **PLANNED**
- **Phase 6**: Production Readiness 🚧 **PLANNED**

## Technology Stack

- **Language**: Rust 2021 Edition
- **Async Runtime**: Tokio
- **Database**: SQLite (dev) / PostgreSQL (prod)
- **Web Framework**: Axum
- **Template Engine**: Handlebars (for configs); Askama (planned for Web UI)
- **Protocol**: Model Context Protocol (MCP)
- **Transport**: WebSocket, HTTP
- **Testing**: Built-in Rust testing + Custom integration framework

---

Built with ❤️ using Rust and the Model Context Protocol