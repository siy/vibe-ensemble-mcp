# Installation Guide

This guide will help you install Vibe Ensemble on your system. Choose the method that works best for you.

## System Requirements

- **Operating System**: macOS 10.15+, Linux (Ubuntu 20.04+), Windows 10+
- **Memory**: 256 MB RAM minimum (512 MB recommended)
- **Storage**: 100 MB free space
- **Network**: Internet connection for installation only

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
3. Create the data directory at `~/.vibe-ensemble/`
4. Verify the installation

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
vibe-ensemble 0.4.0
```

## First Run

Start Vibe Ensemble for the first time:

```bash
vibe-ensemble
```

This will:
- Create the database at `~/.vibe-ensemble/data.db`
- Start the web server on http://127.0.0.1:8080
- Print startup information

You should see:
```
🚀 Vibe Ensemble started successfully
📊 Web dashboard: http://127.0.0.1:8080
💾 Database: (see Data Directory section)
  - macOS: ~/Library/Application Support/vibe-ensemble/data.db
  - Linux: ~/.local/share/vibe-ensemble/data.db
  - Windows: %APPDATA%/vibe-ensemble/data.db
🔧 Configuration: Default settings
```

## Connect to Claude Code

### Option 1: Claude Code Settings UI

1. Open Claude Code
2. Go to Settings (Cmd/Ctrl + ,)
3. Navigate to "MCP Servers"
4. Click "Add Server"
5. Enter:
   - **Name**: `vibe-ensemble`
   - **Command**: `vibe-ensemble --mcp-only --transport=stdio`

### Option 2: Configuration File

Add to your Claude Code MCP configuration file:

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

The configuration file is typically located at:
- **macOS**: `~/Library/Application Support/Claude Code/mcp_settings.json`
- **Linux**: `~/.config/claude-code/mcp_settings.json`
- **Windows**: `%APPDATA%/Claude Code/mcp_settings.json`

## Configuration

Vibe Ensemble works with zero configuration, but you can customize it:

### Command Line Options

```bash
# Run on different port
vibe-ensemble --port=9000

# Disable web dashboard
vibe-ensemble --mcp-only --transport=stdio

# Use custom database location
DATABASE_URL="sqlite://./my-project.db" vibe-ensemble
```

### Environment Variables

```bash
# Database location (file path)
export DATABASE_URL="sqlite:///path/to/my-database.db"

# In-memory database
export DATABASE_URL="sqlite::memory:"

# Server port
export VIBE_ENSEMBLE_PORT=9000

# Log level
export RUST_LOG=info
```

## Data Directory

Vibe Ensemble stores its data in:
- **macOS**: `~/Library/Application Support/vibe-ensemble/`
- **Linux**: `~/.local/share/vibe-ensemble/`
- **Windows**: `%APPDATA%/vibe-ensemble/`

This directory contains:
- `data.db` - SQLite database with agents, issues, and knowledge
- `logs/` - Application logs (if file logging is enabled)

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

### Port Already in Use

If you see "Address already in use" error:

```bash
# Find what's using port 8080
lsof -i :8080  # macOS/Linux
netstat -ano | findstr :8080  # Windows

# Use a different port
vibe-ensemble --port=8081
```

### Permission Denied

**macOS/Linux:**
```bash
# Fix binary permissions
chmod +x /usr/local/bin/vibe-ensemble

# Fix data directory permissions  
chmod -R 755 ~/.local/share/vibe-ensemble/
```

**Windows:**
- Run Command Prompt as Administrator
- Ensure the binary is in a writable location

### Database Issues

```bash
# Check database permissions
ls -la ~/.local/share/vibe-ensemble/data.db

# Reset database (⚠️ this deletes all data)
rm ~/.local/share/vibe-ensemble/data.db
vibe-ensemble  # Will recreate empty database
```

### Connection Issues with Claude Code

1. Verify Vibe Ensemble is running:
   ```bash
   curl http://127.0.0.1:8080/api/health
   ```

2. Check Claude Code MCP configuration
3. Restart Claude Code after adding the MCP server

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