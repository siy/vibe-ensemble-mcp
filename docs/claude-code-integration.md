# Claude Code Integration Guide

This document provides comprehensive information about integrating Vibe Ensemble with Claude Code based on analysis of the Claude Code repository and documentation.

## Key Finding: No Network Auto-Discovery

**IMPORTANT**: Claude Code does **NOT** implement port scanning or automatic network discovery for MCP servers. The "auto-discovery" refers to UI integration of already-configured servers, not network discovery.

## How Claude Code Actually Works

### Auto-Discovery Types
1. **File-based discovery**: Detects `.mcp.json` files in project root directories
2. **Resource discovery**: Exposes MCP server resources via `@` mentions in the UI
3. **Prompt discovery**: Makes MCP server prompts available as slash commands (`/mcp__servername__promptname`)

### No Port Scanning
- No automatic network scanning for endpoints like `/ws`
- No port range scanning (e.g., 3000-4000, 22360-22362)
- Connection establishment requires manual server registration
- All connections must be explicitly configured

## MCP Server Configuration

### Configuration File Format (`.mcp.json`)
Located at project root directory:

```json
{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "vibe-ensemble",
      "args": ["--mcp-only"],
      "env": {
        "RUST_LOG": "vibe_ensemble=info"
      }
    },
    "vibe-ensemble-http": {
      "transport": "http",
      "url": "http://127.0.0.1:22360/mcp",
      "headers": {
        "Content-Type": "application/json"
      }
    },
    "vibe-ensemble-sse": {
      "transport": "sse", 
      "url": "http://127.0.0.1:22360/events",
      "headers": {
        "X-API-Key": "${VIBE_API_KEY:-default}"
      }
    }
  }
}
```

### Configuration Scopes
1. **Local scope** (default): Project-specific user settings
2. **Project scope**: Shared team configuration via `.mcp.json` 
3. **User scope**: Global user configuration

### Connection Types
- **stdio**: Local process communication (recommended for development)
- **SSE**: Server-Sent Events for remote servers
- **HTTP**: Standard HTTP-based communication
- **WebSocket**: Not directly supported by Claude Code

## Claude Code Command Line Interface

### Core Commands
```bash
# Basic usage
claude                           # Start interactive session
claude "query"                  # Start with initial prompt  
claude -p "query"               # Print mode (non-interactive)
claude -c                       # Continue recent conversation
claude -r <session-id>          # Resume specific session

# Directory and tool control
claude --add-dir ../frontend --add-dir ../backend
claude --allowedTools "Bash(git:*)" "Write" "Read"
claude --disallowedTools "Bash(rm:*)" "Bash(sudo:*)"

# Model and output control
claude --model sonnet
claude --model opus  
claude --output-format json
claude --max-turns 5

# Permission control
claude --permission-mode ask
claude --dangerously-skip-permissions

# MCP-specific
claude --mcp-config file1.json file2.json
claude --mcp-debug
```

### MCP Management Commands
```bash
# Add MCP servers
claude mcp add <server-name>
claude mcp add-json <server-name> <json-config>
claude mcp add-from-claude-desktop

# Manage servers  
claude mcp list
claude mcp remove <server-name>
claude mcp reset-project-choices

# Interactive configuration
claude mcp
```

### Environment Variables
```bash
MCP_TIMEOUT=30000               # Server startup timeout (ms)
MCP_TOOL_TIMEOUT=10000          # Tool execution timeout (ms)
MAX_MCP_OUTPUT_TOKENS=10000     # Output limit
CLAUDE_CONFIG_DIR=/custom/path  # Custom config directory
```

## Worker Spawning and Multi-Agent Coordination

### Parallel Processing Methods

#### 1. Multiple Claude Instances
```bash
# Terminal 1
cd /project
claude "work on authentication system"

# Terminal 2  
cd /project-worktree
claude "build data visualization"
```

#### 2. Git Worktrees for Parallel Work
```bash
# Create separate worktrees
git worktree add ../project-auth -b feature-auth
git worktree add ../project-viz -b feature-viz

# Run Claude in each
cd ../project-auth && claude "implement auth"
cd ../project-viz && claude "build dashboard"
```

#### 3. Subagents via Task Tool
Claude Code can spawn multiple subagents simultaneously:
```
"Launch 4 parallel tasks to explore the codebase"
```

### No Direct Worker API
Claude Code doesn't expose a direct worker spawning API. Multi-agent coordination requires:
- Multiple process instances connecting to shared MCP server
- Git worktrees for parallel development
- Internal subagent system coordination

## System Prompts and Customization

### CLAUDE.md Files
Project instruction files provide context:
```markdown
# CLAUDE.md
This project uses Rust with SQLx for database operations.

## Development Commands
cargo build
cargo test --workspace
RUSTFLAGS="-D warnings" cargo clippy --all-targets --all-features

## MCP Integration
Start vibe-ensemble server: cargo run --bin vibe-ensemble --mcp-only
```

### Subagent System
Custom specialized agents with dedicated system prompts.

#### File Structure
```
.claude/agents/coordinator.md     # Project-level coordinator
.claude/agents/code-reviewer.md   # Code review specialist
~/.claude/agents/debugger.md      # User-level debugger
```

#### Subagent Format
```markdown
---
name: coordinator
description: Multi-agent task coordination specialist
tools: Read, Write, Bash, vibe/agent/register, vibe/task/create
model: sonnet
---

You are a coordinator agent for multi-agent development workflows.
Your role is to:
- Break down complex tasks into subtasks
- Assign work to specialist agents
- Monitor progress and resolve conflicts
- Coordinate deliverables and integration

Always use vibe-ensemble MCP tools for agent coordination.
```

## Integration Strategy for Vibe Ensemble

### Current Status Analysis
Based on the user's error message:
```
Found 4 other running IDE(s). However, their workspace/project directories do not match the current cwd.
```

This indicates:
1. **Claude Code detected something** - likely our HTTP server on one of the fallback ports
2. **Workspace mismatch** - Claude Code expects the server to be associated with the current working directory
3. **Not an MCP server** - Claude Code didn't recognize it as a proper MCP server

### Why Auto-Discovery Isn't Working

1. **Wrong Detection Method**: Claude Code isn't scanning for `/ws` endpoints
2. **No MCP Protocol**: Our HTTP server doesn't implement MCP protocol endpoints
3. **Missing Workspace Association**: No mechanism to associate server with project directory
4. **Wrong Transport**: Claude Code expects stdio/SSE/HTTP MCP, not WebSocket upgrade

## Recommended Solutions

### Option 1: Proper MCP HTTP Server (Recommended)
Implement actual MCP protocol over HTTP instead of WebSocket upgrade:

```rust
// Add HTTP MCP endpoints
app.route("/mcp", post(handle_mcp_request))
   .route("/events", get(handle_sse_connection))  // For SSE transport
   .route("/ws", get(handle_websocket_upgrade))   // Keep for direct WebSocket clients
```

### Option 2: Configuration-First Approach
Focus on making configuration extremely simple:

```bash
# Auto-generate .mcp.json in current directory
vibe-ensemble --generate-config

# Or integrate with Claude Code directly
vibe-ensemble --register-with-claude
```

### Option 3: Multi-Transport Server
Support all transport types Claude Code uses:

```rust
pub enum TransportType {
    Stdio,      // For local development
    Http,       // For HTTP-based MCP
    Sse,        // For server-sent events  
    WebSocket,  // For direct WebSocket clients
}
```

### Option 4: Stdio Bridge
Create a bridge process that Claude Code can spawn:

```bash
# Claude Code spawns this
vibe-ensemble-bridge --connect ws://127.0.0.1:22360/ws

# Bridge translates stdio <-> WebSocket
```

## Implementation Priority

1. **Immediate**: Fix workspace directory association issue
2. **Short-term**: Implement HTTP MCP endpoints alongside WebSocket
3. **Medium-term**: Add SSE transport for better Claude Code integration
4. **Long-term**: Create configuration management tools

## Testing Integration

### Manual Testing
```bash
# Test MCP configuration
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | vibe-ensemble --mcp-only --transport=stdio

# Test with Claude Code
cd /your/project
claude mcp add vibe-ensemble
claude "register as coordinator agent"
```

### Automated Testing
```bash
# Test all transport types
cargo test test_stdio_transport
cargo test test_http_transport  
cargo test test_sse_transport
cargo test test_websocket_transport
```

This integration guide provides the foundation for properly connecting Vibe Ensemble with Claude Code's actual architecture and capabilities.