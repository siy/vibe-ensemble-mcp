# Architecture Overview

Vibe Ensemble is designed for a **single user running 5-10 Claude Code agents** across **2-3 projects simultaneously**. This keeps the architecture simple and focused on personal productivity.

## System Design

```
Personal Developer Workspace
┌─────────────────────────────────────┐
│         Claude Code Agents         │
│  code-writer │ code-reviewer │ ... │
└─────────────────────────────────────┘
           │ (headless mode)
           │
┌─────────────────────────────────────┐
│       Vibe Ensemble MCP            │
│  ┌─────────────────────────────┐   │
│  │    Agent Orchestration      │   │
│  │  • Template Management      │   │
│  │  • Workspace Coordination   │   │
│  │  • Knowledge Sharing        │   │
│  └─────────────────────────────┘   │
│  ┌─────────────────────────────┐   │
│  │     Core Services          │   │
│  │  • Issue Tracking          │   │
│  │  • Agent Management        │   │
│  │  • Message Coordination    │   │
│  └─────────────────────────────┘   │
└─────────────────────────────────────┘
           │
           │
┌─────────────────────────────────────┐
│        SQLite Database             │
│  (Single file, no clustering)      │
└─────────────────────────────────────┘
```

## Core Components

### 1. Agent Orchestration System
- **Template-based agent configuration** from `agent-templates/`
- **Workspace management** for isolated project contexts
- **Headless Claude Code execution** with JSON streaming
- **Multi-project coordination** without conflicts

### 2. Core Domain Services
- **Agent Management**: Registration, health monitoring, capabilities
- **Issue Tracking**: Simple task and bug tracking across projects  
- **Knowledge Management**: Shared learning and patterns
- **Message Coordination**: Agent-to-agent communication

### 3. Persistence Layer
- **SQLite** for simplicity (single file database)
- **Migration system** for schema evolution
- **Repository pattern** for data access

## Agent Templates

Built-in agent types:
- **code-writer**: Feature implementation and bug fixing
- **code-reviewer**: Code quality and security review  
- **test-specialist**: Test writing and maintenance
- **docs-specialist**: Documentation and technical writing

Each template includes:
- Handlebars configuration templates
- Language-specific analysis patterns
- Tool permissions and security constraints
- Workflow integration patterns

## Data Flow

1. **User starts MCP server** (once per day)
2. **Agents connect** via Claude Code headless mode
3. **Templates generate** agent-specific configurations
4. **Workspaces coordinate** project-specific contexts
5. **Knowledge sharing** happens automatically between agents
6. **Issues track** progress across all projects

## File Organization

```
vibe-ensemble/
├── vibe-ensemble-core/          # Domain models and business logic
├── vibe-ensemble-storage/       # SQLite persistence layer
├── vibe-ensemble-prompts/       # Prompt management system
├── agent-templates/             # Agent configuration templates
│   ├── code-writer/
│   ├── code-reviewer/
│   ├── test-specialist/
│   └── docs-specialist/
└── workspaces/                  # Agent working directories
    ├── project-a-workspace/
    ├── project-b-workspace/
    └── shared-knowledge/
```

## Scalability Assumptions

**Designed for:**
- 1 user
- 5-10 concurrent agents
- 2-3 active projects
- Local development machine

**Not designed for:**
- Multiple users
- Hundreds of agents
- Enterprise deployment
- High availability or clustering

This keeps the system simple, reliable, and easy to understand for personal use.