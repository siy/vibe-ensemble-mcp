# Vibe Ensemble MCP - Installation Guide

A comprehensive guide to installing and running the Vibe Ensemble MCP server for team coordination between multiple Claude Code instances.

## Quick Install

### macOS (Homebrew)

```bash
# Install via Homebrew (coming soon)
# brew tap siy/vibe-ensemble
# brew install vibe-ensemble-mcp

# Start the server
vibe-ensemble
```

### Linux (Package Manager)

```bash
# Ubuntu/Debian
curl -fsSL https://get.vibeensemble.dev/install.sh | sudo bash
sudo apt update && sudo apt install vibe-ensemble-mcp

# CentOS/RHEL/Fedora
curl -fsSL https://get.vibeensemble.dev/install.sh | sudo bash
sudo yum install vibe-ensemble-mcp  # or dnf for newer systems
```

### Windows (PowerShell)

```powershell
# Install via PowerShell
iex "& { irm https://get.vibeensemble.dev/install.ps1 }"

# Or download MSI installer
# https://github.com/siy/vibe-ensemble-mcp/releases/latest
```


## System Requirements

### Minimum Requirements
- **CPU**: 1 core, 2.0 GHz
- **Memory**: 512 MB RAM
- **Storage**: 100 MB available space
- **Network**: Internet connection for initial setup

### Recommended Requirements
- **CPU**: 2+ cores, 2.4 GHz
- **Memory**: 2 GB RAM
- **Storage**: 1 GB available space
- **Network**: Stable internet connection

### Platform Support
- **macOS**: 10.15+ (Intel/Apple Silicon)
- **Linux**: Ubuntu 20.04+, CentOS 8+, Debian 11+
- **Windows**: Windows 10/11, Windows Server 2019+

## Prerequisites

### Rust (for building from source)
```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustc --version  # Should be 1.80+
```

### Database (Optional)
```bash
# SQLite (included by default)
# No additional setup required

# PostgreSQL (production)
# macOS
brew install postgresql
brew services start postgresql

# Ubuntu/Debian
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql

# Create database
sudo -u postgres createdb vibe_ensemble
sudo -u postgres createuser vibe_ensemble
```

## Installation Methods

### Method 1: Pre-built Binaries (Recommended)

#### macOS
```bash
# Download latest release
curl -L -o vibe-ensemble-mcp.tar.gz \
  https://github.com/siy/vibe-ensemble-mcp/releases/latest/download/vibe-ensemble-mcp-macos.tar.gz

# Extract and install
tar -xzf vibe-ensemble-mcp.tar.gz
sudo mv vibe-ensemble /usr/local/bin/

# Verify installation
vibe-ensemble --version
```

#### Linux
```bash
# Download latest release
curl -L -o vibe-ensemble-mcp.tar.gz \
  https://github.com/siy/vibe-ensemble-mcp/releases/latest/download/vibe-ensemble-mcp-linux.tar.gz

# Extract and install
tar -xzf vibe-ensemble-mcp.tar.gz
sudo mv vibe-ensemble /usr/local/bin/
sudo chmod +x /usr/local/bin/vibe-ensemble

# Verify installation
vibe-ensemble --version
```

#### Windows
1. Download the MSI installer from [releases page](https://github.com/siy/vibe-ensemble-mcp/releases/latest)
2. Run the installer as Administrator
3. Follow the installation wizard
4. Add to PATH if not done automatically

### Method 2: Building from Source

```bash
# Clone repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build release version
cargo build --release

# Install binaries
sudo cp target/release/vibe-ensemble /usr/local/bin/

# Verify installation
vibe-ensemble --version
```

### Method 3: Cargo Install

```bash
# Install from crates.io
cargo install vibe-ensemble

# Or install from Git
cargo install --git https://github.com/siy/vibe-ensemble-mcp.git vibe-ensemble
```

## Configuration

### Basic Configuration

Create a configuration directory:
```bash
# Linux/macOS
mkdir -p ~/.config/vibe-ensemble
cd ~/.config/vibe-ensemble

# Windows
mkdir %APPDATA%\vibe-ensemble
cd %APPDATA%\vibe-ensemble
```

Create basic configuration:
```bash
# Download example configuration
curl -o config.toml \
  https://raw.githubusercontent.com/siy/vibe-ensemble-mcp/main/config/default.toml

# Edit as needed
nano config.toml  # or your preferred editor
```

### Configuration Options

**config.toml:**
```toml
[server]
host = "127.0.0.1"    # Server bind address
port = 8080           # API server port
workers = 4           # Number of worker threads

[database]
url = "sqlite:./vibe_ensemble.db"  # Database URL
max_connections = 10                # Max database connections
migrate_on_startup = true          # Run migrations on startup

[web]
enabled = true        # Enable web dashboard
host = "127.0.0.1"   # Web server bind address
port = 8081          # Web server port

[logging]
level = "info"       # Log level (trace, debug, info, warn, error)
format = "json"      # Log format (json, pretty)
```

### Environment Variables

You can override any configuration value with environment variables:
```bash
# Database configuration
export VIBE_ENSEMBLE_DATABASE__URL="postgres://user:pass@localhost/db"
export VIBE_ENSEMBLE_DATABASE__MAX_CONNECTIONS=20

# Server configuration
export VIBE_ENSEMBLE_SERVER__HOST="0.0.0.0"
export VIBE_ENSEMBLE_SERVER__PORT=8080

# Web interface
export VIBE_ENSEMBLE_WEB__ENABLED=true
export VIBE_ENSEMBLE_WEB__PORT=3000
```

## Starting the Server

### Development Mode
```bash
# Start with default configuration
vibe-ensemble

# Start with custom config
vibe-ensemble --config /path/to/config.toml

# Start with environment override
VIBE_ENSEMBLE_SERVER__PORT=9000 vibe-ensemble
```

### Production Mode

#### Using systemd (Linux)
```bash
# Create service file
sudo tee /etc/systemd/system/vibe-ensemble.service > /dev/null << EOF
[Unit]
Description=Vibe Ensemble MCP Server
After=network.target

[Service]
Type=simple
User=vibe-ensemble
Group=vibe-ensemble
WorkingDirectory=/var/lib/vibe-ensemble
ExecStart=/usr/local/bin/vibe-ensemble
Restart=always
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable vibe-ensemble
sudo systemctl start vibe-ensemble
sudo systemctl status vibe-ensemble
```


## Verification

### Health Check
```bash
# Check server health
curl http://localhost:8080/api/health

# Expected response:
# {
#   "status": "healthy",
#   "timestamp": "<ISO-8601 UTC>",
#   "version": "<semver>"
# }
```

### Web Interface
Open your browser and navigate to:
- **Dashboard**: <http://localhost:8080>
- **Metrics**: <http://localhost:9090/metrics>
- **API Health**: <http://localhost:8080/api/health>

### MCP Tools
Test MCP server integration:
```bash
# Using the MCP client
echo '{"jsonrpc": "2.0", "id": 1, "method": "vibe/agent/list", "params": {}}' | vibe-ensemble-mcp
```

## Updating

### Pre-built Binaries
```bash
# Download latest version
curl -L -o vibe-ensemble-mcp.tar.gz \
  https://github.com/siy/vibe-ensemble-mcp/releases/latest/download/vibe-ensemble-mcp-$(uname -s | tr '[:upper:]' '[:lower:]').tar.gz

# Stop service, update, and restart
sudo systemctl stop vibe-ensemble  # if using systemd
tar -xzf vibe-ensemble-mcp.tar.gz
sudo mv vibe-ensemble /usr/local/bin/
sudo systemctl start vibe-ensemble
```

### Cargo Install
```bash
cargo install --force vibe-ensemble
```


## Troubleshooting

### Common Issues

#### Port Already in Use
```bash
# Find process using port 8080
lsof -i :8080  # macOS/Linux
netstat -ano | findstr :8080  # Windows

# Kill the process or change port in configuration
```

#### Database Connection Issues
```bash
# SQLite permissions
chmod 644 vibe_ensemble.db
chown $(whoami) vibe_ensemble.db

# PostgreSQL connection
pg_isready -h localhost -p 5432
psql -U vibe_ensemble -d vibe_ensemble -h localhost
```

#### Permission Denied
```bash
# Fix binary permissions
chmod +x /usr/local/bin/vibe-ensemble

# Fix config directory permissions
chmod -R 755 ~/.config/vibe-ensemble
```

### Log Analysis
```bash
# View logs (systemd)
sudo journalctl -u vibe-ensemble -f


# Enable debug logging
export RUST_LOG=debug
vibe-ensemble
```

### Performance Issues
```bash
# Monitor resource usage
htop  # or top
df -h  # disk usage
netstat -i  # network usage

# Database optimization
ANALYZE;  -- PostgreSQL
VACUUM;   -- PostgreSQL
```

## Integration with Claude Code

### Setup MCP Server in Claude Code

#### Option 1: Claude CLI (Recommended)

Use the Claude Code CLI to add the MCP server. Choose the appropriate scope for your needs:

```bash
# Local scope (current project only)
claude mcp add vibe-ensemble --transport=stdio -- vibe-ensemble --mcp-only --transport=stdio

# User scope (available across all projects)
claude mcp add vibe-ensemble -s user --transport=stdio -- vibe-ensemble --mcp-only --transport=stdio

# Project scope (shared with team)
claude mcp add vibe-ensemble -s project --transport=stdio -- vibe-ensemble --mcp-only --transport=stdio
```

#### Option 2: Manual JSON Configuration

1. Open Claude Code settings
2. Navigate to MCP servers
3. Add new server:
   ```json
   {
     "command": "vibe-ensemble",
     "args": ["--mcp-only", "--transport=stdio"],
     "env": {
       "VIBE_ENSEMBLE_SERVER_URL": "http://localhost:8080"
     }
   }
   ```

### Available Tools
- `vibe/agent/list` - List registered agents
- `vibe/agent/register` - Register new agent
- `vibe/issue/create` - Create new issue
- `vibe/issue/assign` - Assign issue to agent
- `vibe/message/send` - Send message between agents
- `vibe/knowledge/add` - Add knowledge entry
- `vibe/coordination/status` - Check coordination status

## Support

### Getting Help
- **Documentation**: <https://vibe-ensemble.dev/docs>
- **GitHub Issues**: <https://github.com/siy/vibe-ensemble-mcp/issues>
- **Discussions**: <https://github.com/siy/vibe-ensemble-mcp/discussions>

### Reporting Issues
When reporting issues, please include:
- Operating system and version
- Installation method
- Configuration file (sanitized)
- Error logs
- Steps to reproduce

### Contributing
See [CONTRIBUTING.md](../CONTRIBUTING.md) for development setup and contribution guidelines.

## Security

### Production Hardening
- Use PostgreSQL instead of SQLite
- Enable HTTPS with reverse proxy
- Restrict network access
- Regular security updates
- Monitor logs for suspicious activity

### Authentication
Coming in future releases:
- JWT authentication
- Role-based access control
- API key management
- OAuth integration

## License

This project is licensed under the MIT License - see the [LICENSE](../LICENSE) file for details.