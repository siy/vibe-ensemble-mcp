# Vibe Ensemble MCP Server

A comprehensive MCP (Model Context Protocol) server in Rust for coordinating multiple Claude Code instances with distributed task execution, unified management, communication, and issue tracking.

## Overview

Vibe Ensemble is an advanced MCP server that serves as the central coordination hub for multiple Claude Code instances, enabling:

- **Distributed Task Execution**: Coordinate work across multiple AI agents
- **Unified Management**: Centralized control and monitoring of agent ecosystem
- **Real-time Communication**: Seamless messaging between coordinator and worker agents
- **Issue Tracking**: Persistent task and problem management with web interface
- **Knowledge Management**: Organizational patterns, practices, and guidelines repository
- **System Prompt Management**: Sophisticated AI behavior configuration

## Architecture

The system is built around five core subsystems:

1. **Agent Management System** - Registration, lifecycle, and capability tracking
2. **Issue Tracking System** - Persistent storage and workflow management
3. **Messaging System** - Real-time communication with standardized protocols
4. **Knowledge Management System** - Development patterns and organizational knowledge
5. **Persistence Layer** - Data consistency and recovery across all subsystems

### Agent Hierarchy

- **Coordinator Agent**: Configured as Claude Code Team Coordinator, serves as the primary interface between human users and the worker ecosystem
- **Worker Agents**: Execute individual tasks autonomously while contributing to shared knowledge repositories

## Features

### Core Capabilities
- ✅ Agent registration and discovery
- ✅ Real-time messaging infrastructure
- ✅ Persistent issue tracking
- ✅ Knowledge repository with search
- ✅ System prompt management
- ✅ Web interface for issue management

### Advanced Features
- 🔄 Claude Code agent generation and orchestration
- 🔄 Knowledge-aware messaging
- 🔄 Pattern recognition and extraction
- 🔄 Performance monitoring and analytics
- 🔄 Role-based access control

## Getting Started

### Prerequisites

- Rust 1.70+ with Cargo
- SQLite (for development)
- Git

### Development Setup

1. **Clone the repository**:
   ```bash
   git clone git@github.com:siy/vibe-ensemble-mcp.git
   cd vibe-ensemble-mcp
   ```

2. **Build the project**:
   ```bash
   cargo build
   ```

3. **Run tests**:
   ```bash
   cargo test
   ```

4. **Start the development server**:
   ```bash
   cargo run --bin vibe-ensemble-server
   ```

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
├── crates/
│   ├── vibe-ensemble-core/     # Core domain models and traits
│   ├── vibe-ensemble-mcp/      # MCP protocol implementation
│   ├── vibe-ensemble-server/   # Main server application
│   ├── vibe-ensemble-storage/  # Persistence layer
│   ├── vibe-ensemble-web/      # Web interface
│   └── vibe-ensemble-prompts/  # System prompts management
├── docs/                       # Documentation
├── migrations/                 # Database migrations
└── examples/                   # Usage examples
```

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

- **Phase 1**: Foundation & Core Infrastructure ✅
- **Phase 2**: MCP Protocol Integration 🔄
- **Phase 3**: Agent Coordination System 🔄
- **Phase 4**: Knowledge Management System 🔄
- **Phase 5**: System Prompts & AI Configuration 🔄
- **Phase 6**: Web Interface for Issue Tracking 🔄
- **Phase 7**: Advanced Features & Production Readiness 🔄

## Technology Stack

- **Language**: Rust 2021 Edition
- **Async Runtime**: Tokio
- **Database**: SQLite (dev) / PostgreSQL (prod)
- **Web Framework**: Axum
- **Template Engine**: Askama
- **Protocol**: Model Context Protocol (MCP)
- **Transport**: WebSocket, HTTP
- **Testing**: Built-in Rust testing + Custom integration framework

---

Built with ❤️ using Rust and the Model Context Protocol