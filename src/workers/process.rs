use anyhow::Result;
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::process::{Command, Stdio};
use tokio::time::Duration;
use tracing::{debug, error, info, warn};

use super::json_output::WorkerOutputProcessor;
use super::types::{SpawnWorkerRequest, WorkerInfo, WorkerProcess, WorkerStatus};
use crate::{
    database::{worker_types::WorkerType, workers::Worker},
    server::AppState,
};

pub struct ProcessManager;

impl ProcessManager {
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
    pub async fn spawn_worker(
        state: &AppState,
        request: SpawnWorkerRequest,
    ) -> Result<WorkerProcess> {
        info!(
            "Spawning worker: {} (project: {}, type: {}, queue: {})",
            request.worker_id, request.project_id, request.worker_type, request.queue_name
        );

        // Get project info
        debug!("Looking up project: {}", request.project_id);
        let project =
            crate::database::projects::Project::get_by_name(&state.db, &request.project_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", request.project_id))?;
        info!("Found project at path: {}", project.path);

        // Get worker type info
        debug!(
            "Looking up worker type: {} for project: {}",
            request.worker_type, request.project_id
        );
        let worker_type_info =
            WorkerType::get_by_type(&state.db, &request.project_id, &request.worker_type)
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Worker type '{}' not found for project '{}'",
                        request.worker_type,
                        request.project_id
                    )
                })?;
        debug!(
            "Found worker type with system prompt length: {}",
            worker_type_info.system_prompt.len()
        );

        // Use the provided queue name
        let queue_name = request.queue_name.clone();

        // Create worker info
        let now = chrono::Utc::now();
        let worker_info = WorkerInfo {
            worker_id: request.worker_id.clone(),
            project_id: request.project_id.clone(),
            worker_type: request.worker_type.clone(),
            status: WorkerStatus::Spawning,
            pid: None,
            queue_name: queue_name.clone(),
            started_at: now,
            last_activity: now,
        };

        // Save worker to database
        let db_worker = Worker {
            worker_id: worker_info.worker_id.clone(),
            project_id: worker_info.project_id.clone(),
            worker_type: worker_info.worker_type.clone(),
            status: worker_info.status.as_str().to_string(),
            pid: None,
            queue_name: worker_info.queue_name.clone(),
            started_at: worker_info.started_at.to_rfc3339(),
            last_activity: worker_info.last_activity.to_rfc3339(),
        };
        Worker::create(&state.db, db_worker).await?;
        debug!("✓ Worker saved to database, proceeding with setup");

        // Build worker prompt
        debug!("Building worker prompt...");
        let worker_prompt =
            Self::build_worker_prompt(&worker_info, &worker_type_info.system_prompt);
        debug!("✓ Worker prompt built successfully");

        // Generate MCP config file
        debug!("Creating MCP config file...");
        let mcp_config_path =
            Self::create_mcp_config(&project.path, &worker_info.worker_id, state.config.port)?;
        debug!("✓ MCP config created at: {}", mcp_config_path);

        // Create log file path in centralized logs directory
        debug!("Getting project logs directory...");
        let project_logs_dir = crate::database::get_project_logs_dir(
            &state.config.database_path,
            &project.repository_name,
        )?;
        debug!("✓ Project logs directory: {}", project_logs_dir);
        // Sanitize to safe filename fragments
        let safe_type = worker_info
            .worker_type
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let safe_queue = queue_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        let log_file_path = format!(
            "{}/worker_{}__{}.log",
            project_logs_dir, safe_type, safe_queue
        );
        debug!("Opening log file: {}", log_file_path);
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;
        debug!("✓ Log file opened successfully");

        // DIAGNOSTIC: Enhanced logging for worker spawning
        info!("Starting worker spawn diagnostics");
        debug!("About to check for 'claude' command in PATH");
        match tokio::process::Command::new("which")
            .arg("claude")
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                let claude_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                info!("✓ Found Claude Code at: {}", claude_path);
            }
            Ok(_) => {
                error!("✗ 'claude' command not found in PATH - this will cause spawn failure");
            }
            Err(e) => {
                error!("✗ Failed to check for 'claude' command: {}", e);
            }
        }

        // Log environment and working directory
        debug!(
            "Current working directory: {}",
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("unknown"))
                .display()
        );
        debug!(
            "Project working directory (where Claude will run): {}",
            project.path
        );
        if let Ok(path_var) = std::env::var("PATH") {
            debug!("PATH environment variable: {}", path_var);
        } else {
            error!("PATH environment variable not found!");
        }

        // Spawn Claude Code process
        let mut cmd = Command::new("claude");
        cmd.arg("-p")
            .arg(&worker_prompt)
            .arg("--debug")
            .arg("--verbose")
            .arg("--permission-mode")
            .arg("bypassPermissions")
            .arg("--mcp-config")
            .arg(&mcp_config_path)
            .arg("--output-format")
            .arg("json")
            .current_dir(&project.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file));

        info!("Executing Claude Code command");
        debug!("Command: claude");
        debug!("Arguments: -p [prompt] --debug --verbose --permission-mode bypassPermissions --mcp-config {} --output-format json", mcp_config_path);
        debug!("Working directory: {}", project.path);
        debug!("MCP config file: {}", mcp_config_path);
        debug!("Log file path: {}", log_file_path);
        debug!(
            "Prompt preview: {}",
            worker_prompt.chars().take(100).collect::<String>()
        );
        debug!("Attempting to spawn process...");

        let child = match tokio::process::Command::from(cmd).spawn() {
            Ok(child) => {
                info!("✓ Successfully spawned Claude Code process");
                debug!("Worker spawn diagnostics completed");
                child
            }
            Err(e) => {
                let error_msg = format!(
                    "Failed to spawn Claude Code process: {} (error code: {:?}). This usually means 'claude' command is not installed or not in PATH",
                    e, e.kind()
                );
                error!("{}", error_msg);
                return Err(anyhow::anyhow!("{}", error_msg));
            }
        };

        let pid = child.id();
        info!(
            "Worker {} spawned with PID: {:?}, MCP config: {}, Log file: {}",
            worker_info.worker_id, pid, mcp_config_path, log_file_path
        );

        // Update worker info with PID and status
        let mut updated_info = worker_info.clone();
        updated_info.pid = pid;
        updated_info.status = WorkerStatus::Active;

        // Update database
        Worker::update_status(&state.db, &updated_info.worker_id, "active", pid).await?;

        // Spawn monitoring task for worker output processing
        let worker_id_clone = updated_info.worker_id.clone();
        let worker_type_clone = updated_info.worker_type.clone();
        let log_file_path_clone = log_file_path.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            Self::monitor_worker_output(
                &state_clone,
                &worker_id_clone,
                &worker_type_clone,
                &log_file_path_clone,
                pid,
            )
            .await;
        });

        Ok(WorkerProcess {
            info: updated_info,
            process: Some(child),
        })
    }

    fn build_worker_prompt(worker_info: &WorkerInfo, system_prompt: &str) -> String {
        format!(
            r#"{system_prompt}

WORKER CONFIGURATION:
- Worker ID: {worker_id}
- Project: {project_id}
- Worker Type: {worker_type}
- Stage: {worker_type}

STAGE-BASED MULTI-AGENT SYSTEM:
You are a specialized worker in a stage-based pipeline where:
- Pipeline stage names == ticket current_stage == worker names (e.g., "planning", "design", "coding", "testing")
- All tickets start in "planning" stage with minimal pipeline: ["planning"]
- Workers output structured JSON with exactly one of three outcomes:
  1. "next_stage": Task completed, move ticket to specified next stage
  2. "prev_stage": Issues found, move ticket back to specified previous stage  
  3. "coordinator_attention": Critical issues requiring human coordinator intervention

TASK PROCESSING WORKFLOW:
1. Check for tickets in your stage: get_tickets_by_stage("{worker_type}")
2. Process tickets one by one in priority order (urgent > high > medium > low)
3. For each ticket:
   - Read full ticket content including all comments from previous stages
   - Perform your specialized work for this stage
   - Add a detailed report as a comment with your findings/work
   - Output exactly ONE structured JSON decision (see OUTPUT FORMAT below)
4. When no more tickets in your stage, call finish_worker() and exit gracefully

OUTPUT FORMAT (JSON only, no additional text):
```json
{{
  "outcome": "next_stage|prev_stage|coordinator_attention",
  "target_stage": "stage_name_or_null",
  "pipeline_update": ["stage1", "stage2", "..."] or null,
  "comment": "Your detailed report/findings",
  "reason": "Brief reason for the decision"
}}
```

OUTCOME DESCRIPTIONS:
- "next_stage": Work completed successfully, ticket should advance
  - target_stage: Name of next stage to move to
  - pipeline_update: Optional - new complete pipeline if extending it
- "prev_stage": Issues found that require earlier stage rework
  - target_stage: Stage to return ticket to (must be earlier in pipeline)
  - pipeline_update: Should be null for backward moves
- "coordinator_attention": Critical issues requiring human intervention
  - target_stage: Should be null
  - pipeline_update: Should be null

PIPELINE MANAGEMENT:
- Only "planning" workers can extend pipelines by adding new stages
- Other workers can only move tickets forward/backward in existing pipeline
- When extending pipeline, provide the complete new pipeline array
- Pipeline modifications have constraints: cannot modify stages that tickets have already passed through

MCP TOOLS AVAILABLE:
- get_tickets_by_stage(stage): Get all tickets currently in your stage
- get_ticket(ticket_id): Get full ticket details with all comments
- add_ticket_comment(ticket_id, worker_type, worker_id, stage_number, content): Add your work report
- update_ticket_stage(ticket_id, new_stage): Move ticket to different stage
- update_ticket_pipeline(ticket_id, new_pipeline): Extend ticket pipeline (planning workers only)
- finish_worker(worker_id, "reason"): Mark yourself as finished and exit

PRIORITY HANDLING:
Process tickets in order: urgent → high → medium → low
Focus on completing higher priority tickets before moving to lower priority ones.

Remember: Output ONLY the JSON structure above. No additional commentary or explanation.
"#,
            worker_id = worker_info.worker_id,
            project_id = worker_info.project_id,
            worker_type = worker_info.worker_type,
            system_prompt = system_prompt
        )
    }

    pub async fn stop_worker(state: &AppState, worker_id: &str) -> Result<bool> {
        info!("Stopping worker: {}", worker_id);

        // Check if worker exists and get PID
        let worker = Worker::get_by_id(&state.db, worker_id).await?;

        match worker {
            Some(worker) if worker.pid.is_some() => {
                let pid = worker.pid.unwrap();

                // Try to terminate process gracefully
                if let Ok(mut child) = tokio::process::Command::new("kill")
                    .arg("-TERM")
                    .arg(pid.to_string())
                    .spawn()
                {
                    let _ = child.wait().await;
                }

                // Wait a bit for graceful shutdown
                tokio::time::sleep(Duration::from_millis(1000)).await;

                // Force kill if still running
                let _ = tokio::process::Command::new("kill")
                    .arg("-KILL")
                    .arg(pid.to_string())
                    .spawn();

                // Update database status
                Worker::update_status(&state.db, worker_id, "finished", None).await?;

                // Create event
                crate::database::events::Event::create_worker_stopped(
                    &state.db,
                    worker_id,
                    "stopped by coordinator",
                )
                .await?;

                info!("Worker {} stopped", worker_id);
                Ok(true)
            }
            Some(_) => {
                warn!("Worker {} has no PID, marking as finished", worker_id);
                Worker::update_status(&state.db, worker_id, "finished", None).await?;
                Ok(true)
            }
            None => {
                warn!("Worker {} not found", worker_id);
                Ok(false)
            }
        }
    }

    pub async fn check_worker_health(state: &AppState, worker_id: &str) -> Result<WorkerStatus> {
        let worker = Worker::get_by_id(&state.db, worker_id).await?;

        match worker {
            Some(worker) => {
                if let Some(pid) = worker.pid {
                    // Check if process is still running
                    let is_running = tokio::process::Command::new("kill")
                        .arg("-0")
                        .arg(pid.to_string())
                        .status()
                        .await
                        .map(|status| status.success())
                        .unwrap_or(false);

                    if is_running {
                        // Update last activity
                        Worker::update_last_activity(&state.db, worker_id).await?;
                        Ok(WorkerStatus::Active)
                    } else {
                        // Process died, update status
                        Worker::update_status(&state.db, worker_id, "failed", None).await?;

                        // Create event
                        crate::database::events::Event::create_worker_stopped(
                            &state.db,
                            worker_id,
                            "process died unexpectedly",
                        )
                        .await?;

                        Ok(WorkerStatus::Failed)
                    }
                } else {
                    Ok(WorkerStatus::Spawning)
                }
            }
            None => Err(anyhow::anyhow!("Worker '{}' not found", worker_id)),
        }
    }

    async fn monitor_worker_output(
        state: &AppState,
        worker_id: &str,
        worker_type: &str,
        log_file_path: &str,
        pid: Option<u32>,
    ) {
        info!("Starting output monitoring for worker {}", worker_id);

        let mut last_position = 0;
        let mut check_interval = tokio::time::interval(Duration::from_secs(2));

        loop {
            check_interval.tick().await;

            // Check if worker process is still running
            if let Some(pid) = pid {
                let is_running = tokio::process::Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .status()
                    .await
                    .map(|status| status.success())
                    .unwrap_or(false);

                if !is_running {
                    debug!(
                        "Worker {} process finished, checking final output",
                        worker_id
                    );
                    // Process any remaining output and exit
                    if let Err(e) = Self::process_new_output(
                        state,
                        worker_id,
                        worker_type,
                        log_file_path,
                        &mut last_position,
                    )
                    .await
                    {
                        error!(
                            "Failed to process final output for worker {}: {}",
                            worker_id, e
                        );
                    }
                    break;
                }
            }

            // Process new log content
            if let Err(e) = Self::process_new_output(
                state,
                worker_id,
                worker_type,
                log_file_path,
                &mut last_position,
            )
            .await
            {
                warn!("Failed to process output for worker {}: {}", worker_id, e);
            }
        }

        info!("Output monitoring completed for worker {}", worker_id);
    }

    async fn process_new_output(
        state: &AppState,
        worker_id: &str,
        worker_type: &str,
        log_file_path: &str,
        last_position: &mut u64,
    ) -> Result<()> {
        let content = tokio::fs::read_to_string(log_file_path).await?;

        if content.len() as u64 > *last_position {
            let new_content = &content[*last_position as usize..];
            *last_position = content.len() as u64;

            // Look for JSON outcome in the new content
            if let Some(json_start) = new_content.rfind('{') {
                if let Some(json_end) = new_content[json_start..].find('}') {
                    let json_str = &new_content[json_start..json_start + json_end + 1];

                    // Try to parse as worker output
                    match WorkerOutputProcessor::parse_output(json_str) {
                        Ok(output) => {
                            info!(
                                "Found worker output for {}: {:?}",
                                worker_id, output.outcome
                            );

                            // Find ticket ID from worker output - we need to search for ticket context
                            if let Some(ticket_id) =
                                Self::extract_ticket_id_from_log(&content).await
                            {
                                if let Err(e) = WorkerOutputProcessor::process_output(
                                    state,
                                    &ticket_id,
                                    worker_id,
                                    worker_type,
                                    output,
                                )
                                .await
                                {
                                    error!("Failed to process worker output: {}", e);
                                }
                            } else {
                                debug!("No ticket ID found in worker output for {}", worker_id);
                            }
                        }
                        Err(_) => {
                            // Not a worker output JSON, continue monitoring
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn extract_ticket_id_from_log(log_content: &str) -> Option<String> {
        // Look for ticket ID patterns in the log content
        if let Some(start) = log_content.find("ticket_id") {
            let after_start = &log_content[start..];
            if let Some(colon_pos) = after_start.find(':') {
                let after_colon = &after_start[colon_pos + 1..];
                if let Some(quote_start) = after_colon.find('"') {
                    let after_quote = &after_colon[quote_start + 1..];
                    if let Some(quote_end) = after_quote.find('"') {
                        return Some(after_quote[..quote_end].to_string());
                    }
                }
            }
        }
        None
    }
}
