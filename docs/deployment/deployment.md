# Deployment Guide

This guide provides comprehensive instructions for deploying the Vibe Ensemble MCP Server in production environments. It covers installation, configuration, scaling, monitoring, and maintenance.

## Deployment Overview

### Supported Deployment Methods

- **Docker Containers** (Recommended)
- **Kubernetes** (Production clusters)
- **Systemd Services** (Traditional Linux servers)
- **Binary Installation** (Simple deployments)

### Infrastructure Requirements

#### Minimum Requirements
- **CPU**: 2 cores
- **Memory**: 4GB RAM
- **Storage**: 20GB available space
- **Network**: Outbound internet access
- **OS**: Linux (Ubuntu 20.04+, CentOS 8+, RHEL 8+)

#### Recommended Production Setup
- **CPU**: 4-8 cores
- **Memory**: 8-16GB RAM
- **Storage**: 100GB SSD with backup strategy
- **Network**: Load balancer with SSL termination
- **Database**: Dedicated PostgreSQL instance

## Quick Deployment

### Docker Compose (Fastest Start)

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  vibe-ensemble:
    image: vibe-ensemble:latest
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgresql://postgres:password@db:5432/vibe_ensemble
      - JWT_SECRET=your-secure-jwt-secret-here
      - RUST_LOG=info
    depends_on:
      - db
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  db:
    image: postgres:15-alpine
    environment:
      - POSTGRES_DB=vibe_ensemble
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=password
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql
    restart: unless-stopped

volumes:
  postgres_data:
```

Deploy:
```bash
# Clone repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Generate secure secrets
JWT_SECRET=$(openssl rand -base64 32)
DB_PASSWORD=$(openssl rand -base64 16)

# Update docker-compose.yml with secure secrets
# Start services
docker-compose up -d

# Verify deployment
curl http://localhost:8080/api/health
```

## Container Deployment

### Docker Image

#### Building from Source

```bash
# Clone repository
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp

# Build image
docker build -t vibe-ensemble:latest .

# Or build with specific target
docker build --target production -t vibe-ensemble:latest .
```

#### Pre-built Images

```bash
# Pull from registry
docker pull ghcr.io/siy/vibe-ensemble-mcp:latest

# Run with environment variables
docker run -d \
  --name vibe-ensemble \
  -p 8080:8080 \
  -e DATABASE_URL=sqlite:///data/vibe-ensemble.db \
  -e JWT_SECRET=your-secret-here \
  -v vibe-data:/data \
  ghcr.io/siy/vibe-ensemble-mcp:latest
```

#### Multi-stage Dockerfile

```dockerfile
# Build stage
FROM rust:1.70-slim AS builder
WORKDIR /app

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source
COPY . .

# Build with optimizations
RUN cargo build --release --bin vibe-ensemble-server

# Runtime stage
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create user
RUN useradd -r -u 1001 vibe-ensemble
USER vibe-ensemble

# Copy binary
COPY --from=builder /app/target/release/vibe-ensemble-server /usr/local/bin/

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD curl -f http://localhost:8080/api/health || exit 1

EXPOSE 8080
CMD ["vibe-ensemble-server"]
```

## Kubernetes Deployment

### Basic Kubernetes Manifests

#### Namespace and ConfigMap

```yaml
# namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vibe-ensemble
---
# configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vibe-ensemble-config
  namespace: vibe-ensemble
data:
  config.toml: |
    [server]
    host = "0.0.0.0"
    port = 8080
    max_connections = 1000
    
    [database]
    max_connections = 20
    
    [logging]
    level = "info"
```

#### Secrets

```yaml
# secrets.yaml
apiVersion: v1
kind: Secret
metadata:
  name: vibe-ensemble-secrets
  namespace: vibe-ensemble
type: Opaque
data:
  jwt-secret: <base64-encoded-jwt-secret>
  database-url: <base64-encoded-database-url>
  encryption-key: <base64-encoded-encryption-key>
```

#### Deployment

```yaml
# deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vibe-ensemble-server
  namespace: vibe-ensemble
  labels:
    app: vibe-ensemble-server
spec:
  replicas: 3
  selector:
    matchLabels:
      app: vibe-ensemble-server
  template:
    metadata:
      labels:
        app: vibe-ensemble-server
    spec:
      containers:
      - name: vibe-ensemble-server
        image: ghcr.io/siy/vibe-ensemble-mcp:latest
        imagePullPolicy: Always
        ports:
        - containerPort: 8080
          protocol: TCP
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: vibe-ensemble-secrets
              key: database-url
        - name: JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: vibe-ensemble-secrets
              key: jwt-secret
        - name: ENCRYPTION_KEY
          valueFrom:
            secretKeyRef:
              name: vibe-ensemble-secrets
              key: encryption-key
        - name: RUST_LOG
          value: "info"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /api/health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /api/health
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
        volumeMounts:
        - name: config
          mountPath: /app/config.toml
          subPath: config.toml
      volumes:
      - name: config
        configMap:
          name: vibe-ensemble-config
      restartPolicy: Always
```

#### Service and Ingress

```yaml
# service.yaml
apiVersion: v1
kind: Service
metadata:
  name: vibe-ensemble-service
  namespace: vibe-ensemble
  labels:
    app: vibe-ensemble-server
spec:
  selector:
    app: vibe-ensemble-server
  ports:
  - port: 80
    targetPort: 8080
    protocol: TCP
  type: ClusterIP
---
# ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: vibe-ensemble-ingress
  namespace: vibe-ensemble
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
  - hosts:
    - vibe-ensemble.yourdomain.com
    secretName: vibe-ensemble-tls
  rules:
  - host: vibe-ensemble.yourdomain.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: vibe-ensemble-service
            port:
              number: 80
```

#### Deploy to Kubernetes

```bash
# Create secrets first
kubectl create secret generic vibe-ensemble-secrets \
  --from-literal=jwt-secret="$(openssl rand -base64 32)" \
  --from-literal=database-url="postgresql://user:pass@db:5432/vibe" \
  --from-literal=encryption-key="$(openssl rand -base64 32)" \
  -n vibe-ensemble

# Apply manifests
kubectl apply -f namespace.yaml
kubectl apply -f configmap.yaml
kubectl apply -f secrets.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
kubectl apply -f ingress.yaml

# Check deployment
kubectl get pods -n vibe-ensemble
kubectl logs -f deployment/vibe-ensemble-server -n vibe-ensemble
```

## Database Setup

### PostgreSQL (Recommended for Production)

#### Installation

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# CentOS/RHEL
sudo dnf install postgresql postgresql-server postgresql-contrib

# Initialize database (CentOS/RHEL only)
sudo postgresql-setup --initdb
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

#### Configuration

```sql
-- Connect as postgres user
sudo -u postgres psql

-- Create database and user
CREATE DATABASE vibe_ensemble;
CREATE USER vibe_ensemble_user WITH ENCRYPTED PASSWORD 'secure_password_here';
GRANT ALL PRIVILEGES ON DATABASE vibe_ensemble TO vibe_ensemble_user;

-- Configure for production
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '512MB';
ALTER SYSTEM SET effective_cache_size = '2GB';
ALTER SYSTEM SET maintenance_work_mem = '128MB';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
ALTER SYSTEM SET wal_buffers = '16MB';
ALTER SYSTEM SET default_statistics_target = 100;

-- Restart PostgreSQL to apply settings
sudo systemctl restart postgresql
```

#### Database URL
```bash
DATABASE_URL=postgresql://vibe_ensemble_user:secure_password_here@localhost:5432/vibe_ensemble
```

### SQLite (Development and Small Deployments)

```bash
# SQLite configuration
DATABASE_URL=sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db

# Create directory
sudo mkdir -p /var/lib/vibe-ensemble
sudo chown vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble
```

## Configuration Management

### Environment Variables

#### Required Variables

```bash
# Database connection
DATABASE_URL=postgresql://user:pass@host:5432/dbname

# Security
JWT_SECRET=your-secure-jwt-secret-min-32-chars
ENCRYPTION_KEY=your-encryption-key-exactly-32-chars

# Server configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# Logging
RUST_LOG=info,vibe_ensemble=debug
```

#### Optional Variables

```bash
# Performance tuning
MAX_CONNECTIONS=1000
DATABASE_POOL_SIZE=20

# Features
ENABLE_API_DOCS=false
ENABLE_METRICS=true
ENABLE_TRACING=true

# Security
CORS_ALLOWED_ORIGINS=https://yourdomain.com
RATE_LIMIT_REQUESTS_PER_HOUR=5000

# Monitoring
HEALTH_CHECK_INTERVAL_SECONDS=30
METRICS_PORT=9090
```

### Configuration File

Create `/etc/vibe-ensemble/config.toml`:

```toml
[server]
host = "0.0.0.0"
port = 8080
max_connections = 1000
request_timeout_seconds = 30

[database]
max_connections = 20
connection_timeout_seconds = 5
idle_timeout_seconds = 300

[mcp]
transport = "websocket"
timeout_seconds = 30
max_message_size = 1048576

[security]
jwt_expiry_hours = 24
password_min_length = 8
rate_limit_requests_per_hour = 5000

[logging]
level = "info"
format = "json"
file = "/var/log/vibe-ensemble/server.log"

[metrics]
enabled = true
port = 9090
path = "/metrics"

[features]
api_docs = false
health_endpoint = true
websocket_enabled = true
```

## SSL/TLS Configuration

### Nginx Reverse Proxy

```nginx
# /etc/nginx/sites-available/vibe-ensemble
server {
    listen 80;
    server_name vibe-ensemble.yourdomain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name vibe-ensemble.yourdomain.com;

    ssl_certificate /etc/letsencrypt/live/vibe-ensemble.yourdomain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/vibe-ensemble.yourdomain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;

    # Security headers
    add_header Strict-Transport-Security "max-age=63072000" always;
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header Referrer-Policy strict-origin-when-cross-origin;

    # Proxy configuration
    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket support
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        
        # Timeouts
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;
    }

    # Health check endpoint (no authentication)
    location /api/health {
        proxy_pass http://127.0.0.1:8080;
        access_log off;
    }
}
```

Enable and restart Nginx:
```bash
sudo ln -s /etc/nginx/sites-available/vibe-ensemble /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl restart nginx
```

### Let's Encrypt SSL Certificate

```bash
# Install certbot
sudo apt install certbot python3-certbot-nginx

# Generate certificate
sudo certbot --nginx -d vibe-ensemble.yourdomain.com

# Test renewal
sudo certbot renew --dry-run

# Add cron job for auto-renewal
echo "0 3 * * * root certbot renew --quiet && systemctl reload nginx" | sudo tee -a /etc/crontab
```

## Service Management

### Systemd Service

Create `/etc/systemd/system/vibe-ensemble.service`:

```ini
[Unit]
Description=Vibe Ensemble MCP Server
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

# Environment variables
Environment=DATABASE_URL=postgresql://user:pass@localhost:5432/vibe_ensemble
Environment=JWT_SECRET=your-secure-jwt-secret
Environment=RUST_LOG=info

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/var/lib/vibe-ensemble
ProtectHome=true

# Resource limits
LimitNOFILE=65535
LimitMEMLOCK=64
LimitCORE=0

[Install]
WantedBy=multi-user.target
```

Service management:
```bash
# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable vibe-ensemble
sudo systemctl start vibe-ensemble

# Check status
sudo systemctl status vibe-ensemble

# View logs
sudo journalctl -u vibe-ensemble -f

# Restart service
sudo systemctl restart vibe-ensemble
```

### User and Directory Setup

```bash
# Create user
sudo useradd -r -d /opt/vibe-ensemble -s /bin/false vibe-ensemble

# Create directories
sudo mkdir -p /opt/vibe-ensemble
sudo mkdir -p /var/lib/vibe-ensemble
sudo mkdir -p /var/log/vibe-ensemble
sudo mkdir -p /etc/vibe-ensemble

# Set permissions
sudo chown -R vibe-ensemble:vibe-ensemble /opt/vibe-ensemble
sudo chown -R vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble
sudo chown -R vibe-ensemble:vibe-ensemble /var/log/vibe-ensemble
sudo chown root:vibe-ensemble /etc/vibe-ensemble
sudo chmod 750 /etc/vibe-ensemble

# Install binary
sudo cp target/release/vibe-ensemble-server /usr/local/bin/
sudo chown root:root /usr/local/bin/vibe-ensemble-server
sudo chmod 755 /usr/local/bin/vibe-ensemble-server
```

## Monitoring Setup

### Health Checks

```bash
# Basic health check
curl -f http://localhost:8080/api/health

# Detailed health check with timeout
timeout 5s curl -f http://localhost:8080/api/health || exit 1

# Health check script
#!/bin/bash
HEALTH_URL="http://localhost:8080/api/health"
RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" $HEALTH_URL)
if [ $RESPONSE -eq 200 ]; then
    echo "Service healthy"
    exit 0
else
    echo "Service unhealthy (HTTP $RESPONSE)"
    exit 1
fi
```

### Log Management

```bash
# Logrotate configuration
cat << 'EOF' | sudo tee /etc/logrotate.d/vibe-ensemble
/var/log/vibe-ensemble/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 vibe-ensemble vibe-ensemble
    postrotate
        systemctl reload vibe-ensemble
    endscript
}
EOF
```

### Metrics and Monitoring

See the [Monitoring Guide](monitoring.md) for detailed monitoring setup including:
- Prometheus metrics collection
- Grafana dashboards
- Alert configuration
- Log aggregation

## Security Hardening

### Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 80/tcp    # HTTP (redirects to HTTPS)
sudo ufw allow 443/tcp   # HTTPS
sudo ufw enable

# Firewalld (CentOS/RHEL)
sudo firewall-cmd --permanent --add-service=ssh
sudo firewall-cmd --permanent --add-service=http
sudo firewall-cmd --permanent --add-service=https
sudo firewall-cmd --reload
```

### Security Best Practices

- Use strong, unique passwords and keys
- Enable automatic security updates
- Regularly rotate JWT secrets and encryption keys
- Monitor access logs for suspicious activity
- Use fail2ban for SSH protection
- Keep system and dependencies updated
- Implement proper backup procedures

## Backup and Recovery

### Database Backup

```bash
#!/bin/bash
# Database backup script
BACKUP_DIR="/var/backups/vibe-ensemble"
DATE=$(date +%Y%m%d_%H%M%S)

# Create backup directory
mkdir -p $BACKUP_DIR

# Backup PostgreSQL database
pg_dump -h localhost -U vibe_ensemble_user \
        -d vibe_ensemble \
        --no-password \
        --compress=9 \
        -f "$BACKUP_DIR/vibe-ensemble-$DATE.sql.gz"

# Keep only last 7 days of backups
find $BACKUP_DIR -name "*.sql.gz" -mtime +7 -delete

echo "Backup completed: vibe-ensemble-$DATE.sql.gz"
```

### Configuration Backup

```bash
# Backup configuration files
tar -czf /var/backups/vibe-ensemble-config-$(date +%Y%m%d).tar.gz \
    /etc/vibe-ensemble/ \
    /etc/systemd/system/vibe-ensemble.service \
    /etc/nginx/sites-available/vibe-ensemble
```

## Troubleshooting

### Common Issues

#### Service Won't Start
```bash
# Check systemd logs
sudo journalctl -u vibe-ensemble -n 50

# Check configuration
vibe-ensemble-server --config /etc/vibe-ensemble/config.toml --validate

# Check database connectivity
psql -h localhost -U vibe_ensemble_user -d vibe_ensemble -c "SELECT 1;"
```

#### High Memory Usage
```bash
# Monitor memory usage
top -p $(pgrep vibe-ensemble-server)

# Check for memory leaks
valgrind --tool=memcheck --leak-check=full vibe-ensemble-server
```

#### Database Connection Issues
```bash
# Check PostgreSQL status
sudo systemctl status postgresql

# Check connection limits
psql -c "SELECT count(*) FROM pg_stat_activity;"

# Check database configuration
psql -c "SHOW max_connections;"
```

### Performance Tuning

See the [Performance Guide](../troubleshooting/performance.md) for detailed performance optimization.

## Next Steps

After successful deployment:

1. **Configure Monitoring**: Set up comprehensive monitoring with alerts
2. **Setup Backups**: Implement automated backup procedures
3. **Security Audit**: Perform security assessment and hardening
4. **Load Testing**: Test system under expected load
5. **Documentation**: Update runbooks and operational procedures

For more detailed information, see:
- [Configuration Reference](configuration.md)
- [Monitoring Guide](monitoring.md)
- [Security Guide](security.md)
- [Scaling Guide](scaling.md)

---

*For additional deployment scenarios and advanced configurations, see the [Examples](../examples/deployment-templates.md) section.*