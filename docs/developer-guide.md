# Developer Guide

This guide is for developers who want to contribute to Vibe Ensemble or understand how it works under the hood.

## Architecture Overview

Vibe Ensemble is designed as a simple, reliable coordination system with these core principles:

- **Local-First**: Runs entirely on the user's machine
- **SQLite Storage**: Simple, file-based database with no setup required
- **stdio Transport**: Direct integration with Claude Code via MCP protocol
- **Web Interface**: Optional monitoring dashboard

### System Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Claude Code   │    │   Claude Code    │    │   Claude Code   │
│   Agent #1      │    │   Agent #2       │    │   Agent #3      │
└─────────┬───────┘    └─────────┬────────┘    └─────────┬───────┘
          │                      │                       │
          │ MCP/stdio            │ MCP/stdio             │ MCP/stdio
          └──────────────────────┼───────────────────────┘
                                 │
                    ┌────────────▼─────────────┐
                    │     Vibe Ensemble        │
                    │   Coordination Server    │
                    └────────────┬─────────────┘
                                 │
                    ┌────────────▼─────────────┐
                    │      SQLite Database     │
                    │  - Agents               │
                    │  - Issues               │
                    │  - Messages             │
                    │  - Knowledge Base       │
                    └──────────────────────────┘
```

### Project Structure

```
vibe-ensemble-mcp/
├── vibe-ensemble-core/         # Domain models and business logic
├── vibe-ensemble-storage/      # SQLite persistence layer
├── vibe-ensemble-prompts/      # Agent prompt templates
├── vibe-ensemble-mcp/          # MCP protocol implementation
├── vibe-ensemble-web/          # Web dashboard
├── vibe-ensemble-server/       # Main server application
├── agent-templates/            # Filesystem-based prompt templates
└── docs/                       # Documentation
```

## Development Setup

### Prerequisites

- **Rust 1.80+**: Install from [rustup.rs](https://rustup.rs)
- **Git**: For version control
- **SQLite**: Usually included with your OS

### Clone and Build

```bash
# Clone the repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build in development mode
cargo build

# Run tests
cargo test --workspace

# Run with debug logging
RUST_LOG=debug cargo run --bin vibe-ensemble
```

### Development Commands

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Run linter with strict warnings (CI requirement)
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

# Security audit
cargo audit

# Run specific tests
cargo test test_name

# Clean build artifacts
cargo clean
```

### Database Migrations

```bash
# Create a new migration
sqlx migrate add create_new_table

# Run migrations
sqlx migrate run

# Revert last migration
sqlx migrate revert
```

## Core Components

### Agent Management (`vibe-ensemble-core`)

Handles agent registration, lifecycle, and capabilities:

```rust
pub struct Agent {
    pub id: Uuid,
    pub name: String,
    pub capabilities: Vec<String>,
    pub status: AgentStatus,
    pub last_heartbeat: DateTime<Utc>,
}

impl Agent {
    pub fn register(name: String, capabilities: Vec<String>) -> Self {
        // Registration logic
    }
    
    pub fn update_status(&mut self, status: AgentStatus) {
        // Status update logic
    }
}
```

### Issue Tracking

Persistent task and problem management:

```rust
pub struct Issue {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: IssueStatus,
    pub assigned_to: Option<Uuid>,
    pub priority: Priority,
    pub created_at: DateTime<Utc>,
}
```

### MCP Protocol (`vibe-ensemble-mcp`)

Implements the Model Context Protocol for Claude Code integration:

```rust
#[async_trait]
impl McpServer for VibeEnsembleServer {
    async fn list_tools(&self) -> Result<Vec<Tool>, McpError> {
        // Return available coordination tools
    }
    
    async fn call_tool(&self, request: ToolCall) -> Result<ToolResult, McpError> {
        match request.name.as_str() {
            "vibe/agent/register" => self.register_agent(request).await,
            "vibe/issue/create" => self.create_issue(request).await,
            // ... other tools
        }
    }
}
```

### Storage Layer (`vibe-ensemble-storage`)

SQLx-based persistence with migrations:

```rust
pub struct DatabaseManager {
    pool: SqlitePool,
}

impl DatabaseManager {
    pub async fn create_agent(&self, agent: &Agent) -> Result<(), Error> {
        sqlx::query!(
            "INSERT INTO agents (id, name, capabilities, status) VALUES (?, ?, ?, ?)",
            agent.id, agent.name, agent.capabilities, agent.status
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}
```

## Testing

### Unit Tests

Each component has comprehensive unit tests:

```bash
# Run all tests
cargo test --workspace

# Run tests for specific component
cargo test --package vibe-ensemble-core

# Run with output
cargo test -- --nocapture
```

### Manual Testing

```bash
# Start server in development mode
RUST_LOG=debug cargo run --bin vibe-ensemble

# Test MCP tools directly
echo '{"jsonrpc":"2.0","id":1,"method":"vibe/agent/list","params":{}}' | \
  vibe-ensemble --mcp-only --transport=stdio
```

## Agent Prompt System Architecture

Vibe Ensemble provides a sophisticated dual-layer prompt system that combines built-in defaults with filesystem-based customization.

### System Overview

The prompt system has **two layers** that work together:

1. **Built-in Templates** (Compiled into Binary)
2. **Filesystem Templates** (Runtime Customizable)

### Layer 1: Built-in Templates

**Location**: `vibe-ensemble-prompts/src/templates.rs`

Built-in templates are compiled directly into the binary as string constants:

```rust
pub const COORDINATOR_TEMPLATE: &str = r#"
You are {{agent_name}}, a Claude Code Team Coordinator...
"#;

pub const WORKER_TEMPLATE: &str = r#"
You are {{agent_name}}, a specialized Claude Code Worker Agent...
"#;
```

**Benefits**:
- ✅ Always available (no external dependencies)  
- ✅ Fast loading (no filesystem I/O)
- ✅ Version controlled with codebase
- ✅ Immediate effect after `cargo build`

### Layer 2: Filesystem Templates

**Location**: `agent-templates/` directory structure

```
agent-templates/
├── coordinator/
│   ├── template.json          # Metadata and variables
│   ├── agent-config.md        # Agent configuration template
│   └── prompts/
│       ├── system.md          # System prompt override
│       ├── instructions.md    # Additional instructions
│       └── examples.md        # Usage examples
└── worker/
    ├── template.json
    ├── agent-config.md
    └── prompts/
        └── system.md
```

**Benefits**:
- ✅ Runtime customizable (no recompilation needed)
- ✅ Hot-swappable templates
- ✅ Environment-specific customization
- ✅ User-specific prompt modifications

### Template Priority & Override System

1. **Default**: Built-in templates are used initially
2. **Override**: Filesystem templates override defaults when present
3. **Fallback**: If filesystem template fails to load, falls back to built-in

### Agent Registration Requirements

Both template layers include comprehensive registration specifications to ensure first-attempt success:

#### Coordinator Registration Template:
```json
{
  "name": "coordinator-agent",
  "agentType": "Coordinator",
  "capabilities": [
    "cross_project_coordination",
    "dependency_management", 
    "conflict_resolution",
    "resource_allocation",
    "workflow_orchestration"
  ],
  "connectionMetadata": {
    "endpoint": "system://claude-code",
    "protocol_version": "1.0",
    "transport": "stdio",
    "capabilities": "full_coordination",
    "session_type": "coordinator_primary"
  }
}
```

#### Worker Registration Template:
```json
{
  "name": "worker-agent-{{specialization}}",
  "agentType": "Worker", 
  "capabilities": [
    "code_implementation",
    "testing",
    "debugging",
    "refactoring"
  ],
  "connectionMetadata": {
    "endpoint": "system://claude-code",
    "protocol_version": "1.0",
    "specialization": "{{specialization}}",
    "coordinator_managed": true
  }
}
```

### Template Management

The `PromptManager` in `vibe-ensemble-prompts` handles:

- Loading built-in templates at startup
- Scanning filesystem for override templates  
- Template rendering with Handlebars
- Caching and hot-reload capabilities
- Validation and error handling

### Development Workflow

#### For Built-in Template Changes:
```bash
# 1. Edit templates.rs
vim vibe-ensemble-prompts/src/templates.rs

# 2. Rebuild binary  
cargo build --release

# 3. Templates are immediately active
./target/release/vibe-ensemble
```

#### For Filesystem Template Development:
```bash
# 1. Create/edit filesystem templates
vim agent-templates/coordinator/prompts/system.md

# 2. No rebuild needed - changes are live
# Templates reload automatically or on server restart
```

### Testing Templates

```bash
# Test built-in template loading
cargo test --package vibe-ensemble-prompts test_builtin_templates

# Test filesystem template loading
cargo test --package vibe-ensemble-prompts test_filesystem_templates

# Test template rendering with variables
cargo test --package vibe-ensemble-prompts test_template_rendering
```

### Troubleshooting

**Common Issues:**
1. **Registration failures**: Ensure all `connectionMetadata` fields are present
2. **Template not found**: Check filesystem permissions and file paths
3. **Invalid template syntax**: Validate Handlebars template syntax
4. **Variable substitution errors**: Verify all required template variables are provided

## Adding New Features

### Adding a New MCP Tool

1. **Define the tool** in `vibe-ensemble-mcp/src/tools/`:

```rust
pub async fn my_new_tool(params: Value) -> Result<ToolResult, McpError> {
    // Tool implementation
    Ok(ToolResult::success("Tool completed"))
}
```

2. **Register the tool** in the server:

```rust
async fn list_tools(&self) -> Result<Vec<Tool>, McpError> {
    vec![
        // ... existing tools
        Tool {
            name: "vibe/my/new_tool".to_string(),
            description: "Description of what this tool does".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param1": {"type": "string"}
                },
                "required": ["param1"]
            }),
        }
    ]
}
```

3. **Add routing** in `call_tool`:

```rust
async fn call_tool(&self, request: ToolCall) -> Result<ToolResult, McpError> {
    match request.name.as_str() {
        // ... existing tools
        "vibe/my/new_tool" => my_new_tool(request.arguments).await,
    }
}
```

4. **Write tests**:

```rust
#[tokio::test]
async fn test_my_new_tool() {
    let server = create_test_server().await;
    let result = server.call_tool(ToolCall {
        name: "vibe/my/new_tool".to_string(),
        arguments: json!({"param1": "test"}),
    }).await;
    
    assert!(result.is_ok());
}
```

### Adding Database Tables

1. **Create migration**:

```bash
sqlx migrate add create_my_table
```

2. **Write SQL** in the new migration file:

```sql
CREATE TABLE my_table (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

3. **Add Rust model**:

```rust
#[derive(Debug, Clone)]
pub struct MyModel {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}
```

4. **Add database methods**:

```rust
impl DatabaseManager {
    pub async fn create_my_model(&self, model: &MyModel) -> Result<(), Error> {
        // Implementation
    }
}
```

## Code Standards

### Rust Guidelines

- **Follow Rust idioms**: Use ownership, borrowing, and lifetimes effectively
- **Error handling**: Use `Result` types, avoid panics
- **Async/await**: Use for I/O operations and database access
- **Type safety**: Leverage the type system for correctness

### Code Quality

- **Tests required**: All new functionality must have tests
- **Documentation**: Public APIs must be documented
- **Clippy clean**: No warnings allowed in CI
- **Formatted**: Use `cargo fmt` before committing

### Git Workflow

- **Single-line commits**: Keep commit messages concise
- **Feature branches**: Create branches for new features
- **Pull requests**: All changes go through PR review
- **Quality gates**: CI must pass before merging

## Debugging

### Logging

Use structured logging with tracing:

```rust
use tracing::{info, warn, error, debug};

pub async fn some_function() {
    debug!("Starting function with params: {:?}", params);
    
    match do_something().await {
        Ok(result) => info!("Function completed successfully"),
        Err(e) => error!("Function failed: {}", e),
    }
}
```

Enable debug logging:

```bash
RUST_LOG=debug cargo run --bin vibe-ensemble
```

### Database Debugging

```bash
# Open database directly
sqlite3 ./.vibe-ensemble/data.db

# View tables
.tables

# Query agents
SELECT * FROM agents;
```

### MCP Protocol Debugging

```bash
# Test MCP tools directly
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | \
  RUST_LOG=debug vibe-ensemble --mcp-only --transport=stdio
```

## Performance

### Optimization Guidelines

- **Database**: Use indexes for frequently queried columns
- **Memory**: Prefer streaming over loading all data
- **Async**: Don't block async tasks unnecessarily
- **Connections**: Pool database connections appropriately

### Monitoring

The web dashboard provides:
- **System metrics**: CPU, memory, database size
- **Performance**: Request timing and slow queries  
- **Health**: Component status and error rates

## Contributing

### Before Starting

1. **Read the code**: Understand existing patterns
2. **Check issues**: Look for good first issues
3. **Discuss changes**: Create an issue for major features

### Development Process

1. **Fork the repository**
2. **Create feature branch**: `git checkout -b feature/my-feature`
3. **Make changes** with tests and documentation
4. **Run quality checks**:
   ```bash
   cargo test --workspace
   cargo fmt
   RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features
   ```
5. **Submit pull request**

### Pull Request Requirements

- **All tests pass**: CI must be green
- **Code coverage**: New code should be tested
- **Documentation**: Update docs if needed
- **Single responsibility**: One feature per PR
- **Clear description**: Explain what and why

## Release Process

### Version Management

- **Semantic versioning**: MAJOR.MINOR.PATCH
- **Changelog**: Update for each release
- **Git tags**: Tag releases appropriately

### Release Checklist

1. **Update version** in `Cargo.toml` files
2. **Update CHANGELOG.md** with new features and fixes
3. **Run full test suite**: `cargo test --workspace`
4. **Build release**: `cargo build --release`
5. **Create git tag**: `git tag -a v0.4.2 -m "Release v0.4.2"`
6. **Push changes**: `git push origin main --tags`
7. **GitHub release**: CI will create release automatically

## Getting Help

For development questions:

1. **Check existing code** for similar patterns
2. **Read tests** to understand expected behavior  
3. **Search issues** for related discussions
4. **Ask in discussions** for design questions
5. **Create issue** for bugs or feature requests

## Future Development

Potential areas for contribution:

- **Enhanced web dashboard** with real-time updates
- **Plugin system** for custom coordination tools
- **Multi-instance coordination** for team environments
- **Performance optimizations** for large agent networks
- **Additional integrations** with development tools

The architecture is designed to be extensible while maintaining simplicity and reliability.