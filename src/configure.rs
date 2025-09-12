use anyhow::Result;
use serde_json::json;
use std::fs;

/// Generate Claude Code integration files
pub async fn configure_claude_code(host: &str, port: u16) -> Result<()> {
    println!("🔧 Configuring Claude Code integration...");

    // Create .mcp.json file
    create_mcp_config(host, port).await?;

    // Create .claude directory and files
    create_claude_directory().await?;
    create_claude_settings().await?;
    create_vibe_ensemble_command(host, port).await?;

    println!("✅ Claude Code integration configured successfully!");
    println!("📁 Generated files:");
    println!("  - .mcp.json (MCP server configuration)");
    println!("  - .claude/settings.local.json (Claude settings)");
    println!("  - .claude/commands/vibe-ensemble.md (Coordinator initialization)");
    println!();
    println!("🚀 To use with Claude Code:");
    println!(
        "  1. Start the vibe-ensemble server: vibe-ensemble-mcp --host {} --port {}",
        host, port
    );
    println!("  2. Open Claude Code in this directory");
    println!("  3. Run the 'vibe-ensemble' command to initialize as coordinator");

    Ok(())
}

async fn create_mcp_config(host: &str, port: u16) -> Result<()> {
    let config = json!({
        "mcpServers": {
            "vibe-ensemble-mcp": {
                "type": "http",
                "url": format!("http://{}:{}/mcp", host, port),
                "protocol_version": "2024-11-05"
            },
            "vibe-ensemble-sse": {
                "type": "sse",
                "url": format!("http://{}:{}/sse", host, port),
                "protocol_version": "2024-11-05"
            }
        }
    });

    fs::write(".mcp.json", serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

async fn create_claude_directory() -> Result<()> {
    fs::create_dir_all(".claude/commands")?;
    Ok(())
}

async fn create_claude_settings() -> Result<()> {
    let settings = json!({
        "permissions": {
            "allow": [
                "mcp__vibe-ensemble-mcp__create_project",
                "mcp__vibe-ensemble-mcp__list_projects",
                "mcp__vibe-ensemble-mcp__get_project",
                "mcp__vibe-ensemble-mcp__update_project",
                "mcp__vibe-ensemble-mcp__delete_project",
                "mcp__vibe-ensemble-mcp__spawn_worker_for_stage",
                "mcp__vibe-ensemble-mcp__stop_worker",
                "mcp__vibe-ensemble-mcp__list_workers",
                "mcp__vibe-ensemble-mcp__get_worker_status",
                "mcp__vibe-ensemble-mcp__finish_worker",
                "mcp__vibe-ensemble-mcp__create_worker_type",
                "mcp__vibe-ensemble-mcp__list_worker_types",
                "mcp__vibe-ensemble-mcp__get_worker_type",
                "mcp__vibe-ensemble-mcp__update_worker_type",
                "mcp__vibe-ensemble-mcp__delete_worker_type",
                "mcp__vibe-ensemble-mcp__create_ticket",
                "mcp__vibe-ensemble-mcp__get_ticket",
                "mcp__vibe-ensemble-mcp__list_tickets",
                "mcp__vibe-ensemble-mcp__get_tickets_by_stage",
                "mcp__vibe-ensemble-mcp__add_ticket_comment",
                "mcp__vibe-ensemble-mcp__update_ticket_stage",
                "mcp__vibe-ensemble-mcp__close_ticket",
                "mcp__vibe-ensemble-mcp__list_events"
            ],
            "vibe-ensemble-mcp": {
                "tools": {
                    // Project Management Tools
                    "create_project": "allowed",
                    "list_projects": "allowed",
                    "get_project": "allowed",
                    "update_project": "allowed",
                    "delete_project": "allowed",

                    // Worker Management Tools
                    "spawn_worker_for_stage": "allowed",
                    "stop_worker": "allowed",
                    "list_workers": "allowed",
                    "get_worker_status": "allowed",
                    "finish_worker": "allowed",

                    // Worker Type Management Tools
                    "create_worker_type": "allowed",
                    "list_worker_types": "allowed",
                    "get_worker_type": "allowed",
                    "update_worker_type": "allowed",
                    "delete_worker_type": "allowed",

                    // Ticket Management Tools
                    "create_ticket": "allowed",
                    "get_ticket": "allowed",
                    "list_tickets": "allowed",
                    "get_tickets_by_stage": "allowed",
                    "add_ticket_comment": "allowed",
                    "update_ticket_stage": "allowed",
                    "close_ticket": "allowed",

                    // Event Management Tools
                    "list_events": "allowed"
                }
            },
            "vibe-ensemble-sse": {
                "tools": {
                    "*": "allowed"
                }
            }
        },
        "enabledMcpjsonServers": [
            "vibe-ensemble-mcp",
            "vibe-ensemble-sse"
        ]
    });

    fs::write(
        ".claude/settings.local.json",
        serde_json::to_string_pretty(&settings)?,
    )?;
    Ok(())
}

async fn create_vibe_ensemble_command(host: &str, port: u16) -> Result<()> {
    let command_content = format!(
        r#"# Vibe-Ensemble Coordinator Initialization

**System:** You are a coordinator in the vibe-ensemble multi-agent system. Your primary role is to:

## CORE RESPONSIBILITIES

### 1. PROJECT MANAGEMENT
- Create and manage projects using `create_project(name, path, description)`
- Define worker types with specialized system prompts using `create_worker_type()`
- Monitor project progress through events and worker status

### 2. TASK DELEGATION (PRIMARY BEHAVIOR - ABSOLUTE RULE)
- **DELEGATE EVERYTHING - NO EXCEPTIONS**: Break down requests into specific, actionable tickets
- **NEVER** perform any technical work yourself (writing code, analyzing files, setting up projects, etc.)
- **ALWAYS** create tickets for ALL work, even simple tasks like "create a folder" or "write README"
- Create tickets with minimal initial pipeline: start with just ["planning"] stage
- Let planning workers extend pipelines based on their analysis

### 3. COORDINATION WORKFLOW
1. Analyze incoming requests
2. Break into discrete tickets with clear objectives  
3. Create tickets using `create_ticket()` with minimal pipeline: ["planning"]
4. Define worker types for each stage using `create_worker_type()` if not exists
5. Use `spawn_worker_for_stage()` to spawn workers manually when needed
6. Monitor progress via `list_events()` and `get_tickets_by_stage()`
7. Workers extend pipelines and coordinate stage transitions through JSON outputs

### 4. MONITORING & OVERSIGHT
- Track ticket progress and worker status
- Ensure proper task sequencing and dependencies
- Handle escalations and blocked tasks
- Maintain project documentation through delegation

## DELEGATION EXAMPLES

**User Request:** "Add a login feature to my React app"
**Coordinator Action:**
1. Create ticket: "Implement user authentication system" (starts in "planning" stage)
2. Ensure "planning" worker type exists for requirements analysis
3. Monitor for stage progression to "design", "coding", "testing", etc.
4. Coordinate through automatic worker spawning for each stage

**User Request:** "Fix this bug in my code"
**Coordinator Action:**
1. Create ticket: "Investigate and fix [specific bug]" (starts in "planning" stage)  
2. Ensure appropriate worker types exist for each stage in the pipeline
3. Monitor automatic stage transitions via worker JSON outputs

## AVAILABLE TOOLS
- Project: create_project, get_project, list_projects, update_project, delete_project
- Worker Types: create_worker_type, list_worker_types, get_worker_type, update_worker_type, delete_worker_type
- Workers: spawn_worker_for_stage, stop_worker, list_workers, get_worker_status, finish_worker
- Tickets: create_ticket, get_ticket, list_tickets, get_tickets_by_stage, add_ticket_comment, update_ticket_stage, close_ticket
- Events: list_events

## CONNECTION INFO
- Server: http://{}:{}
- MCP Endpoint: http://{}:{}/mcp
- SSE Endpoint: http://{}:{}/sse

## 🚨 CRITICAL ENFORCEMENT: ABSOLUTE DELEGATION RULE

**⚠️ COORDINATORS ARE STRICTLY FORBIDDEN FROM ANY TECHNICAL WORK ⚠️**

### ❌ NEVER DO THESE (Create Tickets Instead):
- Write code, scripts, or configurations (even simple ones)
- Analyze files, requirements, or technical issues
- Set up project structures, folders, or files
- Install dependencies or configure tools
- Debug problems or troubleshoot issues
- Test features or run validations
- Create documentation, README files, or guides
- Research solutions or investigate approaches
- Read or examine existing code/files
- Perform ANY hands-on technical tasks

### ✅ COORDINATORS ONLY DO:
- Create projects with `create_project`
- Define worker types with `create_worker_type` 
- Create tickets for ALL work (no matter how simple) - all tickets start in "planning" stage
- Monitor progress with `list_events` and `get_tickets_by_stage`
- Workers automatically spawn for stages that have open tickets

**ABSOLUTE RULE: Even tasks that seem "too simple" like "create a folder" or "write one line of code" MUST be delegated through tickets. Your role is 100% orchestration - workers handle 100% of execution.**

**Remember:** You coordinate and delegate. Workers implement. Focus on breaking down complex requests into manageable tickets and ensuring smooth handoffs between specialized workers.
"#,
        host, port, host, port, host, port
    );

    fs::write(".claude/commands/vibe-ensemble.md", command_content)?;
    Ok(())
}
