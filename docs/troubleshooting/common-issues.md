# Troubleshooting Guide

This guide helps diagnose and resolve common issues with the Vibe Ensemble MCP Server. It provides systematic troubleshooting steps, common error messages, and solutions.

## Quick Diagnostic Steps

### System Health Check

Before troubleshooting specific issues, perform a basic system health check:

1. **Server Status Check**
   ```bash
   # Check if server is running
   curl -f http://localhost:8080/api/health
   
   # Check server logs
   sudo journalctl -u vibe-ensemble -f
   
   # Check process status
   systemctl status vibe-ensemble
   ```

2. **Database Connectivity**
   ```bash
   # Test database connection
   psql "$DATABASE_URL" -c "SELECT 1;"
   
   # Check database logs
   sudo journalctl -u postgresql -f
   ```

3. **Resource Usage**
   ```bash
   # Check memory usage
   top -p $(pgrep vibe-ensemble-server)
   
   # Check disk space
   df -h /var/lib/vibe-ensemble
   
   # Check network connectivity
   netstat -tulpn | grep 8080
   ```

## Server Startup Issues

### Server Won't Start

#### Symptoms
- Service fails to start
- Process exits immediately
- No response on configured port

#### Diagnostic Steps
1. **Check Configuration**
   ```bash
   # Validate configuration file
   vibe-ensemble-server --validate-config --config /etc/vibe-ensemble/config.toml
   
   # Check environment variables
   env | grep -E "(DATABASE_URL|JWT_SECRET|ENCRYPTION_KEY)"
   ```

2. **Check Logs**
   ```bash
   # View startup logs
   sudo journalctl -u vibe-ensemble -n 50
   
   # Check for specific errors
   sudo journalctl -u vibe-ensemble | grep -i error
   ```

3. **Test Database Connection**
   ```bash
   # Test database connectivity
   vibe-ensemble-server --test-db
   
   # Run migrations if needed
   vibe-ensemble-server --migrate
   ```

#### Common Solutions

**Missing Environment Variables**
```bash
# Error: "JWT_SECRET environment variable not set"
export JWT_SECRET="$(openssl rand -base64 32)"

# Error: "DATABASE_URL not found"
export DATABASE_URL="sqlite:///var/lib/vibe-ensemble/vibe-ensemble.db"
```

**Database Issues**
```bash
# Error: "database does not exist"
createdb vibe_ensemble

# Error: "relation does not exist"
vibe-ensemble-server --migrate
```

**Permission Issues**
```bash
# Error: "permission denied"
sudo chown -R vibe-ensemble:vibe-ensemble /var/lib/vibe-ensemble
sudo chmod 755 /var/lib/vibe-ensemble
```

**Port Already in Use**
```bash
# Find process using port 8080
sudo lsof -i :8080

# Kill the process or change port
export SERVER_PORT=8081
```

### Configuration Issues

#### Invalid Configuration Values

**JWT Secret Too Short**
```bash
# Error: "JWT secret must be at least 32 characters"
export JWT_SECRET="$(openssl rand -base64 32)"
```

**Invalid Database URL**
```bash
# Error: "invalid database URL"
# Correct format for PostgreSQL:
DATABASE_URL="postgresql://username:password@localhost:5432/database"

# Correct format for SQLite:
DATABASE_URL="sqlite:///absolute/path/to/database.db"
```

**File Permission Issues**
```bash
# Error: "cannot write to log file"
sudo mkdir -p /var/log/vibe-ensemble
sudo chown vibe-ensemble:vibe-ensemble /var/log/vibe-ensemble

# Error: "cannot read configuration file"
sudo chown root:vibe-ensemble /etc/vibe-ensemble/config.toml
sudo chmod 640 /etc/vibe-ensemble/config.toml
```

## Database Issues

### Connection Problems

#### Database Connection Timeouts

**Symptoms**
- "connection timeout" errors in logs
- Slow API responses
- Intermittent database errors

**Diagnostic Steps**
```bash
# Check database status
sudo systemctl status postgresql

# Monitor database connections
psql -c "SELECT count(*) as connections, state FROM pg_stat_activity GROUP BY state;"

# Check connection pool usage
curl http://localhost:8080/api/stats | jq '.database_connections'
```

**Solutions**
```bash
# Increase connection timeout
export DATABASE_CONNECT_TIMEOUT="10"

# Reduce connection pool size
export DATABASE_POOL_SIZE="10"

# Optimize PostgreSQL settings
sudo -u postgres psql -c "ALTER SYSTEM SET max_connections = 200;"
sudo systemctl restart postgresql
```

### Migration Issues

#### Migration Failures

**Symptoms**
- "migration failed" errors
- Database schema version mismatches
- Missing tables or columns

**Diagnostic Steps**
```bash
# Check migration status
vibe-ensemble-server --migration-status

# Verify database schema
psql "$DATABASE_URL" -c "\dt"

# Check migration history
psql "$DATABASE_URL" -c "SELECT * FROM _sqlx_migrations ORDER BY installed_on;"
```

**Solutions**
```bash
# Reset migrations (CAUTION: Data loss)
vibe-ensemble-server --reset-database

# Force specific migration
vibe-ensemble-server --migrate --force-version 5

# Manual migration repair
psql "$DATABASE_URL" -f vibe-ensemble-storage/migrations/001_initial.sql
```

### Performance Issues

#### Slow Database Queries

**Symptoms**
- High response times
- Database CPU usage
- Query timeouts

**Diagnostic Steps**
```bash
# Enable query logging (PostgreSQL)
sudo -u postgres psql -c "ALTER SYSTEM SET log_statement = 'all';"
sudo systemctl reload postgresql

# Monitor slow queries
sudo tail -f /var/log/postgresql/postgresql-*.log | grep "slow query"

# Check query performance
psql "$DATABASE_URL" -c "EXPLAIN ANALYZE SELECT * FROM agents;"
```

**Solutions**
```sql
-- Add missing indexes
CREATE INDEX idx_agents_status ON agents(status);
CREATE INDEX idx_issues_priority ON issues(priority);
CREATE INDEX idx_messages_created_at ON messages(created_at);

-- Update statistics
ANALYZE;

-- Vacuum tables
VACUUM ANALYZE agents;
VACUUM ANALYZE issues;
```

## Authentication and Security Issues

### JWT Token Issues

#### Invalid or Expired Tokens

**Symptoms**
- "invalid token" errors
- Frequent login prompts
- 401 Unauthorized responses

**Diagnostic Steps**
```bash
# Check JWT secret configuration
echo $JWT_SECRET | wc -c  # Should be 32+ characters

# Decode JWT token (without verification)
curl -s "https://jwt.io/" # Use online decoder

# Check token expiration settings
grep -i jwt_expiry /etc/vibe-ensemble/config.toml
```

**Solutions**
```bash
# Generate new JWT secret
export JWT_SECRET="$(openssl rand -base64 32)"

# Increase token lifetime
export JWT_EXPIRY_HOURS="48"

# Clear browser tokens
# Users need to logout and login again
```

#### Authentication Bypass Issues

**Symptoms**
- Unauthorized access to protected endpoints
- Security middleware not working
- Missing authentication headers

**Diagnostic Steps**
```bash
# Test protected endpoint
curl -H "Authorization: Bearer invalid-token" \
     http://localhost:8080/api/agents

# Check middleware configuration
grep -A 5 "auth_middleware" /etc/vibe-ensemble/config.toml
```

**Solutions**
```bash
# Verify authentication middleware is enabled
# In config.toml:
[security]
require_auth = true
auth_middleware_enabled = true

# Restart server after configuration changes
sudo systemctl restart vibe-ensemble
```

## Agent Communication Issues

### Agent Connection Problems

#### Agents Can't Connect

**Symptoms**
- Agents showing as "Disconnected"
- MCP protocol errors
- WebSocket connection failures

**Diagnostic Steps**
```bash
# Check WebSocket endpoint
wscat -c ws://localhost:8080/ws

# Monitor agent connections
sudo journalctl -u vibe-ensemble | grep -i "agent connect"

# Check network connectivity
telnet localhost 8080
```

**Solutions**
```bash
# Check firewall settings
sudo ufw allow 8080/tcp

# Verify WebSocket configuration
export WS_PING_INTERVAL="30"
export WS_PONG_TIMEOUT="10"

# Check reverse proxy WebSocket support (if using nginx)
# In nginx config:
proxy_http_version 1.1;
proxy_set_header Upgrade $http_upgrade;
proxy_set_header Connection "upgrade";
```

### Message Delivery Issues

#### Messages Not Received

**Symptoms**
- Agents not receiving task assignments
- Status updates not propagating
- WebSocket notifications missing

**Diagnostic Steps**
```bash
# Check message queue status
curl http://localhost:8080/api/stats | jq '.messages_count'

# Monitor message flow
sudo journalctl -u vibe-ensemble | grep -i "message"

# Test WebSocket directly
wscat -c ws://localhost:8080/ws
```

**Solutions**
```bash
# Increase message timeout
export MCP_TIMEOUT_SECONDS="60"

# Check message size limits
export MCP_MAX_MESSAGE_SIZE="2097152"  # 2MB

# Restart WebSocket connections
# Force agents to reconnect
sudo systemctl restart vibe-ensemble
```

## Web Interface Issues

### Page Load Problems

#### Slow or Failed Page Loads

**Symptoms**
- Web pages load slowly
- 502 Bad Gateway errors
- Static assets not loading

**Diagnostic Steps**
```bash
# Check web server logs
sudo journalctl -u vibe-ensemble | grep -i "web\|http"

# Test static asset serving
curl -I http://localhost:8080/static/css/style.css

# Monitor request handling
curl -w "%{time_total}\n" -o /dev/null -s http://localhost:8080/
```

**Solutions**
```bash
# Increase worker threads
export WORKER_THREADS="8"

# Enable HTTP/2 if using HTTPS
export HTTP2_ENABLED="true"

# Configure reverse proxy caching (nginx)
location /static/ {
    expires 1y;
    add_header Cache-Control "public, immutable";
}
```

### JavaScript Errors

#### WebSocket Connection Issues

**Symptoms**
- Real-time updates not working
- Console errors about WebSocket
- Stale dashboard data

**Diagnostic Steps**
1. Open browser developer tools (F12)
2. Check console for WebSocket errors
3. Monitor network tab for failed connections
4. Test WebSocket endpoint directly

**Solutions**
```javascript
// Check WebSocket URL in browser console
// Should connect to: ws://localhost:8080/ws

// Clear browser cache and reload
// Or use incognito/private browsing mode
```

## Performance Issues

### High CPU Usage

#### Server Process Using Excessive CPU

**Symptoms**
- High CPU usage by vibe-ensemble-server
- Slow response times
- System becomes unresponsive

**Diagnostic Steps**
```bash
# Monitor CPU usage
top -p $(pgrep vibe-ensemble-server)

# Profile the application
perf record -g -p $(pgrep vibe-ensemble-server)
perf report

# Check for infinite loops in logs
sudo journalctl -u vibe-ensemble | tail -100
```

**Solutions**
```bash
# Reduce worker threads if too high
export WORKER_THREADS="4"

# Limit concurrent connections
export MAX_CONNECTIONS="500"

# Enable request rate limiting
export RATE_LIMITING_ENABLED="true"
export RATE_LIMIT_REQUESTS_PER_HOUR="1000"

# Check for problematic agents
# Look for agents sending excessive messages
```

### Memory Issues

#### Memory Leaks or High Memory Usage

**Symptoms**
- Increasing memory usage over time
- Out of memory errors
- System swapping

**Diagnostic Steps**
```bash
# Monitor memory usage
watch "cat /proc/$(pgrep vibe-ensemble-server)/status | grep -E 'VmSize|VmRSS'"

# Check for memory leaks
valgrind --leak-check=full vibe-ensemble-server

# Monitor database connection pool
curl http://localhost:8080/api/stats | jq '.database_pool_usage'
```

**Solutions**
```bash
# Reduce database connection pool
export DATABASE_POOL_SIZE="10"

# Limit concurrent connections
export MAX_CONNECTIONS="200"

# Enable garbage collection optimization
export RUST_BACKTRACE="1"

# Restart service periodically (temporary fix)
# Add to cron: 0 3 * * * systemctl restart vibe-ensemble
```

## Network Issues

### Connectivity Problems

#### Can't Access Server

**Symptoms**
- Connection refused errors
- Timeouts when accessing web interface
- API requests failing

**Diagnostic Steps**
```bash
# Check if server is listening
sudo netstat -tulpn | grep 8080

# Test local connectivity
curl -I http://localhost:8080/api/health

# Test from external host
curl -I http://your-server-ip:8080/api/health

# Check firewall rules
sudo ufw status
sudo iptables -L
```

**Solutions**
```bash
# Check server bind address
export SERVER_HOST="0.0.0.0"  # Listen on all interfaces

# Open firewall port
sudo ufw allow 8080/tcp

# Check SELinux (if applicable)
sudo setsebool -P httpd_can_network_connect 1

# Verify DNS resolution
nslookup your-domain.com
```

### SSL/TLS Issues

#### Certificate Problems

**Symptoms**
- SSL certificate errors
- "Insecure connection" warnings
- Failed HTTPS connections

**Diagnostic Steps**
```bash
# Check certificate validity
openssl s_client -connect your-domain.com:443

# Verify certificate chain
curl -vI https://your-domain.com

# Check nginx SSL configuration
sudo nginx -t
```

**Solutions**
```bash
# Renew Let's Encrypt certificate
sudo certbot renew

# Check certificate files permissions
sudo chmod 644 /etc/letsencrypt/live/your-domain.com/fullchain.pem
sudo chmod 600 /etc/letsencrypt/live/your-domain.com/privkey.pem

# Update nginx SSL configuration
ssl_protocols TLSv1.2 TLSv1.3;
ssl_ciphers ECDHE-RSA-AES128-GCM-SHA256:ECDHE-RSA-AES256-GCM-SHA384;
```

## Diagnostic Tools

### Log Analysis

#### Structured Log Queries

```bash
# Filter logs by level
sudo journalctl -u vibe-ensemble | grep -i "error\|warn"

# Search for specific patterns
sudo journalctl -u vibe-ensemble | grep "agent.*disconnect"

# Follow logs in real-time
sudo journalctl -u vibe-ensemble -f

# Export logs for analysis
sudo journalctl -u vibe-ensemble --since "1 hour ago" > /tmp/vibe-ensemble.log
```

#### Log Analysis Tools

```bash
# Install log analysis tools
sudo apt install goaccess  # Web log analyzer
sudo apt install jq       # JSON processor

# Analyze JSON logs
cat /var/log/vibe-ensemble/server.log | jq '.level' | sort | uniq -c

# Generate web access report
goaccess /var/log/nginx/access.log -o /tmp/report.html --log-format=COMBINED
```

### Performance Monitoring

#### System Metrics

```bash
# CPU and memory monitoring
htop

# I/O monitoring  
iotop

# Network monitoring
iftop

# Database monitoring
pg_top  # PostgreSQL
```

#### Application Metrics

```bash
# Get system statistics
curl http://localhost:8080/api/stats

# Prometheus metrics (if enabled)
curl http://localhost:9090/metrics

# Health check with details
curl -v http://localhost:8080/api/health
```

### Debugging Commands

#### Service Debugging

```bash
# Check service dependencies
systemctl list-dependencies vibe-ensemble

# View service environment
systemctl show vibe-ensemble --property=Environment

# Run server in foreground for debugging
sudo -u vibe-ensemble vibe-ensemble-server --config /etc/vibe-ensemble/config.toml

# Enable debug logging temporarily
sudo systemctl edit vibe-ensemble
# Add: Environment=RUST_LOG=debug
```

## Getting Additional Help

### Information to Collect

When seeking help, please provide:

1. **System Information**
   ```bash
   uname -a
   cat /etc/os-release
   vibe-ensemble-server --version
   ```

2. **Configuration**
   ```bash
   vibe-ensemble-server --show-config
   ```

3. **Logs**
   ```bash
   sudo journalctl -u vibe-ensemble -n 100 > logs.txt
   ```

4. **Error Details**
   - Exact error messages
   - Steps to reproduce
   - When the issue started
   - What changed recently

### Support Channels

- **GitHub Issues**: For bugs and feature requests
- **GitHub Discussions**: For questions and community support
- **Documentation**: Check FAQ and troubleshooting guides

---

*For advanced troubleshooting scenarios and performance tuning, see the [Performance Guide](performance.md) and [Debug Guide](debugging.md).*