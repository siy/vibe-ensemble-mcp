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