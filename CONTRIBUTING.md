# Contributing to Vibe Ensemble MCP Server

We welcome contributions to the Vibe Ensemble MCP Server! This document provides guidelines for contributing to the project.

## Development Environment

### Prerequisites

- Rust 1.70+ with Cargo
- SQLite (embedded, no separate installation needed)
- Claude Code (for testing worker processes)

### Setup

1. Fork and clone the repository:
   ```bash
   git clone https://github.com/YOUR_USERNAME/vibe-ensemble-mcp.git
   cd vibe-ensemble-mcp
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   ```

4. Start the development server:
   ```bash
   cargo run -- --port 3000 --log-level debug
   ```

## Code Style

### Rust Guidelines

- Follow standard Rust formatting: `cargo fmt`
- Ensure code passes linting: `cargo clippy`
- Add documentation for public APIs
- Use descriptive variable and function names

### Database Schema

- All database changes must include migrations in `src/database/schema.rs`
- Test database operations thoroughly
- Use parameterized queries to prevent SQL injection

### MCP Protocol

- Follow the Model Context Protocol specification
- Include proper error handling for all MCP operations
- Test MCP tools with Claude Code integration

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run with debug output
RUST_LOG=debug cargo test

# Run specific test
cargo test test_name
```

### Test Coverage

- Write unit tests for new functionality
- Test error conditions and edge cases
- Include integration tests for MCP tools
- Test database operations with SQLite

## Submitting Changes

### Pull Request Process

1. Create a feature branch from `main`:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes following the code style guidelines

3. Ensure all tests pass and code is properly formatted:
   ```bash
   cargo fmt --all
   cargo clippy -- -D warnings
   cargo test
   ```

4. Commit your changes with descriptive messages:
   ```bash
   git commit -m "feat: add new MCP tool for worker management"
   ```

5. Push to your fork and create a pull request

### Commit Message Guidelines

Use conventional commit format:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `style:` - Code style/formatting
- `refactor:` - Code refactoring
- `test:` - Adding or updating tests
- `chore:` - Maintenance tasks

### Pull Request Requirements

- [ ] All tests pass
- [ ] Code is properly formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation is updated if needed
- [ ] Commit messages follow conventional format
- [ ] Changes are backward compatible when possible

## Reporting Issues

### Bug Reports

When reporting bugs, please include:
- Rust version (`rustc --version`)
- Operating system and version
- Steps to reproduce the issue
- Expected vs actual behavior
- Relevant log output (use `--log-level debug`)

### Feature Requests

For feature requests, describe:
- The problem you're trying to solve
- Proposed solution or implementation
- Alternative approaches considered
- Impact on existing functionality

## Project Structure

```
src/
├── config.rs          # Configuration management
├── database/           # Database layer and migrations
├── error.rs           # Error types and handling
├── main.rs            # Application entry point
├── mcp/               # MCP protocol implementation
├── server.rs          # HTTP server setup
└── workers/           # Worker process management
```

## Architecture Guidelines

### Separation of Concerns

- Keep MCP protocol logic separate from business logic
- Use the database layer for all data persistence
- Handle errors gracefully with proper logging
- Maintain clear boundaries between modules

### Performance Considerations

- Use async/await for I/O operations
- Implement proper connection pooling for database
- Consider memory usage for long-running processes
- Profile performance-critical code paths

## Getting Help

- Check existing issues and discussions
- Join our community discussions
- Review the MCP protocol documentation
- Look at existing code for patterns and examples

## Code of Conduct

Please be respectful and constructive in all interactions. We're building this project together and value diverse perspectives and contributions.

Thank you for contributing to Vibe Ensemble MCP Server!