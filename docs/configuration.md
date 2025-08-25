# Configuration

Simple configuration for single-user Vibe Ensemble setup with 5-10 agents.

## Environment Variables

Only a few variables needed for basic operation:

```bash
# Required: Database location (SQLite for single-user)
export DATABASE_URL="sqlite:./vibe-ensemble.db"

# Optional: Server configuration  
export SERVER_HOST="127.0.0.1"    # localhost only
export SERVER_PORT="8080"         # default port

# Optional: Logging
export RUST_LOG="info"             # or "debug" for troubleshooting
```

## Worker Settings Deployment

The system automatically deploys shared Claude Code settings to worker directories to ensure controlled permissions and MCP server connectivity.

### Shared Settings Template

Located at `agent-templates/shared/.claude/settings.json`, this template provides:

- **Restricted Permissions**: Workers have limited tool access compared to Claude Code defaults
- **MCP Server Integration**: Automatic connection to the Vibe Ensemble MCP server
- **Environment Variable Substitution**: Dynamic configuration based on workspace context

### Permission Differences: Worker vs Claude Code Default

| Category | Claude Code Default | Worker Settings |
|----------|-------------------|-----------------|
| File Operations | All files, any location | Restricted to workspace |
| Shell Access | Full system access | Curated commands only |
| Network Access | Unrestricted | Limited domains only |
| System Commands | All commands | Development tools only |

### Allowed Worker Permissions

**Core Tools**: Read, Write, Edit, MultiEdit, Glob, Grep, LS, TodoWrite

**Web Access**: Limited to documentation sites (docs.anthropic.com, github.com, docs.rs, crates.io, rust-lang.org)

**Shell Commands**: 
- Git operations (all git commands)
- Rust/Cargo development (build, test, check, fmt, clippy)  
- Common file operations (mkdir, rm, mv, cp, chmod)
- Development utilities (find, grep, rg, echo, cat, ls)

**Denied Operations**: 
- System admin commands (sudo, su)  
- Dangerous operations (rm -rf /, format, fdisk)
- Direct disk operations

### Environment Variable Substitution

The settings template supports these variables:

- `${VIBE_ENSEMBLE_MCP_SERVER:-ws://localhost:8080}` - MCP server URL
- `${VIBE_ENSEMBLE_MCP_BINARY:-vibe-ensemble --mcp-only --transport=stdio}` - MCP server command
- `${WORKSPACE_ID}` - Unique workspace identifier
- `${WORKSPACE_NAME}` - Human-readable workspace name  
- `${TEMPLATE_NAME}` - Agent template being used
- `${AGENT_ID:-${WORKSPACE_ID}}` - Agent identifier (defaults to workspace ID)
- `${VIBE_ENSEMBLE_LOG_LEVEL:-info}` - Logging level for MCP connection
- `${DATABASE_URL:-sqlite:./vibe-ensemble.db}` - Database connection string

### Automatic Deployment

Settings are deployed automatically when executing worker commands through the HeadlessClaudeExecutor:

1. **Pre-execution**: Template is processed and copied to `{workspace}/.claude/settings.json`
2. **Variable Substitution**: Environment variables are replaced with actual values
3. **Validation**: Generated JSON is validated for correctness
4. **Post-execution**: Settings file is cleaned up to prevent stale configurations

## Agent Template Configuration

Templates are in `agent-templates/` directory:

```
agent-templates/
├── code-writer/           # Feature implementation
│   ├── template.json      # Agent metadata and variables
│   ├── agent-config.md    # Handlebars template for config
│   └── prompts/          # Additional prompt templates
├── code-reviewer/         # Code quality and security
├── test-specialist/       # Testing focus
└── docs-specialist/       # Documentation
```

### Creating Custom Templates

1. **Copy existing template**:
   ```bash
   cp -r agent-templates/code-writer agent-templates/my-agent
   ```

2. **Edit `template.json`**:
   ```json
   {
     "name": "my-agent",
     "description": "My custom agent type",
     "variables": [
       {
         "name": "project_name",
         "description": "Project being worked on", 
         "variable_type": "String",
         "required": true
       }
     ],
     "capabilities": ["custom-task"],
     "tool_permissions": {
       "allowed_tools": ["Read", "Write", "Edit", "Bash"]
     }
   }
   ```

3. **Customize `agent-config.md`** with Handlebars templates:
   ```markdown
   # {{project_name}} Agent
   
   You are working on {{project_name}} with focus on {{capability}}.
   
   {{#if (eq primary_language "rust")}}
   - Use cargo for builds: `cargo build`
   - Run tests: `cargo test`
   {{/if}}
   ```

## Workspace Configuration

Agent workspaces are automatically created in `workspaces/` directory:

```bash
workspaces/
├── project-a-writer/      # Isolated workspace for each agent
├── project-a-reviewer/
├── project-b-writer/
└── shared-knowledge/      # Common knowledge base
```

Each workspace contains:
- `.claude/agents/agent.md` - Generated agent configuration
- `project/` - Working directory for the agent
- `workspace.json` - Workspace metadata

## Runtime Configuration

### Starting MCP Server
```bash
# Basic startup
vibe-ensemble --mcp-only --transport=stdio

# With custom port for web server
SERVER_PORT=8081 vibe-ensemble

# With debug logging  
RUST_LOG=debug vibe-ensemble --mcp-only --transport=stdio
```

### Agent Startup
```bash
# Using template-generated config
claude -p "$(cat workspaces/my-workspace/.claude/agents/agent.md)" \
  --mcp-server localhost:8080 \
  --working-directory workspaces/my-workspace/project

# Direct configuration
claude -p "You are a Rust developer for my-project" \
  --output-format stream-json \
  --verbose \
  --mcp-server localhost:8080
```

## Language-Specific Configuration

Built-in support for 11 languages in agent templates:
- rust, python, javascript, typescript
- java, csharp, go, php  
- c, cpp, sql

Each language includes:
- Appropriate build tools and commands
- Language-specific analysis patterns  
- Framework and ecosystem knowledge
- Security and performance guidelines

## Performance Tuning

For 5-10 agents:
```bash
# Increase file descriptor limits
ulimit -n 4096

# Monitor memory usage
ps aux | grep claude | awk '{sum+=$6} END {print sum/1024 " MB"}'

# Limit agents per project
# Recommended: 3-4 agents per active project
```

## File Locations

- **Database**: `./vibe-ensemble.db` (SQLite file)
- **Templates**: `./agent-templates/` (version controlled)
- **Workspaces**: `./workspaces/` (can be temporary)  
- **Logs**: Console output (use `tee` to save)

This keeps configuration simple and focused on the single-user, multi-agent use case.