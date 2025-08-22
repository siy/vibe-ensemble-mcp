# Shared Agent Templates

This directory contains shared configuration and settings used by all worker agents in the vibe-ensemble system.

## Worker Settings Deployment System

The HeadlessClaudeExecutor automatically deploys the shared settings from this directory to worker agent workspaces before execution. This ensures all workers have consistent configuration and access to the vibe-ensemble MCP server for coordination.

### Settings Configuration

The `.claude/settings.json` file contains:

- **Enhanced Permissions**: Worker agents have access to essential development tools, git operations, and web resources
- **Security Restrictions**: Dangerous operations like `sudo`, `rm -rf /`, and system administration commands are explicitly denied
- **MCP Server Integration**: Automatic connection to the vibe-ensemble coordination server
- **Environment Variable Substitution**: Dynamic configuration based on workspace context

### Environment Variables

The following variables are automatically substituted during deployment:

- `${VIBE_ENSEMBLE_MCP_SERVER}` - Server URL (defaults to ws://localhost:8080)
- `${WORKSPACE_ID}` - Unique workspace identifier
- `${WORKSPACE_NAME}` - Human-readable workspace name
- `${TEMPLATE_NAME}` - Agent template name
- `${AGENT_ID}` - Agent identifier (defaults to workspace ID)
- `${VIBE_ENSEMBLE_LOG_LEVEL}` - Logging level (defaults to info)

### Deployment Process

1. **Pre-Execution**: Settings are deployed to workspace's `.claude/settings.json`
2. **Variable Substitution**: Environment variables are resolved with workspace context
3. **Permission Validation**: Security policies are enforced
4. **Post-Execution**: Temporary settings are automatically cleaned up

### Usage in Code

```rust
use vibe_ensemble_core::orchestration::executor::HeadlessClaudeExecutor;

let executor = HeadlessClaudeExecutor::with_agent_templates_path(
    PathBuf::from("./agent-templates")
);

// Settings are automatically deployed when deploy_shared_settings is true (default)
let result = executor.execute_prompt(&workspace, prompt).await?;

// Or with automatic cleanup
let result = executor.execute_prompt_with_cleanup(&workspace, prompt).await?;
```

### Configuration Options

```rust
let mut config = ExecutionConfig::default();
config.deploy_shared_settings = true; // Enable/disable deployment
config.shared_settings_template_path = Some(custom_path); // Override template path
```

## Security Model

Worker agents have restricted permissions compared to the main coordinator:

- ✅ **Allowed**: Development tools, git operations, build systems, testing
- ✅ **Allowed**: Web access to documentation sites and package repositories  
- ✅ **Allowed**: File operations within workspace boundaries
- ❌ **Denied**: System administration commands
- ❌ **Denied**: Dangerous file operations outside workspace
- ❌ **Denied**: Privilege escalation attempts

This balance provides workers with the tools needed for development tasks while maintaining system security.

## Multi-Agent Coordination Framework

### Coordination-Aware Agent Templates

All agent templates have been enhanced with coordination capabilities to enable sophisticated multi-agent workflows:

#### Agent Template Enhancements
- **Code Writer**: Enhanced with proactive dependency detection, resource reservation protocols, and conflict prevention workflows
- **Code Reviewer**: Added coordination assessment checklists and multi-agent review protocols  
- **Docs Specialist**: Integrated knowledge sharing workflows and cross-agent documentation coordination
- **Test Specialist**: Added coordination integration testing and shared test environment management
- **Coordinator**: New template for strategic orchestration and workflow optimization across all agents

#### Core Coordination Protocols

Each agent template includes standardized protocols for:

1. **Dependency Detection and Escalation**
   - Proactive conflict prediction using `vibe_conflict_predict`
   - Resource reservation via `vibe_resource_reserve`
   - Cross-project impact analysis via `vibe_dependency_declare`

2. **Communication Patterns**
   - Status broadcasting via `vibe_worker_message`
   - Work coordination via `vibe_work_coordinate`  
   - Conflict resolution via `vibe_conflict_resolve`
   - Knowledge capture via `vibe_learning_capture`

3. **Decision Trees for Automation**
   - Automated escalation triggers for high-risk scenarios
   - Resource management workflows
   - Merge coordination for complex scenarios
   - Guideline enforcement via `vibe_guideline_enforce`

#### Coordination Etiquette Standards

All templates include consistent etiquette guidelines for:
- **Cross-Project Communication**: Professional, clear messaging with context
- **Resource Management**: Conservative reservation, prompt release
- **Knowledge Contribution**: Pattern documentation and lesson sharing
- **Quality Assurance**: Coordination-aware quality gates and compliance checks

### Available Coordination Tools

The MCP server provides comprehensive coordination capabilities:

#### Issue #52: Intelligent Work Orchestration
- `vibe_schedule_coordinate` - Plan work sequences across workers
- `vibe_conflict_predict` - Detect potential conflicts early
- `vibe_resource_reserve` - Reserve files/modules for exclusive access
- `vibe_merge_coordinate` - Coordinate complex merge scenarios

#### Issue #53: Knowledge-Driven Coordination  
- `vibe_knowledge_query` - Search coordination patterns and solutions
- `vibe_pattern_suggest` - Suggest approaches based on history
- `vibe_guideline_enforce` - Apply organizational policies
- `vibe_learning_capture` - Learn from successes/failures

#### Foundational Coordination Tools
- `vibe_agent_register/list/status` - Agent lifecycle management
- `vibe_worker_message/coordinate` - Direct communication
- `vibe_dependency_declare` - Cross-project dependency coordination
- `vibe_issue_create/assign/update` - Issue tracking integration

### Deployment and Integration

The coordination framework integrates seamlessly with the existing deployment system:

- **Automatic MCP Connection**: All agents automatically connect to coordination server
- **Shared Coordination Settings**: Common coordination protocols across all agents
- **Template Variable Substitution**: Coordination parameters customized per deployment
- **Security-First Design**: Coordination tools respect existing security boundaries

This comprehensive coordination framework enables sophisticated multi-agent development workflows while maintaining individual agent autonomy and specialization.