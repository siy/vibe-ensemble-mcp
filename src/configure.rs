use anyhow::Result;
use std::fs;
use uuid::Uuid;

use crate::lockfile::LockFileManager;
use crate::mcp::constants::build_mcp_config;
use crate::permissions::{ClaudePermissions, ClaudeSettings, PermissionMode};

/// Generate Claude Code integration files
pub async fn configure_claude_code(
    host: &str,
    port: u16,
    permission_mode: PermissionMode,
) -> Result<()> {
    println!("🔧 Configuring Claude Code integration...");

    // Try to read existing lock file first, generate new token if not found
    let lock_manager = LockFileManager::new(host.to_string(), port);
    let websocket_token = match lock_manager.read_lock_file() {
        Ok(lock_file) => {
            println!("📖 Using existing WebSocket token from lock file");
            lock_file.token
        }
        Err(_) => {
            println!("🔑 Generating new WebSocket authentication token");
            Uuid::new_v4().to_string()
        }
    };

    // Create .mcp.json file with WebSocket auth
    create_mcp_config(host, port, &websocket_token).await?;

    // Create .claude directory and files
    create_claude_directory().await?;
    create_claude_settings().await?;
    create_vibe_ensemble_command(host, port).await?;
    create_worker_templates().await?;

    // Create WebSocket token file
    create_websocket_token(&websocket_token).await?;

    // Create Claude Code discovery lock file if we have server info
    create_claude_code_lock_file(host, port, &websocket_token).await?;

    // Handle file permission mode
    if permission_mode == PermissionMode::File {
        create_file_permissions().await?;
    }

    println!("✅ Claude Code integration configured successfully!");
    println!("📁 Generated files:");
    println!("  - .mcp.json (MCP server configuration with WebSocket support)");
    println!("  - .claude/settings.local.json (Claude settings)");
    println!("  - .claude/commands/vibe-ensemble.md (Coordinator initialization)");
    println!("  - .claude/worker-templates/ (8 high-quality worker templates)");
    println!("  - .claude/websocket-token (WebSocket authentication token)");
    println!(
        "  - ~/.claude/ide/{}.lock (Claude Code discovery file)",
        port
    );

    if permission_mode == PermissionMode::File {
        println!("  - .vibe-ensemble-mcp/worker-permissions.json (File-based permissions)");
    }

    println!();
    println!("🚀 To use with Claude Code:");
    println!(
        "  1. Start the vibe-ensemble server: vibe-ensemble-mcp --host {} --port {} --permission-mode {}",
        host, port, permission_mode.as_str()
    );
    println!("  2. Open Claude Code in this directory");
    println!("  3. Run the 'vibe-ensemble' command to initialize as coordinator");
    println!();
    println!("🔄 Bidirectional Communication Features:");
    println!("  • WebSocket transport enabled for real-time collaboration");
    println!("  • Server-initiated tool calls to clients");
    println!("  • Workflow orchestration and parallel execution");
    println!("  • Client tool registration and discovery");
    println!("  • 15 new MCP tools for bidirectional coordination");

    Ok(())
}

async fn create_mcp_config(host: &str, port: u16, websocket_token: &str) -> Result<()> {
    let mut config = build_mcp_config(host, port);

    // Add WebSocket authentication to the configuration
    if let Some(servers) = config.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        if let Some(ws_server) = servers
            .get_mut("vibe-ensemble-ws")
            .and_then(|v| v.as_object_mut())
        {
            ws_server.insert(
                "auth".to_string(),
                serde_json::json!({
                    "type": "token",
                    "token_file": ".claude/websocket-token",
                    "token": websocket_token
                }),
            );
        }
    }

    fs::write(".mcp.json", serde_json::to_string_pretty(&config)?)?;
    Ok(())
}

async fn create_claude_directory() -> Result<()> {
    fs::create_dir_all(".claude/commands")?;
    fs::create_dir_all(".claude/worker-templates")?;
    fs::create_dir_all(".vibe-ensemble-mcp")?;
    Ok(())
}

async fn create_websocket_token(token: &str) -> Result<()> {
    fs::write(".claude/websocket-token", token)?;
    Ok(())
}

async fn create_file_permissions() -> Result<()> {
    let settings = ClaudeSettings {
        permissions: ClaudePermissions::balanced(),
    };

    fs::write(
        ".vibe-ensemble-mcp/worker-permissions.json",
        serde_json::to_string_pretty(&settings)?,
    )?;
    Ok(())
}

async fn create_claude_settings() -> Result<()> {
    let settings = ClaudeSettings {
        permissions: ClaudePermissions::minimal(),
    };

    fs::write(
        ".claude/settings.local.json",
        serde_json::to_string_pretty(&settings)?,
    )?;
    Ok(())
}

async fn create_vibe_ensemble_command(host: &str, port: u16) -> Result<()> {
    let template_content = include_str!("../templates/coordinator_command.md");
    let command_content = template_content
        .replace("{host}", host)
        .replace("{port}", &port.to_string());

    fs::write(".claude/commands/vibe-ensemble.md", command_content)?;
    Ok(())
}

async fn create_worker_templates() -> Result<()> {
    // Load templates from external files using include_str!
    let templates = vec![
        (
            "planning.md",
            include_str!("../templates/worker-templates/planning.md"),
        ),
        (
            "design.md",
            include_str!("../templates/worker-templates/design.md"),
        ),
        (
            "implementation.md",
            include_str!("../templates/worker-templates/implementation.md"),
        ),
        (
            "testing.md",
            include_str!("../templates/worker-templates/testing.md"),
        ),
        (
            "review.md",
            include_str!("../templates/worker-templates/review.md"),
        ),
        (
            "deployment.md",
            include_str!("../templates/worker-templates/deployment.md"),
        ),
        (
            "research.md",
            include_str!("../templates/worker-templates/research.md"),
        ),
        (
            "documentation.md",
            include_str!("../templates/worker-templates/documentation.md"),
        ),
    ];

    // Create .claude/worker-templates directory
    fs::create_dir_all(".claude/worker-templates")?;

    // Write all templates to files
    for (filename, content) in templates {
        fs::write(format!(".claude/worker-templates/{}", filename), content)?;
    }

    Ok(())
}

async fn create_claude_code_lock_file(_host: &str, port: u16, websocket_token: &str) -> Result<()> {
    // Create Claude Code discovery lock file at ~/.claude/ide/{port}.lock
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

    let claude_ide_dir = home_dir.join(".claude").join("ide");
    fs::create_dir_all(&claude_ide_dir)?;

    let lock_file_path = claude_ide_dir.join(format!("{}.lock", port));

    // Get current working directory for workspaceFolders
    let current_dir = std::env::current_dir()?;

    let claude_lock_data = serde_json::json!({
        "pid": std::process::id(),
        "workspaceFolders": [current_dir.to_string_lossy()],
        "ideName": "Vibe Ensemble MCP",
        "transport": "ws",
        "authToken": websocket_token
    });

    fs::write(
        &lock_file_path,
        serde_json::to_string_pretty(&claude_lock_data)?,
    )?;
    println!(
        "📍 Created Claude Code discovery file: {}",
        lock_file_path.display()
    );

    Ok(())
}
