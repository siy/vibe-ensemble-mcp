# Vibe-Ensemble MCP Server

A Rust-based Model Context Protocol (MCP) server that enables multi-agent coordination through a coordinator-worker architecture.

## What It Does

This MCP server allows a coordinator agent (Claude Code) to manage multiple specialized worker agents through:

- **Project Management**: Create and manage development projects
- **Worker Types**: Define specialized worker roles with custom system prompts  
- **Worker Processes**: Spawn and manage headless Claude Code instances
- **Task Queues**: Distribute work through dedicated worker queues
- **Ticket System**: Track multi-stage workflows with automated progression
- **Event System**: Coordinate between workers through system events

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -fsSL https://get.vibeensemble.dev/install.sh | sh
```

**Windows:**
```powershell
iwr -useb https://get.vibeensemble.dev/install.ps1 | iex
```

### From Release

Download the latest release for your platform from the [releases page](https://github.com/siy/vibe-ensemble-mcp/releases).

### From Source

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

## Usage

### 1. Start the Server

```bash
# Default configuration (port 3000)
./target/release/vibe-ensemble-mcp

# Custom port
./target/release/vibe-ensemble-mcp --port 8080

# Custom database location
./target/release/vibe-ensemble-mcp --database-path ./my-project.db
```

### 2. Configure Claude Code Integration

**Automated Setup (Recommended):**
```bash
# Generate all necessary Claude Code configuration files
./target/release/vibe-ensemble-mcp --configure-claude-code --host 127.0.0.1 --port 3000
```

This command automatically creates:
- `.mcp.json` - MCP server configuration (HTTP + SSE transports)
- `.claude/settings.local.json` - Claude Code permissions
- `.claude/commands/vibe-ensemble.md` - Coordinator initialization command

**Manual Setup (Alternative):**
```bash
claude mcp add --transport http vibe-ensemble http://localhost:3000/mcp
```

### 3. Basic Workflow

1. **Configure integration**: Use `--configure-claude-code` to set up Claude Code
2. **Start the server**: Run `vibe-ensemble-mcp` with your chosen settings  
3. **Open Claude Code**: Launch in the configured directory
4. **Initialize as coordinator**: Run the `vibe-ensemble` command in Claude Code
5. **Create a project**: Define your development project
6. **Define worker types**: Set up specialized workers (architect, developer, tester, etc.)
7. **Create tickets**: Define multi-stage tasks with execution plans
8. **Monitor progress**: Track ticket progression through events and SSE notifications

## Available Tools

The server provides 22 MCP tools organized into categories:

- **Projects** (5 tools): create, list, get, update, delete projects
- **Worker Types** (5 tools): manage worker type definitions and prompts  
- **Workers** (4 tools): spawn, stop, list, and check worker status
- **Queues** (3 tools): list, get status, and delete task queues
- **Tickets** (6 tools): create, get, list, update, comment, and close tickets
- **Events** (2 tools): list and mark events as processed

## Requirements

- Rust 1.70+ (for building from source)
- SQLite (embedded, no separate installation needed)
- Claude Code (for worker processes)

## Configuration

The server accepts the following command-line options:

- `--configure-claude-code`: Generate Claude Code integration files and exit
- `--database-path`: SQLite database file path (default: `./vibe-ensemble.db`)
- `--host`: Server bind address (default: `127.0.0.1`)
- `--port`: Server port (default: `3000`)
- `--log-level`: Log level (default: `info`)

## Architecture

```
Coordinator (Claude Code) → HTTP MCP → Vibe-Ensemble Server → Worker Processes
                                              ↓
                                         SQLite Database
                                              ↓
                                        In-Memory Queues
```

The system maintains separation between high-level coordination and detailed task execution to prevent context drift in long-running workflows.

## API Endpoints

- `GET /health` - Health check with database status
- `POST /mcp` - MCP protocol endpoint (JSON-RPC 2.0)
- `GET /sse` - Server-Sent Events endpoint for real-time notifications

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Setting up the development environment
- Code style and testing requirements
- Submitting pull requests
- Reporting issues

## License

Apache 2.0