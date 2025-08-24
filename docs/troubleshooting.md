# Troubleshooting Guide

Common issues when running Vibe Ensemble MCP server in production environments for small user groups.

## Quick Health Check

```bash
# Check production server status
curl http://localhost:8080/health

# Check web dashboard availability
curl http://localhost:8081/dashboard

# View system metrics
curl http://localhost:8080/status

# Check database connectivity  
ls -la vibe_ensemble.db

# View server configuration
curl http://localhost:8080/status | jq '.components'
```

## Production Deployment Issues

### Security Warnings During Startup

**Problem**: Server shows security warnings about 0.0.0.0 binding

**Explanation**: This is expected behavior when using production configuration
```bash
⚠️  SECURITY WARNING: server is bound to 0.0.0.0:8080 (all interfaces).
This exposes the service to external networks.
For production use, bind to specific interfaces (e.g., 127.0.0.1 for local only).
```

**Solutions**:
```bash
# For local/development use, edit config/local.toml
[server]
host = "127.0.0.1"
port = 8080

# For production with firewall protection, keep 0.0.0.0 and ensure:
# - Proper firewall rules
# - Network security groups
# - Load balancer configuration
```

### Configuration File Issues

**Problem**: Server can't find or parse configuration files

**Solutions**:
```bash
# Check configuration files exist
ls -la config/
# Should have: default.toml, local.example.toml, production.toml

# Create local configuration
cp config/local.example.toml config/local.toml

# Test configuration parsing
VIBE_ENSEMBLE_SERVER_HOST=127.0.0.1 cargo run --bin vibe-ensemble-server

# Override specific settings with environment variables
export VIBE_ENSEMBLE_DATABASE_URL="sqlite:./custom.db"
export VIBE_ENSEMBLE_WEB_PORT=9090
```

## Common Issues

### Server Won't Start

**Problem**: `cargo run --bin vibe-ensemble-server` fails

**Diagnostic Steps**:
```bash
# Check Rust version (requires 1.80+)
rustc --version

# Verify workspace compilation
cargo check --workspace

# Check port availability
lsof -i :8080 :8081

# Check disk space for database
df -h .

# Verify database permissions
touch vibe_ensemble.db && ls -la vibe_ensemble.db
```

**Solutions**:
```bash
# Kill processes using required ports
sudo kill $(lsof -ti :8080 :8081)

# Use alternative ports
export VIBE_ENSEMBLE_SERVER_PORT=8082
export VIBE_ENSEMBLE_WEB_PORT=8083

# Check for configuration conflicts
rm config/local.toml  # Will use defaults
```

### Web Dashboard Not Loading

**Problem**: Can't access web dashboard at http://localhost:8081

**Solutions**:
```bash
# Check if web server is enabled in configuration
curl http://localhost:8080/status | jq '.components'

# Verify web server is listening
lsof -i :8081

# Test direct dashboard access
curl -I http://localhost:8081/dashboard

# Check for browser cache issues (try incognito/private mode)
```

### System Metrics Not Displaying

**Problem**: Dashboard shows empty or inaccurate system metrics

**Solutions**:
```bash
# Check system commands availability (Unix/Linux)
which free df

# Verify metrics collection (increase log level)
RUST_LOG=debug cargo run --bin vibe-ensemble-server

# Test metrics API directly
curl http://localhost:8081/dashboard 2>/dev/null | grep -i "cpu\|memory\|disk"

# For Windows, ensure proper system info access
```

### Database Issues

**Problem**: SQLite database corruption or permission errors

**Solutions**:
```bash
# Check database permissions and size
ls -lah vibe_ensemble.db*

# Verify database integrity
sqlite3 vibe_ensemble.db "PRAGMA integrity_check;"

# Check disk space and inodes
df -h . && df -i .

# For PostgreSQL in production
export VIBE_ENSEMBLE_DATABASE_URL="postgres://user:pass@localhost/vibe_ensemble"

# Reset database (WARNING: deletes all data)
rm vibe_ensemble.db*
cargo run --bin vibe-ensemble-server
```

### Performance Issues

**Problem**: System becomes slow or unresponsive

**Diagnostic Steps**:
```bash
# Check system metrics via dashboard
curl http://localhost:8080/status | jq '.components'

# Monitor resource usage
top -p $(pgrep vibe-ensemble-server)

# Check database performance
sqlite3 vibe_ensemble.db ".timer on" "SELECT COUNT(*) FROM agents;"

# View request timing logs
tail -f server.log | grep "elapsed_ms"
```

**Solutions**:
```bash
# Increase database connections for high load
export VIBE_ENSEMBLE_DATABASE_MAX_CONNECTIONS=20

# Use PostgreSQL for better concurrent performance
export VIBE_ENSEMBLE_DATABASE_URL="postgres://user:pass@host:5432/db"

# Monitor slow requests (>1000ms logged as warnings)
RUST_LOG=warn cargo run --bin vibe-ensemble-server
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
cargo run --bin vibe-ensemble-server
```

This will show:
- Configuration validation and security warnings
- HTTP request timing and performance
- Database operations and health checks
- System metrics collection
- WebSocket connections (when implemented)

## Security Considerations

**Review the [Security Best Practices](security-best-practices.md) document for:**
- Network security and firewall configuration
- Database security (SQLite vs PostgreSQL)
- Process security and user permissions
- SSL/TLS setup with reverse proxy
- Monitoring and logging security

## Performance Optimization

### System Resource Monitoring
```bash
# Real-time system metrics via dashboard
open http://localhost:8081/dashboard

# API-based monitoring
watch -n 5 'curl -s http://localhost:8080/status | jq'

# Database performance
sqlite3 vibe_ensemble.db ".timer on" ".schema"
```

### Production Deployment
```bash
# Use production configuration
cp config/production.toml config/local.toml

# Optimize for concurrent connections
export VIBE_ENSEMBLE_DATABASE_MAX_CONNECTIONS=20

# Use PostgreSQL for better performance
export VIBE_ENSEMBLE_DATABASE_URL="postgres://user:pass@host:5432/db"
```

## Getting Help

1. **Check system health**: Visit http://localhost:8081/dashboard for real-time metrics
2. **Review logs**: Look for security warnings and performance issues
3. **Verify configuration**: Ensure all .toml files are properly formatted
4. **Test connectivity**: Use curl to verify API endpoints
5. **Check documentation**: Review security-best-practices.md for deployment guidance

## Quick Recovery

```bash
# Complete reset (WARNING: deletes all data)
pkill vibe-ensemble-server
rm vibe_ensemble.db*
rm config/local.toml
cargo run --bin vibe-ensemble-server

# Verify recovery
curl http://localhost:8080/health
curl http://localhost:8081/dashboard
```

For persistent issues, check the [Security Best Practices](security-best-practices.md) and ensure your deployment follows recommended security guidelines.