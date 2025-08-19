# Quick Start Guide

Get up and running with the Vibe Ensemble MCP Server in 5 minutes. This guide covers the fastest way to deploy and test the system.

## Prerequisites

Before starting, ensure you have:
- **Docker** (recommended) or Rust 1.70+
- **Git** for cloning the repository
- **curl** for testing endpoints
- **Claude Code** for agent testing (optional for initial setup)

## Option 1: Docker Quick Start (Recommended)

### 1. Clone and Start

```bash
# Clone the repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Start with Docker Compose
docker-compose up -d

# Verify the server is running
curl http://localhost:8080/api/health
```

**Expected Response**:
```json
{
  "status": "healthy",
  "timestamp": "2025-08-19T10:00:00Z"
}
```

### 2. Access Web Interface

Open your browser and navigate to:
- **Web Interface**: http://localhost:8080
- **API Documentation**: http://localhost:8080/docs (if enabled)

Default login credentials:
- **Username**: `admin`
- **Password**: `admin` (change immediately in production)

### 3. Test Basic Functionality

```bash
# Get system statistics
curl http://localhost:8080/api/stats

# List agents (should be empty initially)
curl http://localhost:8080/api/agents

# Create a test issue
curl -X POST http://localhost:8080/api/issues \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN_HERE" \
  -d '{
    "title": "Test Issue",
    "description": "Testing the API",
    "priority": "Medium"
  }'
```

## Option 2: Native Installation

### 1. Install Dependencies

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone and build
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

### 2. Configure Environment

```bash
# Set required environment variables
export DATABASE_URL="sqlite:./vibe-ensemble.db"
export JWT_SECRET="development-jwt-secret-change-in-production"
export ENCRYPTION_KEY="development-key-32-chars-here!!"

# Optional: Enable development features
export RUST_LOG="info,vibe_ensemble=debug"
export ENABLE_API_DOCS="true"
```

### 3. Run the Server

```bash
# Run database migrations
./target/release/vibe-ensemble-server --migrate

# Start the server
./target/release/vibe-ensemble-server
```

## Connecting Your First Agent

### 1. Configure Claude Code

```bash
# Configure Claude Code to connect to your server
claude-code config set mcp.server_url "http://localhost:8080"
claude-code config set agent.name "test-agent"
claude-code config set agent.type "Worker"
claude-code config set agent.capabilities "testing,development"
```

### 2. Start Agent

```bash
# Start Claude Code in agent mode
claude-code --agent-mode worker
```

### 3. Verify Registration

Check the web interface at http://localhost:8080/agents or use the API:

```bash
curl http://localhost:8080/api/agents
```

You should see your agent listed with "Active" status.

## Quick Test Workflow

### 1. Create an Issue

Via web interface:
1. Navigate to http://localhost:8080
2. Click "Create New Issue"
3. Fill in the form and submit

Via API:
```bash
curl -X POST http://localhost:8080/api/issues \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Hello World Test",
    "description": "Testing the system end-to-end",
    "priority": "Medium"
  }'
```

### 2. Monitor in Real-time

Open the web interface and watch for:
- Real-time updates as agents connect/disconnect
- Issue status changes
- System notifications

### 3. Test Knowledge Repository

Add a knowledge entry:
```bash
curl -X POST http://localhost:8080/api/knowledge \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Quick Start Test Pattern",
    "content": "This is a test knowledge entry created during quick start setup.",
    "category": "testing",
    "tags": ["quickstart", "test", "documentation"]
  }'
```

## Verification Checklist

Confirm these items are working:

- [ ] Server starts without errors
- [ ] Health endpoint responds
- [ ] Web interface is accessible
- [ ] Can create and view issues
- [ ] Agent registration works (if testing with Claude Code)
- [ ] Real-time updates work in web interface
- [ ] API endpoints respond correctly

## Common Issues

### Server Won't Start

**Issue**: `CONNECTION_URL not set` error
**Solution**:
```bash
export DATABASE_URL="sqlite:./vibe-ensemble.db"
```

**Issue**: Port 8080 already in use
**Solution**:
```bash
export SERVER_PORT=8081
# Or kill the process using port 8080
lsof -ti:8080 | xargs kill
```

### Cannot Access Web Interface

**Issue**: Connection refused
**Solution**: Check server is binding to correct interface:
```bash
export SERVER_HOST="0.0.0.0"  # Listen on all interfaces
```

### Agent Registration Fails

**Issue**: Agent can't connect
**Solution**: Check firewall and server configuration:
```bash
# Check if port is accessible
telnet localhost 8080

# Check server logs
docker logs vibe-ensemble  # For Docker
journalctl -f  # For native installation
```

## Next Steps

Once you have the basic system running:

1. **Explore the Web Interface**: Browse all sections (Dashboard, Agents, Issues, Knowledge)
2. **Try the API**: Use the interactive API documentation at `/docs`
3. **Set Up Multiple Agents**: Connect additional Claude Code instances
4. **Read the Documentation**: Check out the comprehensive guides:
   - [Web Interface Guide](../user/web-interface.md)
   - [API Documentation](../api/overview.md)
   - [Deployment Guide](../deployment/deployment.md)

## Production Considerations

This quick start uses development settings. For production:

1. **Change Default Passwords**: Set strong admin credentials
2. **Use PostgreSQL**: Replace SQLite with PostgreSQL for better performance
3. **Enable HTTPS**: Set up SSL/TLS certificates
4. **Configure Secrets**: Use secure values for JWT_SECRET and ENCRYPTION_KEY
5. **Set Up Monitoring**: Enable metrics and logging
6. **Review Security**: Follow the [Security Guide](../deployment/security.md)

## Getting Help

If you encounter issues:

1. **Check Logs**: Look for error messages in server logs
2. **Review Documentation**: See the [Troubleshooting Guide](../troubleshooting/common-issues.md)
3. **Search Issues**: Check [GitHub Issues](https://github.com/siy/vibe-ensemble-mcp/issues)
4. **Ask Questions**: Use [GitHub Discussions](https://github.com/siy/vibe-ensemble-mcp/discussions)

---

**Congratulations!** You now have a working Vibe Ensemble MCP Server. The system is ready to coordinate multiple Claude Code agents for your development projects.