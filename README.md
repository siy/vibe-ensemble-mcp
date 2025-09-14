# Vibe-Ensemble MCP Server

A multi-agent coordination system that enables Claude Code to manage specialized AI workers for complex development tasks.

> **⚠️ Security Warning**: In version 0.7.0, workers have full access to all tools and permissions. Use this system at your own risk. A comprehensive permission system is planned for the next release.

## What It Does

Vibe-Ensemble allows you to break down complex projects into specialized stages, with each stage handled by a dedicated AI worker:

- **🎯 Smart Task Planning**: Automatically plan multi-stage workflows (architecture → development → testing → deployment)
- **🤖 Specialized Workers**: Create custom AI workers with specific expertise (Rust developer, security reviewer, UI designer, etc.)
- **📋 Automatic Progression**: Workers complete their stage and automatically hand off to the next worker
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

## Example Workflows

### Building a Web Application

Here's a complete workflow for building a modern web application with authentication:

```bash
# 1. Setup your development environment
./vibe-ensemble-mcp --configure-claude-code --port 3000
./vibe-ensemble-mcp &  # Start server in background

# 2. Initialize Claude Code in your project directory
cd /path/to/your/project
# Open Claude Code here and run: /vibe-ensemble
```

**In Claude Code (as Coordinator):**

```
# 3. Create a new project with rules and patterns
create_project("myorg/todo-app", "/path/to/project", "A modern todo application with JWT auth")

# 4. Set up project rules and coding patterns
update_project("myorg/todo-app", {
  "project_rules": "Use TypeScript for frontend, Rust for backend. Follow RESTful API design. All endpoints must have proper error handling.",
  "project_patterns": "Components in /src/components, API routes in /api, database models in /models. Use async/await, proper error types."
})

# 5. Define specialized workers for your team
create_worker_type("myorg/todo-app", "architect", 
  "You are a senior software architect. Design system architecture, database schemas, and API contracts. Focus on scalability and maintainability.")

create_worker_type("myorg/todo-app", "rust-backend-dev", 
  "You implement Rust backend services using Axum and SQLx. Write clean, well-tested code following Rust best practices.")

create_worker_type("myorg/todo-app", "frontend-dev", 
  "You build React TypeScript interfaces. Create responsive, accessible UI components with proper state management.")

create_worker_type("myorg/todo-app", "security-reviewer", 
  "You review code for security vulnerabilities, especially auth systems, input validation, and data protection.")

# 6. Create tickets with execution plans
create_ticket("TODO-001", "myorg/todo-app", "Implement JWT Authentication System", 
  "Build complete authentication with JWT tokens, password hashing, and session management",
  ["architect", "rust-backend-dev", "security-reviewer"])

create_ticket("TODO-002", "myorg/todo-app", "Create User Dashboard", 
  "Build responsive dashboard with todo management, user profile, and settings",
  ["architect", "frontend-dev", "security-reviewer"])

# 7. Submit tickets to start the workflow
submit_task("myorg/todo-app", "architect", "TODO-001")
submit_task("myorg/todo-app", "architect", "TODO-002")
```

**What happens automatically:**
1. **Architect** designs auth system architecture and database schema
2. **Rust Backend Dev** implements JWT endpoints, password hashing, middleware
3. **Security Reviewer** audits implementation for vulnerabilities
4. Each worker provides detailed progress reports and hands off to the next
5. Real-time notifications keep you updated on progress

### Debugging and Testing Workflow

```
# Create a debugging-focused ticket
create_ticket("BUG-001", "myorg/todo-app", "Fix Performance Issues", 
  "Investigate and resolve slow API responses",
  ["investigator", "rust-backend-dev", "performance-tester"])

# Specialized debugging workers
create_worker_type("myorg/todo-app", "investigator", 
  "You analyze performance issues, profile code, and identify bottlenecks.")

create_worker_type("myorg/todo-app", "performance-tester", 
  "You write performance tests and validate optimizations.")
```

### Documentation and DevOps Workflow

```
# Create infrastructure and documentation tickets
create_ticket("DOCS-001", "myorg/todo-app", "Complete Project Documentation", 
  "Write comprehensive API docs, deployment guides, and user manuals",
  ["technical-writer", "devops-engineer"])

create_worker_type("myorg/todo-app", "technical-writer", 
  "You write clear, comprehensive documentation for developers and users.")

create_worker_type("myorg/todo-app", "devops-engineer", 
  "You set up CI/CD pipelines, containerization, and deployment automation.")
```

## Key Features

- **🚀 Zero Configuration**: Auto-setup with `--configure-claude-code`
- **🔄 Automatic Handoffs**: Workers complete stages and trigger next steps
- **📊 Real-time Updates**: Live progress tracking via Server-Sent Events
- **🎨 Custom Workers**: Define workers for any domain (coding, design, analysis, etc.)
- **💬 Detailed Reporting**: Every stage produces comprehensive progress reports
- **⚡ Robust Processing**: Handles failures gracefully with retry mechanisms
- **📋 Project Rules & Patterns**: Define coding standards and project conventions that workers automatically follow
- **🔧 Flexible Workflows**: Support for debugging, testing, documentation, and DevOps workflows

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