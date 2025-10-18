# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

```bash
# Build and run the server
cargo build
cargo run -- --port 3276

# Development with debug logging
cargo run -- --port 3276 --log-level debug

# Generate Claude Code configuration
cargo run -- --configure-claude-code --host 127.0.0.1 --port 3276

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
- Key coordinator tools: `resume_ticket_processing` for restarting stalled or on-hold tickets

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

**Worker Communication**: Workers output JSON in a specific format containing `outcome`, `comment`, and `reason` fields. The system uses the pipeline to automatically determine target stages based on outcome. The system supports both direct JSON and Claude CLI wrapper formats.

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

## Repository Information

- **Correct Repository URL**: `https://github.com/siy/vibe-ensemble-mcp`
- **GitHub Owner**: `siy`
- **Repository Name**: `vibe-ensemble-mcp`

## Important Notes

- The `src/tickets/mod.rs` module is a placeholder - functionality is in `database/tickets.rs` and `mcp/ticket_tools.rs`
- Worker processes require proper Claude Code installation and MCP configuration
- SQLite database and logs are stored in `.vibe-ensemble-mcp/` directory
- Server supports both HTTP MCP and SSE for real-time updates
- Worker output parsing supports both direct JSON and Claude CLI wrapper formats
- Error handling policy: ALWAYS release claims or explicitly place tickets on-hold with clear operator instructions

## Git Workflow and Commit Standards

### MANDATORY Commit Message Format
**CRITICAL: These rules are ABSOLUTE and MUST be followed without exception:**

1. **Single-line conventional commits format ONLY**
   - Format: `type: description` (lowercase type, lowercase description)
   - Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `perf`, `style`
   - Maximum 50 characters for entire message
   - Examples:
     - `feat: add model parameter to worker spawn`
     - `fix: resolve race condition in consumer`
     - `chore: bump version to 0.9.9`

2. **ATTRIBUTION IS ABSOLUTELY PROHIBITED**
   - ❌ NO `Co-authored-by:` lines
   - ❌ NO `Signed-off-by:` lines
   - ❌ NO author credits or signatures of any kind
   - ❌ NO multi-line commit messages with attribution footers
   - ✅ ONLY single-line conventional commit messages

3. **Pre-Commit Quality Checks (MANDATORY)**
   Before EVERY commit, you MUST:
   - Run `cargo fmt` - ensure code is formatted
   - Run `cargo check` - ensure code compiles without errors
   - Run `cargo clippy` - ensure no clippy warnings
   - Run `cargo test` - ensure all tests pass (when applicable)
   - Repeat until all checks pass with zero warnings

### Code Submission Routine

**When asked to commit and/or push changes:**

1. **Quality checks** (repeat until all pass):
   ```bash
   cargo fmt
   cargo check
   cargo clippy  # Must return zero warnings
   cargo test    # If tests exist
   ```

2. **Commit with proper format**:
   ```bash
   git add -A
   git commit -m "type: description"  # Single line, no attribution
   ```

3. **Push to appropriate branch**:
   - If on `main` with no active PR: Create new branch and PR
   - If active PR exists: Push to existing PR branch
   - Never create unnecessary branches

### PR Review Workflow

**When PR is created or under review:**
- Address ALL review comments (including minor, nitpick, optional)
- Ensure CI is green
- Cycle: review → fix → push → repeat until CodeRabbit approves
- Remain on same PR - do NOT create new branches
- Use single-line conventional commits (no attribution)
- When PR is merged: Switch to main and pull changes immediately

### Release Branch Workflow

**"Opening release branch" means:**
1. Check current branch - if not main, switch to main
2. Pull latest changes: `git pull origin main`
3. Create release branch: `git checkout -b release/vX.Y.Z`
4. Assume patch version bump unless specified
5. Bump version in all relevant files
6. First commit: `chore: bump version to X.Y.Z`
7. All subsequent commits follow standard format (no attribution)