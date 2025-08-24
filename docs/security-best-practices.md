# Security Best Practices

Guidelines for secure deployment of Vibe Ensemble MCP server for small user groups and single-user deployments.

## Network Security

### Interface Binding

**Development/Local Use**:
```toml
# config/local.toml
[server]
host = "127.0.0.1"  # Localhost only
port = 8080

[web]
host = "127.0.0.1"  # Dashboard localhost only
port = 8081
```

**Production with External Access**:
```toml
# config/production.toml
[server]
host = "0.0.0.0"    # All interfaces (requires firewall)
port = 8080

[web]
host = "0.0.0.0"    # Dashboard external access
port = 8081
```

### Firewall Configuration

**Ubuntu/Debian**:
```bash
# Allow specific ports only
sudo ufw allow from 192.168.1.0/24 to any port 8080
sudo ufw allow from 192.168.1.0/24 to any port 8081

# Or restrict to specific IPs
sudo ufw allow from 192.168.1.100 to any port 8080
sudo ufw enable
```

**CentOS/RHEL**:
```bash
# Configure firewalld
sudo firewall-cmd --permanent --add-rich-rule="rule family='ipv4' source address='192.168.1.0/24' port protocol='tcp' port='8080' accept"
sudo firewall-cmd --reload
```

**macOS**:
```bash
# Use built-in pfctl or third-party firewall
# Allow only local network access
```

## Database Security

### SQLite (Development)

```bash
# Secure file permissions
chmod 600 vibe_ensemble.db
chown $USER:$USER vibe_ensemble.db

# Place in protected directory
mkdir -p ~/.local/share/vibe-ensemble
export VIBE_ENSEMBLE_DATABASE_URL="sqlite:$HOME/.local/share/vibe-ensemble/db.sqlite"
```

### PostgreSQL (Production)

```toml
# config/production.toml
[database]
url = "postgres://vibe_user:${DATABASE_PASSWORD}@localhost:5432/vibe_ensemble?sslmode=require"
max_connections = 20
migrate_on_startup = false
```

```bash
# Create dedicated database user
sudo -u postgres createuser --no-superuser --no-createdb --no-createrole vibe_user
sudo -u postgres createdb vibe_ensemble --owner vibe_user

# Set secure password
sudo -u postgres psql -c "ALTER USER vibe_user PASSWORD 'SecureP@ssw0rd123';"

# Configure SSL
# In postgresql.conf:
ssl = on
ssl_cert_file = '/path/to/cert.pem'
ssl_key_file = '/path/to/key.pem'
```

## Environment Variables

### Sensitive Configuration

```bash
# Never commit these to git
export VIBE_ENSEMBLE_DATABASE_PASSWORD="your-secure-password"
export VIBE_ENSEMBLE_JWT_SECRET="your-jwt-secret-key"

# Use .env file for development (add to .gitignore)
cat > .env << EOF
DATABASE_PASSWORD=dev-password-123
JWT_SECRET=dev-secret-key
EOF
```

### Production Environment

```bash
# Use systemd environment file
sudo tee /etc/vibe-ensemble/env << EOF
VIBE_ENSEMBLE_DATABASE_PASSWORD=production-password
VIBE_ENSEMBLE_DATABASE_URL=postgres://vibe_user:\${DATABASE_PASSWORD}@localhost/vibe_ensemble
VIBE_ENSEMBLE_SERVER_HOST=0.0.0.0
EOF

# Secure the environment file
sudo chmod 600 /etc/vibe-ensemble/env
sudo chown root:root /etc/vibe-ensemble/env
```

## Process Security

### Running as Non-Root User

```bash
# Create dedicated user
sudo useradd --system --home /var/lib/vibe-ensemble --shell /bin/false vibe-ensemble

# Create working directory
sudo mkdir -p /var/lib/vibe-ensemble
sudo chown vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble

# Create systemd service
sudo tee /etc/systemd/system/vibe-ensemble.service << EOF
[Unit]
Description=Vibe Ensemble MCP Server
After=network.target

[Service]
Type=simple
User=vibe-ensemble
Group=vibe-ensemble
WorkingDirectory=/var/lib/vibe-ensemble
ExecStart=/usr/local/bin/vibe-ensemble-server
EnvironmentFile=/etc/vibe-ensemble/env
Restart=always
RestartSec=5
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/vibe-ensemble

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl enable vibe-ensemble
sudo systemctl start vibe-ensemble
```

### File Permissions

```bash
# Secure configuration files
chmod 600 config/*.toml
chown vibe-ensemble:vibe-ensemble config/*.toml

# Secure database files
chmod 600 vibe_ensemble.db*
chown vibe-ensemble:vibe-ensemble vibe_ensemble.db*

# Secure log files
mkdir -p /var/log/vibe-ensemble
chown vibe-ensemble:vibe-ensemble /var/log/vibe-ensemble
chmod 750 /var/log/vibe-ensemble
```

## Monitoring and Logging

### Log Security

```toml
# config/production.toml
[logging]
level = "info"          # Don't log debug info in production
format = "json"         # Structured logging for analysis
```

```bash
# Rotate logs to prevent disk fill
sudo tee /etc/logrotate.d/vibe-ensemble << EOF
/var/log/vibe-ensemble/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    copytruncate
    create 640 vibe-ensemble vibe-ensemble
}
EOF
```

### Security Monitoring

```bash
# Monitor failed authentication attempts (when implemented)
journalctl -u vibe-ensemble -f | grep -i "failed\|unauthorized\|forbidden"

# Monitor system resources
watch 'curl -s http://localhost:8080/status | jq .components'

# Set up alerts for critical issues
# (Configure with your monitoring system)
```

## Network Hardening

### Reverse Proxy (Recommended)

**Nginx Configuration**:
```nginx
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=dashboard:10m rate=5r/s;

    # API endpoints
    location /api/ {
        limit_req zone=api burst=20 nodelay;
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # Dashboard
    location / {
        limit_req zone=dashboard burst=10 nodelay;
        proxy_pass http://127.0.0.1:8081;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### SSL/TLS Certificate

```bash
# Using Let's Encrypt (certbot)
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d your-domain.com

# Or use self-signed for internal use
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes
```

## Backup and Recovery

### Database Backups

```bash
# SQLite backup
cp vibe_ensemble.db "backup-$(date +%Y%m%d).db"

# PostgreSQL backup
pg_dump vibe_ensemble > "backup-$(date +%Y%m%d).sql"

# Automated backups
crontab -e
# Add: 0 2 * * * /path/to/backup-script.sh
```

### Configuration Backups

```bash
# Backup configuration
tar -czf config-backup-$(date +%Y%m%d).tar.gz config/

# Store backups securely (encrypted)
gpg --cipher-algo AES256 --compress-algo 1 --s2k-digest-algo SHA512 --cert-digest-algo SHA512 --compress-level 3 -c config-backup.tar.gz
```

## Security Checklist

### Pre-Deployment

- [ ] Change all default passwords
- [ ] Configure proper interface binding
- [ ] Set up firewall rules
- [ ] Create dedicated user account
- [ ] Secure file permissions
- [ ] Configure log rotation
- [ ] Set up SSL/TLS certificates
- [ ] Test backup/restore procedures

### Post-Deployment

- [ ] Monitor logs for security events
- [ ] Regular security updates
- [ ] Review access logs weekly
- [ ] Test disaster recovery monthly
- [ ] Update certificates before expiry
- [ ] Monitor resource usage
- [ ] Review configuration quarterly

### Single-User Deployment

For single-user setups, you can simplify but should still:
- [ ] Use localhost binding (127.0.0.1)
- [ ] Set secure file permissions
- [ ] Use strong database passwords
- [ ] Keep system updated
- [ ] Regular backups

### Small Team Deployment

For small teams (2-10 users):
- [ ] Use dedicated server/VM
- [ ] Implement reverse proxy
- [ ] Set up proper SSL certificates
- [ ] Configure user-based access (when auth is implemented)
- [ ] Monitor usage patterns
- [ ] Document access procedures

Remember: Security is an ongoing process, not a one-time setup. Regularly review and update your security configuration.