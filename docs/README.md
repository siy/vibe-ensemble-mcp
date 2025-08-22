# Vibe Ensemble Documentation

Personal multi-agent development environment for coordinating 5-10 Claude Code agents across 2-3 projects.

## Quick Start

- **[Quick Start Guide](getting-started/quick-start.md)** - Get running in 5 minutes
- **[Architecture](architecture.md)** - How the system works  
- **[Configuration](configuration.md)** - Setup and customization
- **[Troubleshooting](troubleshooting.md)** - Common issues and solutions

## Agent Templates

Built-in agent types:
- **code-writer** - Feature implementation and bug fixing
- **code-reviewer** - Code quality and security review
- **test-specialist** - Test writing and maintenance  
- **docs-specialist** - Documentation and technical writing

## Additional Resources

- **[Git Worktrees](git-worktrees.md)** - Parallel development patterns
- **[Implementation Plan](implementation-plan.md)** - Technical roadmap
- **[High-Level Design](high-level-design.md)** - System overview

## Typical Usage

1. Start MCP server: `cargo run --bin vibe-ensemble-mcp`
2. Launch agents for each project you're working on
3. Agents coordinate automatically through the MCP server
4. Work naturally - agents share knowledge and avoid conflicts

Perfect for solo developers managing multiple projects with AI assistance.