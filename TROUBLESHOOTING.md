# Troubleshooting Guide

## Server Startup Issues

### SQLite Database Permission Error

**Error:** `unable to open database file`
```
Error: Storage(Database(Database(SqliteError { code: 14, message: "unable to open database file" })))
```

**Cause:** The server cannot create or access the SQLite database file, usually due to:
- No write permissions in the current directory
- Missing parent directories in the database path
- Running from a system directory (like `/usr/local/bin`)
- Using an older version of the server with relative database paths

**Solutions:**

#### Option 1: Update to Latest Version (Recommended)
The latest version (v0.1.1+) automatically uses platform-appropriate directories:
- **macOS**: `~/Library/Application Support/vibe-ensemble/`
- **Linux**: `~/.local/share/vibe-ensemble/`
- **Windows**: `%APPDATA%\vibe-ensemble\`

Simply run the server from any directory:
```bash
vibe-ensemble-server
```

#### Option 2: Run from your home directory
```bash
cd ~
vibe-ensemble-server
```

#### Option 3: Create a dedicated directory
```bash
mkdir -p ~/.vibe-ensemble
cd ~/.vibe-ensemble
vibe-ensemble-server
```

#### Option 4: Specify custom database location
```bash
DATABASE_URL="sqlite://$HOME/.vibe-ensemble/data.db" vibe-ensemble-server
```

#### Option 5: Use in-memory database (temporary, data not persisted)
```bash
DATABASE_URL="sqlite::memory:" vibe-ensemble-server
```

**Note:** If you see this error only during server shutdown (while the server runs normally), it can be safely ignored - this is a known cleanup issue that doesn't affect server operation.

### Common Database Paths

| OS | Recommended Location |
|----|---------------------|
| macOS | `~/Library/Application Support/vibe-ensemble/` |
| Linux | `~/.local/share/vibe-ensemble/` |
| Windows | `%APPDATA%\vibe-ensemble\` |

### Configuration File

Create a config file to avoid setting environment variables each time:

**~/.vibe-ensemble/config.toml:**
```toml
[server]
host = "127.0.0.1"
port = 8080

[database]
url = "sqlite:///Users/yourusername/.vibe-ensemble/vibe-ensemble.db"
migrate_on_startup = true

[web]
enabled = true
host = "127.0.0.1"
port = 8081

[logging]
level = "info"
format = "pretty"
```

Then run:
```bash
vibe-ensemble-server --config ~/.vibe-ensemble/config.toml
```

## Port Already In Use

**Error:** `Address already in use (os error 48)`

**Solution:**
```bash
# Check what's using the ports
lsof -i :8080
lsof -i :8081

# Use different ports
SERVER_PORT=8082 WEB_PORT=8083 vibe-ensemble-server
```

## Permission Denied for Binary

**Error:** `Permission denied` when running `vibe-ensemble-server`

**Solution:**
```bash
chmod +x /usr/local/bin/vibe-ensemble-server
# or
sudo chmod +x /usr/local/bin/vibe-ensemble-server
```

## Binary Not Found

**Error:** `command not found: vibe-ensemble-server`

**Solutions:**
1. Add to PATH:
   ```bash
   export PATH="/usr/local/bin:$PATH"
   echo 'export PATH="/usr/local/bin:$PATH"' >> ~/.zshrc  # or ~/.bashrc
   ```

2. Run with full path:
   ```bash
   /usr/local/bin/vibe-ensemble-server
   ```

## Health Check

Verify the server is running:
```bash
curl http://127.0.0.1:8080/health
```

Expected response:
```json
{"status":"healthy","timestamp":"...","version":"0.1.1"}
```

## Web Dashboard

Open in browser:
- Dashboard: http://127.0.0.1:8081/dashboard
- Health: http://127.0.0.1:8080/health

## Getting Help

1. Check logs for detailed error messages
2. Verify file permissions in working directory
3. Ensure ports are available
4. Try running from home directory first

For more help, visit: https://github.com/siy/vibe-ensemble-mcp/issues