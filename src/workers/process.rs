use anyhow::{Context, Result};
use std::fs;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

use super::completion_processor::WorkerOutput;
use super::types::SpawnWorkerRequest;
use crate::permissions::{
    load_permission_policy, ClaudePermissions, PermissionMode, PermissionPolicy,
};

pub struct ProcessManager;

impl ProcessManager {
    /// Apply permissions to Claude command based on mode
    fn apply_permissions_to_command(
        cmd: &mut Command,
        permission_mode: PermissionMode,
        project_path: &str,
    ) -> Result<()> {
        let mode = permission_mode;

        match mode {
            PermissionMode::Bypass => {
                debug!("Using bypass mode - adding --dangerously-skip-permissions");
                cmd.arg("--dangerously-skip-permissions");
            }
            PermissionMode::Inherit | PermissionMode::File => {
                debug!("Using {} mode", mode.as_str());
                let policy = load_permission_policy(mode, project_path)?;
                match policy {
                    PermissionPolicy::Bypass => {
                        debug!("Permission policy is bypass for mode: {}", mode.as_str());
                    }
                    PermissionPolicy::Enforce(permissions) => {
                        info!(
                            "Loaded permissions with {} allowed, {} denied tools",
                            permissions.allow.len(),
                            permissions.deny.len()
                        );
                        debug!("Allowed tools before enhancement: {:?}", permissions.allow);
                        debug!("Denied tools: {:?}", permissions.deny);
                        Self::add_permission_args(cmd, &permissions);
                    }
                }
            }
        }
        Ok(())
    }

    /// Add --allowedTools and --disallowedTools arguments to command
    fn add_permission_args(cmd: &mut Command, permissions: &ClaudePermissions) {
        // For workers, we need to ensure our own MCP tools are always allowed
        let mut enhanced_allow_list = permissions.allow.clone();

        // Ensure our vibe-ensemble-mcp tools are always allowed (using explicit tool names)
        use crate::mcp::constants::get_all_mcp_tool_names;
        let mcp_tools = get_all_mcp_tool_names();

        // Check if we already have explicit MCP tools or wildcard
        let has_mcp_tools = enhanced_allow_list
            .iter()
            .any(|tool| tool.starts_with("mcp__vibe-ensemble-mcp__") || tool == "mcp__*");

        if !has_mcp_tools {
            enhanced_allow_list.extend(mcp_tools);
            debug!(
                "Auto-added {} explicit vibe-ensemble-mcp tools to worker permissions",
                enhanced_allow_list.len()
            );
        }

        // Add essential tools if not present
        let essential_tools = [
            "TodoWrite",
            "Bash",
            "Read",
            "Write",
            "Edit",
            "MultiEdit",
            "Glob",
            "Grep",
        ];
        for essential_tool in essential_tools {
            if !enhanced_allow_list
                .iter()
                .any(|tool| tool == essential_tool || tool == "*")
            {
                enhanced_allow_list.push(essential_tool.to_string());
            }
        }

        // Add allowed tools
        if !enhanced_allow_list.is_empty() {
            cmd.arg("--allowedTools");
            for tool in &enhanced_allow_list {
                cmd.arg(tool);
            }
            info!(
                "Added {} allowed tools (including auto-added essentials)",
                enhanced_allow_list.len()
            );
            debug!("Final allowed tools list: {:?}", enhanced_allow_list);
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

    /// Optimized worker output parsing with fallback strategies
    fn try_parse_worker_output(stdout: &str) -> Result<WorkerOutput> {
        debug!("Attempting optimized worker output parsing");

        // Strategy 1: Try to parse as single JSON object (most efficient)
        if let Ok(output) = serde_json::from_str::<WorkerOutput>(stdout.trim()) {
            debug!("Successfully parsed as direct JSON");
            return Ok(output);
        }

        // Strategy 2: Try Claude CLI wrapper format ({"result": "...", "type": "completion"})
        if let Ok(claude_output) = serde_json::from_str::<serde_json::Value>(stdout.trim()) {
            if let Some(result_str) = claude_output.get("result").and_then(|v| v.as_str()) {
                debug!("Found Claude CLI wrapper, extracting result");
                if let Ok(output) = Self::parse_worker_output_from_result(result_str) {
                    return Ok(output);
                }
            }
        }

        // Strategy 3: Line-by-line parsing (fallback for complex outputs)
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Try direct JSON parsing first
            if let Ok(output) = serde_json::from_str::<WorkerOutput>(line) {
                debug!("Successfully parsed worker output from line");
                return Ok(output);
            }

            // Try Claude CLI wrapper on this line
            if line.contains("\"result\"") && line.contains("\"type\"") {
                if let Ok(claude_output) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(result_str) = claude_output.get("result").and_then(|v| v.as_str()) {
                        debug!("Found Claude CLI wrapper in line, extracting result");
                        if let Ok(output) = Self::parse_worker_output_from_result(result_str) {
                            return Ok(output);
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!(
            "Could not parse worker output using any strategy. Workers must output valid JSON."
        ))
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

        // Create .vibe-ensemble-mcp directory for worker configs
        let config_dir = format!("{}/.vibe-ensemble-mcp", project_path);
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create worker config directory: {}", config_dir))?;
        debug!("Created worker config directory: {}", config_dir);

        // Sanitize worker_id for use in filename (replace invalid characters with underscores)
        let sanitized_worker_id = worker_id.replace(['/', ':', ' ', '\\'], "_");

        let config_path = format!(
            "{}/worker_{}_mcp_config.json",
            config_dir, sanitized_worker_id
        );
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

        // Create comprehensive system prompt with project rules and patterns
        let template = include_str!("../../templates/system_prompts/worker_spawn.md");

        // Build the full prompt with worker template, project rules, and patterns
        let mut full_prompt = request.system_prompt.clone();

        // Add project rules if available
        if let Some(ref rules) = request.project_rules {
            if !rules.trim().is_empty() {
                full_prompt.push_str("\n\n=== PROJECT RULES ===\n");
                full_prompt.push_str("CRITICAL: You MUST follow these project rules:\n\n");
                full_prompt.push_str(rules);
            }
        }

        // Add project patterns if available
        if let Some(ref patterns) = request.project_patterns {
            if !patterns.trim().is_empty() {
                full_prompt.push_str("\n\n=== PROJECT PATTERNS ===\n");
                full_prompt.push_str("Follow these project patterns and conventions:\n\n");
                full_prompt.push_str(patterns);
            }
        }

        let system_prompt = template
            .replace("{system_prompt}", &full_prompt)
            .replace("{ticket_id}", &request.ticket_id);

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
        info!(
            "Applying permission mode: {}",
            request.permission_mode.as_str()
        );
        Self::apply_permissions_to_command(
            &mut cmd,
            request.permission_mode,
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

        // Optimized parsing: try whole output first, then line-by-line fallback
        if let Ok(parsed_output) = Self::try_parse_worker_output(&stdout_str) {
            info!(
                "Successfully parsed worker output for ticket {}",
                request.ticket_id
            );
            // Clean up
            let _ = std::fs::remove_file(&config_path);
            return Ok(parsed_output);
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
