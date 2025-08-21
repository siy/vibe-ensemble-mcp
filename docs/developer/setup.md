# Developer Setup Guide

This guide will help you set up a development environment for the Vibe Ensemble MCP Server.

## Prerequisites

### System Requirements

- **Operating System**: Linux, macOS, or Windows (WSL recommended)
- **Memory**: Minimum 8GB RAM, 16GB recommended
- **Storage**: At least 2GB free space for dependencies and build artifacts
- **Network**: Internet connection for downloading dependencies

### Required Software

#### Rust Toolchain
Install Rust using rustup (recommended):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

Verify installation:
```bash
rustc --version
cargo --version
```

Required Rust version: **1.70 or later**

#### Database
For development, SQLite is used by default (no additional setup required).

For production-like testing, install PostgreSQL:

**macOS (Homebrew)**:
```bash
brew install postgresql
brew services start postgresql
```

**Ubuntu/Debian**:
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql
```

#### Git
```bash
# macOS
brew install git

# Ubuntu/Debian
sudo apt install git

# Verify
git --version
```

### Optional Tools

#### Development Tools
```bash
# Code formatting and linting
rustup component add rustfmt clippy

# Database CLI tools
cargo install sqlx-cli --features sqlite,postgres

# Performance profiling
cargo install cargo-flamegraph

# Security auditing
cargo install cargo-audit

# Documentation generation
cargo install cargo-docs
```

#### Editor/IDE Setup

**VS Code** (Recommended):
- Install the `rust-analyzer` extension
- Install the `CodeLLDB` extension for debugging
- Install the `SQLite Viewer` extension for database inspection

**Vim/Neovim**:
- Configure with `rust-analyzer` LSP
- Use plugins like `vim-rust` or `rust.vim`

**IntelliJ IDEA**:
- Install the IntelliJ Rust plugin

## Project Setup

### 1. Clone the Repository

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
```

### 2. Environment Configuration

Create a `.env` file in the project root:

```bash
cp .env.example .env
```

Edit `.env` with your configuration:

```bash
# Database Configuration
DATABASE_URL=sqlite://vibe-ensemble.db

# Server Configuration  
SERVER_HOST=127.0.0.1
SERVER_PORT=8080

# MCP Configuration
MCP_TRANSPORT=websocket
MCP_TIMEOUT_SECONDS=30

# Security Configuration (generate these in production)
JWT_SECRET=your-development-jwt-secret-key-here
ENCRYPTION_KEY=your-development-encryption-key-32-chars

# Logging Configuration
RUST_LOG=info,vibe_ensemble=debug

# Development Features
ENABLE_API_DOCS=true
ENABLE_METRICS=true
```

### 3. Database Setup

Initialize the database and run migrations:

```bash
# Create the database
cargo run --bin vibe-ensemble-server -- migrate

# Or using sqlx-cli (if installed)
sqlx database create
sqlx migrate run
```

### 4. Build the Project

```bash
# Build all workspace members
cargo build

# Build in release mode for performance testing
cargo build --release
```

### 5. Run Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_agent_registration

# Run integration tests only
cargo test --test integration

# Run with coverage (requires tarpaulin)
cargo tarpaulin --out html
```

### 6. Code Quality Checks

```bash
# Format code
cargo fmt

# Check formatting
cargo fmt --check

# Run clippy linter
cargo clippy

# Run clippy with strict settings
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

## Development Workflow

### 1. Start the Development Server

```bash
# Start with default configuration
cargo run --bin vibe-ensemble-server

# Start with custom config
cargo run --bin vibe-ensemble-server -- --config custom-config.toml

# Start with environment overrides
RUST_LOG=debug cargo run --bin vibe-ensemble-server
```

The server will start on `http://localhost:8080` by default.

### 2. Verify Installation

**Health Check**:
```bash
curl http://localhost:8080/api/health
```

**Web Interface**:
Visit `http://localhost:8080` in your browser.

**API Documentation**:
Visit `http://localhost:8080/docs` for interactive API documentation.

### 3. Development Commands

```bash
# Watch for file changes and rebuild
cargo watch -x run

# Run specific workspace member
cargo run -p vibe-ensemble-core --example basic

# Clean build artifacts
cargo clean

# Update dependencies
cargo update
```

## Development Practices

### Code Style

The project follows standard Rust conventions:

- Use `rustfmt` for consistent formatting
- Follow Rust naming conventions (snake_case for functions, PascalCase for types)
- Write comprehensive documentation for public APIs
- Include examples in documentation when helpful

### Testing Strategy

- **Unit Tests**: Test individual functions and components
- **Integration Tests**: Test component interactions
- **End-to-End Tests**: Test complete workflows
- **Property-Based Tests**: Test with generated inputs
- **Performance Tests**: Benchmark critical paths

### Git Workflow

1. **Create Feature Branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make Changes**: Follow the coding standards and include tests

3. **Commit Changes**:
   ```bash
   git add .
   git commit -m "feat: brief description of changes"
   ```

4. **Push and Create PR**:
   ```bash
   git push origin feature/your-feature-name
   ```

### Commit Message Format

Use conventional commits:

```
type: brief description

Optional longer description explaining the change in detail.

- Breaking changes should be noted
- Include references to issues: Fixes #123
```

**Types**:
- `feat`: New features
- `fix`: Bug fixes
- `docs`: Documentation changes
- `style`: Code style changes (no logic changes)
- `refactor`: Code refactoring
- `test`: Adding or updating tests
- `chore`: Build process or auxiliary tool changes

## Debugging

### Logging

The project uses `tracing` for structured logging:

```rust
use tracing::{debug, info, warn, error};

debug!("Debug information");
info!("General information");
warn!("Warning message");
error!("Error occurred: {}", error);
```

### Environment Variables

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable trace logging for specific modules
RUST_LOG=vibe_ensemble::agent=trace cargo run

# Disable all logging except errors
RUST_LOG=error cargo run
```

### Database Debugging

```bash
# Connect to SQLite database
sqlite3 vibe-ensemble.db

# View database schema
.schema

# Run custom queries
SELECT * FROM agents;
```

### Performance Profiling

```bash
# Generate flamegraph (requires cargo-flamegraph)
cargo flamegraph --bin vibe-ensemble-server

# Run benchmarks
cargo bench

# Profile specific test
cargo test --release test_name -- --nocapture
```

## Common Development Tasks

### Adding New Features

1. **Design**: Document the feature in the appropriate design document
2. **Implementation**: Create the feature following existing patterns
3. **Tests**: Add comprehensive tests for the new functionality
4. **Documentation**: Update API documentation and user guides
5. **Integration**: Ensure the feature integrates well with existing code

### Database Schema Changes

1. **Create Migration**:
   ```bash
   sqlx migrate add your_migration_name
   ```

2. **Edit Migration**: Update the generated SQL file in `vibe-ensemble-storage/migrations/`

3. **Test Migration**:
   ```bash
   sqlx migrate run
   cargo test
   ```

### Adding New API Endpoints

1. **Define Handler**: Add handler function in appropriate module
2. **Add Route**: Register route in router configuration
3. **Update OpenAPI**: Update the OpenAPI specification
4. **Add Tests**: Create integration tests for the endpoint
5. **Update Documentation**: Add to API documentation

## Troubleshooting

### Common Issues

**Build Errors**:
```bash
# Update Rust toolchain
rustup update

# Clear target directory
cargo clean

# Check for dependency conflicts
cargo tree --duplicates
```

**Database Issues**:
```bash
# Reset database
rm vibe-ensemble.db
cargo run --bin vibe-ensemble-server -- migrate

# Check migration status
sqlx migrate info
```

**Port Already in Use**:
```bash
# Find process using port 8080
lsof -i :8080

# Kill the process
kill -9 <PID>

# Or use different port
SERVER_PORT=8081 cargo run
```

### Getting Help

- **Documentation**: Check existing documentation first
- **Issues**: Search GitHub issues for similar problems
- **Discussions**: Use GitHub Discussions for questions
- **Community**: Join community channels for real-time help

## Next Steps

After setting up your development environment:

1. **Read Architecture Documentation**: Understand the system design
2. **Run Examples**: Execute provided examples to understand usage
3. **Contribute**: Look for "good first issue" labels on GitHub
4. **Test**: Run the full test suite and add new tests

## Contributing

See the [Contributing Guide](contributing.md) for detailed information about:

- Code review process
- Pull request guidelines
- Release process
- Community guidelines

---

*For additional help, see the [Troubleshooting Guide](../troubleshooting/common-issues.md) or create an issue on GitHub.*