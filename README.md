# Vibe Ensemble

A local team coordination system for Claude Code that helps multiple AI agents work together on your projects without conflicts.

## What is Vibe Ensemble?

Vibe Ensemble is a simple coordination server that runs locally on your computer. It helps when you're using multiple Claude Code instances to work on different parts of your projects - preventing conflicts, sharing knowledge, and keeping everyone organized.

Think of it as a local "mission control" for your AI development team.

## Key Features

- **Conflict Prevention**: Agents check with each other before making changes
- **Knowledge Sharing**: Insights and patterns are shared across all agents  
- **Issue Tracking**: Keep track of tasks, bugs, and coordination needs
- **Web Dashboard**: Monitor agent activity and system health at a glance
- **Zero Configuration**: Works out of the box with smart defaults

## Quick Start

### 1. Install

**macOS/Linux:**
```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash
```

**Windows:**
```bash
iwr https://vibeensemble.dev/install.ps1 -UseBasicParsing | iex
```

### 2. Start the Server

```bash
vibe-ensemble
```

This starts:
- Local coordination server
- Web dashboard at http://127.0.0.1:8080
- SQLite database in `~/.vibe-ensemble/`

### 3. Connect Claude Code

Add this MCP server to Claude Code:

```json
{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "vibe-ensemble --mcp-only --transport=stdio",
      "args": []
    }
  }
}
```

That's it! Your Claude Code instances can now coordinate with each other.

## How It Works

When you have multiple Claude Code instances working:

1. **Agent Registration**: Each Claude Code instance registers as an agent
2. **Conflict Detection**: Before making changes, agents check for conflicts
3. **Knowledge Sharing**: Agents share discoveries and patterns
4. **Issue Coordination**: Track and assign tasks across agents

## Use Cases

**Single Developer with Multiple Agents:**
- One agent working on frontend, another on backend
- Specialized agents for testing, documentation, code review
- Agents coordinate to avoid stepping on each other

**Small Team Coordination:**
- Each developer runs their own agents
- Shared knowledge base and issue tracking
- Prevent duplicate work and conflicting changes

## Architecture

Vibe Ensemble is designed to be simple and reliable:

- **Local-First**: Runs entirely on your machine
- **SQLite Storage**: No external database required  
- **stdio Transport**: Direct integration with Claude Code
- **Web Interface**: Optional dashboard for monitoring

## Building from Source

If you prefer to build from source:

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

The binary will be at `target/release/vibe-ensemble`.

## Configuration

Vibe Ensemble works with zero configuration, but you can customize:

```bash
# Use custom database location
export DATABASE_URL="sqlite:./my-project.db"

# Run web dashboard on different port  
vibe-ensemble --port=9000

# MCP-only mode (no web dashboard)
vibe-ensemble --mcp-only --transport=stdio
```

## Documentation

- [Installation Guide](docs/installation.md) - Detailed installation instructions
- [User Guide](docs/user-guide.md) - Getting started and common workflows
- [Developer Guide](docs/developer-guide.md) - Contributing and local development

## Support

- [GitHub Issues](https://github.com/siy/vibe-ensemble-mcp/issues) - Bug reports and feature requests
- [Discussions](https://github.com/siy/vibe-ensemble-mcp/discussions) - Questions and community

## License

Licensed under the Apache License 2.0. See [LICENSE](LICENSE) for details.