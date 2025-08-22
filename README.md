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

### Core Foundation (✅ Implemented)
- **Agent Management**: Complete registration, lifecycle, and capability tracking
- **Message System**: Full messaging infrastructure with delivery confirmations
- **Issue Tracking**: Comprehensive workflow management and persistence
- **Knowledge Management**: Repository with search, tagging, and access control
- **Prompt Management**: Template system with experimentation and metrics
- **Database Layer**: SQLx-based persistence with migrations and optimizations

### Implementation Status
- **Foundation Phase**: ✅ **Complete** (204 tests passing)
- **Active Crates**: 3 core libraries fully implemented
- **Test Coverage**: Comprehensive unit and integration testing
- **CI/CD**: Minimal, efficient workflow with security auditing

### Next Development Phase (🚧 Planned)
- **MCP Protocol Server**: Protocol implementation and agent coordination
- **Web Dashboard**: Issue tracking and management interface  
- **Security Layer**: Authentication, authorization, and rate limiting
- **Monitoring**: Observability, metrics, and health checks
- **Production Server**: Main application with HTTP endpoints

## Getting Started

### Prerequisites

- Rust 1.80+ with Cargo (as specified in rust-toolchain.toml)
- SQLite (for development and testing)
- Git
- cargo-audit (for security auditing)

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

4. **Verify setup** (foundation crates only):
   ```bash
   # Run all 204 tests
   cargo test --workspace
   
   # Check code quality
   cargo clippy --all-targets --all-features
   
   # Verify security
   cargo audit
   ```

   **Note**: The main server (`vibe-ensemble-server`) is not yet implemented in the current workspace.

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
├── vibe-ensemble-prompts/      # ✅ Prompt management and templating
├── docs/                       # 📚 Comprehensive documentation
├── tests/                      # 🧪 Integration and E2E testing
├── .github/workflows/          # 🔄 Minimal CI/CD pipeline
└── [excluded crates]/          # 🚧 Future development phases:
    ├── vibe-ensemble-mcp/      #    MCP protocol implementation  
    ├── vibe-ensemble-server/   #    Main server application
    ├── vibe-ensemble-web/      #    Web dashboard interface
    ├── vibe-ensemble-security/ #    Security and auth middleware
    └── vibe-ensemble-monitoring/ #   Observability and metrics
```

**Current Status**: Foundation phase complete with 204 passing tests across 3 core crates.

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
  - Prompt management and templating system
  - Comprehensive testing (204 tests passing)
  - Minimal CI/CD pipeline with security auditing
  
- **Phase 2**: MCP Protocol Integration 🚧 **NEXT**
- **Phase 3**: Agent Coordination System 🚧 **PLANNED**  
- **Phase 4**: Web Interface & Dashboard 🚧 **PLANNED**
- **Phase 5**: Security & Monitoring 🚧 **PLANNED**
- **Phase 6**: Production Readiness 🚧 **PLANNED**

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