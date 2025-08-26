## 🔧 Vibe Ensemble MCP v0.2.2 - Database Configuration Fix

This patch release fixes critical database configuration issues discovered in production environments.

### 🐛 Bug Fixes

**Database Configuration Unification**
- ✅ **Fixed URL encoding issue** in `get_default_database_path()` - removed %20 encoding that caused SQLite connection failures
- ✅ **Streamlined configuration system** - unified all operational modes to use consistent configuration approach
- ✅ **Eliminated special-case code** - removed MCP-only bypass logic that caused configuration inconsistencies
- ✅ **Enhanced error handling** - improved database connection error messages and debugging information

### 🧪 Testing & Validation

- ✅ All 316 existing tests pass
- ✅ Added 6 new comprehensive configuration tests
- ✅ Validated all operational modes with default configuration:
  - Full mode: `vibe-ensemble`
  - MCP WebSocket: `vibe-ensemble --mcp-only --transport=websocket`  
  - MCP Stdio: `vibe-ensemble --mcp-only --transport=stdio`
  - Web-only: `vibe-ensemble --web-only`
  - API-only: `vibe-ensemble --api-only`

### 📦 Installation

Docker:
```bash
docker run -d --name vibe-ensemble -p 8080:8080 -p 8081:8081 ghcr.io/siy/vibe-ensemble-mcp:v0.2.2
```

**Full Changelog:** [v0.2.2 commits](https://github.com/siy/vibe-ensemble-mcp/commits/v0.2.2)

---

## 🎉 Vibe Ensemble MCP v0.2.1 - Production-Ready Release

The first stable release of Vibe Ensemble MCP Server is here! This comprehensive MCP server enables seamless coordination between multiple Claude Code instances with intelligent task distribution, real-time communication, and unified management.

### ✨ Key Features

🤖 **Intelligent Agent Coordination**
- AI-powered dependency detection and conflict resolution
- Automated escalation management for complex scenarios
- Cross-project coordination with specialist agents

⚡ **Distributed Task Execution** 
- Seamless work coordination across multiple Claude Code instances
- Proactive monitoring and load balancing
- Real-time progress tracking

🔗 **Cross-Project Integration**
- Advanced dependency management across project boundaries
- Pattern recognition and organizational learning
- Unified knowledge sharing

💬 **Real-time Communication**
- Sophisticated messaging with structured protocols
- Coordination-aware agent interactions
- Delivery confirmations and status updates

📋 **Issue Tracking & Knowledge Management**
- Persistent task and problem management
- Intelligent workflow automation
- Comprehensive knowledge repository with search capabilities

### 🎯 Production Features

✅ **Production Hardening**
- Configuration validation and security headers
- Performance logging and system monitoring
- Cross-platform builds (macOS, Linux, Windows)

🛡️ **Security & Monitoring**
- Real-time CPU, memory, and disk monitoring
- Request timing and slow query detection
- CSRF protection and content validation

📊 **Web Dashboard**
- Real-time system metrics and health monitoring
- Agent management and coordination oversight
- Interactive task and issue tracking

### 🚀 Installation

**Quick Install:**

macOS/Linux:
```bash
curl -fsSL https://vibeensemble.dev/install.sh | bash
```

Windows PowerShell:
```powershell
iex ((New-Object System.Net.WebClient).DownloadString('https://vibeensemble.dev/install.ps1'))
```

Docker:
```bash
docker run -d --name vibe-ensemble -p 8080:8080 -p 8081:8081 ghcr.io/siy/vibe-ensemble-mcp:v0.2.1
```

**Manual Installation:**
Download the appropriate binary for your platform below.

### 🔧 Claude Code Setup

After installation, configure Claude Code:

1. Start the server: `vibe-ensemble`
2. Add to Claude Code MCP settings:
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

3. Access interfaces:
   - Web Dashboard: http://127.0.0.1:8081
   - Health Check: http://127.0.0.1:8080/api/health

### 📈 Technical Achievements

- **316+ Tests Passing** - Comprehensive test coverage across all components
- **42+ MCP Tools** - Complete coordination toolset for multi-agent scenarios
- **6 Production Crates** - Modular architecture with clear separation of concerns
- **Zero-Downtime Deployment** - Production-ready with monitoring and recovery

### 🛠️ Available MCP Tools

**Agent Management:** `vibe/agent/register`, `vibe/agent/list`, `vibe/agent/message`
**Task Coordination:** `vibe/task/create`, `vibe/dependency/analyze`, `vibe/conflict/detect`
**Knowledge Management:** `vibe/knowledge/store`, `vibe/knowledge/search`, `vibe/guideline/enforce`

### 📚 Documentation

- [Installation Guide](https://github.com/siy/vibe-ensemble-mcp/blob/main/docs/installation.md)
- [Configuration Reference](https://github.com/siy/vibe-ensemble-mcp/blob/main/docs/configuration.md)
- [Security Best Practices](https://github.com/siy/vibe-ensemble-mcp/blob/main/docs/security-best-practices.md)
- [High-Level Design](https://github.com/siy/vibe-ensemble-mcp/blob/main/docs/high-level-design.md)

### 🔄 What's Next

Future releases will include additional cross-platform binaries, enhanced monitoring capabilities, and expanded coordination intelligence.

**Full Changelog:** [v0.2.1 commits](https://github.com/siy/vibe-ensemble-mcp/commits/v0.2.1)

---
Built with ❤️ using Rust and the Model Context Protocol
