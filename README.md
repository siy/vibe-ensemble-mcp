# Vibe-Ensemble MCP Server

A multi-agent coordination system that enables Claude Code to manage specialized AI workers for complex development tasks.

> **⚠️ Security Warning**: In version 0.7.0, workers have full access to all tools and permissions. Use this system at your own risk. A comprehensive permission system is planned for the next release.

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

Vibe-Ensemble provides 22 MCP tools organized into four categories:

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
- `claim_ticket` - Claim a ticket for processing
- `close_ticket` - Mark a ticket as completed
- `create_ticket` - Create work tickets with execution plans
- `get_ticket` - Get detailed ticket information
- `list_tickets` - List tickets with filtering options
- `release_ticket` - Release a claimed ticket back to the queue
- `resume_ticket_processing` - Resume stalled or paused tickets
- `update_ticket_stage` - Update ticket to a specific stage

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

## What's New in v0.7.0

- **Project Rules & Patterns**: Define project-specific coding standards and conventions that workers automatically inherit
- **Enhanced Documentation**: Comprehensive workflow examples for development, debugging, testing, and DevOps
- **Improved Database Schema**: Better support for project metadata and worker coordination
- **Updated MCP Tools**: New project management capabilities with rules and patterns support

> **⚠️ Important Security Note**: Workers in this version have unrestricted access to all tools and system capabilities. Exercise caution when using this system, especially in production environments. A granular permission system is in active development for the next release.

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