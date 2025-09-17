# Vibe-Ensemble MCP Server

A multi-agent coordination system that enables Claude Code to manage specialized AI workers for complex development tasks.

## What It Does

Vibe-Ensemble allows you to break down complex projects into specialized stages, with each stage handled by a dedicated AI worker:

- **🎯 Smart Task Planning**: Automatically plan multi-stage workflows (architecture → development → testing → deployment)
- **🤖 Specialized Workers**: Create custom AI workers with specific expertise (Rust developer, security reviewer, UI designer, etc.)
- **📋 Automatic Progression**: Workers are auto-spawned by queues when needed and complete their stage, automatically handing off to the next worker
- **👀 Real-time Monitoring**: Track progress through tickets, comments, and live notifications
- **🔄 Adaptive Workflows**: Workers can dynamically update execution plans as they discover new requirements
- **💾 Persistent State**: All progress is saved, allowing you to pause and resume complex projects

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -fsSL https://get.vibeensemble.dev/install.sh | sh
```

**Windows:**
```powershell
iwr -useb https://get.vibeensemble.dev/install.ps1 | iex
```

### From Release

Download the latest release for your platform from the [releases page](https://github.com/siy/vibe-ensemble-mcp/releases).

### From Source

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

## Usage

### 1. Start the Server

```bash
# Default configuration (port 3000)
./target/release/vibe-ensemble-mcp

# Custom port
./target/release/vibe-ensemble-mcp --port 8080

# Custom database location
./target/release/vibe-ensemble-mcp --database-path ./my-project.db
```

### 2. Configure Claude Code Integration

**Automated Setup (Recommended):**
```bash
# Generate all necessary Claude Code configuration files
./target/release/vibe-ensemble-mcp --configure-claude-code --host 127.0.0.1 --port 3000
```

This command automatically creates:
- `.mcp.json` - MCP server configuration (HTTP + SSE transports)
- `.claude/settings.local.json` - Claude Code permissions
- `.claude/commands/vibe-ensemble.md` - Coordinator initialization command

**Manual Setup (Alternative):**
```bash
claude mcp add --transport http vibe-ensemble http://localhost:3000/mcp
```

### 3. Quick Start

1. **Setup**: Run `./vibe-ensemble-mcp --configure-claude-code --port 3000` to auto-configure Claude Code
2. **Start server**: `./vibe-ensemble-mcp` 
3. **Open Claude Code** in the configured directory
4. **Initialize**: Run `/vibe-ensemble` command to become the coordinator
5. **Create your first project** and define the work you want to accomplish

## Usage Workflow

Once you have Vibe-Ensemble configured and running with Claude Code, you can coordinate complex development tasks:

1. **Start as Coordinator**: Use the `/vibe-ensemble` command in Claude Code to initialize your coordinator session
2. **Define Your Project**: Create projects with specific rules, patterns, and worker types for your domain
3. **Create Workflows**: Set up tickets that define work stages and which specialized workers handle each stage
4. **Monitor Progress**: Workers automatically progress through stages, providing updates and handing off to the next worker
5. **Handle Issues**: Use coordination tools to resume stalled work or adjust workflows as needed

**WARNING:** Vibe-Ensemble is still work in progress. Some features may not be fully implemented or may have bugs. So, 
periodically ask Claude Code to check ticket status and event queue. Sometimes it may report issues, but not address them.
Sending prompt like "Act as a coordinator" usually helps.

**SECURITY WARNING:** The permission mechanisms are not entirely tested and may not work reliably. The only tested option is 'bypass', but you should use it at your own risk because it enables workers to do whatever they want. Use bypass mode only in isolated development environments.

### Example Project Types

**Web Application Development:**
- Workers: Architect, Frontend Developer, Backend Developer, Security Reviewer
- Stages: Architecture Design → Implementation → Security Review → Testing

**Documentation and DevOps:**
- Workers: Technical Writer, DevOps Engineer, QA Tester
- Stages: Documentation → CI/CD Setup → Deployment Testing

**Debugging and Performance:**
- Workers: Investigator, Performance Specialist, Code Reviewer
- Stages: Issue Analysis → Optimization → Validation

Each worker operates independently with their specialized knowledge, ensuring focused expertise at every stage while maintaining coordination across the entire workflow.

## Key Features

- **🚀 Zero Configuration**: Auto-setup with `--configure-claude-code`
- **🔄 Automatic Handoffs**: Workers complete stages and trigger next steps
- **📊 Real-time Updates**: Live progress tracking via Server-Sent Events
- **🎨 Custom Workers**: Define workers for any domain (coding, design, analysis, etc.)
- **💬 Detailed Reporting**: Every stage produces comprehensive progress reports
- **⚡ Robust Processing**: Handles failures gracefully with retry mechanisms
- **📋 Project Rules & Patterns**: Define coding standards and project conventions that workers automatically follow
- **🔧 Flexible Workflows**: Support for debugging, testing, documentation, and DevOps workflows

## MCP Tools

Vibe-Ensemble provides 19 MCP tools organized into four categories:

### Project Management
- `create_project` - Create a new project with rules and patterns
- `delete_project` - Delete an existing project
- `get_project` - Get project details by ID
- `list_projects` - List all projects
- `update_project` - Update project settings, rules, or patterns

### Worker Type Management
- `create_worker_type` - Define specialized worker types with custom system prompts
- `delete_worker_type` - Remove a worker type definition
- `get_worker_type` - Get worker type details and configuration
- `list_worker_types` - List all available worker types for a project
- `update_worker_type` - Modify worker type settings and prompts

### Ticket Management
- `add_ticket_comment` - Add progress comments to tickets
- `close_ticket` - Mark a ticket as completed
- `create_ticket` - Create work tickets with execution plans
- `get_ticket` - Get detailed ticket information
- `list_tickets` - List tickets with filtering options
- `resume_ticket_processing` - Resume stalled or paused tickets

### Event and Queue Management
- `get_tickets_by_stage` - Get all tickets currently at a specific stage
- `list_events` - List system events and notifications
- `resolve_event` - Mark system events as resolved

> **Note on Worker Management**: Workers are automatically spawned when tickets are assigned to stages. There are no explicit worker spawn/stop tools - the queue system handles worker lifecycle automatically based on workload.

## Requirements

- Rust 1.70+ (for building from source)
- SQLite (embedded, no separate installation needed)
- Claude Code (for worker processes)

## Configuration

The server accepts the following command-line options:

- `--configure-claude-code`: Generate Claude Code integration files and exit
- `--database-path`: SQLite database file path (default: `./vibe-ensemble.db`)
- `--host`: Server bind address (default: `127.0.0.1`)
- `--port`: Server port (default: `3000`)
- `--log-level`: Log level (default: `info`)
- `--permission-mode`: Permission mode for workers (default: `inherit`)
- `--no-respawn`: Disable automatic respawning of workers on startup

## Security & Permissions

Vibe-Ensemble includes a comprehensive permission system to control what tools and capabilities are available to AI workers. This is essential for security since workers run headless with access to system resources.

### Permission Modes

The server supports three permission modes controlled by the `--permission-mode` flag:

#### 1. **Bypass Mode** (`--permission-mode bypass`)
- **Use Case**: Development, testing, or when you need unrestricted access
- **Behavior**: Workers run with `--dangerously-skip-permissions` flag
- **Security Level**: ⚠️ **No restrictions** - workers have access to all tools and system capabilities
- **When to Use**: Only in trusted environments where full system access is acceptable

```bash
./vibe-ensemble-mcp --permission-mode bypass
```

#### 2. **Inherit Mode** (`--permission-mode inherit`) - **Default**
- **Use Case**: Production deployments where you want to reuse existing Claude Code permissions
- **Behavior**: Workers inherit permissions from your project's `.claude/settings.local.json` file
- **Security Level**: 🛡️ **Project-level control** - uses the same permissions as your interactive Claude Code session
- **When to Use**: When you want workers to have the same access level as your coordinator session

```bash
./vibe-ensemble-mcp --permission-mode inherit
# or simply (default)
./vibe-ensemble-mcp
```

**Required File**: `.claude/settings.local.json` in your project directory

#### 3. **File Mode** (`--permission-mode file`)
- **Use Case**: Custom worker-specific permissions different from your coordinator permissions
- **Behavior**: Workers use permissions from `.vibe-ensemble-mcp/worker-permissions.json`
- **Security Level**: 🔐 **Worker-specific control** - precisely control what workers can access
- **When to Use**: When you want fine-grained control over worker capabilities

```bash
./vibe-ensemble-mcp --permission-mode file
```

**Required File**: `.vibe-ensemble-mcp/worker-permissions.json` in your project directory

### Permission File Format

All permission modes use the same JSON structure that Claude Code uses internally:

```json
{
  "permissions": {
    "allow": [
      "Read",
      "Write",
      "Edit", 
      "MultiEdit",
      "Bash",
      "mcp__*"
    ],
    "deny": [
      "WebFetch",
      "WebSearch"
    ],
    "ask": [],
    "additionalDirectories": [
      "/home/user/safe-directory"
    ],
    "defaultMode": "acceptEdits"
  }
}
```

#### Permission Fields

- **`allow`**: Array of tools that workers can use without restriction
- **`deny`**: Array of tools that workers are prohibited from using  
- **`ask`**: Array of tools that require user confirmation (⚠️ ignored in headless worker mode)
- **`additionalDirectories`**: Additional directories workers can access beyond the project directory
- **`defaultMode`**: Default permission behavior (`"acceptEdits"` or `"rejectEdits"`)

#### Common Tool Names

- **File Operations**: `Read`, `Write`, `Edit`, `MultiEdit`, `Glob`, `Grep`
- **System Commands**: `Bash`
- **MCP Tools**: `mcp__*` (wildcard for all MCP server tools)
- **Web Access**: `WebFetch`, `WebSearch`
- **Version Control**: `git*` (if you want to restrict git operations)

### Setting Up Permissions

#### For Inherit Mode (Recommended)

1. **Auto-setup** (creates basic permissions):
   ```bash
   ./vibe-ensemble-mcp --configure-claude-code --port 3000
   ```

2. **Manual setup** - Create/edit `.claude/settings.local.json`:
   ```json
   {
     "permissions": {
       "allow": ["Read", "Write", "Edit", "Bash", "mcp__*"],
       "deny": ["WebFetch", "WebSearch"],
       "defaultMode": "acceptEdits"
     }
   }
   ```

#### For File Mode (Advanced)

Create `.vibe-ensemble-mcp/worker-permissions.json`. You can start with one of these examples:

**Balanced Permissions** (recommended starting point):
```bash
cp docs/example-worker-permissions.json .vibe-ensemble-mcp/worker-permissions.json
```

**Restrictive Permissions** (high security):
```bash
cp docs/example-restrictive-permissions.json .vibe-ensemble-mcp/worker-permissions.json
```

**Custom Configuration** - Create your own `.vibe-ensemble-mcp/worker-permissions.json`:

```json
{
  "permissions": {
    "allow": [
      "Read",
      "Write", 
      "Edit",
      "MultiEdit",
      "Bash:cargo*",
      "Bash:git*",
      "mcp__*"
    ],
    "deny": [
      "WebFetch",
      "WebSearch",
      "Bash:rm*",
      "Bash:sudo*"
    ],
    "additionalDirectories": [
      "./temp",
      "./build"
    ],
    "defaultMode": "acceptEdits"
  }
}
```

### Security Best Practices

1. **Start Restrictive**: Begin with minimal permissions and add tools as needed
2. **Use Inherit Mode**: In most cases, inherit mode provides the right balance of security and functionality
3. **Monitor Worker Activity**: Check logs in `.vibe-ensemble-mcp/logs/` to understand what tools workers are using
4. **Separate Environments**: Use bypass mode only in isolated development environments
5. **Regular Reviews**: Periodically review and update permission configurations

### Dynamic Permission Updates

Permission files are read fresh from disk each time a worker starts, allowing you to:
- Update permissions without restarting the server
- Adjust worker capabilities on-the-fly
- Test different permission configurations quickly

### Troubleshooting Permissions

**Worker fails to start**: Check that the required permission file exists and has valid JSON syntax

**Worker can't access needed tools**: Add the required tools to the `allow` array in your permissions file

**Security concerns**: Switch to `file` mode and create restrictive permissions tailored to your specific use case

**Permission file location issues**: Ensure files are in the correct location relative to your project directory:
- Inherit mode: `.claude/settings.local.json` 
- File mode: `.vibe-ensemble-mcp/worker-permissions.json`

## What's New in v0.9.0

- **🧠 Task Breakdown Sizing Methodology**: Intelligent task breakdown with optimal context-performance optimization (~120K token budget per stage)
- **📐 Natural Boundary Detection**: Automatic task splitting along technology, functional, and expertise boundaries
- **⚡ Enhanced Planning Workers**: Built-in token estimation and pipeline optimization with comprehensive validation
- **📊 Real-Time SSE Integration**: Full Server-Sent Events protocol for live progress monitoring and event streaming
- **🔧 Improved Worker Templates**: 8 specialized worker templates with task sizing methodology integration
- **📋 Enhanced Coordinator Prompts**: Updated coordination with systematic task delegation and sizing guidance
- **🛠️ Robust MCP Tools**: 22 MCP tools with enhanced project metadata and worker coordination
- **📚 Comprehensive Documentation**: Complete SSE protocol implementation and task breakdown sizing methodology
- **🔒 Enhanced Security**: Removed manual ticket manipulation tools to prevent pipeline stalls

## How It Works

```
    ┌─────────────────┐      ┌──────────────────┐      ┌─────────────┐
    │   Coordinator   │─────►│ Vibe-Ensemble    │─────►│  Workers    │
    │ (Claude Code)   │      │    Server        │      │ (Headless)  │
    │                 │      │                  │      │             │
    │ • Plans tasks   │      │ • Manages state  │      │ • Execute   │
    │ • Creates flows │      │ • Routes work    │      │ • Report    │ 
    │ • Monitors      │      │ • Coordinates    │      │ • Handoff   │
    └─────────────────┘      └──────────────────┘      └─────────────┘
```

**Key Benefits:**
- **No Context Drift**: Each worker focuses on one specific task
- **Parallel Processing**: Multiple workers can run simultaneously  
- **Persistent Progress**: All work is saved and can be resumed
- **Smart Coordination**: Automatic workflow progression based on completion

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Setting up the development environment
- Code style and testing requirements
- Submitting pull requests
- Reporting issues

## License

Apache 2.0