//! Worker Process Management for Claude Code Instances
//!
//! This module provides comprehensive lifecycle management for spawned Claude Code worker processes,
//! including process spawning, connection correlation, output capture, and graceful shutdown.

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, trace, warn};
use uuid::Uuid;

/// Maximum number of output lines to buffer per worker
const MAX_OUTPUT_BUFFER_SIZE: usize = 1000;

/// Timeout for graceful worker shutdown before force kill
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

/// Worker process management system
///
/// Manages the lifecycle of Claude Code worker processes, including:
/// - Process spawning with task-specific prompts
/// - Connection correlation with WebSocket transport
/// - Real-time output capture and streaming
/// - Graceful and forced shutdown capabilities
/// - Worker registry for status tracking
pub struct WorkerManager {
    /// Active worker processes indexed by worker ID
    workers: Arc<RwLock<HashMap<Uuid, WorkerHandle>>>,
    /// WebSocket connection mapping (connection_id -> worker_id)
    connections: Arc<RwLock<HashMap<String, Uuid>>>,
    /// Reverse connection mapping (worker_id -> connection_id)
    worker_connections: Arc<RwLock<HashMap<Uuid, String>>>,
    /// Output broadcast channels for real-time dashboard updates
    output_channels: Arc<RwLock<HashMap<Uuid, broadcast::Sender<WorkerOutput>>>>,
    /// MCP server configuration
    mcp_config: McpServerConfig,
    /// Worker output logging configuration
    output_logging: WorkerOutputConfig,
}

impl WorkerManager {
    /// Create a new worker manager
    pub fn new(mcp_config: McpServerConfig, output_logging: WorkerOutputConfig) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            worker_connections: Arc::new(RwLock::new(HashMap::new())),
            output_channels: Arc::new(RwLock::new(HashMap::new())),
            mcp_config,
            output_logging,
        }
    }

    /// Spawn a new Claude Code worker process
    ///
    /// # Arguments
    /// * `prompt` - Task-specific prompt for the worker
    /// * `capabilities` - List of capabilities/tools the worker should have
    /// * `working_directory` - Optional working directory for the worker
    ///
    /// # Returns
    /// Worker ID for tracking the spawned process
    pub async fn spawn_worker(
        &self,
        prompt: String,
        capabilities: Vec<String>,
        working_directory: Option<PathBuf>,
    ) -> Result<Uuid> {
        let worker_id = Uuid::new_v4();
        
        // INFO level: Basic spawn information
        info!("Spawning Claude Code worker {} for prompt: '{}'", 
              worker_id, 
              prompt.chars().take(80).collect::<String>());

        // Build Claude Code command (use 'claude' not 'claude-code')
        // Resolve full path to claude binary to handle PATH issues
        let claude_path = resolve_claude_binary_path().await?;
        debug!("Worker {} resolved claude binary path: {}", worker_id, claude_path);
        
        let mut cmd = Command::new(&claude_path);
        cmd.arg("-p").arg(&prompt);

        // Add MCP server configuration
        let mcp_url = format!("ws://{}:{}/mcp", self.mcp_config.host, self.mcp_config.port);
        cmd.arg("--mcp-server").arg(&mcp_url);

        // DEBUG level: Command details
        debug!("Worker {} command: claude -p \"{}\" --mcp-server \"{}\"", 
               worker_id, 
               prompt.chars().take(50).collect::<String>(),
               mcp_url);
        
        debug!("Worker {} capabilities: {:?}", worker_id, capabilities);

        // Set working directory if provided
        if let Some(working_dir) = &working_directory {
            cmd.current_dir(working_dir);
            debug!("Worker {} working directory: {:?}", worker_id, working_dir);
        } else {
            debug!("Worker {} using current working directory", worker_id);
        }

        // Configure stdio for output capture
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // TRACE level: Full command details
        trace!("Worker {} full command args: {:?}", worker_id, cmd.as_std().get_args().collect::<Vec<_>>());
        trace!("Worker {} environment: {:?}", worker_id, cmd.as_std().get_envs().collect::<Vec<_>>());

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            error!("Failed to spawn Claude Code worker {}: {} (command: 'claude')", worker_id, e);
            Error::Worker(format!("Failed to spawn worker process 'claude': {} - Is Claude Code installed and in PATH?", e))
        })?;

        let started_at = Utc::now();

        // Get process ID for tracking
        let process_id = child.id();
        
        // INFO level: Success with PID
        if let Some(pid) = process_id {
            info!("Claude Code worker {} spawned successfully with PID: {}", worker_id, pid);
        } else {
            info!("Claude Code worker {} spawned successfully (PID not available)", worker_id);
        }
        
        // DEBUG level: Additional spawn details
        debug!("Worker {} started at: {}", worker_id, started_at.format("%Y-%m-%d %H:%M:%S UTC"));

        // Setup output capture with broadcast channel for multiple subscribers
        let (broadcast_sender, _) = broadcast::channel(1000);
        self.output_channels
            .write()
            .await
            .insert(worker_id, broadcast_sender.clone());

        // Create internal channel for output processing
        let (output_sender, output_receiver) = mpsc::unbounded_channel();

        // Capture stdout
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Worker("Failed to capture worker stdout".to_string()))?;

        // Capture stderr
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Worker("Failed to capture worker stderr".to_string()))?;

        // Create worker handle
        let worker_handle = WorkerHandle {
            id: worker_id,
            process_id,
            child,
            status: WorkerStatus::Starting,
            started_at,
            prompt: prompt.clone(),
            capabilities,
            working_directory,
            output_buffer: Arc::new(Mutex::new(OutputBuffer::new(MAX_OUTPUT_BUFFER_SIZE))),
            connection_id: None,
        };

        // Store worker handle
        self.workers.write().await.insert(worker_id, worker_handle);

        // Start output capture tasks
        self.start_output_capture(worker_id, stdout, stderr, output_sender)
            .await;

        // Start output processing task
        self.start_output_processing(worker_id, output_receiver)
            .await;

        // Start process exit watcher
        self.start_process_exit_watcher(worker_id).await;

        info!("Worker {} initialization completed", worker_id);
        Ok(worker_id)
    }

    /// Associate a worker with a WebSocket connection
    pub async fn register_connection(&self, worker_id: Uuid, connection_id: String) -> Result<()> {
        info!(
            "Registering connection {} for worker {}",
            connection_id, worker_id
        );

        // Update connection mappings
        self.connections
            .write()
            .await
            .insert(connection_id.clone(), worker_id);
        self.worker_connections
            .write()
            .await
            .insert(worker_id, connection_id.clone());

        // Update worker handle
        if let Some(worker) = self.workers.write().await.get_mut(&worker_id) {
            worker.connection_id = Some(connection_id);
            worker.status = WorkerStatus::Connected;
            info!("Worker {} marked as connected", worker_id);
        }

        Ok(())
    }

    /// Get worker ID by connection ID
    pub async fn get_worker_by_connection(&self, connection_id: &str) -> Option<Uuid> {
        self.connections.read().await.get(connection_id).copied()
    }

    /// Get connection ID by worker ID
    pub async fn get_connection_by_worker(&self, worker_id: &Uuid) -> Option<String> {
        self.worker_connections.read().await.get(worker_id).cloned()
    }

    /// Get worker status
    pub async fn get_worker_status(&self, worker_id: &Uuid) -> Option<WorkerInfo> {
        self.workers
            .read()
            .await
            .get(worker_id)
            .map(|worker| WorkerInfo {
                id: worker.id,
                process_id: worker.process_id,
                status: worker.status.clone(),
                started_at: worker.started_at,
                prompt: worker.prompt.clone(),
                capabilities: worker.capabilities.clone(),
                working_directory: worker.working_directory.clone(),
                connection_id: worker.connection_id.clone(),
            })
    }

    /// List all active workers
    pub async fn list_workers(&self) -> Vec<WorkerInfo> {
        self.workers
            .read()
            .await
            .values()
            .map(|worker| WorkerInfo {
                id: worker.id,
                process_id: worker.process_id,
                status: worker.status.clone(),
                started_at: worker.started_at,
                prompt: worker.prompt.clone(),
                capabilities: worker.capabilities.clone(),
                working_directory: worker.working_directory.clone(),
                connection_id: worker.connection_id.clone(),
            })
            .collect()
    }

    /// Get worker output buffer
    pub async fn get_worker_output(&self, worker_id: &Uuid) -> Result<Vec<OutputLine>> {
        if let Some(worker) = self.workers.read().await.get(worker_id) {
            Ok(worker.output_buffer.lock().await.get_lines())
        } else {
            Err(Error::Worker(format!("Worker {} not found", worker_id)))
        }
    }

    /// Subscribe to worker output stream
    pub async fn subscribe_to_output(
        &self,
        worker_id: &Uuid,
    ) -> Option<broadcast::Receiver<WorkerOutput>> {
        if let Some(broadcast_sender) = self.output_channels.read().await.get(worker_id) {
            let receiver = broadcast_sender.subscribe();

            // Send subscription notification
            let _ = broadcast_sender.send(WorkerOutput {
                worker_id: *worker_id,
                output_type: OutputType::Info,
                content: "Subscribed to worker output stream".to_string(),
                timestamp: Utc::now(),
            });

            Some(receiver)
        } else {
            None
        }
    }

    /// Gracefully shutdown a worker
    pub async fn shutdown_worker(&self, worker_id: &Uuid) -> Result<()> {
        info!("Initiating graceful shutdown for worker {}", worker_id);

        let mut workers = self.workers.write().await;
        if let Some(mut worker) = workers.remove(worker_id) {
            worker.status = WorkerStatus::Stopping;

            // Try graceful shutdown first via MCP close message
            if let Some(_connection_id) = &worker.connection_id {
                info!("Sending MCP close message to worker {}", worker_id);
                // Note: This would be implemented by the MCP server component
                // sending a proper close message through the WebSocket connection
            }

            // Wait for graceful shutdown or timeout
            let shutdown_result = timeout(GRACEFUL_SHUTDOWN_TIMEOUT, async {
                // Wait for process to exit gracefully
                if let Ok(status) = worker.child.wait().await {
                    info!(
                        "Worker {} exited gracefully with status: {:?}",
                        worker_id, status
                    );
                    return Ok(());
                }
                Ok::<(), Error>(())
            })
            .await;

            match shutdown_result {
                Ok(_) => {
                    info!("Worker {} shutdown gracefully", worker_id);
                }
                Err(_) => {
                    warn!(
                        "Worker {} did not respond to graceful shutdown, forcing termination",
                        worker_id
                    );

                    // Force kill the process
                    if let Err(e) = worker.child.kill().await {
                        error!("Failed to force kill worker {}: {}", worker_id, e);
                        return Err(Error::Worker(format!(
                            "Failed to kill worker process: {}",
                            e
                        )));
                    }

                    // Wait for the killed process to be reaped to avoid zombies
                    if let Ok(status) = worker.child.wait().await {
                        info!(
                            "Worker {} force killed and reaped with status: {:?}",
                            worker_id, status
                        );
                    } else {
                        warn!("Worker {} force killed but reaping failed", worker_id);
                    }

                    info!("Worker {} force terminated", worker_id);
                }
            }

            // Clean up connection mappings
            if let Some(connection_id) = &worker.connection_id {
                self.connections.write().await.remove(connection_id);
            }
            self.worker_connections.write().await.remove(worker_id);
            self.output_channels.write().await.remove(worker_id);

            info!("Worker {} cleanup completed", worker_id);
            Ok(())
        } else {
            Err(Error::Worker(format!("Worker {} not found", worker_id)))
        }
    }

    /// Shutdown all workers
    pub async fn shutdown_all(&self) -> Result<()> {
        info!("Shutting down all workers");

        let worker_ids: Vec<Uuid> = self.workers.read().await.keys().copied().collect();

        for worker_id in worker_ids {
            if let Err(e) = self.shutdown_worker(&worker_id).await {
                error!("Failed to shutdown worker {}: {}", worker_id, e);
            }
        }

        info!("All workers shutdown completed");
        Ok(())
    }

    /// Handle worker disconnection
    pub async fn handle_worker_disconnection(&self, connection_id: &str) -> Result<()> {
        if let Some(worker_id) = self.get_worker_by_connection(connection_id).await {
            warn!(
                "Worker {} disconnected via connection {}",
                worker_id, connection_id
            );

            // Update worker status
            if let Some(worker) = self.workers.write().await.get_mut(&worker_id) {
                worker.status = WorkerStatus::Disconnected;
                worker.connection_id = None;
            }

            // Clean up connection mappings but keep worker for potential reconnection
            self.connections.write().await.remove(connection_id);
            self.worker_connections.write().await.remove(&worker_id);

            info!("Connection cleanup completed for worker {}", worker_id);
        }
        Ok(())
    }

    /// Start output capture tasks for a worker
    async fn start_output_capture(
        &self,
        worker_id: Uuid,
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
        output_sender: mpsc::UnboundedSender<WorkerOutput>,
    ) {
        let worker_output_config = self.output_logging.clone();

        // Capture stdout
        let stdout_sender = output_sender.clone();
        let stdout_config = worker_output_config.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut line).await {
                if bytes_read == 0 {
                    break; // EOF
                }

                let output = WorkerOutput {
                    worker_id,
                    output_type: OutputType::Stdout,
                    content: line.trim_end().to_string(),
                    timestamp: Utc::now(),
                };

                if let Err(e) = stdout_sender.send(output) {
                    debug!(
                        "Failed to send stdout output for worker {}: {}",
                        worker_id, e
                    );
                    break;
                }

                // Log to file if enabled
                if stdout_config.enabled {
                    if let Some(ref log_dir) = stdout_config.log_directory {
                        let log_path = log_dir.join(format!("worker-{}-stdout.log", worker_id));
                        if let Ok(mut file) = tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_path)
                            .await
                        {
                            let _ = file.write_all(line.as_bytes()).await;
                        }
                    }
                }

                line.clear();
            }
            debug!("Stdout capture ended for worker {}", worker_id);
        });

        // Capture stderr
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut line = String::new();

            while let Ok(bytes_read) = reader.read_line(&mut line).await {
                if bytes_read == 0 {
                    break; // EOF
                }

                let output = WorkerOutput {
                    worker_id,
                    output_type: OutputType::Stderr,
                    content: line.trim_end().to_string(),
                    timestamp: Utc::now(),
                };

                if let Err(e) = output_sender.send(output) {
                    debug!(
                        "Failed to send stderr output for worker {}: {}",
                        worker_id, e
                    );
                    break;
                }

                // Log to file if enabled
                if worker_output_config.enabled {
                    if let Some(ref log_dir) = worker_output_config.log_directory {
                        let log_path = log_dir.join(format!("worker-{}-stderr.log", worker_id));
                        if let Ok(mut file) = tokio::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_path)
                            .await
                        {
                            let _ = file.write_all(line.as_bytes()).await;
                        }
                    }
                }

                line.clear();
            }
            debug!("Stderr capture ended for worker {}", worker_id);
        });
    }

    /// Start output processing task for a worker
    async fn start_output_processing(
        &self,
        worker_id: Uuid,
        mut output_receiver: mpsc::UnboundedReceiver<WorkerOutput>,
    ) {
        let workers = self.workers.clone();
        let output_channels = self.output_channels.clone();

        tokio::spawn(async move {
            while let Some(output) = output_receiver.recv().await {
                // Add to worker's output buffer
                if let Some(worker) = workers.read().await.get(&worker_id) {
                    worker.output_buffer.lock().await.add_line(OutputLine {
                        timestamp: output.timestamp,
                        output_type: output.output_type.clone(),
                        content: output.content.clone(),
                    });
                }

                // Broadcast to all subscribers
                if let Some(broadcast_sender) = output_channels.read().await.get(&worker_id) {
                    // Ignore send errors (no subscribers)
                    let _ = broadcast_sender.send(output.clone());
                }

                debug!(
                    "Worker {} output: {} - {}",
                    worker_id,
                    output.output_type,
                    output.content.chars().take(100).collect::<String>()
                );
            }
            debug!("Output processing ended for worker {}", worker_id);
        });
    }

    /// Start process exit watcher for a worker
    async fn start_process_exit_watcher(&self, worker_id: Uuid) {
        let workers = self.workers.clone();
        let connections = self.connections.clone();
        let worker_connections = self.worker_connections.clone();
        let output_channels = self.output_channels.clone();

        tokio::spawn(async move {
            // Wait for the process to exit
            let mut child_exit = None;

            // Extract the child handle for waiting
            if let Some(worker) = workers.write().await.get_mut(&worker_id) {
                // We can't move the child out while it's in the HashMap, so we'll use try_wait instead
                loop {
                    match worker.child.try_wait() {
                        Ok(Some(status)) => {
                            child_exit = Some(status);
                            break;
                        }
                        Ok(None) => {
                            // Process is still running, wait a bit and check again
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        Err(e) => {
                            error!("Error checking worker {} exit status: {}", worker_id, e);
                            break;
                        }
                    }

                    // Check if the worker still exists (might have been shut down)
                    if workers.read().await.get(&worker_id).is_none() {
                        debug!("Worker {} was removed during exit watching", worker_id);
                        return;
                    }
                }
            }

            if let Some(exit_status) = child_exit {
                info!("Worker {} exited with status: {:?}", worker_id, exit_status);

                // Update worker status to stopped
                if let Some(worker) = workers.write().await.get_mut(&worker_id) {
                    worker.status = WorkerStatus::Stopped;

                    // Send final output message
                    if let Some(broadcast_sender) = output_channels.read().await.get(&worker_id) {
                        let _ = broadcast_sender.send(WorkerOutput {
                            worker_id,
                            output_type: OutputType::Info,
                            content: format!("Process exited with status: {:?}", exit_status),
                            timestamp: Utc::now(),
                        });
                    }
                }

                // Clean up connection mappings if worker was connected
                if let Some(connection_id) = worker_connections.read().await.get(&worker_id) {
                    connections.write().await.remove(connection_id);
                    worker_connections.write().await.remove(&worker_id);
                    info!("Cleaned up connection mappings for worker {}", worker_id);
                }
            }
        });
    }
}

/// Handle for managing an individual worker process
pub struct WorkerHandle {
    /// Unique worker identifier
    pub id: Uuid,
    /// Operating system process ID
    pub process_id: Option<u32>,
    /// Tokio child process handle
    pub child: Child,
    /// Current worker status
    pub status: WorkerStatus,
    /// Process start time
    pub started_at: DateTime<Utc>,
    /// Task-specific prompt used to spawn the worker
    pub prompt: String,
    /// Worker capabilities
    pub capabilities: Vec<String>,
    /// Working directory
    pub working_directory: Option<PathBuf>,
    /// Output buffer for storing worker output
    pub output_buffer: Arc<Mutex<OutputBuffer>>,
    /// Associated WebSocket connection ID
    pub connection_id: Option<String>,
}

/// Worker status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkerStatus {
    /// Worker process is starting up
    Starting,
    /// Worker is connected via WebSocket
    Connected,
    /// Worker is running but not connected
    Running,
    /// Worker connection was lost
    Disconnected,
    /// Worker is being stopped
    Stopping,
    /// Worker has completed or crashed
    Stopped,
}

impl std::fmt::Display for WorkerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkerStatus::Starting => write!(f, "starting"),
            WorkerStatus::Connected => write!(f, "connected"),
            WorkerStatus::Running => write!(f, "running"),
            WorkerStatus::Disconnected => write!(f, "disconnected"),
            WorkerStatus::Stopping => write!(f, "stopping"),
            WorkerStatus::Stopped => write!(f, "stopped"),
        }
    }
}

/// Worker information for external queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub id: Uuid,
    pub process_id: Option<u32>,
    pub status: WorkerStatus,
    pub started_at: DateTime<Utc>,
    pub prompt: String,
    pub capabilities: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub connection_id: Option<String>,
}

/// Worker output message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
    pub worker_id: Uuid,
    pub output_type: OutputType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

/// Type of worker output
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputType {
    /// Standard output stream
    Stdout,
    /// Standard error stream
    Stderr,
    /// System information
    Info,
}

impl std::fmt::Display for OutputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputType::Stdout => write!(f, "stdout"),
            OutputType::Stderr => write!(f, "stderr"),
            OutputType::Info => write!(f, "info"),
        }
    }
}

/// Individual output line in the buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputLine {
    pub timestamp: DateTime<Utc>,
    pub output_type: OutputType,
    pub content: String,
}

/// Circular buffer for worker output
pub struct OutputBuffer {
    lines: Vec<OutputLine>,
    max_size: usize,
    current_index: usize,
}

impl OutputBuffer {
    pub fn new(max_size: usize) -> Self {
        Self {
            lines: Vec::with_capacity(max_size),
            max_size,
            current_index: 0,
        }
    }

    pub fn add_line(&mut self, line: OutputLine) {
        if self.lines.len() < self.max_size {
            self.lines.push(line);
        } else {
            self.lines[self.current_index] = line;
            self.current_index = (self.current_index + 1) % self.max_size;
        }
    }

    pub fn get_lines(&self) -> Vec<OutputLine> {
        if self.lines.len() < self.max_size {
            self.lines.clone()
        } else {
            // Return lines in chronological order
            let mut result = Vec::with_capacity(self.max_size);
            for i in 0..self.max_size {
                let index = (self.current_index + i) % self.max_size;
                result.push(self.lines[index].clone());
            }
            result
        }
    }
}

/// MCP server configuration for worker connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8081,
        }
    }
}

/// Worker output logging configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkerOutputConfig {
    pub enabled: bool,
    pub log_directory: Option<PathBuf>,
}

/// Resolve the full path to the Claude binary using the system PATH
/// This handles cases where `claude` is in non-standard locations like ~/.local/bin
async fn resolve_claude_binary_path() -> Result<String> {
    // First try using `which claude` command to find the binary
    let output = Command::new("which")
        .arg("claude")
        .output()
        .await;

    match output {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() && std::path::Path::new(&path).exists() {
                info!("Found Claude binary at: {}", path);
                return Ok(path);
            }
        }
        Ok(output) => {
            debug!("'which claude' failed with status: {:?}, stderr: {}", 
                   output.status, String::from_utf8_lossy(&output.stderr));
        }
        Err(e) => {
            debug!("Failed to execute 'which claude': {}", e);
        }
    }

    // Fallback: try common locations where Claude might be installed
    let common_paths = [
        "/usr/local/bin/claude",
        "/usr/bin/claude", 
        "/opt/homebrew/bin/claude",
        &format!("{}/.local/bin/claude", std::env::var("HOME").unwrap_or_default()),
        &format!("{}/bin/claude", std::env::var("HOME").unwrap_or_default()),
    ];

    for path in &common_paths {
        if std::path::Path::new(path).exists() {
            info!("Found Claude binary at fallback location: {}", path);
            return Ok(path.to_string());
        }
    }

    // Final fallback: return "claude" and let the system handle it
    warn!("Could not resolve Claude binary path, falling back to 'claude' (may fail if not in PATH)");
    Ok("claude".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_worker_manager_creation() {
        let mcp_config = McpServerConfig::default();
        let output_config = WorkerOutputConfig::default();
        let manager = WorkerManager::new(mcp_config, output_config);

        // Verify initial state
        assert_eq!(manager.list_workers().await.len(), 0);
    }

    #[tokio::test]
    async fn test_output_buffer() {
        let mut buffer = OutputBuffer::new(3);

        // Add lines to buffer
        buffer.add_line(OutputLine {
            timestamp: Utc::now(),
            output_type: OutputType::Stdout,
            content: "Line 1".to_string(),
        });

        buffer.add_line(OutputLine {
            timestamp: Utc::now(),
            output_type: OutputType::Stdout,
            content: "Line 2".to_string(),
        });

        buffer.add_line(OutputLine {
            timestamp: Utc::now(),
            output_type: OutputType::Stdout,
            content: "Line 3".to_string(),
        });

        assert_eq!(buffer.get_lines().len(), 3);

        // Add one more line to test circular behavior
        buffer.add_line(OutputLine {
            timestamp: Utc::now(),
            output_type: OutputType::Stdout,
            content: "Line 4".to_string(),
        });

        let lines = buffer.get_lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[2].content, "Line 4");
    }

    #[tokio::test]
    async fn test_worker_status_display() {
        assert_eq!(WorkerStatus::Starting.to_string(), "starting");
        assert_eq!(WorkerStatus::Connected.to_string(), "connected");
        assert_eq!(WorkerStatus::Stopped.to_string(), "stopped");
    }

    #[tokio::test]
    async fn test_output_type_display() {
        assert_eq!(OutputType::Stdout.to_string(), "stdout");
        assert_eq!(OutputType::Stderr.to_string(), "stderr");
        assert_eq!(OutputType::Info.to_string(), "info");
    }

    #[test]
    fn test_mcp_server_config_default() {
        let config = McpServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8081);
    }

    #[test]
    fn test_worker_output_config_default() {
        let config = WorkerOutputConfig::default();
        assert!(!config.enabled);
        assert!(config.log_directory.is_none());
    }
}
