# Installation Guide

Complete installation guide for Vibe Ensemble's WebSocket-based multi-agent coordination system.

## System Requirements

- **Operating System**: macOS 10.15+, Linux (Ubuntu 20.04+), Windows 10+
- **Memory**: 512 MB RAM minimum (1 GB recommended for multi-agent coordination)
- **Storage**: 200 MB free space for application and coordination database
- **Network**: 
  - Internet connection for installation
  - Localhost ports 8080 (web) and 8081 (WebSocket) available
  - Firewall allowing localhost connections on these ports

## Quick Install (Recommended)

The easiest way to get started:

### macOS and Linux

```bash
curl -fsSLO https://vibeensemble.dev/install.sh
shasum -a 256 install.sh  # or sha256sum
# Compare against published checksum, then:
bash install.sh
```

### Windows

```powershell
iwr https://vibeensemble.dev/install.ps1 -UseBasicParsing -OutFile install.ps1
Get-FileHash .\install.ps1 -Algorithm SHA256
# Compare against published checksum, then:
.\install.ps1
```

The installer will:
1. Download the latest binary for your platform
2. Install it to your PATH
3. Create the data directory at `./.vibe-ensemble/` (project-local coordination)
4. Configure WebSocket server with default settings
5. Verify the installation with connection tests

## Manual Installation

### Download Binary

Visit the [releases page](https://github.com/siy/vibe-ensemble-mcp/releases/latest) and download the binary for your platform:

- `vibe-ensemble-macos` - macOS (Intel and Apple Silicon)
- `vibe-ensemble-linux` - Linux x86_64
- `vibe-ensemble-windows.exe` - Windows x86_64

### Install the Binary

**macOS/Linux:**
```bash
# Download (replace URL with the correct version)
curl -L -o vibe-ensemble https://github.com/siy/vibe-ensemble-mcp/releases/latest/download/vibe-ensemble-macos

# Make executable
chmod +x vibe-ensemble

# Move to PATH
sudo mv vibe-ensemble /usr/local/bin/
```

**Windows:**
1. Download `vibe-ensemble-windows.exe`
2. Rename to `vibe-ensemble.exe`
3. Move to a user PATH entry (e.g., `%USERPROFILE%\bin\`) to avoid admin rights

## Building from Source

If you prefer to build from source or want to contribute:

### Prerequisites

- Rust 1.80 or later
- Git

### Build Steps

```bash
# Clone the repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build release version
cargo build --release

# The binary is now at target/release/vibe-ensemble
cp target/release/vibe-ensemble /usr/local/bin/  # or add to PATH
```

## Verify Installation

Check that Vibe Ensemble is properly installed:

```bash
vibe-ensemble --version
```

You should see output like:
```
vibe-ensemble 0.4.2
```

## First Run

Start the WebSocket coordination system:

```bash
vibe-ensemble
```

This launches:
- **WebSocket MCP Server** on `ws://127.0.0.1:8081`
- **Web Dashboard** on `http://127.0.0.1:8080`
- **SQLite Database** at `./.vibe-ensemble/data.db` (project-local)
- **Task Orchestrator** ready for multi-agent coordination

Expected startup output:
```
🚀 Vibe Ensemble WebSocket Server started successfully
🔌 WebSocket MCP: ws://127.0.0.1:8081 (ready for agent connections)
📊 Web Dashboard: http://127.0.0.1:8080 (monitoring & control)  
💾 Database: ./.vibe-ensemble/data.db (project coordination storage)
🤖 Task Orchestrator: Ready for worker spawning and coordination
⚡ Transport: WebSocket (real-time) + stdio (legacy compatibility)
```

### WebSocket-Only Mode

For production deployments or MCP-only usage:

```bash
vibe-ensemble --mcp-only --transport=websocket
```

This starts:
- **WebSocket MCP Server** only (no web dashboard)
- **Lower resource usage** for dedicated coordination
- **Production-ready** WebSocket transport with full MCP compliance

## Connect Claude Code Agents

### WebSocket MCP Configuration (Recommended)

Configure Claude Code to connect via WebSocket for real-time multi-agent coordination:

```json
{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "vibe-ensemble",
      "args": ["--mcp-only", "--transport=websocket", "--port=8081"],
      "transport": {
        "type": "websocket", 
        "url": "ws://127.0.0.1:8081",
        "reconnect": true,
        "timeout": 30000
      },
      "env": {
        "RUST_LOG": "vibe_ensemble=info"
      }
    }
  }
}
```

### Multiple Agent Configuration

For coordinated multi-agent workflows:

```json
{
  "mcpServers": {
    "vibe-coordinator": {
      "command": "vibe-ensemble",
      "args": ["--mcp-only", "--transport=websocket", "--port=8081"],
      "transport": {
        "type": "websocket",
        "url": "ws://127.0.0.1:8081"
      },
      "role": "coordinator"
    },
    "vibe-worker-1": {
      "command": "vibe-ensemble", 
      "args": ["--mcp-only", "--transport=websocket", "--port=8081"],
      "transport": {
        "type": "websocket",
        "url": "ws://127.0.0.1:8081"
      },
      "role": "worker",
      "specialization": "frontend"
    },
    "vibe-worker-2": {
      "command": "vibe-ensemble",
      "args": ["--mcp-only", "--transport=websocket", "--port=8081"], 
      "transport": {
        "type": "websocket",
        "url": "ws://127.0.0.1:8081"
      },
      "role": "worker",
      "specialization": "backend"
    }
  }
}
```

### Legacy stdio Transport

For backward compatibility or simpler single-agent use:

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

### Configuration File Location

The Claude Code MCP configuration is typically located at:
- **macOS**: `~/Library/Application Support/Claude Code/mcp_settings.json`
- **Linux**: `~/.config/claude-code/mcp_settings.json`  
- **Windows**: `%APPDATA%/Claude Code/mcp_settings.json`

### Verify Agent Connections

After configuration, verify agents are connecting:

```bash
# Check WebSocket server status
curl http://127.0.0.1:8080/api/health

# List connected agents
curl http://127.0.0.1:8080/api/agents

# View real-time agent activity
curl http://127.0.0.1:8080/api/stats
```

## Advanced Configuration

### WebSocket Server Options

```bash
# Custom WebSocket and web ports
vibe-ensemble --port=8081 --web-port=8080

# WebSocket-only deployment (no web dashboard)
vibe-ensemble --mcp-only --transport=websocket

# Custom host binding (default: 127.0.0.1)
vibe-ensemble --host=0.0.0.0 --port=8081

# Production settings with worker limits
vibe-ensemble --max-workers=50 --task-timeout=7200
```

### Environment Variables

```bash
# Network configuration
export VIBE_ENSEMBLE_PORT=8081      # WebSocket MCP port
export VIBE_WEB_PORT=8080           # Web dashboard port  
export VIBE_HOST="127.0.0.1"        # Bind address

# Database configuration
export DATABASE_URL="sqlite://./project-coordination.db"

# Task orchestration
export VIBE_MAX_WORKERS=25          # Maximum concurrent workers
export VIBE_TASK_TIMEOUT=3600       # Task timeout in seconds
export VIBE_RETRY_ATTEMPTS=3        # Max retry attempts for failed tasks

# Logging and monitoring
export RUST_LOG="vibe_ensemble=info,vibe_ensemble_mcp=debug"
export VIBE_METRICS_ENABLED=true
```

### Multi-Project Configuration

```bash
# Shared coordination database across projects
DATABASE_URL="sqlite:///shared/team-coordination.db" vibe-ensemble

# Project-specific coordination (default)
cd /path/to/project
vibe-ensemble  # Creates ./.vibe-ensemble/data.db

# Temporary in-memory coordination (testing)
DATABASE_URL="sqlite::memory:" vibe-ensemble
```

## Data Directory Structure

Vibe Ensemble uses project-local coordination storage in `./.vibe-ensemble/`:

```
./.vibe-ensemble/
├── data.db              # SQLite coordination database
├── logs/                # Application logs (if enabled)
├── workers/             # Worker process outputs and state
├── tasks/               # Task orchestration data
└── websocket-state/     # WebSocket connection state
```

### Database Schema
The coordination database includes:
- **agents** - Connected agent registry and capabilities
- **tasks** - Task definitions and orchestration state
- **workers** - Worker lifecycle and assignment tracking  
- **messages** - Inter-agent communication history
- **knowledge** - Shared patterns and insights
- **issues** - Issue tracking and resolution state

### Project-Local Benefits
- **Isolation**: Each project maintains separate coordination data
- **Portability**: Coordination state moves with project directory
- **Version Control**: Add `.vibe-ensemble/` to `.gitignore` or commit for shared state
- **Cleanup**: Remove directory to reset coordination state completely

## Updating

### Quick Update (if installed via script)

```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash  # or download + verify as above
```

### Manual Update

1. Download the new binary from releases
2. Replace the existing binary
3. Restart Vibe Ensemble

Your data and configuration will be preserved.

## Troubleshooting

### WebSocket Connection Issues

**Problem**: Agents cannot connect to WebSocket server
```bash
# Check if WebSocket server is running
curl http://127.0.0.1:8080/api/health

# Test WebSocket connection manually
wscat -c ws://127.0.0.1:8081

# Check server logs for WebSocket errors
RUST_LOG=vibe_ensemble_mcp=debug vibe-ensemble
```

**Problem**: WebSocket connections timing out
```bash
# Check for firewall blocking localhost connections
telnet 127.0.0.1 8081

# Try with increased timeouts
vibe-ensemble --read-timeout=60 --write-timeout=30
```

### Port Conflicts

**WebSocket (8081) or Web (8080) ports in use:**
```bash
# Find what's using the ports
lsof -i :8080 -i :8081  # macOS/Linux
netstat -ano | findstr :8080  # Windows
netstat -ano | findstr :8081  # Windows

# Use different ports
vibe-ensemble --port=8082 --web-port=8083

# Update Claude Code configuration accordingly
```

### Agent Registration Problems

**Problem**: Agents connect but don't register properly
```bash
# Check agent connection status
curl http://127.0.0.1:8080/api/agents

# Enable debug logging for agent registration
RUST_LOG="vibe_ensemble=debug,vibe_ensemble_mcp=debug" vibe-ensemble

# Check for JSON-RPC 2.0 compliance errors in logs
```

### Database and Storage Issues

```bash
# Check database file permissions
ls -la ./.vibe-ensemble/data.db

# Check database connectivity
sqlite3 ./.vibe-ensemble/data.db "SELECT COUNT(*) FROM agents;"

# Reset coordination database (⚠️ deletes all data)
rm -rf ./.vibe-ensemble/
vibe-ensemble  # Will recreate empty coordination state
```

### Performance and Resource Issues

**Problem**: High memory usage with many agents
```bash
# Monitor resource usage
top -p $(pgrep vibe-ensemble)

# Limit concurrent workers
vibe-ensemble --max-workers=10 --task-timeout=1800

# Use WebSocket-only mode to reduce overhead
vibe-ensemble --mcp-only --transport=websocket
```

### Claude Code Integration Issues

**Problem**: MCP server not appearing in Claude Code
1. Verify configuration file location and syntax
2. Check Claude Code logs for MCP loading errors
3. Restart Claude Code after configuration changes
4. Test with minimal configuration:
   ```json
   {
     "mcpServers": {
       "vibe-test": {
         "command": "vibe-ensemble",
         "args": ["--mcp-only", "--transport=websocket", "--port=8081"]
       }
     }
   }
   ```

**Problem**: WebSocket transport not working in Claude Code
- Ensure Claude Code supports WebSocket MCP transport
- Fall back to stdio transport as alternative:
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

### Network and Security Issues

**Problem**: Cannot access web dashboard from other machines
```bash
# Bind to all interfaces (⚠️ security implications)
vibe-ensemble --host=0.0.0.0

# Or use SSH tunnel for secure remote access
ssh -L 8080:127.0.0.1:8080 user@your-server
```

### Logging and Diagnostics

Enable comprehensive logging for debugging:
```bash
# Full debug logging
RUST_LOG="vibe_ensemble=debug,vibe_ensemble_mcp=debug,vibe_ensemble_web=debug" vibe-ensemble

# WebSocket-specific debugging
RUST_LOG="vibe_ensemble_mcp::transport=trace" vibe-ensemble --mcp-only --transport=websocket

# Save logs to file
vibe-ensemble 2>&1 | tee vibe-ensemble.log
```

## Uninstalling

To completely remove Vibe Ensemble:

```bash
# Remove binary
sudo rm /usr/local/bin/vibe-ensemble

# Remove data directory
rm -rf ~/.local/share/vibe-ensemble/
```

## Getting Help

If you encounter issues:

1. Check this troubleshooting section
2. Look at [GitHub Issues](https://github.com/siy/vibe-ensemble-mcp/issues)
3. Start a [Discussion](https://github.com/siy/vibe-ensemble-mcp/discussions)

When reporting issues, please include:
- Your operating system and version
- Installation method used
- Error messages or logs
- Steps to reproduce the problem