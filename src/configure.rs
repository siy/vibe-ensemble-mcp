use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::lockfile::LockFileManager;
use crate::mcp::constants::{build_claude_permissions, build_mcp_config};
use crate::permissions::PermissionMode;

/// Generate Claude Code integration files
pub async fn configure_claude_code(
    host: &str,
    port: u16,
    permission_mode: PermissionMode,
) -> Result<()> {
    println!("🔧 Configuring Claude Code integration...");

    // Client mode: Check if Claude IDE lock file exists and validate workspace folder
    let lock_manager = LockFileManager::new(host.to_string(), port);
    let websocket_token = match lock_manager.validate_claude_lock_file_for_client() {
        Ok(token) => {
            println!("📖 Using existing WebSocket token from Claude IDE lock file");
            token
        }
        Err(e) => {
            println!("❌ Error: {}", e);
            println!(
                "💡 Hint: Start the vibe-ensemble server first, then run --configure-claude-code"
            );
            return Err(anyhow::anyhow!("Claude IDE lock file validation failed"));
        }
    };

    // Create .mcp.json file with WebSocket auth
    create_mcp_config(host, port, &websocket_token).await?;

    // Create .claude directory and files
    create_claude_directory().await?;
    create_claude_settings().await?;
    create_vibe_ensemble_command(host, port).await?;
    create_coordinator_commands().await?;
    create_worker_templates().await?;

    // Note: WebSocket token file is no longer needed and not generated

    // Note: Worker permissions are now generated per-project during project creation
    // to support project-specific permission isolation

    println!("✅ Claude Code integration configured successfully!");
    println!("📁 Configuration completed with preservation of existing customizations:");
    println!("📄 Updated existing file:");
    println!(
        "  - ~/.claude/ide/{}.lock (added current workspace folder)",
        port
    );

    if permission_mode == PermissionMode::File {
        println!(
            "📝 Note: Worker permissions will be generated automatically when creating projects"
        );
    }

    println!();
    println!("📝 Configuration Preservation:");
    println!("  • Existing files are preserved with customizations intact");
    println!("  • Only .mcp.json port configuration is updated when changed");
    println!("  • Missing files are created with default templates");
    println!();
    println!("🚀 To use with Claude Code:");
    println!(
        "  1. Start the vibe-ensemble server: vibe-ensemble-mcp --host {} --port {} --permission-mode {}",
        host, port, permission_mode.as_str()
    );
    println!("  2. Open Claude Code in this directory");
    println!("  3. Run the 'vibe-ensemble' command to initialize as coordinator");
    println!();
    println!("🔄 Real-Time Communication Features:");
    println!("  • WebSocket transport enabled for real-time event streaming");
    println!("  • JSON-RPC notifications for instant coordination");
    println!("  • 28 MCP tools for comprehensive project coordination");
    println!("  • Server-Sent Events (SSE) and WebSocket event broadcasting");

    Ok(())
}

async fn create_mcp_config(host: &str, port: u16, _websocket_token: &str) -> Result<()> {
    let config_path = ".mcp.json";

    // If config exists, preserve user customizations and only update port
    if Path::new(config_path).exists() {
        match fs::read_to_string(config_path) {
            Ok(existing_content) => {
                match serde_json::from_str::<serde_json::Value>(&existing_content) {
                    Ok(mut existing_config) => {
                        // Update only the URL to reflect new port, preserve everything else
                        if let Some(mcp_servers) = existing_config.get_mut("mcpServers") {
                            if let Some(vibe_server) = mcp_servers.get_mut("vibe-ensemble-mcp") {
                                if let Some(url) = vibe_server.get_mut("url") {
                                    *url = serde_json::Value::String(format!(
                                        "http://{}:{}/mcp",
                                        host, port
                                    ));
                                }
                            }
                        }
                        fs::write(config_path, serde_json::to_string_pretty(&existing_config)?)?;
                        println!(
                            "  ✓ Updated .mcp.json port configuration (preserved customizations)"
                        );
                        return Ok(());
                    }
                    Err(_) => {
                        println!(
                            "  ⚠ Warning: .mcp.json exists but couldn't parse, creating new one"
                        );
                    }
                }
            }
            Err(_) => {
                println!("  ⚠ Warning: .mcp.json exists but couldn't read, creating new one");
            }
        }
    }

    // Create new config if doesn't exist or couldn't parse existing
    let config = build_mcp_config(host, port);
    fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
    println!("  ✓ Created new .mcp.json configuration");
    Ok(())
}

async fn create_claude_directory() -> Result<()> {
    fs::create_dir_all(".claude/commands")?;
    fs::create_dir_all(".claude/worker-templates")?;
    Ok(())
}

// Removed: create_websocket_token() - websocket token is no longer needed

// Removed: create_file_permissions() - permissions are now generated per-project

async fn create_claude_settings() -> Result<()> {
    let settings_path = ".claude/settings.local.json";

    // If settings exist, preserve them
    if Path::new(settings_path).exists() {
        println!("  ✓ Preserved existing .claude/settings.local.json");
        return Ok(());
    }

    // Create new settings if they don't exist
    let settings = build_claude_permissions();
    fs::write(settings_path, serde_json::to_string_pretty(&settings)?)?;
    println!("  ✓ Created new .claude/settings.local.json");
    Ok(())
}

async fn create_vibe_ensemble_command(host: &str, port: u16) -> Result<()> {
    let command_path = ".claude/commands/vibe-ensemble.md";

    // If command exists, preserve it
    if Path::new(command_path).exists() {
        println!("  ✓ Preserved existing vibe-ensemble.md command");
        return Ok(());
    }

    // Create new command if it doesn't exist
    let template_content = include_str!("../templates/coordinator_command.md");
    let command_content = template_content
        .replace("{host}", host)
        .replace("{port}", &port.to_string());

    fs::write(command_path, command_content)?;
    println!("  ✓ Created new vibe-ensemble.md command");
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

    // Check and write each template individually (only if missing)
    for (filename, content) in templates {
        let template_path = format!(".claude/worker-templates/{}", filename);
        if !std::path::Path::new(&template_path).exists() {
            fs::write(&template_path, content)?;
            println!("  ✓ Created missing template: {}", filename);
        }
    }

    Ok(())
}

/// Load a worker template from disk, with fallback to embedded version
pub fn load_worker_template(template_name: &str) -> Result<String> {
    load_worker_template_from_directory(template_name, None)
}

/// Load a worker template from disk in specified directory, with fallback to embedded version
pub fn load_worker_template_from_directory(
    template_name: &str,
    working_directory: Option<&str>,
) -> Result<String> {
    let base_dir = working_directory.unwrap_or(".");
    let template_path = format!("{}/.claude/worker-templates/{}.md", base_dir, template_name);

    // Try to load from disk first
    if Path::new(&template_path).exists() {
        match fs::read_to_string(&template_path) {
            Ok(content) => return Ok(content),
            Err(e) => {
                eprintln!(
                    "Warning: Failed to read template from disk ({}), using embedded version: {}",
                    template_path, e
                );
            }
        }
    }

    // Fallback to embedded templates
    let embedded_content = match template_name {
        "planning" => include_str!("../templates/worker-templates/planning.md"),
        "design" => include_str!("../templates/worker-templates/design.md"),
        "implementation" => include_str!("../templates/worker-templates/implementation.md"),
        "testing" => include_str!("../templates/worker-templates/testing.md"),
        "review" => include_str!("../templates/worker-templates/review.md"),
        "deployment" => include_str!("../templates/worker-templates/deployment.md"),
        "research" => include_str!("../templates/worker-templates/research.md"),
        "documentation" => include_str!("../templates/worker-templates/documentation.md"),
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown worker template: {}",
                template_name
            ))
        }
    };

    Ok(embedded_content.to_string())
}

/// List available worker templates
pub fn list_worker_templates() -> Vec<String> {
    vec![
        "planning".to_string(),
        "design".to_string(),
        "implementation".to_string(),
        "testing".to_string(),
        "review".to_string(),
        "deployment".to_string(),
        "research".to_string(),
        "documentation".to_string(),
    ]
}

/// Ensure all worker templates exist on disk (create missing ones)
pub fn ensure_worker_templates_exist() -> Result<()> {
    ensure_worker_templates_exist_in_directory(None)
}

/// Ensure all worker templates exist on disk in specified directory (create missing ones)
pub fn ensure_worker_templates_exist_in_directory(working_directory: Option<&str>) -> Result<()> {
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

    // Determine the base directory
    let base_dir = working_directory.unwrap_or(".");
    let templates_dir = format!("{}/.claude/worker-templates", base_dir);

    // Create .claude/worker-templates directory if it doesn't exist
    fs::create_dir_all(&templates_dir)?;

    // Check and create missing templates
    let mut created_count = 0;
    for (filename, content) in templates {
        let template_path = format!("{}/{}", templates_dir, filename);
        if !Path::new(&template_path).exists() {
            fs::write(&template_path, content)?;
            created_count += 1;
        }
    }

    if created_count > 0 {
        println!("✓ Created {} missing worker templates", created_count);
    }

    Ok(())
}

async fn create_coordinator_commands() -> Result<()> {
    let commands = vec![
        (
            "vibe-events.md",
            include_str!("../templates/commands/vibe-events.md"),
        ),
        (
            "vibe-status.md",
            include_str!("../templates/commands/vibe-status.md"),
        ),
        (
            "vibe-poll.md",
            include_str!("../templates/commands/vibe-poll.md"),
        ),
    ];

    for (filename, content) in commands {
        let command_path = format!(".claude/commands/{}", filename);
        if Path::new(&command_path).exists() {
            println!("  ✓ Preserved existing {} command", filename);
        } else {
            fs::write(&command_path, content)?;
            println!("  ✓ Created new {} command", filename);
        }
    }

    Ok(())
}
