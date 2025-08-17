# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust project implementing an MCP (Model Context Protocol) server for team coordination between multiple Claude Code instances. The system enables distributed task execution with unified management, communication, and issue tracking capabilities.

## Development Commands

This project uses standard Rust toolchain commands:

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run specific test
cargo test <test_name>

# Format code
cargo fmt

# Run clippy linter
cargo clippy

# Run clippy with strict warnings
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Check compilation without building
cargo check

# Clean build artifacts
cargo clean
```

## Architecture Overview

The system is designed around five core subsystems:

1. **Agent Management System** - Handles registration, lifecycle, and capability tracking for connected Claude Code agents
2. **Issue Tracking System** - Provides persistent storage and workflow management for tasks requiring resolution
3. **Messaging System** - Enables real-time communication between agents using standardized protocols
4. **Knowledge Management System** - Collects, organizes, and distributes development patterns and practices
5. **Persistence Layer** - Ensures data consistency and recovery across all subsystems

### Agent Hierarchy

- **Coordinator Agent** - Configured as Claude Code Team Coordinator, serves as primary interface between human users and worker ecosystem
- **Worker Agents** - Execute individual tasks autonomously while contributing to shared knowledge repositories

The coordinator maintains global context, performs strategic planning, manages resource allocation, and serves as the central repository for organizational knowledge and development standards.

## Knowledge Management

The system includes a comprehensive knowledge repository that:
- Maintains development patterns and proven solutions
- Stores organizational standards and methodologies  
- Provides access control and versioning for development guidelines
- Enables pattern recognition and knowledge extraction from agent interactions

## Configuration

The project includes Claude Code-specific configuration in `.claude/settings.local.json` with pre-approved permissions for common Rust development commands and git operations.

## Development Focus

When working on this codebase:
- Follow idiomatic Rust patterns with ownership, borrowing, and lifetimes
- Use the type system for correctness and zero-cost abstractions
- Implement explicit error handling with Result types
- Leverage async/await for concurrent operations
- Include comprehensive tests and documentation