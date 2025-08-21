# Configuration Reference

This document provides a complete reference for all configuration options available in the Vibe Ensemble MCP Server. It covers environment variables, configuration files, and runtime settings.

## Configuration Hierarchy

The system uses a hierarchical configuration approach with the following priority order (highest to lowest):

1. **Command Line Arguments** - Override all other settings
2. **Environment Variables** - Override config files and defaults  
3. **Configuration Files** - Override built-in defaults
4. **Built-in Defaults** - Baseline system configuration

## Environment Variables

### Required Variables

#### Database Configuration
```bash
# Database connection URL
DATABASE_URL="sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db"
# OR for PostgreSQL
DATABASE_URL="postgresql://username:password@localhost:5432/vibe_ensemble"
```

#### Security Configuration
```bash
# JWT signing secret (minimum 32 characters)
JWT_SECRET="your-secure-jwt-secret-key-here-min-32-chars"

# Encryption key for sensitive data (exactly 32 characters)
ENCRYPTION_KEY="your-32-char-encryption-key-here"
```

### Server Configuration

#### Basic Server Settings
```bash
# Server bind address
SERVER_HOST="127.0.0.1"          # Default: 127.0.0.1
SERVER_HOST="0.0.0.0"            # Listen on all interfaces

# Server port
SERVER_PORT="8080"               # Default: 8080

# Maximum concurrent connections
MAX_CONNECTIONS="1000"           # Default: 1000

# Request timeout in seconds
REQUEST_TIMEOUT_SECONDS="30"     # Default: 30
```

#### Advanced Server Settings
```bash
# Connection keep-alive timeout
KEEP_ALIVE_TIMEOUT="5"           # Default: 5 seconds

# Maximum request payload size (bytes)
MAX_PAYLOAD_SIZE="1048576"       # Default: 1MB

# Number of worker threads
WORKER_THREADS="4"               # Default: CPU cores

# Enable HTTP/2
HTTP2_ENABLED="true"             # Default: false
```

### Database Configuration

#### Connection Pool Settings
```bash
# Maximum database connections in pool
DATABASE_POOL_SIZE="20"          # Default: 10

# Connection timeout in seconds
DATABASE_CONNECT_TIMEOUT="5"     # Default: 5

# Idle connection timeout in seconds
DATABASE_IDLE_TIMEOUT="300"      # Default: 300 (5 minutes)

# Maximum connection lifetime in seconds
DATABASE_MAX_LIFETIME="1800"     # Default: 1800 (30 minutes)
```

#### PostgreSQL Specific
```bash
# Enable SSL for PostgreSQL connections
DATABASE_SSL_MODE="prefer"       # Options: disable, allow, prefer, require

# PostgreSQL SSL certificate file
DATABASE_SSL_CERT="/path/to/client-cert.pem"

# PostgreSQL SSL key file  
DATABASE_SSL_KEY="/path/to/client-key.pem"

# PostgreSQL SSL root certificate
DATABASE_SSL_ROOT_CERT="/path/to/ca-cert.pem"
```

### MCP Protocol Configuration

#### Basic MCP Settings
```bash
# MCP transport protocol
MCP_TRANSPORT="websocket"        # Options: websocket, http

# MCP operation timeout
MCP_TIMEOUT_SECONDS="30"         # Default: 30

# Maximum message size for MCP
MCP_MAX_MESSAGE_SIZE="1048576"   # Default: 1MB

# MCP protocol version
MCP_PROTOCOL_VERSION="2024-11-05" # Default: latest
```

#### WebSocket Configuration
```bash
# WebSocket ping interval
WS_PING_INTERVAL="30"            # Default: 30 seconds

# WebSocket pong timeout
WS_PONG_TIMEOUT="10"             # Default: 10 seconds

# Maximum WebSocket frame size
WS_MAX_FRAME_SIZE="16777216"     # Default: 16MB

# WebSocket compression
WS_COMPRESSION="true"            # Default: true
```

### Security and Authentication

#### JWT Configuration
```bash
# JWT token expiration time
JWT_EXPIRY_HOURS="24"            # Default: 24 hours

# JWT refresh token expiration
JWT_REFRESH_EXPIRY_HOURS="168"   # Default: 168 hours (7 days)

# JWT algorithm
JWT_ALGORITHM="HS256"            # Options: HS256, HS384, HS512

# JWT issuer
JWT_ISSUER="vibe-ensemble"       # Default: vibe-ensemble
```

#### CORS Configuration
```bash
# Allowed origins for CORS
CORS_ALLOWED_ORIGINS="https://yourdomain.com,https://app.yourdomain.com"

# Allowed methods
CORS_ALLOWED_METHODS="GET,POST,PUT,DELETE,OPTIONS"

# Allowed headers
CORS_ALLOWED_HEADERS="Content-Type,Authorization,X-Requested-With"

# Allow credentials
CORS_ALLOW_CREDENTIALS="true"    # Default: false

# Max age for preflight cache
CORS_MAX_AGE="3600"              # Default: 3600 seconds
```

#### Rate Limiting
```bash
# Requests per hour per IP
RATE_LIMIT_REQUESTS_PER_HOUR="5000"  # Default: 5000

# Authenticated user rate limit
RATE_LIMIT_AUTH_REQUESTS_PER_HOUR="10000"  # Default: 10000

# Rate limit window size
RATE_LIMIT_WINDOW_SIZE="3600"        # Default: 3600 seconds

# Enable rate limiting
RATE_LIMITING_ENABLED="true"         # Default: true
```

### Logging and Monitoring

#### Logging Configuration
```bash
# Log level
RUST_LOG="info"                      # Options: trace, debug, info, warn, error
RUST_LOG="debug,vibe_ensemble=trace" # Module-specific logging

# Log format
LOG_FORMAT="json"                    # Options: json, pretty, compact

# Log file path
LOG_FILE="/var/log/vibe-ensemble/server.log"

# Log rotation size (MB)
LOG_ROTATION_SIZE="100"              # Default: 100MB

# Number of log files to keep
LOG_ROTATION_COUNT="5"               # Default: 5
```

#### Metrics Configuration
```bash
# Enable metrics collection
METRICS_ENABLED="true"               # Default: true

# Metrics server port
METRICS_PORT="9090"                  # Default: 9090

# Metrics endpoint path
METRICS_PATH="/metrics"              # Default: /metrics

# Metrics collection interval
METRICS_INTERVAL="15"                # Default: 15 seconds
```

#### Tracing Configuration
```bash
# Enable distributed tracing
TRACING_ENABLED="true"               # Default: false

# Jaeger endpoint
JAEGER_ENDPOINT="http://localhost:14268/api/traces"

# Service name for tracing
JAEGER_SERVICE_NAME="vibe-ensemble"

# Sampling ratio (0.0 to 1.0)
JAEGER_SAMPLING_RATIO="0.1"         # Default: 0.1 (10%)
```

### Feature Flags

#### Web Interface Features
```bash
# Enable API documentation
ENABLE_API_DOCS="false"              # Default: false in production

# Enable web-based admin interface
ENABLE_ADMIN_UI="true"               # Default: true

# Enable knowledge intelligence features
ENABLE_KNOWLEDGE_INTELLIGENCE="true" # Default: true

# Enable real-time WebSocket notifications
ENABLE_WEBSOCKET_NOTIFICATIONS="true" # Default: true
```

#### Development Features
```bash
# Enable development mode
DEVELOPMENT_MODE="false"             # Default: false

# Enable debug endpoints
ENABLE_DEBUG_ENDPOINTS="false"      # Default: false

# Enable request logging
ENABLE_REQUEST_LOGGING="false"      # Default: false

# Enable CORS for all origins (development only)
DEVELOPMENT_CORS="false"             # Default: false
```

## Configuration File Format

### Main Configuration File

Create `/etc/vibe-ensemble/config.toml`:

```toml
# Vibe Ensemble MCP Server Configuration

[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000
request_timeout_seconds = 30
keep_alive_timeout = 5
max_payload_size = 1048576
worker_threads = 4
http2_enabled = false

[database]
# URL can be overridden by DATABASE_URL environment variable
url = "sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db"
max_connections = 20
connection_timeout_seconds = 5
idle_timeout_seconds = 300
max_lifetime_seconds = 1800

# PostgreSQL specific settings
ssl_mode = "prefer"
# ssl_cert = "/path/to/client-cert.pem"
# ssl_key = "/path/to/client-key.pem"  
# ssl_root_cert = "/path/to/ca-cert.pem"

[mcp]
transport = "websocket"
timeout_seconds = 30
max_message_size = 1048576
protocol_version = "2024-11-05"

# WebSocket specific settings
ping_interval = 30
pong_timeout = 10
max_frame_size = 16777216
compression = true

[security]
# These should be set via environment variables in production
# jwt_secret = "development-jwt-secret-key-here"
# encryption_key = "development-encryption-key-32"

jwt_expiry_hours = 24
jwt_refresh_expiry_hours = 168
jwt_algorithm = "HS256"
jwt_issuer = "vibe-ensemble"

# Password policies
password_min_length = 8
password_require_uppercase = true
password_require_lowercase = true
password_require_numbers = true
password_require_symbols = false

[cors]
allowed_origins = []
allowed_methods = ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
allowed_headers = ["Content-Type", "Authorization", "X-Requested-With"]
allow_credentials = false
max_age = 3600

[rate_limiting]
enabled = true
requests_per_hour = 5000
auth_requests_per_hour = 10000
window_size = 3600

[logging]
level = "info"
format = "json"
file = "/var/log/vibe-ensemble/server.log"
rotation_size_mb = 100
rotation_count = 5

# Console logging (in addition to file)
console_enabled = true
console_format = "pretty"

[metrics]
enabled = true
port = 9090
path = "/metrics"
collection_interval = 15

[tracing]
enabled = false
jaeger_endpoint = "http://localhost:14268/api/traces"
service_name = "vibe-ensemble"
sampling_ratio = 0.1

[features]
api_docs = false
admin_ui = true
knowledge_intelligence = true
websocket_notifications = true

[development]
# Only used when development_mode = true
mode = false
debug_endpoints = false
request_logging = false
cors_all_origins = false
```

### Agent-Specific Configuration

Create agent configuration in `/etc/vibe-ensemble/agents/default.toml`:

```toml
# Default agent configuration

[agent]
default_timeout = 30
max_concurrent_tasks = 5
health_check_interval = 30

[capabilities]
# Default capabilities for new agents
default_capabilities = [
    "task-execution",
    "status-reporting",
    "knowledge-access"
]

[coordinator]
# Coordinator-specific settings
task_distribution_algorithm = "round_robin"  # Options: round_robin, least_loaded, capability_match
escalation_timeout = 3600  # 1 hour
max_retries = 3

[worker]
# Worker-specific settings  
max_task_duration = 7200  # 2 hours
progress_reporting_interval = 300  # 5 minutes
idle_timeout = 1800  # 30 minutes
```

## Command Line Arguments

### Server Startup Options

```bash
# Start with custom configuration file
vibe-ensemble-server --config /path/to/config.toml

# Override log level
vibe-ensemble-server --log-level debug

# Validate configuration without starting
vibe-ensemble-server --validate-config

# Show all configuration options
vibe-ensemble-server --help

# Print version information
vibe-ensemble-server --version

# Run database migrations
vibe-ensemble-server --migrate

# Export default configuration
vibe-ensemble-server --export-config > config.toml
```

### Configuration Validation

```bash
# Validate current configuration
vibe-ensemble-server --config /etc/vibe-ensemble/config.toml --validate

# Check database connectivity
vibe-ensemble-server --test-db

# Verify JWT secret strength
vibe-ensemble-server --verify-secrets

# Test all external connections
vibe-ensemble-server --health-check
```

## Runtime Configuration

### Database-Stored Settings

Some settings can be modified at runtime and are stored in the database:

#### System Settings Table
```sql
-- View current runtime settings
SELECT * FROM system_settings;

-- Update settings via API
PUT /api/admin/settings
{
  "setting_name": "rate_limit_requests_per_hour", 
  "setting_value": "10000"
}
```

#### Configurable Runtime Settings
- Rate limiting thresholds
- Feature flag enablement
- Maintenance mode status
- Agent assignment algorithms
- Notification preferences

### Dynamic Configuration Reload

```bash
# Send SIGHUP to reload configuration
kill -HUP $(pgrep vibe-ensemble-server)

# Or use systemd
systemctl reload vibe-ensemble

# Via API (requires admin privileges)
POST /api/admin/reload-config
```

## Environment-Specific Configurations

### Development Environment

```toml
# development.toml
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "sqlite://./development.db"
max_connections = 5

[logging]
level = "debug"
console_enabled = true
console_format = "pretty"

[features]
api_docs = true

[development]
mode = true
debug_endpoints = true
request_logging = true
```

### Production Environment

```toml
# production.toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000

[database]
# Set via DATABASE_URL environment variable
max_connections = 50

[logging]
level = "info"
format = "json"
file = "/var/log/vibe-ensemble/server.log"

[security]
# All secrets set via environment variables

[metrics]
enabled = true

[features]
api_docs = false
```

### Testing Environment

```toml
# testing.toml
[server]
host = "127.0.0.1"
port = 8081

[database]
url = "sqlite:///:memory:"
max_connections = 2

[logging]
level = "warn"
console_enabled = false

[rate_limiting]
enabled = false

[development]
mode = true
```

## Configuration Examples

### High-Performance Setup

```bash
# Environment variables for high-performance deployment
export SERVER_PORT=8080
export MAX_CONNECTIONS=10000
export WORKER_THREADS=16
export DATABASE_POOL_SIZE=100
export HTTP2_ENABLED=true
export METRICS_ENABLED=true
export LOG_LEVEL=warn
```

### High-Security Setup

```bash
# Security-focused configuration
export JWT_EXPIRY_HOURS=8
export RATE_LIMIT_REQUESTS_PER_HOUR=1000
export CORS_ALLOWED_ORIGINS="https://yourdomain.com"
export CORS_ALLOW_CREDENTIALS=false
export DATABASE_SSL_MODE=require
export ENABLE_DEBUG_ENDPOINTS=false
```

### Multi-Instance Setup

```toml
# Instance 1: Web interface and API
[server]
port = 8080

[features]
admin_ui = true
api_docs = false

# Instance 2: MCP protocol only
[server] 
port = 8081

[features]
admin_ui = false
websocket_notifications = false

[mcp]
transport = "websocket"
```

## Configuration Validation

### Required Validations

The system performs these validations at startup:

#### Security Validations
- JWT secret minimum length (32 characters)
- Encryption key exact length (32 characters)
- Database connection successful
- SSL certificate validity (if configured)

#### Performance Validations
- Database pool size reasonable for connections
- Worker thread count within system limits
- Memory allocation within available RAM
- Port availability and permissions

#### Feature Validations
- Required dependencies available for enabled features
- External service connectivity (if configured)
- File system permissions for log and data directories

### Validation Commands

```bash
# Comprehensive configuration validation
vibe-ensemble-server --validate-config --config /etc/vibe-ensemble/config.toml

# Database-specific validation
vibe-ensemble-server --test-database

# Security validation
vibe-ensemble-server --verify-security

# Performance validation
vibe-ensemble-server --benchmark-config
```

## Troubleshooting Configuration Issues

### Common Configuration Problems

#### Database Connection Issues
```bash
# Test database connectivity
psql "$DATABASE_URL" -c "SELECT 1;"

# Check connection pool settings
# Symptoms: "too many connections" errors
# Solution: Reduce DATABASE_POOL_SIZE
```

#### JWT/Security Issues
```bash
# Validate JWT secret
# Symptoms: Authentication failures
# Solution: Ensure JWT_SECRET is at least 32 characters

# Check encryption key
# Symptoms: Data encryption/decryption errors  
# Solution: Ensure ENCRYPTION_KEY is exactly 32 characters
```

#### Performance Issues
```bash
# Check thread configuration
# Symptoms: High CPU usage, slow responses
# Solution: Adjust WORKER_THREADS based on CPU cores

# Memory issues
# Symptoms: Out of memory errors
# Solution: Reduce MAX_CONNECTIONS and DATABASE_POOL_SIZE
```

### Configuration Debugging

```bash
# Show effective configuration
vibe-ensemble-server --show-config

# Export current configuration
vibe-ensemble-server --export-effective-config

# Validate specific settings
vibe-ensemble-server --validate-setting security.jwt_secret

# Test configuration changes
vibe-ensemble-server --dry-run --config new-config.toml
```

---

*For advanced configuration scenarios and environment-specific examples, see the [Deployment Examples](../examples/deployment-templates.md).*