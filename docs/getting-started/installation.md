# Installation Guide

Complete installation instructions for the Vibe Ensemble MCP Server across different platforms and deployment scenarios.

## System Requirements

### Minimum Requirements
- **Operating System**: Linux (Ubuntu 20.04+, CentOS 8+, RHEL 8+), macOS 10.15+, or Windows 10+ (with WSL2)
- **CPU**: 2 cores
- **Memory**: 4GB RAM
- **Storage**: 20GB available space
- **Network**: Outbound internet access for downloads

### Recommended Production Setup
- **CPU**: 4-8 cores
- **Memory**: 8-16GB RAM
- **Storage**: 100GB SSD with backup strategy
- **Network**: Load balancer with SSL termination
- **Database**: Dedicated PostgreSQL instance

### Software Dependencies
- **Docker**: Version 20.10+ (for containerized deployment)
- **Rust**: Version 1.70+ (for native compilation)
- **Git**: Any recent version
- **curl**: For testing and health checks

## Installation Methods

### Method 1: Docker (Recommended)

Docker provides the easiest installation and management experience.

#### Prerequisites
```bash
# Install Docker (Ubuntu/Debian)
sudo apt update
sudo apt install -y docker.io docker-compose
sudo systemctl start docker
sudo systemctl enable docker

# Add your user to docker group
sudo usermod -aG docker $USER
# Log out and back in for group changes to take effect

# Verify Docker installation
docker --version
docker-compose --version
```

#### Installation Steps
```bash
# 1. Clone the repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# 2. Create environment configuration
cp .env.example .env

# 3. Generate secure secrets
echo "JWT_SECRET=$(openssl rand -base64 32)" >> .env
echo "ENCRYPTION_KEY=$(openssl rand -base64 32 | cut -c1-32)" >> .env

# 4. Start services
docker-compose up -d

# 5. Verify installation
curl http://localhost:8080/api/health
```

### Method 2: Pre-built Binaries

Download pre-compiled binaries for your platform.

#### Download and Install
```bash
# 1. Download latest release
LATEST_VERSION=$(curl -s https://api.github.com/repos/siy/vibe-ensemble-mcp/releases/latest | grep tag_name | cut -d '"' -f 4)
curl -L "https://github.com/siy/vibe-ensemble-mcp/releases/download/${LATEST_VERSION}/vibe-ensemble-server-linux-x86_64.tar.gz" -o vibe-ensemble.tar.gz

# 2. Extract and install
tar -xzf vibe-ensemble.tar.gz
sudo mv vibe-ensemble-server /usr/local/bin/
sudo chmod +x /usr/local/bin/vibe-ensemble-server

# 3. Create directories and user
sudo useradd -r -d /opt/vibe-ensemble -s /bin/false vibe-ensemble
sudo mkdir -p /opt/vibe-ensemble /var/lib/vibe-ensemble /var/log/vibe-ensemble /etc/vibe-ensemble
sudo chown -R vibe-ensemble:vibe-ensemble /opt/vibe-ensemble /var/lib/vibe-ensemble /var/log/vibe-ensemble

# 4. Configure environment
sudo tee /etc/vibe-ensemble/environment << EOF
DATABASE_URL=sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db
JWT_SECRET=$(openssl rand -base64 32)
ENCRYPTION_KEY=$(openssl rand -base64 32 | cut -c1-32)
RUST_LOG=info
EOF

# 5. Set permissions
sudo chown root:vibe-ensemble /etc/vibe-ensemble/environment
sudo chmod 640 /etc/vibe-ensemble/environment

# 6. Run database migrations
sudo -u vibe-ensemble /usr/local/bin/vibe-ensemble-server --migrate

# 7. Test installation
sudo -u vibe-ensemble /usr/local/bin/vibe-ensemble-server --validate-config
```

### Method 3: Build from Source

Compile from source for latest features or custom modifications.

#### Prerequisites
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustc --version  # Should be 1.70+

# Install build dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev

# Install build dependencies (CentOS/RHEL)
sudo dnf groupinstall "Development Tools"
sudo dnf install pkgconfig openssl-devel

# Install build dependencies (macOS)
xcode-select --install
brew install pkg-config openssl
```

#### Build and Install
```bash
# 1. Clone repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# 2. Build optimized release
cargo build --release

# 3. Run tests (optional but recommended)
cargo test --all

# 4. Install binary
sudo cp target/release/vibe-ensemble-server /usr/local/bin/
sudo chmod +x /usr/local/bin/vibe-ensemble-server

# 5. Verify build
vibe-ensemble-server --version
```

## Database Setup

### SQLite (Default)

SQLite requires no additional setup and is suitable for development and small deployments.

```bash
# Configuration (already handled in environment setup)
export DATABASE_URL="sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db"

# Create database directory
sudo mkdir -p /var/lib/vibe-ensemble
sudo chown vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble

# Initialize database
vibe-ensemble-server --migrate
```

### PostgreSQL (Production Recommended)

#### Install PostgreSQL
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# CentOS/RHEL
sudo dnf install postgresql postgresql-server postgresql-contrib
sudo postgresql-setup --initdb

# macOS
brew install postgresql
brew services start postgresql

# Start and enable service
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

#### Configure Database
```bash
# Switch to postgres user
sudo -u postgres psql

# In PostgreSQL shell:
CREATE DATABASE vibe_ensemble;
CREATE USER vibe_ensemble_user WITH ENCRYPTED PASSWORD 'secure_password_here';
GRANT ALL PRIVILEGES ON DATABASE vibe_ensemble TO vibe_ensemble_user;

# Performance tuning (adjust based on your system)
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '256MB';
ALTER SYSTEM SET effective_cache_size = '1GB';
ALTER SYSTEM SET maintenance_work_mem = '64MB';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
ALTER SYSTEM SET wal_buffers = '16MB';

# Restart to apply settings
\q
sudo systemctl restart postgresql

# Update configuration
export DATABASE_URL="postgresql://vibe_ensemble_user:secure_password_here@localhost:5432/vibe_ensemble"
```

## Service Configuration

### Systemd Service (Linux)

Create a systemd service for automatic startup and management.

#### Create Service File
```bash
sudo tee /etc/systemd/system/vibe-ensemble.service << 'EOF'
[Unit]
Description=Vibe Ensemble MCP Server
Documentation=https://github.com/siy/vibe-ensemble-mcp
After=network.target postgresql.service
Wants=postgresql.service

[Service]
Type=exec
User=vibe-ensemble
Group=vibe-ensemble
WorkingDirectory=/opt/vibe-ensemble
ExecStart=/usr/local/bin/vibe-ensemble-server
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5

# Environment
EnvironmentFile=/etc/vibe-ensemble/environment

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/vibe-ensemble /var/log/vibe-ensemble

# Resource limits
LimitNOFILE=65535
LimitMEMLOCK=64
LimitCORE=0

[Install]
WantedBy=multi-user.target
EOF
```

#### Enable and Start Service
```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable vibe-ensemble

# Start service
sudo systemctl start vibe-ensemble

# Check status
sudo systemctl status vibe-ensemble

# View logs
sudo journalctl -u vibe-ensemble -f
```

### Docker Service

Use Docker Compose for containerized deployment.

#### Create Docker Compose File
```yaml
# docker-compose.yml
version: '3.8'

services:
  vibe-ensemble:
    image: ghcr.io/siy/vibe-ensemble-mcp:latest
    container_name: vibe-ensemble-server
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///data/vibe-ensemble.db
      - JWT_SECRET=${JWT_SECRET}
      - ENCRYPTION_KEY=${ENCRYPTION_KEY}
      - RUST_LOG=info
    volumes:
      - vibe_data:/data
      - vibe_logs:/var/log/vibe-ensemble
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  vibe_data:
  vibe_logs:
```

#### Deploy with Docker Compose
```bash
# Start services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down

# Update services
docker-compose pull
docker-compose up -d
```

## Initial Configuration

### Environment Variables

Create comprehensive environment configuration:

```bash
# /etc/vibe-ensemble/environment or .env file

# Database Configuration
DATABASE_URL=postgresql://user:pass@localhost:5432/vibe_ensemble
DATABASE_POOL_SIZE=20

# Security Configuration
JWT_SECRET=your-secure-jwt-secret-key-here-minimum-32-characters
ENCRYPTION_KEY=your-32-character-encryption-key-here
JWT_EXPIRY_HOURS=24

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
MAX_CONNECTIONS=1000

# Logging Configuration
RUST_LOG=info,vibe_ensemble=debug
LOG_FILE=/var/log/vibe-ensemble/server.log

# Feature Configuration
ENABLE_API_DOCS=false
ENABLE_METRICS=true
METRICS_PORT=9090

# CORS Configuration
CORS_ALLOWED_ORIGINS=https://yourdomain.com
```

### Configuration File

Create a comprehensive configuration file:

```toml
# /etc/vibe-ensemble/config.toml

[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000

[database]
max_connections = 20
connection_timeout_seconds = 5
idle_timeout_seconds = 300

[security]
jwt_expiry_hours = 24
password_min_length = 8

[logging]
level = "info"
format = "json"
file = "/var/log/vibe-ensemble/server.log"

[metrics]
enabled = true
port = 9090

[features]
api_docs = false
admin_ui = true
```

## Verification

### Health Checks

Verify your installation is working correctly:

```bash
# 1. Basic connectivity
curl -f http://localhost:8080/api/health

# 2. System statistics
curl http://localhost:8080/api/stats

# 3. Web interface (open in browser)
open http://localhost:8080  # macOS
xdg-open http://localhost:8080  # Linux

# 4. Service status
sudo systemctl status vibe-ensemble  # Systemd
docker-compose ps  # Docker
```

### Functional Tests

```bash
# Test API endpoints
curl -X POST http://localhost:8080/api/issues \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Installation Test",
    "description": "Testing installation",
    "priority": "Low"
  }'

# Test knowledge repository
curl -X POST http://localhost:8080/api/knowledge \
  -H "Content-Type: application/json" \
  -d '{
    "title": "Installation Success",
    "content": "System installed successfully",
    "category": "system",
    "tags": ["installation", "test"]
  }'
```

## Platform-Specific Notes

### Ubuntu/Debian
- Use `apt` package manager
- SystemD service management
- UFW firewall configuration
- AppArmor security considerations

### CentOS/RHEL
- Use `dnf/yum` package manager
- SystemD service management  
- firewalld configuration
- SELinux security considerations

### macOS
- Use Homebrew for dependencies
- launchd for service management
- Consider security and privacy settings
- Development-friendly default configuration

### Windows (WSL2)
- Install WSL2 with Ubuntu
- Follow Ubuntu instructions within WSL
- Consider Windows Firewall settings
- File path and permission considerations

## Security Considerations

### Initial Security Setup
```bash
# 1. Generate strong secrets
JWT_SECRET=$(openssl rand -base64 32)
ENCRYPTION_KEY=$(openssl rand -base64 32 | cut -c1-32)

# 2. Set secure file permissions
sudo chmod 600 /etc/vibe-ensemble/environment
sudo chown root:vibe-ensemble /etc/vibe-ensemble/environment

# 3. Configure firewall
sudo ufw allow 22/tcp     # SSH
sudo ufw allow 8080/tcp   # Vibe Ensemble
sudo ufw enable

# 4. Set up SSL/TLS (production)
# See deployment guide for SSL certificate setup
```

### User Management
```bash
# Create dedicated service user
sudo useradd -r -d /opt/vibe-ensemble -s /bin/false vibe-ensemble

# Create admin user for web interface
# This will be done through the web interface on first access
```

## Troubleshooting Installation

### Common Issues

#### Permission Denied
```bash
# Fix file permissions
sudo chown -R vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble
sudo chmod 755 /var/lib/vibe-ensemble
```

#### Port Already in Use
```bash
# Find process using port
sudo lsof -i :8080

# Kill process or change port
export SERVER_PORT=8081
```

#### Database Connection Failed
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Test connection
psql -h localhost -U vibe_ensemble_user -d vibe_ensemble -c "SELECT 1;"
```

#### Service Won't Start
```bash
# Check service logs
sudo journalctl -u vibe-ensemble -f

# Test configuration
vibe-ensemble-server --validate-config

# Run in foreground for debugging
sudo -u vibe-ensemble vibe-ensemble-server
```

## Next Steps

After successful installation:

1. **Initial Setup**: Access web interface and complete setup wizard
2. **Security**: Change default passwords and configure security settings
3. **Monitoring**: Set up logging and metrics collection
4. **Agents**: Configure and connect Claude Code agents
5. **Backup**: Set up database backup procedures

## Additional Resources

- [Configuration Reference](../reference/configuration.md)
- [Deployment Guide](../deployment/deployment.md)
- [Security Guide](../deployment/security.md)
- [Troubleshooting Guide](../troubleshooting/common-issues.md)

---

*For production deployments, please review the [Deployment Guide](../deployment/deployment.md) for additional security and performance considerations.*