# Vibe-Ensemble MCP Server - Implementation Plan

## Project Overview

The Vibe-Ensemble MCP Server is a Rust-based Model Context Protocol (MCP) server that enables multi-agent coordination through a coordinator-worker architecture. This system prevents context drift and focus dilution by allowing one coordinator agent (regular Claude Code instance) to manage multiple specialized worker agents (Claude Code headless processes).

## Architecture

```
┌─────────────────┐    HTTP MCP    ┌──────────────────┐    stdio    ┌─────────────┐
│   Coordinator   │◄──────────────►│  MCP Server      │◄───────────►│   Worker 1  │
│ (Claude Code)   │                │  (Rust/HTTP)     │             │ (Headless)  │
└─────────────────┘                │                  │    stdio    ├─────────────┤
                                   │  ┌─────────────┐ │◄───────────►│   Worker 2  │
                                   │  │   SQLite    │ │             │ (Headless)  │
                                   │  │  Database   │ │    stdio    ├─────────────┤
                                   │  └─────────────┘ │◄───────────►│   Worker N  │
                                   └──────────────────┘             │ (Headless)  │
                                                                    └─────────────┘
```

## Core Components

### 1. Coordinator Agent
- Regular Claude Code instance connected via HTTP MCP
- Plans tasks and creates tickets with execution plans
- Spawns/stops workers based on workload
- Monitors progress through event system

### 2. MCP Server (Rust)
- HTTP-based MCP protocol implementation
- SQLite database for persistent storage
- In-memory task queues (1:1 worker-queue mapping)
- Process management for worker lifecycle
- Event system for async notifications

### 3. Worker Agents
- Claude Code headless processes (`claude -p "prompt"`)
- Specialized system prompts per worker type
- Autonomous task processing from dedicated queues
- Report generation for next stage workers

### 4. Ticket System
- SQLite-based issue tracking
- Multi-stage execution with worker reports
- Atomic stage completion with comment addition
- Event generation for coordinator notifications

## Implementation Stages

### [Stage 1: Project Setup](stages/STAGE_1_PROJECT_SETUP.md)
**Duration**: 1-2 hours  
**Goal**: Basic Rust project structure with HTTP server

- Initialize Cargo project with dependencies
- Basic HTTP server with health check endpoint
- Logging and configuration framework
- CLI argument parsing for database path

### [Stage 2: Database Layer](stages/STAGE_2_DATABASE_LAYER.md)
**Duration**: 2-3 hours  
**Goal**: Complete SQLite schema and operations

- Database schema creation and migrations
- CRUD operations for all entities (Projects, Worker Types, Tickets, Comments, Workers, Events)
- Connection pooling and transaction management
- Database initialization and seeding

### [Stage 3: MCP Protocol Implementation](stages/STAGE_3_MCP_PROTOCOL.md)
**Duration**: 3-4 hours  
**Goal**: Full HTTP MCP server with basic tools

- MCP protocol handlers (initialize, list_tools, call_tool)
- Tool framework with parameter validation
- Basic project and worker type management tools
- Error handling and response formatting

### [Stage 4: Worker Management](stages/STAGE_4_WORKER_MANAGEMENT.md)
**Duration**: 3-4 hours  
**Goal**: Complete worker lifecycle and queue management

- Process spawning for Claude Code headless workers
- Worker status tracking and health monitoring
- In-memory queue implementation with persistence hooks
- Worker-queue binding and task distribution

### [Stage 5: Ticket System](stages/STAGE_5_TICKET_SYSTEM.md)
**Duration**: 2-3 hours  
**Goal**: Complete ticket workflow and event system

- Ticket CRUD operations with stage management
- Comment system with atomic updates
- Event generation and queue management
- Stage progression logic and validation

### [Stage 6: Integration & Testing](stages/STAGE_6_INTEGRATION_TESTING.md)
**Duration**: 2-3 hours  
**Goal**: End-to-end validation and documentation

- Coordinator integration testing
- Worker spawning and task execution validation
- Performance testing and optimization
- Final documentation and usage examples

## Technical Specifications

### Database Schema
- **Projects**: Repository-based project management
- **Worker Types**: Project-specific worker definitions with system prompts
- **Tickets**: Multi-stage task tracking with execution plans
- **Comments**: Stage reports and coordinator instructions
- **Workers**: Process tracking and status management
- **Events**: Async notification system

### MCP Tools (22 total)
- **Project Management**: 5 tools (CRUD operations)
- **Worker Type Management**: 5 tools (CRUD operations)
- **Worker Management**: 4 tools (spawn, stop, list, status)
- **Queue Management**: 3 tools (list, status, delete)
- **Ticket Management**: 6 tools (CRUD, stage management)
- **Event Management**: 2 tools (get, mark processed)

### Dependencies
```toml
tokio = { version = "1.0", features = ["full"] }
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "sqlite", "chrono", "uuid"] }
axum = "0.7"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
clap = { version = "4.0", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Success Criteria

### Functional Requirements
- ✅ Coordinator can manage projects and worker types
- ✅ Workers spawn successfully with correct system prompts
- ✅ Tickets progress through execution stages automatically
- ✅ Events notify coordinator of stage completions
- ✅ Worker processes exit cleanly when queues empty

### Performance Requirements
- ✅ Handle 10+ concurrent workers per project
- ✅ Process 100+ tickets without memory leaks
- ✅ Respond to MCP calls within 100ms average
- ✅ Database operations complete within 50ms

### Reliability Requirements
- ✅ Graceful handling of worker process failures
- ✅ Database transaction integrity maintained
- ✅ Event delivery guaranteed (at-least-once)
- ✅ Configuration validation and error reporting

## Next Steps

1. [Stage 1: Project Setup](stages/STAGE_1_PROJECT_SETUP.md) - Initialize project structure
2. Track progress in [TODO.md](TODO.md)
3. Preserve context in [CLAUDE.md](CLAUDE.md)

## Timeline

**Total Estimated Duration**: 13-19 hours  
**Target Completion**: 3-5 days (part-time development)

Each stage builds incrementally with testing and validation before proceeding to the next stage.