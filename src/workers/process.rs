use anyhow::{Context, Result};
use std::fs;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

use super::completion_processor::WorkerOutput;
use super::types::SpawnWorkerRequest;
use super::validation::WorkerInputValidator;
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

        // Extract all JSON code blocks (```json ... ```) in the result
        let mut json_blocks = Vec::new();
        let mut search_pos = 0;

        while let Some(json_start) = result_str[search_pos..].find("```json") {
            let absolute_start = search_pos + json_start + 7; // Skip past "```json"
            if let Some(json_end_relative) = result_str[absolute_start..].find("```") {
                let absolute_end = absolute_start + json_end_relative;
                let json_block = result_str[absolute_start..absolute_end].trim();
                json_blocks.push(json_block);
                search_pos = absolute_end + 3; // Move past the closing ```
            } else {
                break;
            }
        }

        if json_blocks.is_empty() {
            return Err(anyhow::anyhow!(
                "No valid JSON code block found in worker result. Workers must output JSON in ```json...``` blocks."
            ));
        }

        debug!("Found {} JSON block(s) in worker output", json_blocks.len());
        if json_blocks.len() > 1 {
            info!(
                "Worker output contains multiple JSON blocks ({}), using last one (worker self-correction)",
                json_blocks.len()
            );
        }

        // Use the last JSON block (most recent worker decision after self-correction)
        let final_json = json_blocks.last().unwrap();
        debug!("Using final JSON block: {}", final_json);

        serde_json::from_str::<WorkerOutput>(final_json)
            .with_context(|| "Failed to parse WorkerOutput from final JSON code block")
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

        // Validate inputs before proceeding
        info!("Validating worker spawn request inputs");

        WorkerInputValidator::validate_ticket_id(&request.ticket_id)
            .context("Invalid ticket ID")?;

        WorkerInputValidator::validate_worker_id(&request.worker_id)
            .context("Invalid worker ID")?;

        let validated_path = WorkerInputValidator::validate_project_path(&request.project_path)
            .context("Invalid project path")?;

        WorkerInputValidator::validate_prompt_content(
            "system_prompt",
            &request.system_prompt,
            100_000, // 100KB max
        )
        .context("Invalid system prompt")?;

        if let Some(ref rules) = request.project_rules {
            WorkerInputValidator::validate_prompt_content("project_rules", rules, 50_000)
                .context("Invalid project rules")?;
        }

        if let Some(ref patterns) = request.project_patterns {
            WorkerInputValidator::validate_prompt_content("project_patterns", patterns, 50_000)
                .context("Invalid project patterns")?;
        }

        info!("Input validation passed");

        // Create MCP config file using validated path
        let config_path = Self::create_mcp_config(
            validated_path.to_str().unwrap(),
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
                full_prompt.push_str("CRITICAL: You MUST follow these project rules:\n\n```md\n");
                full_prompt.push_str(rules);
                full_prompt.push_str("\n```\n");
            }
        }

        // Add project patterns if available
        if let Some(ref patterns) = request.project_patterns {
            if !patterns.trim().is_empty() {
                full_prompt.push_str("\n\n=== PROJECT PATTERNS ===\n");
                full_prompt.push_str("Follow these project patterns and conventions:\n\n```md\n");
                full_prompt.push_str(patterns);
                full_prompt.push_str("\n```\n");
            }
        }

        let system_prompt = template
            .replace("{ticket_id}", &request.ticket_id)
            .replace("{system_prompt}", &full_prompt);

        // Create simple input prompt that instructs worker to get ticket details
        let input_prompt = format!(
            "You are working on ticket: {}. Use the get_ticket MCP tool to retrieve the ticket details and proceed with your assigned role.",
            request.ticket_id
        );

        // Spawn Claude Code process with the system prompt
        info!(
            "Spawning Claude Code with working directory: {}",
            validated_path.display()
        );
        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&system_prompt)
            .arg(&input_prompt)
            .arg("--debug")
            //.arg("--verbose")
            .arg("--mcp-config")
            .arg(&config_path)
            .arg("--output-format")
            .arg("json");

        // Analyzing workers (planning, review, research, design) always use default model (most capable)
        // Producing workers (implementation, testing, documentation, deployment) can use lighter models
        let worker_type_lower = request.worker_type.to_lowercase();
        let is_analyzing_worker = worker_type_lower.contains("planning")
            || worker_type_lower.contains("review")
            || worker_type_lower.contains("research")
            || worker_type_lower.contains("design");

        if is_analyzing_worker {
            info!(
                "Analyzing worker ({}): using default model (ignoring --model parameter)",
                request.worker_type
            );
        } else if let Some(ref model) = request.model {
            info!("Producing worker: using model {}", model);
            cmd.arg("--model").arg(model);

            // Increase output token limit for haiku models
            if model.to_lowercase().contains("haiku") {
                info!("Haiku model detected: setting CLAUDE_CODE_MAX_OUTPUT_TOKENS to 16384");
                cmd.env("CLAUDE_CODE_MAX_OUTPUT_TOKENS", "16384");
            }
        }

        cmd.current_dir(&validated_path)
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
            validated_path.to_str().unwrap(),
        )?;

        debug!("Executing command: {:?}", cmd);
        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = std::fs::remove_file(&config_path);
                return Err(e.into());
            }
        };
        let pid = child.id().unwrap_or(0);
        info!("Worker process spawned with PID: {}", pid);

        // Add timeout to worker execution (default: 10 minutes)
        let worker_timeout = Duration::from_secs(
            std::env::var("WORKER_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(600), // Default: 600 seconds (10 minutes)
        );

        info!(
            "Waiting for worker to complete (timeout: {} seconds)",
            worker_timeout.as_secs()
        );

        // Wait for the process to complete and capture output with timeout
        let start_time = std::time::Instant::now();
        let output_result = timeout(worker_timeout, child.wait_with_output()).await;

        let output = match output_result {
            Ok(Ok(output)) => {
                let duration = start_time.elapsed();
                info!(
                    "Worker process completed with status: {} (duration: {:.2}s)",
                    output.status,
                    duration.as_secs_f64()
                );

                if duration.as_secs() > 300 {
                    warn!(
                        "Worker took unusually long to complete: {:.2}s (ticket: {})",
                        duration.as_secs_f64(),
                        request.ticket_id
                    );
                }

                output
            }
            Ok(Err(e)) => {
                error!(
                    "Worker process failed after {:.2}s: {}",
                    start_time.elapsed().as_secs_f64(),
                    e
                );
                let _ = std::fs::remove_file(&config_path);
                return Err(e.into());
            }
            Err(_) => {
                // Timeout occurred - note that child has been consumed by wait_with_output
                error!(
                    "Worker process timed out after {} seconds (PID: {}, ticket: {})",
                    worker_timeout.as_secs(),
                    pid,
                    request.ticket_id
                );

                warn!(
                    "Worker process (PID: {}) timed out. The process may still be running and should be terminated manually if necessary.",
                    pid
                );

                let _ = std::fs::remove_file(&config_path);

                return Err(anyhow::anyhow!(
                    "Worker process timed out after {} seconds",
                    worker_timeout.as_secs()
                ));
            }
        };

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
