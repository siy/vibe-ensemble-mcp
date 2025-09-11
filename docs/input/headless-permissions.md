# Claude Code Headless Mode Permission Model

## Overview

Claude Code's headless mode (activated with the `-p` flag) operates with a fundamentally different permission model compared to interactive mode. In headless environments, all permission decisions must be predetermined and configured through static configuration files or command-line parameters, as interactive prompts are not available.

## Core Principles

### 1. Pre-Configured Permission Model
- **No Interactive Prompts**: The system cannot ask for user confirmation during execution
- **Fail-Safe Defaults**: Tools are denied by default unless explicitly allowed
- **Configuration-Driven**: All permissions must be defined before execution begins
- **Deterministic Behavior**: Same configuration produces identical permission decisions

### 2. Permission Decision Hierarchy
The permission system evaluates tool usage requests in the following order:

1. **Command-Line Overrides** (highest priority)
   - `--disallowedTools` flag for runtime restrictions
   - `--settings` flag for external configuration files

2. **Project-Local Settings**
   - `.claude/settings.local.json` (user-specific, not committed)
   - `.claude/settings.json` (shared project configuration)

3. **Global User Settings**
   - `~/.claude.json` (deprecated but still supported)
   - User-level settings directory

4. **Default Deny** (lowest priority)
   - Tools not explicitly allowed are automatically denied

## Configuration File Structure

### Permission Configuration Schema

```json
{
  "permissions": {
    "allow": [
      "Bash(git status)",
      "Bash(git add:*)",
      "Read",
      "Write", 
      "Glob",
      "Grep"
    ],
    "deny": [
      "Bash(rm:*)",
      "Bash(sudo:*)"
    ],
    "ask": []  // Ignored in headless mode
  }
}
```

### Tool Specification Patterns

#### Basic Tool Allowance
```json
{
  "allow": [
    "Read",           // Allow all Read operations
    "Write",          // Allow all Write operations
    "Bash"            // Allow all Bash commands (dangerous)
  ]
}
```

#### Parameterized Tool Restrictions
```json
{
  "allow": [
    "Bash(git status)",        // Specific command only
    "Bash(git add:*)",         // Command with any parameters
    "Bash(npm:install|test)",  // Multiple allowed subcommands
    "Read(/project/src/*)"     // Path-restricted file access
  ]
}
```

#### Environment-Specific Permissions
```json
{
  "permissions": {
    "allow": [
      "Bash(env)",              // Environment variable access
      "WebFetch",               // Network access
      "Task"                    // Subagent invocation
    ]
  }
}
```

## GitHub Actions Integration

### Action Configuration

Claude Code provides specialized GitHub Actions that demonstrate headless permission configuration:

```yaml
- name: Run Claude Code for Issue Triage
  uses: anthropics/claude-code-base-action@beta
  with:
    prompt_file: /tmp/claude-prompts/triage-prompt.txt
    allowed_tools: "Bash(gh label list),mcp__github__get_issue,mcp__github__get_issue_comments,mcp__github__update_issue,mcp__github__search_issues,mcp__github__list_issues"
    timeout_minutes: "5"
    anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
    mcp_config: /tmp/mcp-config/mcp-servers.json
    claude_env: |
      GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### Key Action Parameters

- **`allowed_tools`**: Comma-separated list of explicitly allowed tools
- **`timeout_minutes`**: Maximum execution time (prevents runaway processes)
- **`mcp_config`**: Path to MCP server configuration file
- **`claude_env`**: Environment variables available to Claude Code

## MCP (Model Context Protocol) Permissions

### MCP Server Configuration

MCP servers in headless mode require explicit configuration and permission grants:

```json
{
  "mcpServers": {
    "github": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm", "-e", "GITHUB_PERSONAL_ACCESS_TOKEN",
        "ghcr.io/github/github-mcp-server:sha-7aced2b"
      ],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    }
  }
}
```

### MCP Tool Permission Pattern

MCP tools follow the naming convention `mcp__<server>__<tool>` and must be explicitly allowed:

```json
{
  "allow": [
    "mcp__github__get_issue",
    "mcp__github__update_issue", 
    "mcp__github__search_issues",
    "mcp__github__list_issues"
  ]
}
```

### MCP Security Features

- **Container Isolation**: Docker-based MCP servers provide process isolation
- **Environment Scoping**: Each MCP server receives only necessary environment variables
- **Token Management**: API tokens scoped to specific MCP server instances
- **Network Restrictions**: Container networking can be restricted as needed

## Advanced Permission Features

### Hook-Based Permission Control

PreToolUse hooks can implement custom permission logic even in headless mode:

```python
# bash_command_validator_example.py
import json
import sys

def validate_command(command: str) -> list[str]:
    issues = []
    if re.search(r"^rm\s+", command):
        issues.append("rm commands are not allowed in headless mode")
    return issues

# Hook configuration
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command", 
            "command": "python3 /path/to/validator.py"
          }
        ]
      }
    ]
  }
}
```

### Hook Exit Codes
- **Exit 0**: Allow tool execution
- **Exit 1**: Show error to user but not to Claude
- **Exit 2**: Block tool execution and show error to Claude

### Environment Variables for Hooks

Hooks receive additional context in headless mode:

- **`CLAUDE_PROJECT_DIR`**: Current project directory
- **`CLAUDE_CODE_ENTRYPOINT`**: Set to "cli" for headless mode
- **`CLAUDECODE`**: Set to "1" when running under Claude Code

## Platform-Specific Considerations

### Windows Environments

```json
{
  "permissions": {
    "allow": [
      "Read(//c/Users/*/project/*)",  // POSIX-style Windows paths
      "Bash(powershell.exe:*)",       // PowerShell access
      "Bash(cmd.exe:/c dir)"          // CMD access with parameters
    ]
  }
}
```

### WSL (Windows Subsystem for Linux)

- **Path Translation**: Automatic handling of Windows/WSL path differences
- **Extension Path Handling**: Improved IDE integration in WSL environments
- **Cross-Platform Tools**: Consistent tool naming across Windows and Linux

### macOS Environments

```json
{
  "permissions": {
    "allow": [
      "Read(/Users/*/project/**/*)",
      "Bash(/usr/bin/security:*)",    // Keychain access
      "Bash(/usr/local/bin/*)"        // Homebrew tools
    ]
  }
}
```

## Security Best Practices

### Principle of Least Privilege

```json
{
  "permissions": {
    "allow": [
      "Read(/project/src/**/*)",      // Specific directory access
      "Write(/project/output/**/*)",  // Limited write access
      "Bash(git:status|diff|log)",   // Specific git commands only
      "Grep",                        // Safe search operations
      "Glob"                         // Safe file pattern matching
    ],
    "deny": [
      "Bash(rm:*)",                  // Prevent file deletion
      "Bash(sudo:*)",                // Prevent privilege escalation
      "Bash(curl:*)",                // Block network access
      "WebFetch"                     // Block web access
    ]
  }
}
```

### Tool-Specific Restrictions

#### File System Access
```json
{
  "allow": [
    "Read(/safe/directory/**/*)",
    "Write(/output/directory/**/*)"
  ],
  "deny": [
    "Read(/etc/**/*)",
    "Read(/home/*/.ssh/**/*)",
    "Write(/system/**/*)"
  ]
}
```

#### Network Access Control
```json
{
  "permissions": {
    "allow": [
      "WebFetch(https://api.github.com/*)",
      "Bash(curl:https://safe-api.com/*)"
    ],
    "deny": [
      "WebFetch",                    // Block all other web access
      "Bash(wget:*)",                // Block wget
      "Bash(nc:*)"                   // Block netcat
    ]
  }
}
```

## Troubleshooting and Validation

### Configuration Validation

Use the `/doctor` command to validate permission configurations:

```bash
claude --settings /path/to/config.json /doctor
```

The doctor command will:
- Validate JSON syntax
- Check permission rule syntax
- Suggest corrections for invalid patterns
- Verify file path accessibility

### Common Issues and Solutions

#### 1. Tool Blocked in Headless Mode
```
Error: Tool 'Bash(git commit)' not allowed in headless mode
```

**Solution**: Add the tool to your allow list:
```json
{
  "permissions": {
    "allow": ["Bash(git commit:*)"]
  }
}
```

#### 2. MCP Tool Permission Denied
```
Error: MCP tool 'mcp__github__get_issue' not permitted
```

**Solution**: Add MCP tools to allowed_tools in action configuration:
```yaml
allowed_tools: "mcp__github__get_issue,mcp__github__update_issue"
```

#### 3. Configuration File Not Found
```
Warning: Settings file '/path/to/settings.json' not found
```

**Solution**: Create the configuration file or use `--settings` flag:
```bash
claude --settings /absolute/path/to/settings.json -p "Your prompt"
```

### Debug Mode

Enable MCP debugging for permission issues:
```bash
claude --mcp-debug -p "Your prompt"
```

This provides detailed information about:
- MCP server startup
- Tool registration
- Permission checking
- Authentication flows

## Migration from Interactive Mode

### Converting Interactive Permissions

If you have an existing interactive setup, you can extract permissions:

1. **Review Current Permissions**: Use `/permissions` command in interactive mode
2. **Export Configuration**: Copy settings to headless configuration file
3. **Test in Headless Mode**: Validate with `claude -p` before deployment

### Example Migration

Interactive configuration:
```bash
# Interactive commands that were allowed
/permissions allow Bash(git:*)
/permissions allow Read
/permissions allow Write
```

Headless equivalent:
```json
{
  "permissions": {
    "allow": [
      "Bash(git:*)",
      "Read", 
      "Write"
    ]
  }
}
```

## Performance Considerations

### Configuration Loading
- Settings files are loaded once at startup
- Changes require restart in headless mode (unlike interactive mode)
- Large permission lists can impact startup time

### Tool Resolution
- Permission checking adds minimal overhead per tool invocation
- Pattern matching is optimized for common use cases
- MCP tool permissions are cached after first resolution

### Memory Usage
- Permission configurations are held in memory
- Large configurations increase baseline memory usage
- Consider splitting complex configurations across multiple files

## Integration Examples

### CI/CD Pipeline Integration

```yaml
name: Code Review with Claude
on: [pull_request]

jobs:
  claude-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Create Permission Config
        run: |
          mkdir -p .claude
          cat > .claude/settings.json << 'EOF'
          {
            "permissions": {
              "allow": [
                "Read",
                "Bash(git:diff|log|show)",
                "Grep",
                "Glob"
              ],
              "deny": [
                "Write",
                "Bash(rm:*)",
                "WebFetch"
              ]
            }
          }
          EOF
      
      - name: Run Claude Code Review
        uses: anthropics/claude-code-base-action@beta
        with:
          prompt: "Review this PR for potential issues"
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          allowed_tools: "Read,Bash(git diff),Bash(git log),Grep,Glob"
```

### Docker Container Setup

```dockerfile
FROM ubuntu:22.04

# Install Claude Code
RUN npm install -g @anthropic-ai/claude-code

# Copy permission configuration
COPY claude-config.json /app/.claude/settings.json

# Set working directory
WORKDIR /app

# Run in headless mode
CMD ["claude", "-p", "$PROMPT"]
```

```json
// claude-config.json
{
  "permissions": {
    "allow": [
      "Read(/app/**/*)",
      "Write(/app/output/**/*)", 
      "Bash(echo:*)",
      "Bash(date)",
      "Grep",
      "Glob"
    ],
    "deny": [
      "Bash(rm:*)",
      "Bash(curl:*)",
      "WebFetch"
    ]
  }
}
```

This comprehensive permission model ensures that Claude Code can operate safely and predictably in automated environments while maintaining the flexibility needed for complex workflows.