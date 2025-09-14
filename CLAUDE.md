# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

```bash
# Build and run the server
cargo build
cargo run -- --port 3000

# Development with debug logging
cargo run -- --port 3000 --log-level debug

# Generate Claude Code configuration
cargo run -- --configure-claude-code --host 127.0.0.1 --port 3000

# Build release version
cargo build --release

# Check code without building
cargo check

# Run tests (when implemented)
cargo test
```

## Architecture Overview

This is a multi-agent coordination MCP server that enables Claude Code instances to work as specialized workers on different stages of complex projects.

### Core Components

**Server Layer** (`src/server.rs`):
- Axum HTTP server with CORS, request limiting, and tracing
- Health check endpoint with database connectivity testing
- Worker respawn system for unfinished tasks on startup

**Database Layer** (`src/database/`):
- SQLite with connection pooling via sqlx
- Migration system in `migrations.rs` with version tracking
- Modular database operations: `projects.rs`, `tickets.rs`, `workers.rs`, `worker_types.rs`, `comments.rs`, `events.rs`

**Worker Management** (`src/workers/`):
- **Queue System** (`queue.rs`): Thread-safe task queues using DashMap, async channels for worker coordination
- **Process Management** (`process.rs`): Spawns Claude Code worker processes, parses JSON output from workers
- **Consumer Pattern**: Each project/stage combination gets its own consumer thread

**MCP Integration** (`src/mcp/`):
- Tool-based API with trait system in `tools.rs`
- Specialized tool handlers: `project_tools.rs`, `ticket_tools.rs`, `worker_tools.rs`, `worker_type_tools.rs`, `event_tools.rs`
- Type definitions in `types.rs` for MCP protocol compliance

**Real-time Updates** (`src/sse.rs`):
- Server-Sent Events for live progress tracking
- Event publishing system for worker status changes

### Data Flow

1. **Coordinator** (Claude Code instance) creates projects and defines worker types through MCP tools
2. **Tickets** are created and submitted to appropriate project/stage queues
3. **Queue Manager** spawns consumer threads for each unique project/stage combination
4. **Worker Processes** are spawned as headless Claude Code instances with specific system prompts
5. **Output Processing** parses worker JSON responses and updates database state
6. **Event System** publishes real-time updates via SSE for progress monitoring

### Key Patterns

**Database Access**: All database operations use the shared `DbPool` with proper async handling. Database modules provide high-level operations that handle SQLite-specific details.

**Worker Communication**: Workers output JSON in a specific format containing `outcome`, `target_stage`, `comment`, and `reason` fields. The system supports both direct JSON and Claude CLI wrapper formats.

**Error Handling**: Comprehensive error types in `src/error.rs` with proper HTTP status mapping and Axum integration.

**Configuration**: Simple config in `src/config.rs` with helper methods for URL building. Auto-configuration system generates `.mcp.json` and Claude Code settings.

### Worker Lifecycle

1. Task submitted to queue via `submit_task()`
2. Consumer thread picks up task and calls `spawn_worker()`
3. Worker process spawned with project path, system prompt, and ticket context
4. Worker outputs JSON response
5. Output parsed and processed by `output_processor_loop()`
6. Database updated and events published
7. Next stage determined and new task potentially queued

### Database Schema

Projects contain worker types, which define specialized AI workers. Tickets track work items moving through project stages. Workers represent active processes. Events log all system activity. Comments provide detailed progress tracking.

## Important Notes

- The `src/tickets/mod.rs` module is a placeholder - functionality is in `database/tickets.rs` and `mcp/ticket_tools.rs`
- Worker processes require proper Claude Code installation and MCP configuration
- SQLite database and logs are stored in `.vibe-ensemble-mcp/` directory
- Server supports both HTTP MCP and SSE for real-time updates
- Worker output parsing supports both direct JSON and Claude CLI wrapper formats