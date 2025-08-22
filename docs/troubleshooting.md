# Troubleshooting

Common issues when running Vibe Ensemble with 5-10 Claude Code agents.

## Quick Health Check

```bash
# Check MCP server is running
curl http://localhost:8080/api/health

# Check database is accessible  
ls -la vibe-ensemble.db

# List running agents
curl http://localhost:8080/api/agents

# Check agent processes
ps aux | grep claude
```

## Common Issues

### MCP Server Won't Start

**Problem**: `cargo run --bin vibe-ensemble-server` fails

**Solutions**:
```bash
# Check database path
export DATABASE_URL="sqlite:./vibe-ensemble.db"
touch ./vibe-ensemble.db

# Check port availability
lsof -i :8080
# Kill process if needed: kill $(lsof -ti :8080)

# Use different port
export SERVER_PORT=8081
```

### Claude Code Agent Won't Connect

**Problem**: Agent fails to connect to MCP server

**Solutions**:
```bash
# Verify MCP server is running
curl http://localhost:8080/api/health

# Check Claude Code version
claude --version

# Try explicit connection
claude -p "test agent" --mcp-server http://localhost:8080 --verbose

# Check firewall (macOS)
sudo pfctl -sr | grep 8080
```

### Database Issues

**Problem**: SQLite database corruption or permission errors

**Solutions**:
```bash
# Check database permissions
ls -la vibe-ensemble.db
chmod 644 vibe-ensemble.db

# Verify database integrity
sqlite3 vibe-ensemble.db "PRAGMA integrity_check;"

# Reset database (WARNING: deletes all data)
rm vibe-ensemble.db
cargo run --bin vibe-ensemble-server
```

### Too Many Agents / Performance Issues

**Problem**: System slow with 10+ agents running

**Solutions**:
```bash
# Monitor resource usage
top -p $(pgrep claude | tr '\n' ',')

# Limit agents per project
# Project A: 3 agents max
# Project B: 2 agents max  
# Project C: 2 agents max

# Restart agents periodically
killall claude
# Then restart needed agents
```

### Workspace Conflicts

**Problem**: Agents interfering with each other's work

**Solutions**:
```bash
# Use separate workspaces per project
mkdir -p workspaces/project-a
mkdir -p workspaces/project-b
mkdir -p workspaces/project-c

# Set workspace in agent startup
claude -p "Code writer for ProjectA" \
  --working-directory workspaces/project-a \
  --mcp-server localhost:8080

# Use git worktrees for parallel development
cd your-project
git worktree add ../project-feature-a feature-a
git worktree add ../project-feature-b feature-b
```

### Git Conflicts with Multiple Agents

**Problem**: Agents creating conflicting commits

**Solutions**:
```bash
# Use separate branches per agent
git checkout -b agent-writer-work
git checkout -b agent-reviewer-work  

# Coordinate with MCP server
curl -X POST http://localhost:8080/api/coordination/lock \
  -d '{"resource": "git-repo", "agent": "writer-1"}'

# Set up git hooks for coordination
# .git/hooks/pre-commit
#!/bin/bash
curl -f http://localhost:8080/api/coordination/check || exit 1
```

### Agent Template Issues

**Problem**: Agent configuration not loading properly

**Solutions**:
```bash
# Verify template structure
ls -la agent-templates/code-writer/
# Should have: template.json, agent-config.md, prompts/

# Test template loading
cargo test orchestration::template_manager::tests::test_load_template

# Validate template JSON
python -m json.tool agent-templates/code-writer/template.json

# Check Handlebars syntax
# Look for unclosed {{}} or invalid helpers
```

## Debug Mode

Enable detailed logging:
```bash
export RUST_LOG="debug,vibe_ensemble=trace"
cargo run --bin vibe-ensemble-mcp
```

This will show:
- Agent connection attempts
- Template loading details
- Workspace creation
- Inter-agent coordination
- Database operations

## Getting Help

1. **Check logs first**: Look for error messages in console output
2. **Verify versions**: Ensure Claude Code and Rust are up to date
3. **Minimal reproduction**: Try with just 1-2 agents first
4. **Reset state**: Stop all agents, restart MCP server, try again

## Performance Tips

- **Limit concurrent agents**: Start with 3-5 agents, add more gradually
- **Use git worktrees**: Avoid file system conflicts
- **Restart periodically**: Claude Code agents can accumulate memory over time
- **Monitor resources**: Keep an eye on CPU and memory usage

Most issues are resolved by restarting the MCP server and agents in the correct order.