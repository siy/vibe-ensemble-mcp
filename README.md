# Vibe-Ensemble MCP Server

A **highly customizable** multi-agent coordination system that enables Claude Code to manage specialized AI workers for complex development tasks.

This architecture addresses context drift and focus dilution in complex projects by breaking
them down into specialized stages, allowing workers to focus on specific tasks. The high-level planning is left to the
coordinator, which serves as a single point of control. **Worker templates are fully customizable** to adapt to your team's methodologies, coding standards, and specific requirements.  

## What It Does

Vibe-Ensemble allows you to break down complex projects into specialized stages, with each stage handled by a dedicated AI worker:

- **🎯 Smart Task Planning**: Automatically plan multi-stage workflows (architecture → development → testing → deployment)
- **🤖 Specialized Workers**: Create custom AI workers with specific expertise (Rust developer, security reviewer, UI designer, etc.)
- **📋 Automatic Progression**: Workers are auto-spawned by queues when needed and complete their stage, automatically handing off to the next worker
- **👀 Real-time Monitoring**: Track progress through tickets, comments, and live notifications
- **🔄 Adaptive Workflows**: Workers can dynamically update execution plans as they discover new requirements
- **💾 Persistent State**: All progress is saved, allowing you to pause and resume complex projects
- **🎨 Live Customization**: Edit worker templates in real-time to adapt to your team's processes and coding standards
- **🌐 Bidirectional Communication**: Full WebSocket support for real-time coordination with connected Claude Code clients
- **🔗 Multi-Client Orchestration**: Coordinate work across multiple specialized Claude Code instances simultaneously

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -fsSL https://www.vibeensemble.dev/install.sh | sh
```

**Windows:**
```powershell
iwr -useb https://www.vibeensemble.dev/install.ps1 | iex
```

### From Release

Download the latest release for your platform from the [releases page](https://github.com/siy/vibe-ensemble-mcp/releases).

### From Source

```bash
git clone https://github.com/siy/vibe-ensemble-mcp.git
cd vibe-ensemble-mcp
cargo build --release
```

## Setup Steps

### Step 1: Start the Server (separate directory recommended)

```bash
# Create and start server in separate directory
mkdir vibe-server && cd vibe-server && vibe-ensemble-mcp
```

### Step 2: Configure Claude Code (separate coordinator directory)

```bash
# Create coordinator directory and configure Claude Code
mkdir vibe-coordinator && cd vibe-coordinator && vibe-ensemble-mcp --configure-claude-code
```

This command automatically creates:
- `.mcp.json` - MCP server configuration (HTTP + SSE + WebSocket transports)
- `.claude/settings.local.json` - Claude Code permissions
- `.claude/commands/vibe-ensemble.md` - Coordinator initialization command
- `.claude/websocket-token` - Authentication token for WebSocket connections

### Step 3: Start Claude Code and Initialize

1. **Open Claude Code** in the coordinator directory
2. **Run the command**: `/vibe-ensemble` to initialize as coordinator
3. **Create your first project** and define the work you want to accomplish

> **Note**: Bidirectional WebSocket communication is experimental and may not always work reliably. This feature isn't documented in Claude Code, so we're doing our best to make it as convenient as possible.

## Intended Usage Workflow

Once the server is running and Claude Code is configured, here's the typical workflow:

1. **Start Claude Code**: Open Claude Code in your coordinator directory and run the `/vibe-ensemble` command to initialize as a coordinator
2. **Create Project**: Write a prompt describing your intended project and answer the coordinator's questions about scope and requirements
3. **Monitor Progress**: Periodically prompt Claude Code with:
   - **"address events"** - Handle any issues or escalations
   - **"report project state"** - Get overall progress updates

The coordinator will break down your project into tickets, spawn appropriate workers for each stage, and manage the workflow automatically.

**WARNING:** Vibe-Ensemble is still a work in progress. Some features may not be fully implemented or may have bugs. So, 
periodically ask Claude Code to check ticket status and event queue. Sometimes it may report issues, but not address them.
Sending prompt like "Act as a coordinator" usually helps.

**SECURITY WARNING:** Always review and test permission configurations before production use. While the permission system is designed to be secure, proper configuration is essential. Use 'bypass' mode only in isolated development environments as it grants unrestricted access. For production use, the default 'file' mode provides explicit permission control.

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

Vibe-Ensemble provides 47 MCP tools organized into ten categories:

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

### Permission Management
- `get_permission_model` - Get information about the current permission model and configuration

### Dependency Management
- `add_ticket_dependency` - Add dependencies between tickets to control execution order
- `remove_ticket_dependency` - Remove ticket dependencies
- `get_dependency_graph` - Visualize ticket dependencies and execution order
- `list_ready_tickets` - List tickets ready for execution (dependencies satisfied)
- `list_blocked_tickets` - List tickets blocked by pending dependencies

### WebSocket Client Management (Bidirectional Communication)
- `list_connected_clients` - View all connected Claude Code instances with their capabilities
- `list_client_tools` - Discover tools available on connected clients
- `client_health_monitor` - Monitor connection status and client health metrics
- `client_group_manager` - Organize clients into logical groups for targeted operations

### Bidirectional Tool Execution
- `call_client_tool` - Execute tools on specific connected Claude Code clients
- `list_pending_requests` - Track ongoing client tool calls and their status
- `parallel_call` - Execute the same tool across multiple clients simultaneously
- `broadcast_to_clients` - Send notifications or commands to all connected clients

### Workflow Orchestration
- `execute_workflow` - Coordinate complex multi-step workflows across clients
- `collaborative_sync` - Synchronize state and data between coordinator and clients
- `poll_client_status` - Get real-time status updates from specific clients


### Integration Testing
- `validate_websocket_integration` - Comprehensive WebSocket functionality validation
- `test_websocket_compatibility` - Test compatibility with different MCP client types

> **Note on Worker Management**: Workers are automatically spawned when tickets are assigned to stages. There are no explicit worker spawn/stop tools - the queue system handles worker lifecycle automatically based on workload.

> **Note on Bidirectional Communication**: WebSocket tools enable real-time coordination with connected Claude Code clients, allowing for distributed task execution and multi-client workflows. This is particularly useful for complex projects requiring specialized environments or parallel processing capabilities.

## Requirements

- Rust 1.70+ (for building from source)
- SQLite (embedded, no separate installation needed)
- Claude Code (for worker processes)

## Configuration

The server accepts the following command-line options:

- `--configure-claude-code`: Generate Claude Code integration files and exit
- `--database-path`: SQLite database file path (default: `./.vibe-ensemble-mcp/vibe-ensemble.db`)
- `--host`: Server bind address (default: `127.0.0.1`)
- `--port`: Server port (default: `3000`)
- `--log-level`: Log level (default: `info`)
- `--permission-mode`: Permission mode for workers (default: `file`)
- `--no-respawn`: Disable automatic respawning of workers on startup
- `--enable-websocket`: Enable WebSocket transport for bidirectional communication (default: `true`)
- `--websocket-auth-required`: Require authentication for WebSocket connections (default: `false`)
- `--client-tool-timeout-secs`: Timeout for client tool calls in seconds (default: `30`)
- `--max-concurrent-client-requests`: Maximum concurrent client requests (default: `50`)

## Permission System

Vibe-Ensemble supports flexible permission modes to control worker access to tools and resources. Workers use project-specific permissions for security and isolation.

### Permission Modes

The server supports three permission modes controlled by the `--permission-mode` flag:

#### 1. **File Mode** (`--permission-mode file`) - **Default**
- **Use Case**: Project-specific permissions with comprehensive defaults
- **Behavior**: Workers use permissions from `.vibe-ensemble-mcp/worker-permissions.json`
- **Security Level**: 🔐 **Project-specific control** - each project gets its own permissions
- **Auto-Generated**: New projects automatically get comprehensive default permissions including all MCP tools and essential Claude Code tools (Read, Write, Edit, Bash, etc.)

```bash
vibe-ensemble-mcp --permission-mode file
# or simply (default)
vibe-ensemble-mcp
```


#### 2. **Bypass Mode** (`--permission-mode bypass`)
- **Use Case**: Development, testing, or when you need unrestricted access
- **Behavior**: Workers run with `--dangerously-skip-permissions` flag
- **Security Level**: ⚠️ **No restrictions** - workers have access to all tools and system capabilities
- **When to Use**: Only in trusted environments where full system access is acceptable

```bash
vibe-ensemble-mcp --permission-mode bypass
```

To change permission mode, start the server with: `vibe-ensemble-mcp --permission-mode [file|bypass]`

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
2. **Use File Mode**: The default file mode provides explicit control over worker permissions
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

**Permission file location issues**: Ensure the file is in the correct location relative to your project directory:
- File mode: `.vibe-ensemble-mcp/worker-permissions.json`

## Customization

### Worker Templates

Vibe-Ensemble provides **8 high-quality, customizable worker templates** that define specialized AI workers for different stages of development. Templates are designed for **easy customization** while maintaining system compatibility.

#### 📁 Template Locations & Runtime Behavior

**Templates are distributed to coordinators** during setup and loaded **dynamically at runtime**:

1. **Server Embedded**: Templates are embedded in the server binary as defaults
2. **Coordinator Distribution**: `--configure-claude-code` creates templates in `.claude/worker-templates/`
3. **Runtime Loading**: Server loads templates from coordinator's `.claude/worker-templates/` directory
4. **Live Customization**: Edit templates on disk → changes take effect immediately
5. **Automatic Fallback**: Missing templates automatically recreated from embedded versions


#### 📋 Available Worker Templates

- **`planning.md`** - Strategic planning, requirements analysis, pipeline design
- **`design.md`** - UI/UX design, system architecture, technical specifications
- **`implementation.md`** - Code development, feature implementation, integration
- **`testing.md`** - Quality assurance, test writing, validation strategies
- **`review.md`** - Code review, documentation review, quality checks
- **`deployment.md`** - DevOps, infrastructure, deployment automation
- **`research.md`** - Investigation, exploration, technology evaluation
- **`documentation.md`** - Technical writing, API docs, user guides

#### 🎨 **Customization Guidelines**

**✅ Encouraged Customizations:**
- Add project-specific context and requirements
- Customize methodologies and approaches
- Add domain-specific guidance and best practices
- Modify tone and communication style
- Include company/team-specific processes

**⚠️ Important: Preserve System Integration**
When customizing templates, **DO NOT** modify elements marked as **important for system functionality**:
- JSON output format specifications
- Required output fields (`outcome`, `target_stage`, `comment`, `reason`)
- Stage coordination instructions
- Pipeline extension mechanisms
- Token budget guidelines
- Worker type creation instructions

**🔧 Safe Customization Pattern:**
1. Copy the original template as backup
2. Modify content sections while preserving system directives
3. Test with simple tickets to ensure proper JSON output
4. Monitor worker behavior for correct stage transitions

**💡 Pro Tips:**
- Templates are loaded fresh each time → instant customization
- Use `load_worker_template` to preview changes before creating worker types
- Different projects can have different template customizations
- Share successful customizations across your organization

#### 🏗️ Template Architecture

Each template includes:
- **Role Definition**: Clear worker specialization and responsibilities
- **System Integration**: Proper JSON output format and coordination protocols
- **Methodology Guidance**: Stage-specific approaches and best practices
- **Quality Standards**: Output requirements and validation criteria
- **Coordination Instructions**: Pipeline extension and worker type creation guidance

Templates are designed to be **both powerful out-of-the-box and highly customizable** for specific project needs.

## What's New in v0.9.3

- **🔧 Enhanced WebSocket Protocol Compliance**: Fixed MCP subprotocol validation for proper Claude Code IDE integration
- **🐛 Bug Fixes**: Various stability improvements and issue resolutions
- **📚 Documentation Updates**: Improved protocol compliance documentation

## What's New in v0.9.0

- **🧠 Task Breakdown Sizing Methodology**: Intelligent task breakdown with optimal context-performance optimization (~120K token budget per stage)
- **📐 Natural Boundary Detection**: Automatic task splitting along technology, functional, and expertise boundaries
- **⚡ Enhanced Planning Workers**: Built-in token estimation and pipeline optimization with comprehensive validation
- **📊 Real-Time SSE Integration**: Full Server-Sent Events protocol for live progress monitoring and event streaming
- **🔧 Enhanced Worker Templates**: 8 highly customizable worker templates with live editing, runtime loading, and safe customization guidelines
- **📋 Enhanced Coordinator Prompts**: Updated coordination with systematic task delegation and sizing guidance
- **🛠️ Robust MCP Tools**: 47 MCP tools with enhanced project metadata and worker coordination
- **📚 Comprehensive Documentation**: Complete SSE protocol implementation and task breakdown sizing methodology
- **🔒 Enhanced Security**: Removed manual ticket manipulation tools to prevent pipeline stalls

## Bidirectional WebSocket Communication

Vibe-Ensemble v0.9.1+ introduces **full bidirectional WebSocket communication** with Claude Code clients, enabling advanced multi-client coordination and real-time collaboration.

### Key Capabilities

**🔗 Real-time Client Coordination:**
- Connect multiple Claude Code instances as specialized clients
- Server can initiate tool calls on connected clients
- Clients can register their own tools for server use
- Bi-directional JSON-RPC 2.0 over WebSocket protocol

**🚀 Advanced Workflow Patterns:**
- **Distributed Task Execution**: Delegate specialized tasks to clients with specific capabilities
- **Parallel Processing**: Execute tasks across multiple client environments simultaneously
- **Multi-Environment Development**: Coordinate across different OS, tools, or configuration setups
- **Expert Specialization**: Route tasks to clients with domain-specific expertise

**🛠️ Integration Features:**
- **Authentication**: Secure token-based authentication for WebSocket connections
- **Health Monitoring**: Real-time monitoring of client connections and capabilities
- **Group Management**: Organize clients into logical groups for targeted operations
- **Workflow Orchestration**: Complex multi-step workflows spanning multiple clients

### Getting Started with WebSocket

1. **Configure with WebSocket Support**:
   ```bash
   ./vibe-ensemble-mcp --configure-claude-code --host 127.0.0.1 --port 3000
   ```
   This automatically generates WebSocket authentication tokens and configuration.

2. **Start Server with WebSocket Enabled** (default):
   ```bash
   ./vibe-ensemble-mcp --port 3000
   ```

3. **Connect Claude Code Clients**:
   - Use the generated `.mcp.json` configuration which includes WebSocket transport
   - Clients authenticate using the generated `.claude/websocket-token`
   - Multiple clients can connect simultaneously for distributed coordination

4. **Use Bidirectional Tools**:
   - `list_connected_clients` - See available client environments
   - `call_client_tool` - Execute tools on specific clients
   - `parallel_call` - Execute across multiple clients simultaneously
   - `collaborative_sync` - Synchronize state across clients

### Use Cases

**Multi-Platform Development:**
- Windows client for Windows-specific testing
- Linux client for deployment and Docker operations
- macOS client for iOS-related development tasks

**Specialized Environments:**
- Client with specialized security analysis tools
- Client with access to cloud infrastructure
- Client with specific development environment setup

**Large-Scale Operations:**
- Distributed code analysis across multiple instances
- Parallel testing across different environments
- Multi-region deployment coordination

## How It Works

```
    ┌─────────────────┐      ┌──────────────────┐      ┌─────────────┐
    │   Coordinator   │◄────►│ Vibe-Ensemble    │─────►│  Workers    │
    │ (Claude Code)   │      │    Server        │      │ (Headless)  │
    │                 │      │                  │      │             │
    │ • Plans tasks   │      │ • Manages state  │      │ • Execute   │
    │ • Creates flows │      │ • Routes work    │      │ • Report    │
    │ • Monitors      │      │ • Coordinates    │      │ • Handoff   │
    └─────────────────┘      └────────┬─────────┘      └─────────────┘
                                      │
                                      │ WebSocket
                                      │ (Bidirectional)
                                      ▼
                             ┌─────────────────┐
                             │ Connected Clients│
                             │ (Claude Code)    │
                             │                  │
                             │ • Specialized    │
                             │ • Distributed    │
                             │ • Collaborative  │
                             └─────────────────┘
```

**Key Benefits:**
- **No Context Drift**: Each worker focuses on one specific task
- **Parallel Processing**: Multiple workers can run simultaneously
- **Persistent Progress**: All work is saved and can be resumed
- **Smart Coordination**: Automatic workflow progression based on completion
- **Bidirectional Communication**: Real-time coordination with connected Claude Code clients
- **Distributed Execution**: Leverage specialized environments and tools across multiple clients
- **Multi-Client Orchestration**: Coordinate complex workflows across diverse client capabilities

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Setting up the development environment
- Code style and testing requirements
- Submitting pull requests
- Reporting issues

## License

Apache 2.0