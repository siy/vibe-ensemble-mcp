# MCP Integration in Claude Code Headless Mode

## Overview

Model Context Protocol (MCP) integration in Claude Code's headless mode provides a sophisticated plugin architecture that enables external systems, APIs, and tools to extend Claude's capabilities without requiring interactive user intervention. This document provides comprehensive coverage of MCP server configuration, communication patterns, and implementation details specifically for automated and headless environments.

## MCP Architecture in Headless Mode

### Core Principles

**1. Pre-Configuration Requirement**
- All MCP servers must be configured before execution begins
- No interactive setup wizards or user prompts available
- Configuration validation occurs at startup

**2. Transport Agnostic**
- Support for multiple transport protocols (stdio, SSE, HTTP)
- Automatic transport selection based on configuration
- Graceful fallback mechanisms for failed connections

**3. Container-First Security**
- Docker-based isolation for untrusted MCP servers
- Configurable network and filesystem restrictions
- Process-level security boundaries

**4. Fault Tolerance**
- Non-blocking startup prevents MCP failures from stopping Claude Code
- Individual server failures don't affect other MCP servers
- Graceful degradation when servers become unavailable

## MCP Server Configuration

### Configuration Hierarchy

MCP servers are configured through a three-tier hierarchy, evaluated in order of precedence:

#### 1. Command-Line Configuration (Highest Priority)
```bash
# Single configuration file
claude --mcp-config /path/to/servers.json -p "Your prompt"

# Multiple configuration files (merged)
claude --mcp-config base.json overrides.json -p "Your prompt"

# One-off server for single execution
claude --mcp-config /tmp/github-server.json -p "Analyze repository issues"
```

#### 2. Project-Level Configuration
**File: `.mcp.json` (committable to version control)**
```json
{
  "mcpServers": {
    "project-tools": {
      "command": "python",
      "args": ["/project/tools/mcp-server.py"],
      "env": {
        "PROJECT_ROOT": "${PWD}",
        "LOG_LEVEL": "INFO"
      }
    }
  }
}
```

#### 3. User-Level Configuration
**File: `~/.claude/mcp-servers.json` (user-specific)**
```json
{
  "mcpServers": {
    "personal-tools": {
      "command": "/usr/local/bin/my-mcp-server",
      "env": {
        "USER_CONFIG": "${HOME}/.config/my-tool"
      }
    }
  }
}
```

### Transport Configuration Patterns

#### stdio Transport (Process-Based)
```json
{
  "mcpServers": {
    "local-filesystem": {
      "command": "/usr/bin/python3",
      "args": [
        "/opt/mcp-servers/filesystem/server.py",
        "--root", "/safe/directory"
      ],
      "env": {
        "PYTHONPATH": "/opt/mcp-servers",
        "LOG_LEVEL": "WARNING"
      },
      "cwd": "/opt/mcp-servers/filesystem",
      "timeout": 30000
    }
  }
}
```

**stdio Configuration Options**:
- **`command`**: Executable path (required)
- **`args`**: Command-line arguments array
- **`env`**: Environment variables with expansion support
- **`cwd`**: Working directory for the process
- **`timeout`**: Startup timeout in milliseconds

#### SSE (Server-Sent Events) Transport
```json
{
  "mcpServers": {
    "real-time-api": {
      "url": "https://api.example.com/mcp/sse",
      "transport": "sse",
      "headers": {
        "Authorization": "Bearer ${API_TOKEN}",
        "User-Agent": "Claude-Code/1.0",
        "X-Client-Version": "headless"
      },
      "reconnect": {
        "max_attempts": 5,
        "initial_delay": 1000,
        "max_delay": 30000,
        "backoff_multiplier": 2.0
      }
    }
  }
}
```

**SSE Configuration Options**:
- **`url`**: SSE endpoint URL (required)
- **`headers`**: Custom HTTP headers with variable expansion
- **`reconnect`**: Auto-reconnection settings
- **`timeout`**: Connection timeout settings

#### HTTP Transport with Streaming
```json
{
  "mcpServers": {
    "streaming-service": {
      "url": "https://streaming-api.example.com/mcp",
      "transport": "http",
      "streaming": true,
      "headers": {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream"
      },
      "oauth": {
        "discovery_url": "https://auth.example.com/.well-known/oauth",
        "client_id": "${OAUTH_CLIENT_ID}",
        "client_secret": "${OAUTH_CLIENT_SECRET}",
        "scope": "mcp:tools mcp:resources"
      }
    }
  }
}
```

**HTTP Configuration Options**:
- **`streaming`**: Enable chunked response handling
- **`oauth`**: OAuth 2.0 configuration for authentication
- **`retry`**: HTTP retry policy configuration

### Docker-Based MCP Servers

#### Basic Docker Configuration
```json
{
  "mcpServers": {
    "github-api": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "-e", "GITHUB_PERSONAL_ACCESS_TOKEN",
        "--network", "bridge",
        "--read-only",
        "--tmpfs", "/tmp:rw,noexec,nosuid,size=100m",
        "ghcr.io/github/github-mcp-server:sha-7aced2b"
      ],
      "env": {
        "GITHUB_PERSONAL_ACCESS_TOKEN": "${GITHUB_TOKEN}"
      }
    }
  }
}
```

#### Advanced Docker Configuration with Security
```json
{
  "mcpServers": {
    "secure-processor": {
      "command": "docker",
      "args": [
        "run",
        "-i",
        "--rm",
        "--security-opt", "no-new-privileges",
        "--cap-drop", "ALL",
        "--cap-add", "NET_BIND_SERVICE",
        "--user", "1000:1000",
        "--network", "none",
        "--read-only",
        "--tmpfs", "/tmp:rw,noexec,nosuid,size=50m",
        "-v", "${PROJECT_ROOT}:/data:ro",
        "-e", "DATA_PATH=/data",
        "-e", "OUTPUT_PATH=/tmp",
        "my-secure-mcp:latest"
      ],
      "env": {
        "PROJECT_ROOT": "${PWD}",
        "SECURITY_LEVEL": "high"
      }
    }
  }
}
```

**Docker Security Features**:
- **Process Isolation**: Complete process separation
- **Network Restrictions**: Configurable network access (`--network none`)
- **Filesystem Security**: Read-only mounts and tmpfs for temporary data
- **User Mapping**: Non-root user execution
- **Capability Dropping**: Minimal Linux capabilities
- **Resource Limits**: CPU, memory, and disk usage limits

### Environment Variable Expansion

#### Expansion Patterns
```json
{
  "mcpServers": {
    "dynamic-config": {
      "command": "${MCP_SERVER_PATH}/server",
      "args": ["--config", "${CONFIG_PATH}/server.conf"],
      "env": {
        "API_ENDPOINT": "${API_BASE_URL}/v1",
        "AUTH_TOKEN": "${SERVICE_TOKEN}",
        "PROJECT_NAME": "${PWD##*/}",
        "TIMESTAMP": "${TIMESTAMP}",
        "USER": "${USER}"
      }
    }
  }
}
```

**Supported Variable Sources**:
- **Environment Variables**: `${VAR_NAME}` expands to environment value
- **Shell Parameter Expansion**: `${PWD##*/}` for path manipulation
- **Default Values**: `${VAR_NAME:-default}` syntax
- **Conditional Expansion**: `${VAR_NAME:+value}` when variable is set

#### Environment-Specific Configurations
```bash
# Development environment
export MCP_ENV=development
export API_BASE_URL=https://dev-api.example.com
export LOG_LEVEL=DEBUG

# Production environment  
export MCP_ENV=production
export API_BASE_URL=https://api.example.com
export LOG_LEVEL=WARN
```

## GitHub Actions Integration

### Complete GitHub Actions Example

```yaml
name: Automated Code Analysis with MCP
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  claude-analysis:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      issues: write
      pull-requests: write

    steps:
      - name: Checkout repository
        uses: actions/checkout@v4
        with:
          fetch-depth: 0  # Full history for comprehensive analysis

      - name: Setup MCP Configuration
        run: |
          mkdir -p /tmp/mcp-config
          
          # GitHub MCP Server Configuration
          cat > /tmp/mcp-config/github-server.json << 'EOF'
          {
            "mcpServers": {
              "github": {
                "command": "docker",
                "args": [
                  "run", "-i", "--rm",
                  "--network", "bridge",
                  "-e", "GITHUB_PERSONAL_ACCESS_TOKEN",
                  "-e", "GITHUB_REPOSITORY",
                  "-e", "GITHUB_SHA",
                  "ghcr.io/github/github-mcp-server:sha-7aced2b"
                ],
                "env": {
                  "GITHUB_PERSONAL_ACCESS_TOKEN": "${{ secrets.GITHUB_TOKEN }}",
                  "GITHUB_REPOSITORY": "${{ github.repository }}",
                  "GITHUB_SHA": "${{ github.sha }}"
                }
              }
            }
          }
          EOF
          
          # Code Analysis MCP Server Configuration
          cat > /tmp/mcp-config/analysis-server.json << 'EOF'
          {
            "mcpServers": {
              "code-analyzer": {
                "command": "docker",
                "args": [
                  "run", "-i", "--rm",
                  "--read-only",
                  "--tmpfs", "/tmp:rw,noexec,nosuid,size=100m",
                  "-v", "${{ github.workspace }}:/code:ro",
                  "-e", "CODE_PATH=/code",
                  "my-org/code-analyzer-mcp:latest"
                ],
                "env": {
                  "CODE_PATH": "/code",
                  "ANALYSIS_LEVEL": "comprehensive"
                }
              }
            }
          }
          EOF

      - name: Create Analysis Prompt
        run: |
          mkdir -p /tmp/prompts
          cat > /tmp/prompts/analysis.txt << 'EOF'
          Analyze this pull request for:
          1. Code quality issues and potential bugs
          2. Security vulnerabilities
          3. Performance concerns
          4. Architectural improvements

          Use the GitHub MCP server to:
          - Get PR details and changed files
          - Review existing issues and discussions
          - Check for similar PRs or related work

          Use the code analyzer MCP server to:
          - Perform static code analysis
          - Check coding standards compliance
          - Identify potential optimizations

          Provide a comprehensive analysis with specific recommendations.
          EOF

      - name: Run Claude Code Analysis
        uses: anthropics/claude-code-base-action@beta
        with:
          prompt_file: /tmp/prompts/analysis.txt
          mcp_config: /tmp/mcp-config/github-server.json /tmp/mcp-config/analysis-server.json
          allowed_tools: |
            mcp__github__get_pull_request,
            mcp__github__get_pull_request_files,
            mcp__github__search_issues,
            mcp__github__list_pull_requests,
            mcp__code_analyzer__analyze_file,
            mcp__code_analyzer__check_standards,
            mcp__code_analyzer__security_scan
          timeout_minutes: 10
          anthropic_api_key: ${{ secrets.ANTHROPIC_API_KEY }}
          claude_env: |
            GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
            PR_NUMBER: ${{ github.event.number }}
            REPOSITORY: ${{ github.repository }}
```

### Multi-Environment MCP Configuration

```yaml
# Development environment
- name: Setup Development MCP
  if: github.ref == 'refs/heads/develop'
  run: |
    cat > /tmp/mcp-config/env-servers.json << 'EOF'
    {
      "mcpServers": {
        "dev-database": {
          "command": "python",
          "args": ["/scripts/dev-db-mcp.py"],
          "env": {
            "DB_HOST": "dev-db.internal",
            "DB_NAME": "development",
            "DEBUG": "true"
          }
        }
      }
    }
    EOF

# Production environment  
- name: Setup Production MCP
  if: github.ref == 'refs/heads/main'
  run: |
    cat > /tmp/mcp-config/env-servers.json << 'EOF'
    {
      "mcpServers": {
        "prod-monitoring": {
          "url": "https://monitoring-api.company.com/mcp",
          "transport": "https",
          "headers": {
            "Authorization": "Bearer ${{ secrets.MONITORING_TOKEN }}"
          }
        }
      }
    }
    EOF
```

## Authentication and Security

### Token-Based Authentication

#### API Token Configuration
```json
{
  "mcpServers": {
    "secure-api": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        "--security-opt", "no-new-privileges",
        "-e", "API_TOKEN",
        "-e", "API_ENDPOINT",
        "secure-mcp-server:latest"
      ],
      "env": {
        "API_TOKEN": "${SERVICE_API_TOKEN}",
        "API_ENDPOINT": "${API_BASE_URL}/v2"
      }
    }
  }
}
```

#### Multiple Token Support
```json
{
  "mcpServers": {
    "multi-service": {
      "command": "python",
      "args": ["/opt/mcp/multi-service.py"],
      "env": {
        "GITHUB_TOKEN": "${GITHUB_TOKEN}",
        "JIRA_TOKEN": "${JIRA_API_TOKEN}",
        "SLACK_TOKEN": "${SLACK_BOT_TOKEN}",
        "AWS_ACCESS_KEY_ID": "${AWS_ACCESS_KEY_ID}",
        "AWS_SECRET_ACCESS_KEY": "${AWS_SECRET_ACCESS_KEY}"
      }
    }
  }
}
```

### OAuth 2.0 Integration

#### Service Account Flow (Headless-Compatible)
```json
{
  "mcpServers": {
    "oauth-service": {
      "url": "https://api.service.com/mcp",
      "transport": "http",
      "oauth": {
        "grant_type": "client_credentials",
        "token_url": "https://auth.service.com/oauth/token",
        "client_id": "${OAUTH_CLIENT_ID}",
        "client_secret": "${OAUTH_CLIENT_SECRET}",
        "scope": "mcp:read mcp:write"
      }
    }
  }
}
```

#### Pre-Configured Token Flow
```bash
# Obtain token beforehand (manual or automated process)
export OAUTH_ACCESS_TOKEN=$(curl -s -X POST \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=${CLIENT_ID}&client_secret=${CLIENT_SECRET}" \
  https://auth.example.com/token | jq -r '.access_token')
```

```json
{
  "mcpServers": {
    "pre-auth-service": {
      "url": "https://api.service.com/mcp",
      "headers": {
        "Authorization": "Bearer ${OAUTH_ACCESS_TOKEN}"
      }
    }
  }
}
```

### Security Best Practices

#### Principle of Least Privilege
```json
{
  "mcpServers": {
    "restricted-server": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        // User restrictions
        "--user", "1000:1000",
        
        // Capability restrictions
        "--cap-drop", "ALL",
        "--cap-add", "NET_BIND_SERVICE",
        
        // Security options
        "--security-opt", "no-new-privileges",
        "--security-opt", "apparmor:docker-default",
        
        // Network restrictions
        "--network", "none",
        
        // Filesystem restrictions
        "--read-only",
        "--tmpfs", "/tmp:rw,noexec,nosuid,size=50m",
        
        // Resource limits
        "--memory", "256m",
        "--cpus", "0.5",
        
        // Environment
        "-e", "SECURITY_MODE=strict",
        "restricted-mcp:latest"
      ]
    }
  }
}
```

#### Secrets Management
```json
{
  "mcpServers": {
    "secrets-aware": {
      "command": "/opt/mcp/secrets-server",
      "env": {
        // Use external secret management
        "SECRET_SOURCE": "vault",
        "VAULT_ADDR": "${VAULT_ADDR}",
        "VAULT_TOKEN": "${VAULT_TOKEN}",
        
        // Or use secure environment variables
        "DB_PASSWORD_FILE": "/run/secrets/db_password",
        "API_KEY_FILE": "/run/secrets/api_key"
      }
    }
  }
}
```

## Communication Patterns and Data Flow

### Request-Response Pattern

#### Basic Tool Invocation
```
1. Claude Code → MCP Server
   POST /tools/invoke
   {
     "tool": "get_user_info",
     "arguments": {
       "user_id": "12345"
     }
   }

2. MCP Server → Claude Code
   HTTP 200 OK
   {
     "result": {
       "content": "User details...",
       "metadata": {
         "source": "database",
         "timestamp": "2024-01-15T10:30:00Z"
       }
     }
   }
```

#### Enhanced Response with Resources
```
MCP Server → Claude Code
{
  "result": {
    "content": "Analysis complete",
    "resource_links": [
      {
        "uri": "mcp://analyzer/report/abc123",
        "name": "Detailed Analysis Report",
        "description": "Comprehensive code quality analysis",
        "metadata": {
          "type": "analysis_report",
          "format": "json",
          "size": 2048
        }
      }
    ]
  }
}
```

### Streaming Communication Pattern

#### Server-Sent Events (SSE) Stream
```
Claude Code ← MCP Server (SSE)

event: start
data: {"task_id": "analysis_001", "status": "initiated"}

event: progress  
data: {"task_id": "analysis_001", "progress": 25, "message": "Analyzing file 1 of 4"}

event: progress
data: {"task_id": "analysis_001", "progress": 50, "message": "Analyzing file 2 of 4"}

event: result
data: {"task_id": "analysis_001", "partial_result": {"file": "app.py", "issues": [...]}}

event: complete
data: {"task_id": "analysis_001", "final_result": {"summary": "...", "total_issues": 5}}
```

#### HTTP Streaming (Chunked Transfer)
```
Claude Code → MCP Server
POST /tools/stream_analysis
Transfer-Encoding: chunked

MCP Server → Claude Code  
HTTP 200 OK
Transfer-Encoding: chunked

// Chunk 1
{"type": "start", "analysis_id": "xyz789"}

// Chunk 2  
{"type": "file_complete", "file": "main.py", "results": [...]}

// Chunk 3
{"type": "file_complete", "file": "utils.py", "results": [...]}

// Chunk 4
{"type": "complete", "summary": {...}}
```

### Bidirectional Communication

#### Resource Subscription Pattern
```json
// Claude Code requests resource subscription
{
  "action": "subscribe",
  "resource": "mcp://monitoring/system/alerts",
  "filters": {
    "severity": ["high", "critical"],
    "component": "api-server"
  }
}

// MCP Server acknowledges subscription
{
  "subscription_id": "sub_001",
  "status": "active"
}

// MCP Server pushes updates (SSE)
event: resource_update
data: {
  "subscription_id": "sub_001",
  "resource": "mcp://monitoring/system/alerts",
  "update": {
    "alert_id": "alert_456",
    "severity": "high",
    "message": "API response time exceeded threshold"
  }
}
```

#### Interactive Command Pattern
```json
// Claude Code initiates interactive command
{
  "tool": "interactive_debugger",
  "arguments": {
    "target": "production-server-01"
  }
}

// MCP Server requests input
{
  "status": "awaiting_input",
  "prompt": "Enter command to execute on production-server-01:",
  "input_id": "input_001"
}

// Claude Code provides input (through Claude's reasoning)
{
  "input_id": "input_001", 
  "command": "ps aux | grep python"
}

// MCP Server returns command result
{
  "input_id": "input_001",
  "result": "root  1234  0.0  5.2 python app.py\n..."
}
```

## Error Handling and Resilience

### Connection Failure Recovery

#### Automatic Retry Configuration
```json
{
  "mcpServers": {
    "resilient-service": {
      "url": "https://api.example.com/mcp",
      "transport": "sse",
      "retry": {
        "max_attempts": 5,
        "initial_delay": 1000,
        "max_delay": 30000,
        "backoff_multiplier": 2.0,
        "jitter": true
      },
      "health_check": {
        "enabled": true,
        "interval": 30000,
        "timeout": 5000,
        "endpoint": "/health"
      }
    }
  }
}
```

#### Circuit Breaker Pattern
```json
{
  "mcpServers": {
    "protected-service": {
      "url": "https://unreliable-api.example.com/mcp",
      "circuit_breaker": {
        "failure_threshold": 5,
        "recovery_timeout": 60000,
        "success_threshold": 2
      }
    }
  }
}
```

### Graceful Degradation

#### Fallback Server Configuration
```json
{
  "mcpServers": {
    "primary-service": {
      "url": "https://primary-api.example.com/mcp",
      "fallback": [
        {
          "url": "https://backup-api.example.com/mcp",
          "priority": 1
        },
        {
          "command": "python",
          "args": ["/local/fallback/server.py"],
          "priority": 2
        }
      ]
    }
  }
}
```

#### Timeout Management
```json
{
  "mcpServers": {
    "timeout-controlled": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "slow-server:latest"],
      "timeouts": {
        "startup": 30000,
        "tool_call": 120000,
        "shutdown": 10000
      }
    }
  }
}
```

### Error Classification and Handling

#### Error Response Format
```json
{
  "error": {
    "type": "authentication_failed",
    "code": "AUTH_001",
    "message": "API token has expired",
    "details": {
      "token_expiry": "2024-01-15T10:00:00Z",
      "refresh_url": "https://auth.example.com/refresh"
    },
    "recoverable": true,
    "retry_after": 60
  }
}
```

#### Error Handling Strategy
```json
{
  "mcpServers": {
    "error-aware": {
      "url": "https://api.example.com/mcp",
      "error_handling": {
        "authentication_failed": {
          "action": "refresh_token",
          "max_retries": 3
        },
        "rate_limited": {
          "action": "backoff",
          "respect_retry_after": true
        },
        "server_error": {
          "action": "retry",
          "max_retries": 2,
          "backoff": "exponential"
        },
        "client_error": {
          "action": "fail",
          "log_level": "error"
        }
      }
    }
  }
}
```

## Performance Optimization

### Connection Management

#### Connection Pooling
```json
{
  "mcpServers": {
    "high-throughput": {
      "url": "https://api.example.com/mcp",
      "connection_pool": {
        "max_connections": 10,
        "max_idle_time": 300000,
        "keep_alive": true
      }
    }
  }
}
```

#### Concurrent Request Handling
```json
{
  "mcpServers": {
    "parallel-processor": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "parallel-server:latest"],
      "concurrency": {
        "max_parallel_tools": 5,
        "queue_size": 20,
        "timeout_per_tool": 30000
      }
    }
  }
}
```

### Caching Strategies

#### Response Caching
```json
{
  "mcpServers": {
    "cached-service": {
      "url": "https://slow-api.example.com/mcp", 
      "cache": {
        "enabled": true,
        "ttl": 300000,
        "max_size": "50MB",
        "strategy": "lru",
        "cache_key_includes": ["tool", "arguments"]
      }
    }
  }
}
```

#### Resource Caching
```json
{
  "mcpServers": {
    "resource-heavy": {
      "command": "python",
      "args": ["/opt/heavy-processor.py"],
      "resource_cache": {
        "enabled": true,
        "max_resources": 100,
        "cache_duration": 600000,
        "preload_patterns": [
          "mcp://processor/templates/*",
          "mcp://processor/config/default"
        ]
      }
    }
  }
}
```

## Monitoring and Observability

### Health Monitoring

#### Server Health Checks
```bash
# Built-in health monitoring
claude mcp list
```

Output example:
```
MCP Servers Status:
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
github-api          ✅ Connected    (stdio)
code-analyzer       ✅ Connected    (http)
database-tools      ⚠️  Degraded     (sse)
monitoring-service  ❌ Disconnected (http)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Tools Available: 23
Resources Available: 7
Active Subscriptions: 2
```

#### Detailed Server Information
```bash
# Detailed MCP server inspection
claude /mcp
```

Output example:
```
📊 MCP Server Details

🔧 github-api (ghcr.io/github/github-mcp-server)
   Status: Connected
   Transport: stdio (docker)
   Tools: 8 available
   ├── mcp__github__get_issue
   ├── mcp__github__list_issues  
   ├── mcp__github__search_issues
   └── ...
   Resources: 3 available
   Last Activity: 2 minutes ago

⚡ code-analyzer (HTTP)
   Status: Connected
   URL: https://analyzer.internal/mcp
   Tools: 12 available
   Response Time: avg 245ms
   Success Rate: 98.5% (last 100 calls)
```

### Debug Mode

#### Comprehensive Debug Output
```bash
claude --mcp-debug -p "Analyze the codebase"
```

Debug output includes:
```
[MCP DEBUG] Starting MCP server initialization...
[MCP DEBUG] github-api: Starting docker container...
[MCP DEBUG] github-api: Container started with ID abc123...
[MCP DEBUG] github-api: Waiting for server handshake...
[MCP DEBUG] github-api: Handshake successful, 8 tools registered
[MCP DEBUG] github-api: Server ready in 2.3s

[MCP DEBUG] code-analyzer: Connecting to https://analyzer.internal/mcp...
[MCP DEBUG] code-analyzer: SSL handshake completed
[MCP DEBUG] code-analyzer: Authentication successful  
[MCP DEBUG] code-analyzer: 12 tools registered, 3 resources available

[MCP DEBUG] Tool call: mcp__github__get_issue(id="123")
[MCP DEBUG] github-api: Executing tool get_issue...
[MCP DEBUG] github-api: Tool completed in 0.8s
[MCP DEBUG] github-api: Result size: 2.1KB
```

### Logging Configuration

#### Structured Logging
```json
{
  "mcpServers": {
    "logged-service": {
      "command": "python",
      "args": ["/opt/server.py"],
      "logging": {
        "level": "INFO",
        "format": "structured",
        "output": "/var/log/mcp/service.log",
        "rotation": {
          "max_size": "10MB",
          "max_files": 5
        }
      }
    }
  }
}
```

#### Telemetry Integration
```json
{
  "mcpServers": {
    "monitored-service": {
      "url": "https://api.example.com/mcp",
      "telemetry": {
        "enabled": true,
        "endpoint": "https://telemetry.company.com/mcp",
        "api_key": "${TELEMETRY_API_KEY}",
        "metrics": [
          "request_count",
          "response_time", 
          "error_rate",
          "tool_usage"
        ],
        "interval": 60000
      }
    }
  }
}
```

## Advanced Use Cases

### Multi-Stage Processing Pipeline

```json
{
  "mcpServers": {
    "data-ingestion": {
      "command": "python",
      "args": ["/pipeline/ingestion.py"],
      "env": {
        "STAGE": "ingestion",
        "OUTPUT_QUEUE": "processing-queue"
      }
    },
    "data-processing": {
      "command": "python", 
      "args": ["/pipeline/processing.py"],
      "env": {
        "STAGE": "processing",
        "INPUT_QUEUE": "processing-queue",
        "OUTPUT_QUEUE": "analysis-queue"
      }
    },
    "data-analysis": {
      "command": "python",
      "args": ["/pipeline/analysis.py"],
      "env": {
        "STAGE": "analysis",
        "INPUT_QUEUE": "analysis-queue"
      }
    }
  }
}
```

### Conditional Server Loading

```json
{
  "mcpServers": {
    "conditional-server": {
      "enabled": "${ENABLE_ADVANCED_FEATURES:-false}",
      "command": "python",
      "args": ["/opt/advanced-server.py"],
      "env": {
        "FEATURE_FLAGS": "${FEATURE_FLAGS}",
        "LICENSE_KEY": "${ADVANCED_LICENSE_KEY}"
      }
    }
  }
}
```

### Cross-Server Communication

```json
{
  "mcpServers": {
    "coordinator": {
      "command": "python",
      "args": ["/opt/coordinator.py"],
      "env": {
        "MANAGED_SERVERS": "worker1,worker2,worker3",
        "COORDINATION_MODE": "leader"
      }
    },
    "worker1": {
      "command": "python",
      "args": ["/opt/worker.py"],
      "env": {
        "WORKER_ID": "worker1",
        "COORDINATOR_URL": "mcp://coordinator"
      }
    }
  }
}
```

## Troubleshooting Guide

### Common Issues and Solutions

#### 1. MCP Server Startup Failures

**Issue**: Server fails to start within timeout
```bash
[ERROR] MCP server 'github-api' failed to start within 30000ms
```

**Solutions**:
```json
{
  "mcpServers": {
    "github-api": {
      "timeout": 60000,  // Increase timeout
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        "--pull", "always",  // Ensure latest image
        "ghcr.io/github/github-mcp-server:latest"
      ]
    }
  }
}
```

#### 2. Permission Denied Errors

**Issue**: Tools are blocked even with MCP server configured
```bash
[ERROR] Tool 'mcp__github__get_issue' not permitted
```

**Solution**: Add MCP tools to allowed tools list
```yaml
# In GitHub Actions
allowed_tools: "mcp__github__get_issue,mcp__github__list_issues"
```

#### 3. Authentication Failures

**Issue**: API authentication fails
```bash
[ERROR] MCP server authentication failed: invalid token
```

**Solutions**:
```bash
# Check environment variable
echo $GITHUB_TOKEN

# Verify token permissions
curl -H "Authorization: token $GITHUB_TOKEN" https://api.github.com/user

# Update configuration with correct token
export GITHUB_TOKEN="ghp_newvalidtoken123"
```

#### 4. Network Connectivity Issues

**Issue**: Cannot connect to remote MCP server
```bash
[ERROR] Failed to connect to https://api.example.com/mcp: connection timeout
```

**Solutions**:
```json
{
  "mcpServers": {
    "api-service": {
      "url": "https://api.example.com/mcp",
      "timeout": 30000,
      "retry": {
        "max_attempts": 3,
        "initial_delay": 2000
      },
      "headers": {
        "User-Agent": "Claude-Code-Headless/1.0"
      }
    }
  }
}
```

#### 5. Docker Permission Issues

**Issue**: Docker commands fail in containerized environments
```bash
[ERROR] Cannot connect to Docker daemon
```

**Solutions**:
```yaml
# In GitHub Actions, use docker-in-docker
services:
  docker:
    image: docker:dind
    privileged: true

# Or use alternative container runtime
- name: Setup Podman
  run: |
    sudo apt-get install -y podman
    # Configure MCP to use podman instead of docker
```

### Performance Troubleshooting

#### Slow Tool Execution
```bash
# Enable detailed timing
claude --mcp-debug -p "your prompt" 2>&1 | grep -E "(Tool|timing|duration)"
```

#### Memory Usage Issues
```bash
# Monitor container resource usage
docker stats --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}"
```

#### Connection Pooling Problems
```json
{
  "mcpServers": {
    "optimized-server": {
      "url": "https://api.example.com/mcp",
      "connection_pool": {
        "max_connections": 20,
        "connection_timeout": 10000,
        "idle_timeout": 30000
      }
    }
  }
}
```

## Best Practices Summary

### Configuration Management
1. **Version Control**: Commit `.mcp.json` files for reproducible environments
2. **Environment Variables**: Use variable expansion for environment-specific values
3. **Validation**: Test configurations with `claude --mcp-debug`
4. **Documentation**: Document custom MCP servers and their purposes

### Security
1. **Principle of Least Privilege**: Minimal permissions and capabilities
2. **Container Isolation**: Use Docker for untrusted MCP servers
3. **Secret Management**: Secure handling of API tokens and credentials
4. **Network Restrictions**: Limit network access when possible

### Performance
1. **Connection Reuse**: Configure connection pooling for HTTP servers
2. **Timeout Management**: Set appropriate timeouts for different environments
3. **Resource Caching**: Cache expensive operations and resources
4. **Parallel Processing**: Enable concurrent tool execution when safe

### Reliability
1. **Error Handling**: Implement retry logic and graceful degradation
2. **Health Monitoring**: Regular health checks and status monitoring
3. **Fallback Strategies**: Configure backup servers or local alternatives
4. **Logging**: Comprehensive logging for troubleshooting

This comprehensive guide provides the foundation for successfully implementing MCP integration in Claude Code's headless mode, enabling powerful automation scenarios while maintaining security and reliability.