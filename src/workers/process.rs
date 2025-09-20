use anyhow::{Context, Result};
use std::fs;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

use super::queue::WorkerOutput;
use super::types::SpawnWorkerRequest;
use crate::permissions::{load_permissions, ClaudePermissions, PermissionMode};

pub struct ProcessManager;

impl ProcessManager {
    /// Apply permissions to Claude command based on mode
    fn apply_permissions_to_command(
        cmd: &mut Command,
        permission_mode: &str,
        project_path: &str,
    ) -> Result<()> {
        let mode: PermissionMode = permission_mode.parse()?;

        match mode {
            PermissionMode::Bypass => {
                debug!("Using bypass mode - adding --dangerously-skip-permissions");
                cmd.arg("--dangerously-skip-permissions");
            }
            PermissionMode::Inherit | PermissionMode::File => {
                debug!("Using {} mode", mode.as_str());
                if let Some(permissions) = load_permissions(mode, project_path)? {
                    info!(
                        "Loaded permissions with {} allowed, {} denied tools",
                        permissions.allow.len(),
                        permissions.deny.len()
                    );
                    Self::add_permission_args(cmd, &permissions);
                } else {
                    debug!("No permissions loaded for mode: {}", mode.as_str());
                }
            }
        }
        Ok(())
    }

    /// Add --allowedTools and --disallowedTools arguments to command
    fn add_permission_args(cmd: &mut Command, permissions: &ClaudePermissions) {
        // Add allowed tools
        if !permissions.allow.is_empty() {
            cmd.arg("--allowedTools");
            for tool in &permissions.allow {
                cmd.arg(tool);
            }
            debug!("Added {} allowed tools", permissions.allow.len());
        }

        // Add disallowed tools
        if !permissions.deny.is_empty() {
            cmd.arg("--disallowedTools");
            for tool in &permissions.deny {
                cmd.arg(tool);
            }
            debug!("Added {} disallowed tools", permissions.deny.len());
        }

        // Note: We don't handle "ask" permissions since workers run headless
        if !permissions.ask.is_empty() {
            warn!(
                "Worker has {} 'ask' permissions that will be ignored in headless mode",
                permissions.ask.len()
            );
        }
    }

    /// Parse worker JSON output from the result string within Claude CLI JSON wrapper
    fn parse_worker_output_from_result(result_str: &str) -> Result<WorkerOutput> {
        debug!("Parsing worker output from result string: {}", result_str);

        // Look for JSON code blocks (```json ... ```) in the result
        if let Some(json_start) = result_str.find("```json") {
            let search_start = json_start + 7; // Skip past "```json"
            if let Some(json_end_relative) = result_str[search_start..].find("```") {
                let json_end = search_start + json_end_relative;
                let json_block = result_str[search_start..json_end].trim();
                debug!("Found JSON in code block: {}", json_block);
                return serde_json::from_str::<WorkerOutput>(json_block)
                    .with_context(|| "Failed to parse WorkerOutput from JSON code block");
            }
        }

        Err(anyhow::anyhow!(
            "No valid JSON code block found in worker result. Workers must output JSON in ```json...``` blocks."
        ))
    }

    fn create_mcp_config(
        project_path: &str,
        worker_id: &str,
        host: &str,
        server_port: u16,
    ) -> Result<String> {
        debug!(
            "Creating MCP config for worker {} in project path: {}",
            worker_id, project_path
        );

        use crate::mcp::constants::build_mcp_config;
        let config = build_mcp_config(host, server_port);
        debug!("MCP config JSON created successfully");

        let config_path = format!("{}/worker_{}_mcp_config.json", project_path, worker_id);
        debug!("Target config file path: {}", config_path);

        debug!("Serializing config to pretty JSON...");
        let config_json = serde_json::to_string_pretty(&config)
            .with_context(|| "Failed to serialize MCP config to JSON")?;
        debug!(
            "JSON serialization successful, length: {} bytes",
            config_json.len()
        );

        debug!("Writing config file to: {}", config_path);
        fs::write(&config_path, config_json)
            .with_context(|| format!("Failed to write MCP config to {}", config_path))?;
        debug!("File write successful");

        info!("Generated MCP config file: {}", config_path);
        Ok(config_path)
    }

    pub async fn spawn_worker(request: SpawnWorkerRequest) -> Result<WorkerOutput> {
        info!(
            "Spawning worker: {} for ticket: {} (project: {}, type: {})",
            request.worker_id, request.ticket_id, request.project_id, request.worker_type
        );

        // Create MCP config file
        let config_path = Self::create_mcp_config(
            &request.project_path,
            &request.worker_id,
            &request.server_host,
            request.server_port,
        )?;

        // Create system prompt that includes ticket_id
        let system_prompt = format!(
            "{}\n\n=== CRITICAL OUTPUT REQUIREMENT ===\nYou are working on ticket_id: {}\n\nIMPORTANT: You MUST end your response with a valid JSON block that the system can parse. This JSON determines what happens next to the ticket.\n\n🔐 PERMISSION HANDLING:\nIf you encounter permission restrictions while attempting to use tools:\n1. NEVER use \"error\" outcome - use \"coordinator_attention\" instead\n2. Include detailed information about which specific tool(s) you need access to\n3. Explain what you were trying to accomplish and why that tool is necessary\n4. The coordinator will handle communicating with the user about permission updates\n\nEXAMPLE for permission issues:\n```json\n{{\n  \"ticket_id\": \"{}\",\n  \"outcome\": \"coordinator_attention\",\n  \"target_stage\": null,\n  \"pipeline_update\": null,\n  \"comment\": \"Need permission to access required tools\",\n  \"reason\": \"Permission denied for tool 'WebSearch'. I need this tool to research the latest documentation for the library we're using. Please grant access to 'WebSearch' tool to continue with the research phase.\"\n}}\n```\n\nREQUIRED JSON FORMAT:\n```json\n{{\n  \"ticket_id\": \"{}\",\n  \"outcome\": \"next_stage\",\n  \"target_stage\": \"next_worker_type_name\",\n  \"pipeline_update\": [\"stage1\", \"stage2\", \"stage3\"],\n  \"comment\": \"Brief summary of what you accomplished\",\n  \"reason\": \"Why moving to next stage\"\n}}\n```\n\nFIELD DEFINITIONS:\n- \"outcome\": MUST be one of: \"next_stage\", \"prev_stage\", \"coordinator_attention\"\n- \"target_stage\": Name of the worker type for the next stage (required if outcome is \"next_stage\" or \"prev_stage\")\n- \"pipeline_update\": Complete array of all stages in order (INCLUDE THIS to update the execution plan)\n- \"comment\": Your work summary (will be added to ticket comments)\n- \"reason\": Explanation for the outcome (for permission issues, specify exactly which tools you need)\n\nEXAMPLES:\n1. For planning stage completing and moving to development:\n```json\n{{\n  \"ticket_id\": \"abc-123\",\n  \"outcome\": \"next_stage\",\n  \"target_stage\": \"development\",\n  \"pipeline_update\": [\"planning\", \"development\", \"testing\", \"review\"],\n  \"comment\": \"Completed project analysis and created development plan\",\n  \"reason\": \"Planning phase complete, ready for implementation\"\n}}\n```\n\n2. If you need coordinator help (general):\n```json\n{{\n  \"ticket_id\": \"abc-123\",\n  \"outcome\": \"coordinator_attention\",\n  \"target_stage\": null,\n  \"pipeline_update\": null,\n  \"comment\": \"Encountered issue that needs coordinator decision\",\n  \"reason\": \"Missing requirements or blocked by external dependency\"\n}}\n```\n\n3. If you need specific tool permissions:\n```json\n{{\n  \"ticket_id\": \"abc-123\",\n  \"outcome\": \"coordinator_attention\",\n  \"target_stage\": null,\n  \"pipeline_update\": null,\n  \"comment\": \"Permission required for essential tools\",\n  \"reason\": \"Need access to 'Bash' and 'WebSearch' tools. Bash is required to run tests and check build status. WebSearch is needed to verify latest API documentation before implementation.\"\n}}\n```\n\nREMEMBER: Your response should include your normal work/analysis, followed by the JSON block at the end.",
            request.system_prompt,
            request.ticket_id,
            request.ticket_id,
            request.ticket_id
        );

        // Spawn Claude Code process with the system prompt
        info!(
            "Spawning Claude Code with working directory: {}",
            request.project_path
        );
        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&system_prompt)
            .arg("--debug")
            //.arg("--verbose")
            .arg("--mcp-config")
            .arg(&config_path)
            .arg("--output-format")
            .arg("json")
            .current_dir(&request.project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Apply permissions based on mode
        info!("Applying permission mode: {}", request.permission_mode);
        Self::apply_permissions_to_command(
            &mut cmd,
            &request.permission_mode,
            &request.project_path,
        )?;

        debug!("Executing command: {:?}", cmd);
        let child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);
        info!("Worker process spawned with PID: {}", pid);

        // Wait for the process to complete and capture output
        let output = child.wait_with_output().await?;

        info!("Worker process completed with status: {}", output.status);

        // Parse stdout for WorkerOutput JSON
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        debug!("Worker stdout: {}", stdout_str);
        debug!("Worker stderr: {}", stderr_str);

        // Parse Claude CLI JSON output format
        // Claude CLI with --output-format json wraps response in {"result": "...", "type": "completion"}
        for line in stdout_str.lines() {
            if line.contains("\"result\"") && line.contains("\"type\"") {
                debug!("Parsing Claude CLI wrapper JSON: {}", line);
                match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(claude_output) => {
                        if let Some(result_str) =
                            claude_output.get("result").and_then(|v| v.as_str())
                        {
                            debug!("Extracted result string from Claude CLI wrapper");
                            match Self::parse_worker_output_from_result(result_str) {
                                Ok(parsed_output) => {
                                    info!(
                                        "Successfully parsed worker output for ticket {}",
                                        request.ticket_id
                                    );
                                    // Clean up
                                    let _ = std::fs::remove_file(&config_path);
                                    return Ok(parsed_output);
                                }
                                Err(e) => {
                                    debug!("Failed to parse worker output from result: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        debug!("Failed to parse Claude CLI wrapper JSON: {}", e);
                    }
                }
            }
        }

        // Clean up config file
        let _ = std::fs::remove_file(&config_path);

        // If we get here, the worker didn't produce valid output
        // This should be handled by the caller via WorkerOutput::CoordinatorAttention
        // rather than directly releasing tickets here since process.rs doesn't have DB access
        Err(anyhow::anyhow!(
            "Worker {} did not produce valid output for ticket {}. This will be handled as coordinator attention by WorkerConsumer.",
            request.worker_id,
            request.ticket_id
        ))
    }
}
