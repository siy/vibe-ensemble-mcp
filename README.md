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
- **👀 Progress Tracking**: Track progress through tickets, comments, and system events (live notifications WIP)
- **🔄 Stage-Based Workflows**: Workers follow structured execution plans through defined stages
- **💾 Persistent State**: All progress is saved, allowing you to pause and resume complex projects
- **🎨 Live Customization**: Edit worker templates in real-time to adapt to your team's processes and coding standards
- **🔄 Automatic Updates**: Built-in update checking and one-command upgrade system (`--upgrade`)
- **🌐 WebSocket Infrastructure**: WebSocket server available for future real-time communication features

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -fsSL https://vibeensemble.dev/install.sh | sh
```

**Windows:**
```powershell
iwr -useb https://vibeensemble.dev/install.ps1 | iex
```

### Upgrade to Latest Version

**One-Command Upgrade:**
```bash
vibe-ensemble-mcp --upgrade
```

This will automatically download and run the latest install script.

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
3. **Monitor Progress**: Use commands `/vibe-events`, `/vibe-poll` and `/vibe-status` to process events generated during project execution and check process status.

The coordinator will break down your project into tickets, spawn appropriate workers for each stage, and manage the workflow automatically.

**WARNING:** Vibe-Ensemble is still a work in progress. Some features may not be fully implemented or may have bugs. Sometimes coordinator may report issues, but not address them.
Prompting it with something like "Act as a coordinator" usually helps.

**SECURITY WARNING:** Always review and test permission configurations before production use. While the permission system is designed to be secure, proper configuration is essential. Use 'bypass' mode only in isolated development environments as it grants unrestricted access. For production use, the default 'file' mode provides explicit permission control.

## How Vibe-Ensemble Works

### The Big Picture

When you describe a project to Vibe-Ensemble, here's what happens behind the scenes:

1. **You describe your project** → Coordinator breaks it into major pieces (epics/stories)
2. **Planning worker creates detailed tickets** → Each with a step-by-step pipeline
3. **Workers automatically execute the pipeline** → Moving tickets from stage to stage
4. **Progress is tracked and reported** → Every step is documented with comments

Think of it like an assembly line where each worker is a specialist who does their part and passes the work to the next specialist.

### The Ticket System

#### **What is a Ticket?**
A ticket represents a specific piece of work that needs to be done. Examples:
- "Create user login page"
- "Set up database schema"
- "Write API documentation"
- "Deploy application to production"

#### **What is a Pipeline?**
Each ticket has a pipeline - a series of stages it goes through from start to finish:

```
Ticket: "User Login Page"
Pipeline: Planning → Design → Implementation → Review → Testing → Complete
```

Each stage is handled by a different type of worker with specific expertise.

### The Workflow Process

#### **Step 1: Project Breakdown**
When you describe your project, the **Coordinator** works with a **Planning Worker** to:
- Break your project into major components (epics)
- Create specific work items (stories/tickets) for each component
- Design the pipeline each ticket will follow

**Example:**
```
Your request: "Build a todo app with user accounts"

Coordinator creates:
├── Epic: User Authentication
│   ├── Ticket: "User registration system"
│   └── Ticket: "Login/logout functionality"
├── Epic: Todo Management
│   ├── Ticket: "Todo CRUD operations"
│   └── Ticket: "Todo list UI"
└── Epic: Deployment
    └── Ticket: "Production deployment setup"
```

#### **Step 2: Pipeline Design**
The **Planning Worker** designs a custom pipeline for each ticket based on its complexity:

**Simple Ticket Pipeline:**
```
Planning → Implementation → Review → Complete
```

**Complex Ticket Pipeline:**
```
Planning → Design → Implementation → Review → Testing → Documentation → Complete
```

**Critical System Pipeline:**
```
Planning → Research → Design → Implementation → Security Review → Testing → Deployment → Complete
```

#### **Step 3: Automatic Execution**
Once tickets and pipelines are created, Vibe-Ensemble automatically:

1. **Spawns the right worker** for each stage (planning worker, implementation worker, etc.)
2. **Moves tickets through their pipeline** as each stage completes
3. **Handles handoffs** between different types of workers
4. **Tracks all progress** with detailed comments

### How Workers Collaborate

#### **Worker Types and Responsibilities**

**Planning Worker:**
- Analyzes requirements and breaks down complex tasks
- Creates detailed implementation plans
- Designs the pipeline for each ticket

**Implementation Worker:**
- Writes the actual code
- Follows the specifications from planning
- Documents what was implemented

**Review Worker:**
- Checks code quality and adherence to standards
- Identifies issues that need fixing
- Can send tickets back to implementation if problems are found

**Testing Worker:**
- Creates and runs tests
- Validates that everything works correctly
- Reports any bugs found

#### **The Comment Trail**
Every worker leaves detailed comments on tickets, creating a complete history:

```
Ticket: "User Login Page"

[Planning Worker]: "Analyzed requirements. Will implement using React components
with form validation and JWT authentication. Estimated 4 hours of work."

[Implementation Worker]: "Created LoginForm component with email/password fields.
Added validation for email format. Integrated with authentication API.
Code is ready for review."

[Review Worker]: "Code looks good overall. Minor suggestion: add loading state
during login. No blocking issues. Approved for testing."

[Testing Worker]: "All tests passing. Verified login works with valid credentials,
shows errors for invalid ones, and handles network failures gracefully. Ready for deployment."
```

### Dependencies and Coordination

#### **When Tickets Need Each Other**
Sometimes one ticket can't start until another is finished:

```
Ticket A: "Database Schema Setup" → Must finish first
Ticket B: "User API Endpoints" → Waits for Ticket A
Ticket C: "Frontend Login Form" → Waits for Ticket B
```

Vibe-Ensemble automatically:
- **Holds tickets** that are waiting for dependencies
- **Releases tickets** when their dependencies are complete
- **Runs independent tickets in parallel** to save time

#### **Quality Gates**
Workers can send tickets backward in the pipeline when issues are found:

```
Implementation → Review → ❌ Issues Found → Back to Implementation
Implementation → Review → ✅ Approved → Testing
```

This ensures quality while allowing for iterative improvement.

### What You See as the User

#### **Real-Time Monitoring**
You can track progress using coordinator commands:
- **`/vibe-status`** - See overall project health and ticket progress
- **`/vibe-events`** - Handle any issues or worker requests for guidance
- **`/vibe-poll`** - Continuously monitor until all work is complete

#### **Transparent Process**
You can see:
- Which tickets are in which stages
- What each worker accomplished
- Any blockers or issues that need attention
- Estimated completion times

#### **Minimal Intervention Required**
Most of the time, the system runs automatically. You only need to:
- Provide initial project requirements
- Answer clarifying questions from workers
- Approve major decisions when requested
- Monitor progress and address any escalated issues

### The Power of Specialization

Each worker type is optimized for their specific role:
- **Planning workers** excel at breaking down complex requirements
- **Implementation workers** focus on writing quality code efficiently
- **Review workers** catch issues and maintain standards
- **Testing workers** ensure everything works reliably

This specialization means each piece of work gets expert attention while maintaining coordination across the entire project.

## Key Features

- **🚀 Zero Configuration**: Auto-setup with `--configure-claude-code`
- **🔄 Automatic Handoffs**: Workers complete stages and trigger next steps
- **🎨 Custom Workers**: Worker templates are completely externalized, you can tune them to your needs and standards
- **💬 Detailed Reporting**: Every stage produces comprehensive progress reports
- **⚡ Robust Processing**: Handles failures gracefully with retry mechanisms
- **📋 Project Rules & Patterns**: Define coding standards and project conventions that workers automatically follow
- **🔧 Flexible Workflows**: Support for debugging, testing, documentation, and DevOps workflows

## MCP Tools

Vibe-Ensemble provides 28 MCP tools organized into seven categories:

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

### Template Management
- `list_worker_templates` - List available worker templates
- `load_worker_template` - Load a specific worker template
- `ensure_worker_templates_exist` - Ensure all worker templates are available

> **Note on Worker Management**: Workers are automatically spawned when tickets are assigned to stages. There are no explicit worker spawn/stop tools - the queue system handles worker lifecycle automatically based on workload.

## Requirements

- Rust 1.70+ (for building from source)
- SQLite (embedded, no separate installation needed)
- Claude Code (for coordinator and worker processes)

## Configuration

The server accepts the following command-line options:

- `--configure-claude-code`: Generate Claude Code integration files and exit
- `--database-path`: SQLite database file path (default: `./.vibe-ensemble-mcp/vibe-ensemble.db`)
- `--host`: Server bind address (default: `127.0.0.1`)
- `--port`: Server port (default: `3276`)
- `--log-level`: Log level (default: `info`)
- `--permission-mode`: Permission mode for workers (default: `file`)
- `--no-respawn`: Disable automatic respawning of workers on startup
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

> **Note**: Complete permission examples are available in the source code at [docs/](https://github.com/siy/vibe-ensemble-mcp/tree/main/docs) directory. The `cp` commands below require access to the source repository.

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
- Required output fields (`outcome`, `comment`, `reason`)
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

## What's New in v0.9.7

- **🎫 Human-Friendly Ticket IDs**: New format PREFIX-SUBSYSTEM-NUMBER (e.g., TVR-FE-001) for better readability and organization
- **📋 Declarative Planning Workflow**: Planning workers output complete ticket specifications instead of calling MCP tools
- **⚛️ Atomic Ticket Creation**: Transaction-based ticket creation with dependency management for reliability
- **🔍 Subsystem Inference**: Automatic subsystem detection from stage names (frontend_impl → FE, backend_api → BE)
- **📊 Comprehensive Error Logging**: Added error logging to 115+ database operations across all modules for improved observability
- **🆔 Worker ID Validation**: Sanitize project IDs in worker ID construction to handle repository-style names
- **🔗 Dependency Status Consistency**: Automatic dependency_status updates when closing tickets

## What's New in v0.9.6

- **🔄 Automatic Update Tracking**: Built-in update checking and one-command upgrade system
- **🛡️ Enhanced Validation**: Comprehensive input validation for worker spawning with automatic on-hold placement
- **🐛 Critical Bug Fixes**: Fixed worker spawn race conditions, path validation, and review template enhancements

## WebSocket Infrastructure

> **Note**: WebSocket infrastructure is available for real-time communication, but bidirectional MCP tools have been removed to focus on core multi-agent coordination functionality.

The server provides WebSocket support for:
- Real-time event notifications (when implemented)
- Future bidirectional communication features
- Infrastructure for IDE integration

### WebSocket Configuration

1. **Configure with WebSocket Support**:
   ```bash
   ./vibe-ensemble-mcp --configure-claude-code --host 127.0.0.1 --port 3276
   ```
   This generates WebSocket authentication tokens and configuration.

2. **Start Server** (WebSocket enabled by default):
   ```bash
   ./vibe-ensemble-mcp --port 3276
   ```

3. **Monitor Progress**:
   - Ask Claude Code to "check events" or "report project status"
   - Monitor the `.vibe-ensemble-mcp/logs/` directory for detailed activity
   - Use MCP tools through Claude Code for project management

## How It Works

```
    ┌─────────────────┐      ┌──────────────────┐      ┌─────────────┐
    │   Coordinator   │◄────►│ Vibe-Ensemble    │─────►│  Workers    │
    │ (Claude Code)   │      │    Server        │      │ (Headless)  │
    │                 │      │                  │      │             │
    │ • Plans tasks   │      │ • Manages state  │      │ • Execute   │
    │ • Creates flows │      │ • Routes work    │      │ • Report    │
    │ • Monitors      │      │ • Coordinates    │      │ • Handoff   │
    └─────────────────┘      └──────────────────┘      └─────────────┘
                                      │
                                      │ SSE/WebSocket
                                      │ (Events)
                                      ▼
                             ┌─────────────────┐
                             │ Event Streams   │
                             │ & Monitoring    │
                             │                 │
                             │ • Progress      │
                             │ • Notifications │
                             │ • Status        │
                             └─────────────────┘
```

**Key Benefits:**
- **No Context Drift**: Each worker focuses on one specific task
- **Sequential Processing**: Workers handle stages in defined order
- **Persistent Progress**: All work is saved and can be resumed
- **Smart Coordination**: Automatic workflow progression based on completion
- **WebSocket Infrastructure**: Foundation for future real-time features

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Setting up the development environment
- Code style and testing requirements
- Submitting pull requests
- Reporting issues

## License

Apache 2.0