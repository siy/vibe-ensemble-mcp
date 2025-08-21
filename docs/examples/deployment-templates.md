# Deployment Templates

This document provides ready-to-use deployment templates for various environments and scenarios. Each template includes complete configuration files and deployment instructions.

## Docker Compose Templates

### Basic Single-Instance Setup

**File**: `docker-compose.basic.yml`

```yaml
version: '3.8'

services:
  vibe-ensemble:
    image: ghcr.io/siy/vibe-ensemble-mcp:latest
    container_name: vibe-ensemble-server
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=sqlite:///data/vibe-ensemble.db
      - JWT_SECRET=${JWT_SECRET:-development-jwt-secret-change-in-production}
      - ENCRYPTION_KEY=${ENCRYPTION_KEY:-development-key-32-chars-here}
      - RUST_LOG=info
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
    volumes:
      - vibe_data:/data
      - vibe_logs:/var/log/vibe-ensemble
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s

volumes:
  vibe_data:
    driver: local
  vibe_logs:
    driver: local
```

**Deployment**:
```bash
# Generate secure secrets
export JWT_SECRET=$(openssl rand -base64 32)
export ENCRYPTION_KEY=$(openssl rand -base64 32 | cut -c1-32)

# Deploy
docker-compose -f docker-compose.basic.yml up -d

# Verify
curl http://localhost:8080/api/health
```

### Production Setup with PostgreSQL

**File**: `docker-compose.production.yml`

```yaml
version: '3.8'

services:
  db:
    image: postgres:15-alpine
    container_name: vibe-ensemble-db
    environment:
      POSTGRES_DB: vibe_ensemble
      POSTGRES_USER: vibe_user
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init-db.sql:/docker-entrypoint-initdb.d/init-db.sql
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U vibe_user -d vibe_ensemble"]
      interval: 10s
      timeout: 5s
      retries: 3
    networks:
      - vibe-network

  vibe-ensemble:
    image: ghcr.io/siy/vibe-ensemble-mcp:latest
    container_name: vibe-ensemble-server
    depends_on:
      - db
    environment:
      - DATABASE_URL=postgresql://vibe_user:${DB_PASSWORD}@db:5432/vibe_ensemble
      - JWT_SECRET=${JWT_SECRET}
      - ENCRYPTION_KEY=${ENCRYPTION_KEY}
      - RUST_LOG=info,vibe_ensemble=debug
      - SERVER_HOST=0.0.0.0
      - SERVER_PORT=8080
      - MAX_CONNECTIONS=1000
      - DATABASE_POOL_SIZE=20
      - METRICS_ENABLED=true
      - METRICS_PORT=9090
      - CORS_ALLOWED_ORIGINS=https://${DOMAIN}
    volumes:
      - vibe_logs:/var/log/vibe-ensemble
      - ./config.toml:/app/config.toml:ro
    ports:
      - "127.0.0.1:8080:8080"
      - "127.0.0.1:9090:9090"
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/api/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
    networks:
      - vibe-network

  nginx:
    image: nginx:alpine
    container_name: vibe-ensemble-proxy
    depends_on:
      - vibe-ensemble
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf:ro
      - ./ssl:/etc/nginx/ssl:ro
      - vibe_logs:/var/log/nginx
    restart: unless-stopped
    networks:
      - vibe-network

volumes:
  postgres_data:
    driver: local
  vibe_logs:
    driver: local

networks:
  vibe-network:
    driver: bridge
```

**Supporting Files**:

**File**: `init-db.sql`
```sql
-- Database initialization
CREATE DATABASE vibe_ensemble;
CREATE USER vibe_user WITH ENCRYPTED PASSWORD 'your-secure-password';
GRANT ALL PRIVILEGES ON DATABASE vibe_ensemble TO vibe_user;

-- Performance optimizations
ALTER SYSTEM SET max_connections = 200;
ALTER SYSTEM SET shared_buffers = '256MB';
ALTER SYSTEM SET effective_cache_size = '1GB';
ALTER SYSTEM SET maintenance_work_mem = '64MB';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
ALTER SYSTEM SET wal_buffers = '16MB';
```

**File**: `nginx.conf`
```nginx
events {
    worker_connections 1024;
}

http {
    upstream vibe-ensemble {
        server vibe-ensemble:8080;
    }

    server {
        listen 80;
        server_name your-domain.com;
        return 301 https://$server_name$request_uri;
    }

    server {
        listen 443 ssl http2;
        server_name your-domain.com;

        ssl_certificate /etc/nginx/ssl/fullchain.pem;
        ssl_certificate_key /etc/nginx/ssl/privkey.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        
        # Security headers
        add_header Strict-Transport-Security "max-age=63072000" always;
        add_header X-Frame-Options DENY;
        add_header X-Content-Type-Options nosniff;

        location / {
            proxy_pass http://vibe-ensemble;
            proxy_set_header Host $host;
            proxy_set_header X-Real-IP $remote_addr;
            proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
            proxy_set_header X-Forwarded-Proto $scheme;
            
            # WebSocket support
            proxy_http_version 1.1;
            proxy_set_header Upgrade $http_upgrade;
            proxy_set_header Connection "upgrade";
        }

        location /metrics {
            proxy_pass http://vibe-ensemble:9090/metrics;
            # Restrict access to monitoring systems
            allow 10.0.0.0/8;
            allow 172.16.0.0/12;
            allow 192.168.0.0/16;
            deny all;
        }
    }
}
```

**Deployment Script**: `deploy-production.sh`
```bash
#!/bin/bash
set -e

echo "🚀 Deploying Vibe Ensemble Production Environment"

# Generate secure secrets if not provided
if [ -z "$JWT_SECRET" ]; then
    export JWT_SECRET=$(openssl rand -base64 32)
    echo "Generated JWT_SECRET (save this): $JWT_SECRET"
fi

if [ -z "$ENCRYPTION_KEY" ]; then
    export ENCRYPTION_KEY=$(openssl rand -base64 32 | cut -c1-32)
    echo "Generated ENCRYPTION_KEY (save this): $ENCRYPTION_KEY"
fi

if [ -z "$DB_PASSWORD" ]; then
    export DB_PASSWORD=$(openssl rand -base64 16)
    echo "Generated DB_PASSWORD (save this): $DB_PASSWORD"
fi

# Validate required environment variables
if [ -z "$DOMAIN" ]; then
    echo "Error: DOMAIN environment variable must be set"
    exit 1
fi

# Create required directories
mkdir -p ssl logs

# Generate self-signed SSL certificate if not provided
if [ ! -f "ssl/fullchain.pem" ]; then
    echo "Generating self-signed SSL certificate..."
    openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
        -keyout ssl/privkey.pem \
        -out ssl/fullchain.pem \
        -subj "/C=US/ST=State/L=City/O=Organization/CN=$DOMAIN"
fi

# Deploy services
docker-compose -f docker-compose.production.yml up -d

# Wait for services to be healthy
echo "Waiting for services to start..."
sleep 30

# Verify deployment
if curl -f "http://localhost:8080/api/health" > /dev/null 2>&1; then
    echo "✅ Vibe Ensemble is running successfully!"
    echo "🌐 Web interface: https://$DOMAIN"
    echo "📊 Metrics: https://$DOMAIN/metrics"
else
    echo "❌ Deployment failed - check logs"
    docker-compose -f docker-compose.production.yml logs
    exit 1
fi
```

### Development Environment

**File**: `docker-compose.dev.yml`

```yaml
version: '3.8'

services:
  vibe-ensemble-dev:
    build:
      context: .
      dockerfile: Dockerfile.dev
    container_name: vibe-ensemble-dev
    ports:
      - "8080:8080"
      - "9090:9090"
    environment:
      - DATABASE_URL=sqlite:///data/development.db
      - JWT_SECRET=development-jwt-secret-not-for-production
      - ENCRYPTION_KEY=development-key-32-chars-here
      - RUST_LOG=debug,vibe_ensemble=trace
      - DEVELOPMENT_MODE=true
      - ENABLE_API_DOCS=true
      - ENABLE_DEBUG_ENDPOINTS=true
      - DEVELOPMENT_CORS=true
    volumes:
      - ./:/app:cached
      - vibe_dev_data:/data
      - cargo_cache:/usr/local/cargo/registry
    restart: unless-stopped
    command: ["cargo", "watch", "-x", "run --bin vibe-ensemble-server"]

volumes:
  vibe_dev_data:
  cargo_cache:
```

**File**: `Dockerfile.dev`
```dockerfile
FROM rust:1.70

# Install development dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install cargo-watch for hot reloading
RUN cargo install cargo-watch

WORKDIR /app

# Copy source code
COPY . .

# Build dependencies
RUN cargo build --release

EXPOSE 8080 9090
CMD ["cargo", "run", "--bin", "vibe-ensemble-server"]
```

## Kubernetes Templates

### Basic Kubernetes Deployment

**File**: `kubernetes/namespace.yaml`
```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: vibe-ensemble
  labels:
    name: vibe-ensemble
```

**File**: `kubernetes/configmap.yaml`
```yaml
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
    format = "json"
    
    [metrics]
    enabled = true
    port = 9090
    
    [features]
    api_docs = false
    admin_ui = true
```

**File**: `kubernetes/secrets.yaml`
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: vibe-ensemble-secrets
  namespace: vibe-ensemble
type: Opaque
data:
  # Base64 encoded values - replace with actual values
  jwt-secret: <base64-encoded-jwt-secret>
  encryption-key: <base64-encoded-encryption-key>
  database-url: <base64-encoded-database-url>
```

**File**: `kubernetes/deployment.yaml`
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vibe-ensemble-server
  namespace: vibe-ensemble
  labels:
    app: vibe-ensemble-server
spec:
  replicas: 2
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
          name: http
        - containerPort: 9090
          name: metrics
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
        - name: METRICS_ENABLED
          value: "true"
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
        - name: data
          mountPath: /data
      volumes:
      - name: config
        configMap:
          name: vibe-ensemble-config
      - name: data
        persistentVolumeClaim:
          claimName: vibe-ensemble-data
      restartPolicy: Always
```

**File**: `kubernetes/service.yaml`
```yaml
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
    name: http
  - port: 9090
    targetPort: 9090
    protocol: TCP
    name: metrics
  type: ClusterIP
```

**File**: `kubernetes/ingress.yaml`
```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: vibe-ensemble-ingress
  namespace: vibe-ensemble
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    nginx.ingress.kubernetes.io/websocket-services: vibe-ensemble-service
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
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

**File**: `kubernetes/pvc.yaml`
```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: vibe-ensemble-data
  namespace: vibe-ensemble
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
  storageClassName: fast-ssd
```

**Deployment Script**: `deploy-kubernetes.sh`
```bash
#!/bin/bash
set -e

NAMESPACE="vibe-ensemble"

echo "🚀 Deploying Vibe Ensemble to Kubernetes"

# Create namespace
kubectl apply -f kubernetes/namespace.yaml

# Create secrets (you need to update with actual values)
echo "⚠️  Please update kubernetes/secrets.yaml with actual base64-encoded values"
read -p "Press enter when secrets are configured..."
kubectl apply -f kubernetes/secrets.yaml

# Apply configuration
kubectl apply -f kubernetes/configmap.yaml
kubectl apply -f kubernetes/pvc.yaml
kubectl apply -f kubernetes/deployment.yaml
kubectl apply -f kubernetes/service.yaml
kubectl apply -f kubernetes/ingress.yaml

# Wait for deployment
echo "Waiting for deployment to be ready..."
kubectl rollout status deployment/vibe-ensemble-server -n $NAMESPACE

# Verify deployment
kubectl get pods -n $NAMESPACE
kubectl get services -n $NAMESPACE
kubectl get ingress -n $NAMESPACE

echo "✅ Deployment complete!"
echo "🌐 Check your ingress configuration for the public URL"
```

## Systemd Service Templates

### Production Systemd Service

**File**: `/etc/systemd/system/vibe-ensemble.service`
```ini
[Unit]
Description=Vibe Ensemble MCP Server
Documentation=https://github.com/siy/vibe-ensemble-mcp
After=network.target postgresql.service
Wants=postgresql.service
StartLimitIntervalSec=60
StartLimitBurst=3

[Service]
Type=exec
User=vibe-ensemble
Group=vibe-ensemble
WorkingDirectory=/opt/vibe-ensemble
ExecStart=/usr/local/bin/vibe-ensemble-server --config /etc/vibe-ensemble/config.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
RestartPreventExitStatus=255

# Environment variables (secrets should be in separate file)
EnvironmentFile=/etc/vibe-ensemble/environment
Environment=RUST_BACKTRACE=1
Environment=RUST_LOG=info

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/vibe-ensemble /var/log/vibe-ensemble
ProtectKernelTunables=true
ProtectKernelModules=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictSUIDSGID=true
LockPersonality=true
MemoryDenyWriteExecute=true

# Resource limits
LimitNOFILE=65535
LimitMEMLOCK=64
LimitCORE=0

[Install]
WantedBy=multi-user.target
```

**File**: `/etc/vibe-ensemble/environment`
```bash
# Database Configuration
DATABASE_URL=postgresql://vibe_user:secure_password@localhost:5432/vibe_ensemble

# Security Configuration  
JWT_SECRET=your-secure-jwt-secret-key-here
ENCRYPTION_KEY=your-32-char-encryption-key-here

# Performance Configuration
MAX_CONNECTIONS=1000
DATABASE_POOL_SIZE=20
WORKER_THREADS=4

# Feature Configuration
METRICS_ENABLED=true
ENABLE_API_DOCS=false
```

**Installation Script**: `install-systemd.sh`
```bash
#!/bin/bash
set -e

echo "🔧 Installing Vibe Ensemble as systemd service"

# Create user and directories
sudo useradd -r -d /opt/vibe-ensemble -s /bin/false vibe-ensemble
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

# Install service files
sudo cp vibe-ensemble.service /etc/systemd/system/
sudo cp environment /etc/vibe-ensemble/
sudo cp config.toml /etc/vibe-ensemble/

# Set secure permissions for secrets
sudo chown root:vibe-ensemble /etc/vibe-ensemble/environment
sudo chmod 640 /etc/vibe-ensemble/environment

# Enable and start service
sudo systemctl daemon-reload
sudo systemctl enable vibe-ensemble
sudo systemctl start vibe-ensemble

# Verify installation
sleep 5
if sudo systemctl is-active --quiet vibe-ensemble; then
    echo "✅ Vibe Ensemble service started successfully"
    sudo systemctl status vibe-ensemble
else
    echo "❌ Service failed to start - check logs"
    sudo journalctl -u vibe-ensemble -n 20
    exit 1
fi
```

## Cloud Provider Templates

### AWS ECS Fargate

**File**: `aws/task-definition.json`
```json
{
  "family": "vibe-ensemble",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "1024",
  "memory": "2048",
  "executionRoleArn": "arn:aws:iam::ACCOUNT:role/ecsTaskExecutionRole",
  "taskRoleArn": "arn:aws:iam::ACCOUNT:role/ecsTaskRole",
  "containerDefinitions": [
    {
      "name": "vibe-ensemble",
      "image": "ghcr.io/siy/vibe-ensemble-mcp:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "essential": true,
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/vibe-ensemble",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "environment": [
        {
          "name": "RUST_LOG",
          "value": "info"
        },
        {
          "name": "METRICS_ENABLED",
          "value": "true"
        }
      ],
      "secrets": [
        {
          "name": "DATABASE_URL",
          "valueFrom": "arn:aws:ssm:us-east-1:ACCOUNT:parameter/vibe-ensemble/database-url"
        },
        {
          "name": "JWT_SECRET",
          "valueFrom": "arn:aws:ssm:us-east-1:ACCOUNT:parameter/vibe-ensemble/jwt-secret"
        },
        {
          "name": "ENCRYPTION_KEY",
          "valueFrom": "arn:aws:ssm:us-east-1:ACCOUNT:parameter/vibe-ensemble/encryption-key"
        }
      ],
      "healthCheck": {
        "command": ["CMD-SHELL", "curl -f http://localhost:8080/api/health || exit 1"],
        "interval": 30,
        "timeout": 10,
        "retries": 3,
        "startPeriod": 60
      }
    }
  ]
}
```

### Google Cloud Run

**File**: `gcp/service.yaml`
```yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: vibe-ensemble
  annotations:
    run.googleapis.com/ingress: all
spec:
  template:
    metadata:
      annotations:
        autoscaling.knative.dev/minScale: "1"
        autoscaling.knative.dev/maxScale: "10"
        run.googleapis.com/cpu-throttling: "false"
    spec:
      containerConcurrency: 1000
      timeoutSeconds: 3600
      containers:
      - image: ghcr.io/siy/vibe-ensemble-mcp:latest
        ports:
        - containerPort: 8080
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
          limits:
            cpu: "2"
            memory: "4Gi"
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
```

## Monitoring and Observability

### Prometheus Configuration

**File**: `monitoring/prometheus.yml`
```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "vibe-ensemble-rules.yml"

scrape_configs:
  - job_name: 'vibe-ensemble'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 5s
    metrics_path: /metrics
    
alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

**File**: `monitoring/vibe-ensemble-rules.yml`
```yaml
groups:
- name: vibe-ensemble
  rules:
  - alert: VibeEnsembleDown
    expr: up{job="vibe-ensemble"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Vibe Ensemble server is down"
      
  - alert: HighErrorRate
    expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.1
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High error rate detected"
```

### Grafana Dashboard

**File**: `monitoring/grafana-dashboard.json`
```json
{
  "dashboard": {
    "id": null,
    "title": "Vibe Ensemble Dashboard",
    "description": "Monitoring dashboard for Vibe Ensemble MCP Server",
    "panels": [
      {
        "title": "Active Agents",
        "type": "stat",
        "targets": [
          {
            "expr": "vibe_ensemble_active_agents_total",
            "legendFormat": "Active Agents"
          }
        ]
      },
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "Requests/sec"
          }
        ]
      },
      {
        "title": "Response Times",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(http_request_duration_seconds_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ]
      }
    ]
  }
}
```

---

*These templates provide production-ready starting points for deploying Vibe Ensemble in various environments. Customize them according to your specific requirements and security policies.*