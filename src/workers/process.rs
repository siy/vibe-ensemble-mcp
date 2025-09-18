use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use super::queue::WorkerOutput;
use super::types::SpawnWorkerRequest;
use crate::mcp::MCP_PROTOCOL_VERSION;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePermissions {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub ask: Vec<String>,
    #[serde(rename = "additionalDirectories", default)]
    pub additional_directories: Vec<String>,
    #[serde(rename = "defaultMode", default = "default_mode")]
    pub default_mode: String,
}

fn default_mode() -> String {
    "acceptEdits".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: ClaudePermissions,
}

impl Default for ClaudePermissions {
    fn default() -> Self {
        Self {
            allow: vec![],
            deny: vec![],
            ask: vec![],
            additional_directories: vec![],
            default_mode: default_mode(),
        }
    }
}

pub struct ProcessManager;

impl ProcessManager {
    /// Load permissions from .claude/settings.local.json
    fn load_inherit_permissions(project_path: &str) -> Result<ClaudePermissions> {
        let settings_path = Path::new(project_path).join(".claude/settings.local.json");
        debug!(
            "Loading inherit permissions from: {}",
            settings_path.display()
        );

        if !settings_path.exists() {
            warn!(
                "Settings file not found: {}, using defaults",
                settings_path.display()
            );
            return Ok(ClaudePermissions::default());
        }

        let content = fs::read_to_string(&settings_path)?;
        let settings: ClaudeSettings = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse Claude settings: {}", e))?;

        info!(
            "Loaded inherit permissions with {} allowed, {} denied tools",
            settings.permissions.allow.len(),
            settings.permissions.deny.len()
        );
        Ok(settings.permissions)
    }

    /// Load permissions from <project_path>/.vibe-ensemble-mcp/worker-permissions.json
    fn load_file_permissions(project_path: &str) -> Result<ClaudePermissions> {
        let permissions_path =
            Path::new(project_path).join(".vibe-ensemble-mcp/worker-permissions.json");
        debug!(
            "Loading file permissions from: {}",
            permissions_path.display()
        );

        if !permissions_path.exists() {
            warn!(
                "Worker permissions file not found: {}, using defaults",
                permissions_path.display()
            );
            return Ok(ClaudePermissions::default());
        }

        let content = fs::read_to_string(&permissions_path)?;
        let settings: ClaudeSettings = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse worker permissions: {}", e))?;

        info!(
            "Loaded file permissions with {} allowed, {} denied tools",
            settings.permissions.allow.len(),
            settings.permissions.deny.len()
        );
        Ok(settings.permissions)
    }

    /// Apply permissions to Claude command based on mode
    fn apply_permissions_to_command(
        cmd: &mut Command,
        permission_mode: &str,
        project_path: &str,
    ) -> Result<()> {
        match permission_mode {
            "bypass" => {
                debug!("Using bypass mode - adding --dangerously-skip-permissions");
                cmd.arg("--dangerously-skip-permissions");
            }
            "inherit" => {
                debug!("Using inherit mode - loading from .claude/settings.local.json");
                let permissions = Self::load_inherit_permissions(project_path)?;
                Self::add_permission_args(cmd, &permissions);
            }
            "file" => {
                debug!("Using file mode - loading from .vibe-ensemble-mcp/worker-permissions.json");
                let permissions = Self::load_file_permissions(project_path)?;
                Self::add_permission_args(cmd, &permissions);
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Invalid permission mode: {}",
                    permission_mode
                ));
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

    /// Parse worker JSON output from a string
    pub fn parse_output(output: &str) -> Result<WorkerOutput> {
        debug!("Attempting to parse worker output: {}", output);

        // Strategy 1: Look for JSON code blocks (```json ... ```)
        if let Some(json_start) = output.find("```json") {
            let search_start = json_start + 7; // Skip past "```json"
            if let Some(json_end_relative) = output[search_start..].find("```") {
                let json_end = search_start + json_end_relative;
                let json_block = output[search_start..json_end].trim();
                debug!("Found JSON in code block: {}", json_block);
                match serde_json::from_str::<WorkerOutput>(json_block) {
                    Ok(worker_output) => return Ok(worker_output),
                    Err(e) => debug!("Failed to parse JSON from code block: {}", e),
                }
            }
        }

        // Strategy 2: Look for the last complete JSON object in the output
        let mut last_valid_json = None;
        let mut brace_count = 0;
        let mut start_pos = None;

        for (i, char) in output.char_indices() {
            match char {
                '{' => {
                    if brace_count == 0 {
                        start_pos = Some(i);
                    }
                    brace_count += 1;
                }
                '}' => {
                    brace_count -= 1;
                    if brace_count == 0 {
                        if let Some(start) = start_pos {
                            let json_candidate = &output[start..=i];
                            if json_candidate.contains("\"ticket_id\"")
                                && json_candidate.contains("\"outcome\"")
                            {
                                last_valid_json = Some(json_candidate);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(json_str) = last_valid_json {
            debug!("Found valid JSON candidate: {}", json_str);
            match serde_json::from_str::<WorkerOutput>(json_str) {
                Ok(worker_output) => return Ok(worker_output),
                Err(e) => debug!("Failed to parse JSON candidate: {}", e),
            }
        }

        // Strategy 3: Original simple approach (fallback)
        let json_start = output.find('{');
        let json_end = output.rfind('}');

        match (json_start, json_end) {
            (Some(start), Some(end)) if start <= end => {
                let json_str = &output[start..=end];
                debug!("Fallback: parsing worker JSON: {}", json_str);
                match serde_json::from_str::<WorkerOutput>(json_str) {
                    Ok(worker_output) => return Ok(worker_output),
                    Err(e) => debug!("Fallback parsing failed: {}", e),
                }
            }
            _ => {}
        }

        error!("No valid JSON found in worker output: {}", output);
        Err(anyhow::anyhow!("No valid JSON found in worker output"))
    }

    fn create_mcp_config(project_path: &str, worker_id: &str, server_port: u16) -> Result<String> {
        debug!(
            "Creating MCP config for worker {} in project path: {}",
            worker_id, project_path
        );

        let config = json!({
            "mcpServers": {
                "vibe-ensemble-mcp": {
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}/mcp", server_port),
                    "protocol_version": MCP_PROTOCOL_VERSION
                }
            }
        });
        debug!("MCP config JSON created successfully");

        let config_path = format!("{}/worker_{}_mcp_config.json", project_path, worker_id);
        debug!("Target config file path: {}", config_path);

        debug!("Serializing config to pretty JSON...");
        let config_json = serde_json::to_string_pretty(&config)?;
        debug!(
            "JSON serialization successful, length: {} bytes",
            config_json.len()
        );

        debug!("Writing config file to: {}", config_path);
        fs::write(&config_path, config_json)?;
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
        // Claude CLI with --output-format json wraps response in {"result": "...", ...}
        for line in stdout_str.lines() {
            // First try to parse as Claude CLI wrapper format
            if line.contains("\"result\"") && line.contains("\"type\"") {
                debug!("Attempting to parse Claude CLI wrapper JSON: {}", line);
                if let Ok(claude_output) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(result_str) = claude_output.get("result").and_then(|v| v.as_str()) {
                        debug!("Extracted result string: {}", result_str);
                        // Parse the inner result string as WorkerOutput JSON
                        match Self::parse_output(result_str) {
                            Ok(parsed_output) => {
                                info!(
                                    "Successfully parsed worker output for ticket {} (Claude CLI format)",
                                    request.ticket_id
                                );
                                // Clean up
                                let _ = std::fs::remove_file(&config_path);
                                return Ok(parsed_output);
                            }
                            Err(e) => {
                                debug!(
                                    "Failed to parse inner JSON from result: {} - error: {}",
                                    result_str, e
                                );
                            }
                        }
                    }
                }
            }
            // Fallback: try direct parsing for lines containing "outcome" (backwards compatibility)
            else if line.contains("\"outcome\"") {
                debug!("Attempting direct JSON parsing: {}", line);
                match Self::parse_output(line) {
                    Ok(parsed_output) => {
                        info!(
                            "Successfully parsed worker output for ticket {} (direct format)",
                            request.ticket_id
                        );
                        // Clean up
                        let _ = std::fs::remove_file(&config_path);
                        return Ok(parsed_output);
                    }
                    Err(e) => {
                        debug!("Failed to parse JSON from line: {} - error: {}", line, e);
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
