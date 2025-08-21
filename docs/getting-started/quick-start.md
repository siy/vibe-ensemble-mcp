# Quick Start Guide

Get up and running with Vibe Ensemble in 5 minutes. This guide covers setting up a personal workspace with 5-10 Claude Code agents working on 2-3 projects simultaneously.

## Prerequisites

- **Rust 1.70+** (install from https://rustup.rs)
- **Git** for version control
- **Claude Code** (install from https://claude.ai/code)

## Setup

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build the system
cargo build --release
```

### 2. Basic Configuration

```bash
# Set database location (SQLite for single-user)
export DATABASE_URL="sqlite:./vibe-ensemble.db"

# Optional: Enable debug logging
export RUST_LOG="info,vibe_ensemble=debug"
```

### 3. Initialize Database

```bash
# Run migrations to set up the database
cargo run --bin vibe-ensemble-server -- --migrate
```

### 4. Start the MCP Server

```bash
# Start the MCP server (runs in background)
cargo run --bin vibe-ensemble-mcp &
```

## Connect Your First Agent

### 1. Create Agent Template

Use one of the built-in templates:
- `code-writer` - For implementing features and fixing bugs
- `code-reviewer` - For reviewing code quality and security
- `test-specialist` - For writing and maintaining tests
- `docs-specialist` - For documentation tasks

### 2. Start a Worker Agent

```bash
# Start Claude Code as a code-writer agent
claude -p "You are a code writer agent for my-project. Focus on implementing features in Rust." \
  --output-format stream-json \
  --verbose \
  --mcp-server http://localhost:8080
```

### 3. Verify Connection

Check that your agent is connected:
```bash
curl http://localhost:8080/api/agents
```

## Typical Usage Pattern

### For 2-3 Projects with 5-10 Agents

1. **Project A** (3-4 agents):
   - 1x code-writer (main implementation)
   - 1x code-reviewer (quality checks)
   - 1x test-specialist (testing)
   - 1x docs-specialist (documentation)

2. **Project B** (2-3 agents):
   - 1x code-writer 
   - 1x code-reviewer
   - 1x test-specialist

3. **Project C** (2-3 agents):
   - 1x code-writer
   - 1x code-reviewer
   - 1x docs-specialist

### Example Multi-Agent Setup

```bash
# Project A agents
claude -p "Code writer for ProjectA using Rust" --mcp-server localhost:8080 &
claude -p "Code reviewer for ProjectA focusing on security" --mcp-server localhost:8080 &
claude -p "Test specialist for ProjectA using cargo test" --mcp-server localhost:8080 &

# Project B agents  
claude -p "Code writer for ProjectB using Python" --mcp-server localhost:8080 &
claude -p "Code reviewer for ProjectB following PEP 8" --mcp-server localhost:8080 &

# Documentation agent (shared across projects)
claude -p "Documentation specialist for technical writing" --mcp-server localhost:8080 &
```

## File Structure

Your workspace will look like:
```
your-workspace/
├── vibe-ensemble.db          # SQLite database
├── agent-templates/          # Agent configurations
│   ├── code-writer/
│   ├── code-reviewer/
│   ├── test-specialist/
│   └── docs-specialist/
├── workspaces/              # Agent workspaces
│   ├── project-a-workspace/
│   ├── project-b-workspace/
│   └── shared-docs/
└── projects/                # Your actual projects
    ├── project-a/
    ├── project-b/
    └── project-c/
```

## Basic Commands

```bash
# Check system health
curl http://localhost:8080/api/health

# List active agents
curl http://localhost:8080/api/agents

# View recent issues
curl http://localhost:8080/api/issues

# Check knowledge base
curl http://localhost:8080/api/knowledge
```

## Common Workflow

1. **Start your MCP server** once in the morning
2. **Launch agents** for each project you're working on
3. **Agents coordinate automatically** through the MCP server
4. **Work naturally** - agents share knowledge and avoid conflicts
5. **Stop agents** when switching contexts or done for the day

## Troubleshooting

### Server Won't Start
```bash
# Check database path is writable
touch ./vibe-ensemble.db

# Check port isn't in use
lsof -i :8080
```

### Agent Won't Connect
```bash
# Verify MCP server is running
curl http://localhost:8080/api/health

# Check Claude Code version
claude --version
```

### Performance Issues
```bash
# Monitor resource usage
ps aux | grep claude
top -p $(pgrep claude | tr '\n' ',')
```

## Next Steps

- Try different agent templates for different tasks
- Set up git worktrees for parallel development
- Explore the knowledge sharing between agents
- Customize agent configurations for your specific needs

That's it! You now have a personal multi-agent development environment running locally.