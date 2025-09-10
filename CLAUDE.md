# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust project implementing an MCP (Model Context Protocol) server for team coordination between multiple Claude Code instances. The system enables distributed task execution with unified management, communication, and issue tracking capabilities.

## Development Commands

This project uses standard Rust toolchain commands:

```bash
# Build the project (active workspace only)
cargo build

# Run tests (378 tests passing)
cargo test --workspace

# Run specific test
cargo test <test_name>

# Format code
cargo fmt

# Run clippy linter
cargo clippy

# Run clippy with strict warnings (CI requirement)
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Check compilation without building
cargo check

# Security audit (CI requirement)
cargo audit

# Clean build artifacts
cargo clean

# Start the unified server (default: API + Web Dashboard + MCP WebSocket + SSE)
cargo run --bin vibe-ensemble

# Start in MCP-only mode with WebSocket transport (for multi-agent coordination)
cargo run --bin vibe-ensemble -- --mcp-only

# Start web dashboard only
cargo run --bin vibe-ensemble -- --web-only

# Start API server only (no web interface)
cargo run --bin vibe-ensemble -- --api-only

# Start with custom database
DATABASE_URL="sqlite:./custom.db" cargo run --bin vibe-ensemble

# Start with custom ports
cargo run --bin vibe-ensemble -- --port=9000 --web-port=9001
```

## Production Workspace Structure

The workspace includes all production-ready components:

```
vibe-ensemble-mcp/
├── vibe-ensemble-core/       # ✅ Domain models and business logic
├── vibe-ensemble-storage/    # ✅ SQLx persistence layer  
├── vibe-ensemble-prompts/    # ✅ Prompt management system
├── vibe-ensemble-mcp/        # ✅ MCP protocol implementation
├── vibe-ensemble-web/        # ✅ Web dashboard with monitoring
└── vibe-ensemble-server/     # ✅ Main server application
```

**Additional crates** (available but not in active workspace):
- `vibe-ensemble-security` - Security components (integrated into server/web)
- `vibe-ensemble-monitoring` - Advanced observability features (integrated into web dashboard)

## Architecture Overview

The system is designed around five core subsystems:

1. **Agent Management System** - Handles registration, lifecycle, and capability tracking for connected Claude Code agents
2. **Issue Tracking System** - Provides persistent storage and workflow management for tasks requiring resolution
3. **Messaging System** - Enables real-time communication between agents using standardized protocols
4. **Knowledge Management System** - Collects, organizes, and distributes development patterns and practices
5. **Persistence Layer** - Ensures data consistency and recovery across all subsystems

### Current Implementation Status

**✅ PRODUCTION READY (v0.4.2):**
- `vibe-ensemble-core` - Complete domain models, business logic, and orchestration (184 tests)
- `vibe-ensemble-storage` - Full persistence layer with SQLx, migrations, and health monitoring (110 tests)
- `vibe-ensemble-prompts` - Intelligent coordination prompts with specialist templates (15 tests)
- `vibe-ensemble-mcp` - Complete MCP protocol server with dual-transport coordination (39 tests)
- `vibe-ensemble-web` - Production web dashboard with real-time monitoring and comprehensive API (13 tests)
- Comprehensive integration testing and quality assurance (17 tests)

**🛡️ PRODUCTION FEATURES:**
- **Security**: Configuration validation, security headers, safe defaults for small teams
- **Monitoring**: Real-time system metrics (CPU, memory, disk), performance tracking, health checks
- **Documentation**: Comprehensive installation guides, security best practices, troubleshooting
- **Cross-Platform**: Automated releases for Mac/Linux/Windows with one-line installers
- **User Experience**: Friendly setup for both newcomers and experienced developers

The project is now **production-ready** with 378 passing tests, comprehensive security hardening, and complete documentation. The system provides powerful multi-agent coordination with dual-transport architecture (WebSocket + SSE), automated permission approval workflows, and simple deployment suitable for small teams and individual developers. Start with `cargo run --bin vibe-ensemble` for the full experience or `cargo run --bin vibe-ensemble -- --mcp-only` for WebSocket MCP-only mode.

### Agent Hierarchy & Coordination Intelligence

- **Coordinator Agent** - Enhanced with intelligent coordination prompts featuring dependency detection, conflict resolution protocols, and automated escalation management
- **Worker Agents** - Execute tasks with coordination awareness, proactive dependency detection, and intelligent escalation protocols
- **Specialist Coordinators** - Cross-project coordinators, conflict resolvers, and escalation managers for complex multi-agent scenarios

The system features knowledge-driven coordination with pattern recognition, automated conflict detection, intelligent escalation decision trees, and cross-project dependency orchestration. All agents use MCP tools for seamless coordination: `vibe/dependency/analyze`, `vibe/conflict/detect`, `vibe/agent/message`, `vibe/coordination/escalate`.

## Knowledge Management & Coordination Intelligence

The system includes a comprehensive knowledge repository enhanced with coordination intelligence:
- **Pattern Recognition** - Automatically detects and suggests proven coordination strategies
- **Development Standards** - Enforces organizational guidelines via `vibe/guideline/enforce`
- **Conflict History** - Learns from past conflicts to prevent recurring issues
- **Cross-Project Learning** - Shares coordination patterns across project boundaries
- **Intelligent Escalation** - Data-driven escalation decisions based on historical outcomes
- **Proactive Monitoring** - Continuous scanning for dependency violations and conflicts

## Configuration

The project includes Claude Code-specific configuration in `.claude/settings.local.json` with pre-approved permissions for common Rust development commands and git operations.

## GitHub Integration

This project leverages the GitHub MCP server for streamlined development workflows. Always use the GitHub MCP tools for:

### CI/CD Monitoring
- **Check workflow status**: Use `mcp__github__list_workflow_runs` to monitor CI progress
- **Debug failures**: Use `mcp__github__get_job_logs` with `failed_only=true` for efficient debugging
- **View specific runs**: Use `mcp__github__get_workflow_run` for detailed run information

### Issue and PR Management  
- **Track issues**: Use `mcp__github__list_issues` and `mcp__github__get_issue` for issue management
- **Monitor PRs**: Use `mcp__github__list_pull_requests` and `mcp__github__get_pull_request` for code review
- **Check PR status**: Use `mcp__github__get_pull_request_status` before merging

### Security and Quality Assurance
- **Security alerts**: Use `mcp__github__list_dependabot_alerts` to monitor dependency vulnerabilities
- **Code scanning**: Use `mcp__github__list_code_scanning_alerts` for security analysis
- **Release management**: Use `mcp__github__list_releases` and `mcp__github__get_latest_release` for version tracking

### Best Practices
- **Proactive monitoring**: Check CI status after pushing changes
- **Efficient debugging**: Use failed_only logs instead of full workflow logs
- **Security awareness**: Regularly check for security alerts and dependency issues
- **Release readiness**: Verify all checks pass before creating releases

Example workflow:
```bash
# After pushing changes, check CI status
mcp__github__list_workflow_runs owner=siy repo=vibe-ensemble-mcp workflow_id=ci.yml

# If CI fails, get failure details  
mcp__github__get_job_logs owner=siy repo=vibe-ensemble-mcp run_id=<run_id> failed_only=true

# Check for security issues
mcp__github__list_dependabot_alerts owner=siy repo=vibe-ensemble-mcp
```

## Dual Transport (WebSocket + SSE) for Multi-Agent Coordination

The dual-transport architecture provides optimal multi-agent coordination capabilities:

### Key Features
- **JSON-RPC 2.0 Compliance**: Strict validation of message format and protocol version over WebSocket frames
- **Multi-Agent Support**: Concurrent connections for multiple Claude Code instances
- **SSE Message Delivery**: Server-Sent Events for reliable coordinator-worker communication
- **Auto-Approval Workflow**: Seamless coordinator approval of worker permissions
- **Connection Lifecycle**: Proper WebSocket upgrade protocol and connection management
- **MCP Protocol State Tracking**: Initialization sequence management and protocol compliance
- **Performance Optimization**: Efficient message handling with configurable timeouts
- **Robust Error Handling**: WebSocket frame handling, reconnection support, and graceful degradation
- **Resource Management**: Proper WebSocket close frames and connection cleanup

### Configuration Options

```rust
// Default WebSocket transport
let transport = TransportFactory::websocket(websocket);

// Custom configuration for specific requirements
let transport = TransportFactory::websocket_with_config(
    websocket,
    Duration::from_secs(30),    // Read timeout
    Duration::from_secs(10),    // Write timeout  
    Some(remote_addr),          // Remote address for logging
);
```

### Performance Characteristics
- **Connection Handling**: Multiple concurrent WebSocket connections
- **Read Timeout**: 30 seconds (prevents indefinite blocking)
- **Write Timeout**: 10 seconds (ensures responsive communication)
- **WebSocket Frames**: Efficient binary and text frame handling
- **Message Validation**: JSON-RPC 2.0 compliance validation

### Multi-Agent Coordination
The dual-transport system enables advanced multi-agent scenarios:
- Supports all MCP protocol methods (initialize, tools/list, tools/call, resources/list, etc.)
- Handles multiple concurrent agent connections with SSE updates
- Provides proper error responses with JSON-RPC 2.0 error format
- Supports coordination tools for agent communication
- Enables distributed task execution and collaboration
- Automated permission approval workflow between coordinators and workers

### Usage Examples

```bash
# Dual-transport MCP server for multi-agent coordination (WebSocket + SSE)
cargo run --bin vibe-ensemble -- --mcp-only

# With custom database for persistent state
DATABASE_URL="sqlite:./multi-agent.db" cargo run --bin vibe-ensemble

# Debug mode with enhanced logging
RUST_LOG=vibe_ensemble_mcp=debug cargo run --bin vibe-ensemble -- --mcp-only
```

## Development Focus

When working on this codebase:
- Follow idiomatic Rust patterns with ownership, borrowing, and lifetimes
- Use the type system for correctness and zero-cost abstractions
- Implement explicit error handling with Result types
- Leverage async/await for concurrent operations
- Include comprehensive tests and documentation
- **Special Attention**: Keep attribution in only one place - README.md. Attribution in commits or PR's is strictly prohibited.
- **Special Attention**: Strictly follow single line convenient commits convention for commit messages.

## Task Implementation Protocol

**MANDATORY: Always use ticket-implementer agent for ALL implementation tasks**, regardless of task source:

### Task Sources Requiring ticket-implementer:
- **GitHub Issues**: Any issue from the repository issue tracker
- **Direct User Tasks**: Tasks provided directly by the user in conversation
- **Follow-up Work**: PR comments, CI fixes, refactoring requests
- **Feature Requests**: New functionality or enhancements
- **Bug Reports**: Any debugging or fixing work
- **Code Reviews**: Implementing review feedback

### ticket-implementer Usage:
```markdown
Use ticket-implementer for:
- Initial task analysis and planning
- Implementation with fresh start protocol
- Quality assurance and testing
- PR creation and management
- Review comment resolution
- CI issue debugging and fixes
```

### Quality Assurance Requirements (MANDATORY):
**CRITICAL**: Before each and every commit, ticket-implementer MUST verify:
1. **Tests Pass**: `cargo test --workspace` - All tests must pass without failures
2. **Clippy Clean**: `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features` - No warnings or errors allowed
3. **Formatting Applied**: `cargo fmt` - Code must be properly formatted
4. **Build Success**: `cargo build` - Project must compile without errors

**Zero Tolerance Policy**: No commit is allowed if any of these checks fail. The ticket-implementer must fix all issues before proceeding.

### Fresh Start Protocol (Mandatory):
Every task MUST begin with:
1. `git checkout main`
2. `git pull origin main` 
3. Create new feature branch
4. Systematic implementation following project standards

### ticket-implementer Instructions (ALWAYS INCLUDE):
When using ticket-implementer, ALWAYS include this mandatory instruction in the prompt:

**"CRITICAL REQUIREMENT: Before each commit, you MUST run and verify ALL of the following pass without any errors or warnings:
- `cargo test --workspace` (all tests must pass)
- `RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features` (no warnings allowed)
- `cargo fmt` (formatting must be applied)
- `cargo build` (must compile successfully)

No commit is permitted if any of these checks fail. Fix all issues before committing."**

**No Exceptions**: ticket-implementer handles ALL coding tasks to ensure consistency, quality, and proper workflow adherence across the entire development process.

## Git Worktree Workflow

This project supports parallel development using git worktrees for multiple worker agents:

### Worker Agent Coordination
- **Workers can work on multiple projects in parallel** using separate worktrees
- **Workers on the same project use git worktrees** to avoid conflicts
- Use descriptive worktree names: `../project-agent-feature-name`

### Common Worktree Commands
```bash
# Create worktree for new feature
git worktree add -b feature-name ../vibe-ensemble-feature

# List active worktrees
git worktree list

# Clean up completed work
git worktree remove ../vibe-ensemble-feature
```

### Best Practices
- **One primary worktree per active development task**
- **Temporary worktrees for experimentation**
- **Regular cleanup** of completed worktrees
- **Descriptive naming** to indicate purpose and agent

See [docs/git-worktrees.md](docs/git-worktrees.md) for comprehensive worktree usage patterns.
