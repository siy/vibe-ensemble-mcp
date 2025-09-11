use anyhow::{Context, Result};
use serde_json::json;
use std::fs::{self, OpenOptions};
use std::process::{Command, Stdio};
use tokio::time::Duration;
use tracing::{debug, info, warn};

use super::types::{SpawnWorkerRequest, WorkerInfo, WorkerProcess, WorkerStatus};
use crate::{
    database::{worker_types::WorkerType, workers::Worker},
    server::AppState,
};

pub struct ProcessManager;

impl ProcessManager {
    fn create_mcp_config(project_path: &str, worker_id: &str, server_port: u16) -> Result<String> {
        let config = json!({
            "mcpServers": {
                "vibe-ensemble-mcp": {
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}/mcp", server_port),
                    "protocol_version": "2024-11-05"
                }
            }
        });

        let config_path = format!("{}/worker_{}_mcp_config.json", project_path, worker_id);
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        info!("Generated MCP config file: {}", config_path);
        Ok(config_path)
    }
    pub async fn spawn_worker(
        state: &AppState,
        request: SpawnWorkerRequest,
    ) -> Result<WorkerProcess> {
        info!("Spawning worker: {}", request.worker_id);

        // Get project info
        let project =
            crate::database::projects::Project::get_by_name(&state.db, &request.project_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", request.project_id))?;

        // Get worker type info
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

        // Build worker prompt
        let worker_prompt =
            Self::build_worker_prompt(&worker_info, &worker_type_info.system_prompt, &queue_name);

        // Generate MCP config file
        let mcp_config_path =
            Self::create_mcp_config(&project.path, &worker_info.worker_id, state.config.port)?;

        // Create log file path using worker type (since only one worker per queue/type can be active)
        let log_file_path = format!("{}/worker_{}.log", project.path, worker_info.worker_type);
        let log_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)?;

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
            .current_dir(&project.path)
            .stdin(Stdio::piped())
            .stdout(Stdio::from(log_file.try_clone()?))
            .stderr(Stdio::from(log_file));

        debug!("Executing command: {:?}", cmd);

        let child = tokio::process::Command::from(cmd)
            .spawn()
            .context("Failed to spawn Claude Code process")?;

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

        Ok(WorkerProcess {
            info: updated_info,
            process: Some(child),
        })
    }

    fn build_worker_prompt(
        worker_info: &WorkerInfo,
        system_prompt: &str,
        queue_name: &str,
    ) -> String {
        format!(
            r#"{system_prompt}

WORKER CONFIGURATION:
- Worker ID: {worker_id}
- Project: {project_id}
- Worker Type: {worker_type}
- Queue Name: {queue_name}

TASK PROCESSING INSTRUCTIONS:
1. You are a specialized worker for the vibe-ensemble multi-agent system
2. Your queue is: {queue_name}
3. Process tasks from your queue one by one
4. When queue is empty, exit gracefully
5. For each task, read the full ticket content including previous worker reports
6. Complete your stage and add a detailed report as a comment
7. Update the ticket's completed stage when done
8. Continue to next task or exit when queue is empty

Use the vibe-ensemble MCP server to:
- Get tasks from your queue: get_queue_tasks("{queue_name}")
- Get ticket details: get_ticket(ticket_id)
- Add your report: add_ticket_comment(ticket_id, worker_type, worker_id, stage_number, content)
- Update stage completion: complete_ticket_stage(ticket_id, stage)

COORDINATOR WORKFLOW:
The coordinator uses this streamlined workflow to manage the multi-agent system:
1. Create project: create_project(project_id, name, path, description)
2. Define worker types: create_worker_type(project_id, worker_type, system_prompt, description)
3. Create tickets: create_ticket(project_id, title, description)
4. Assign tasks: assign_task(ticket_id, queue_name) - workers auto-spawn on first task assignment
5. Monitor progress: list_events(), get_queue_status(queue_name), get_ticket(ticket_id)

IMPORTANT: Workers are now AUTO-SPAWNED when tasks are assigned to queues! 
- No need to manually spawn workers or create queues
- Simply assign tasks to appropriate queue names (e.g., "architect-queue", "developer-queue")
- The system automatically detects if a worker exists for the queue and spawns one if needed
- Workers stop automatically when their queue becomes empty

Remember: You are working autonomously. Process tasks thoroughly and provide detailed reports for the next worker or coordinator.
"#,
            worker_id = worker_info.worker_id,
            project_id = worker_info.project_id,
            worker_type = worker_info.worker_type,
            queue_name = queue_name,
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
}
