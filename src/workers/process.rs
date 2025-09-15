use anyhow::Result;
use serde_json::json;
use std::fs;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info};

use super::queue::WorkerOutput;
use super::types::SpawnWorkerRequest;

pub struct ProcessManager;

impl ProcessManager {
    /// Parse worker JSON output from a string
    pub fn parse_output(output: &str) -> Result<WorkerOutput> {
        // Try to find JSON in the output (workers might output other text too)
        let json_start = output.find('{');
        let json_end = output.rfind('}');

        match (json_start, json_end) {
            (Some(start), Some(end)) if start <= end => {
                let json_str = &output[start..=end];
                debug!("Parsing worker JSON: {}", json_str);
                let worker_output: WorkerOutput = serde_json::from_str(json_str)?;
                Ok(worker_output)
            }
            _ => {
                error!("No valid JSON found in worker output: {}", output);
                Err(anyhow::anyhow!("No valid JSON found in worker output"))
            }
        }
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
                    "protocol_version": "2024-11-05"
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
            "{}\n\nYou are working on ticket_id: {}\nWhen you complete your work, you must output a JSON response with the following structure:\n{{\n  \"ticket_id\": \"{}\",\n  \"outcome\": \"next_stage\" | \"prev_stage\" | \"coordinator_attention\",\n  \"target_stage\": \"stage_name_if_moving\",\n  \"pipeline_update\": [\"optional\", \"array\", \"of\", \"stages\"],\n  \"comment\": \"Description of what you did\",\n  \"reason\": \"Reason for the outcome\"\n}}",
            request.system_prompt,
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
            .arg("--permission-mode")
            .arg("bypassPermissions")
            .arg("--mcp-config")
            .arg(&config_path)
            .arg("--output-format")
            .arg("json")
            // cmd.arg("--system")
            //    .arg(&system_prompt)
            //    .arg("--mcp-config")
            //    .arg(&config_path)
            .current_dir(&request.project_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
        Err(anyhow::anyhow!(
            "Worker {} did not produce valid output for ticket {}",
            request.worker_id,
            request.ticket_id
        ))
    }
}
