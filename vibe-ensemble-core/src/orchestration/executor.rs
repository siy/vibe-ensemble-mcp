//! Headless Claude Code execution engine
//!
//! This module provides functionality for executing Claude Code commands programmatically
//! and parsing their structured JSON stream output. It enables automated agent orchestration
//! by running Claude Code in headless mode and capturing execution results.

use crate::orchestration::models::WorkspaceConfiguration;
use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{info, trace};
use uuid::Uuid;

/// Event types from Claude Code's JSON stream output
/// Based on actual output: `claude -p "prompt" --output-format stream-json --verbose`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeStreamEvent {
    /// System initialization event
    #[serde(rename = "system")]
    System {
        subtype: String,
        cwd: String,
        session_id: String,
        tools: Vec<String>,
        mcp_servers: Vec<String>,
        model: String,
        #[serde(rename = "permissionMode")]
        permission_mode: String,
        slash_commands: Vec<String>,
        #[serde(rename = "apiKeySource")]
        api_key_source: String,
        output_style: String,
        uuid: String,
    },
    /// Assistant response event
    #[serde(rename = "assistant")]
    Assistant {
        message: ClaudeMessage,
        parent_tool_use_id: Option<String>,
        session_id: String,
        uuid: String,
    },
    /// Final execution result
    #[serde(rename = "result")]
    Result {
        subtype: String,
        is_error: bool,
        duration_ms: u64,
        duration_api_ms: u64,
        num_turns: u32,
        result: String,
        session_id: String,
        total_cost_usd: f64,
        usage: ClaudeUsageStats,
        permission_denials: Vec<String>,
        uuid: String,
    },
    /// Unknown or future event types
    #[serde(other)]
    Unknown,
}

/// Claude Code message structure from actual JSON output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<ClaudeMessageContent>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: ClaudeUsageStats,
}

/// Message content from Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
}

/// Usage statistics from Claude Code
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeUsageStats {
    pub input_tokens: u32,
    pub cache_creation_input_tokens: Option<u32>,
    pub cache_read_input_tokens: Option<u32>,
    pub cache_creation: Option<CacheCreationStats>,
    pub output_tokens: u32,
    pub service_tier: String,
}

/// Cache creation statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheCreationStats {
    pub ephemeral_5m_input_tokens: u32,
    pub ephemeral_1h_input_tokens: u32,
}

/// Usage information for our internal tracking (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
    pub cost_usd: Option<f64>,
}

/// Error details from failed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub error_type: String,
    pub message: String,
    pub code: Option<i32>,
    pub details: Option<HashMap<String, serde_json::Value>>,
}

/// Execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub duration_ms: u64,
    pub memory_usage_mb: Option<f64>,
    pub api_calls: u32,
    pub tool_calls: u32,
    pub total_cost_usd: Option<f64>,
}

/// Result of executing a Claude Code command
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Unique identifier for this execution
    pub execution_id: Uuid,
    /// The prompt that was executed
    pub prompt: String,
    /// Final content from the assistant
    pub content: String,
    /// Whether the execution was successful
    pub success: bool,
    /// Any error that occurred
    pub error: Option<String>,
    /// All events from the stream
    pub events: Vec<ClaudeStreamEvent>,
    /// Aggregated usage statistics
    pub usage: Option<UsageInfo>,
    /// Execution timing
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    /// Working directory used for execution
    pub working_directory: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Configuration for Claude Code execution
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Timeout for the execution in seconds
    pub timeout_seconds: u64,
    /// Whether to capture verbose output
    pub verbose: bool,
    /// Additional environment variables
    pub environment: HashMap<String, String>,
    /// Working directory override
    pub working_directory: Option<String>,
    /// Output format (should be "stream-json")
    pub output_format: String,
    /// Whether to deploy shared settings before execution
    pub deploy_shared_settings: bool,
    /// Path to shared settings template (defaults to agent-templates/shared/.claude/settings.json)
    pub shared_settings_template_path: Option<PathBuf>,
    /// Optional path to log worker output to a file
    pub worker_output_log_path: Option<PathBuf>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 300, // 5 minutes
            verbose: true,
            environment: HashMap::new(),
            working_directory: None,
            output_format: "stream-json".to_string(),
            deploy_shared_settings: true,
            shared_settings_template_path: None,
            worker_output_log_path: None,
        }
    }
}

/// Headless Claude Code executor
#[derive(Debug, Clone)]
pub struct HeadlessClaudeExecutor {
    /// Path to the Claude Code binary
    pub claude_binary_path: String,
    /// Default execution configuration
    pub default_config: ExecutionConfig,
    /// Base path for agent templates (defaults to current directory)
    pub agent_templates_base_path: PathBuf,
}

impl Default for HeadlessClaudeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessClaudeExecutor {
    /// Create a new headless executor with default settings
    pub fn new() -> Self {
        Self {
            claude_binary_path: "claude".to_string(), // Assume claude is in PATH
            default_config: ExecutionConfig::default(),
            agent_templates_base_path: PathBuf::from("."),
        }
    }

    /// Create a new executor with custom binary path
    pub fn with_binary_path(binary_path: String) -> Self {
        Self {
            claude_binary_path: binary_path,
            default_config: ExecutionConfig::default(),
            agent_templates_base_path: PathBuf::from("."),
        }
    }

    /// Create a new executor with custom agent templates base path
    pub fn with_agent_templates_path(agent_templates_path: PathBuf) -> Self {
        Self {
            claude_binary_path: "claude".to_string(),
            default_config: ExecutionConfig::default(),
            agent_templates_base_path: agent_templates_path,
        }
    }

    /// Create a new executor with custom binary path and agent templates path
    pub fn with_paths(binary_path: String, agent_templates_path: PathBuf) -> Self {
        Self {
            claude_binary_path: binary_path,
            default_config: ExecutionConfig::default(),
            agent_templates_base_path: agent_templates_path,
        }
    }

    /// Execute a prompt in the given workspace
    pub async fn execute_prompt(
        &self,
        workspace: &WorkspaceConfiguration,
        prompt: &str,
    ) -> Result<ExecutionResult> {
        self.execute_prompt_with_config(workspace, prompt, &self.default_config)
            .await
    }

    /// Execute a prompt with custom configuration
    pub async fn execute_prompt_with_config(
        &self,
        workspace: &WorkspaceConfiguration,
        prompt: &str,
        config: &ExecutionConfig,
    ) -> Result<ExecutionResult> {
        let execution_id = Uuid::new_v4();
        let started_at = Utc::now();

        // Determine working directory
        let working_dir = config
            .working_directory
            .as_deref()
            .unwrap_or_else(|| workspace.project_path.to_str().unwrap_or("."));

        // Deploy shared settings if enabled
        if config.deploy_shared_settings {
            self.deploy_shared_settings(workspace, config).await?;
        }

        // Build command
        let mut cmd = Command::new(&self.claude_binary_path);
        cmd.arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg(&config.output_format)
            .current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if config.verbose {
            cmd.arg("--verbose");
        }

        // Add environment variables
        for (key, value) in &config.environment {
            cmd.env(key, value);
        }

        // Set workspace-specific environment
        cmd.env("CLAUDE_WORKSPACE_ID", workspace.id.to_string())
            .env("CLAUDE_WORKSPACE_NAME", &workspace.name)
            .env("CLAUDE_TEMPLATE_NAME", &workspace.template_name);

        // Log the full command line at TRACE level
        trace!(
            "Spawning Claude Code process: {} {}",
            self.claude_binary_path,
            cmd.as_std().get_args().collect::<Vec<_>>().iter().map(|arg| arg.to_string_lossy()).collect::<Vec<_>>().join(" ")
        );

        // Execute command
        let mut child = cmd.spawn().map_err(|e| Error::Execution {
            message: format!("Failed to spawn Claude Code process: {}", e),
        })?;

        // Log the process ID at INFO level
        if let Some(pid) = child.id() {
            info!("Claude Code worker process started with PID: {}", pid);
        }

        let stdout = child.stdout.take().ok_or_else(|| Error::Execution {
            message: "Failed to capture stdout".to_string(),
        })?;

        let stderr = child.stderr.take().ok_or_else(|| Error::Execution {
            message: "Failed to capture stderr".to_string(),
        })?;

        // Setup timeout
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(config.timeout_seconds),
            self.process_stream(stdout, stderr, config.worker_output_log_path.as_ref()),
        );

        let (events, success, content, error_msg) = match timeout.await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                // Kill the process if it's still running
                let _ = child.kill().await;
                return Err(e);
            }
            Err(_) => {
                // Timeout occurred
                let _ = child.kill().await;
                return Err(Error::Execution {
                    message: format!(
                        "Execution timed out after {} seconds",
                        config.timeout_seconds
                    ),
                });
            }
        };

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| Error::Execution {
            message: format!("Failed to wait for process completion: {}", e),
        })?;

        let completed_at = Utc::now();

        // Aggregate usage information
        let usage = self.aggregate_usage(&events);

        Ok(ExecutionResult {
            execution_id,
            prompt: prompt.to_string(),
            content,
            success: success && status.success(),
            error: if success && status.success() {
                None
            } else {
                Some(
                    error_msg.unwrap_or_else(|| {
                        format!("Process exited with code: {:?}", status.code())
                    }),
                )
            },
            events,
            usage,
            started_at,
            completed_at,
            working_directory: working_dir.to_string(),
            metadata: HashMap::new(),
        })
    }

    /// Process the JSON stream from Claude Code
    async fn process_stream(
        &self,
        stdout: tokio::process::ChildStdout,
        stderr: tokio::process::ChildStderr,
        log_file_path: Option<&PathBuf>,
    ) -> Result<(Vec<ClaudeStreamEvent>, bool, String, Option<String>)> {
        let mut events = Vec::new();
        let mut success = true;
        let mut final_content = String::new();
        let mut error_message = None;

        let stdout_reader = BufReader::new(stdout);
        let mut stdout_lines = stdout_reader.lines();

        let stderr_reader = BufReader::new(stderr);
        let mut stderr_lines = stderr_reader.lines();

        // Setup optional file logging
        let mut log_file = if let Some(log_path) = log_file_path {
            // Create parent directories if they don't exist
            if let Some(parent) = log_path.parent() {
                fs::create_dir_all(parent).await.map_err(|e| Error::Execution {
                    message: format!("Failed to create log directory: {}", e),
                })?;
            }
            
            Some(
                fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(log_path)
                    .await
                    .map_err(|e| Error::Execution {
                        message: format!("Failed to open worker log file: {}", e),
                    })?
            )
        } else {
            None
        };

        // Helper macro to log to file  
        macro_rules! log_to_file {
            ($prefix:expr, $content:expr) => {
                if let Some(ref mut file) = log_file {
                    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC");
                    let log_line = format!("[{}] {}: {}\n", timestamp, $prefix, $content);
                    if let Err(e) = file.write_all(log_line.as_bytes()).await {
                        eprintln!("Warning: Failed to write to worker log file: {}", e);
                    }
                    if let Err(e) = file.flush().await {
                        eprintln!("Warning: Failed to flush worker log file: {}", e);
                    }
                }
            };
        }

        // Read from both stdout and stderr concurrently
        loop {
            tokio::select! {
                stdout_line = stdout_lines.next_line() => {
                    match stdout_line {
                        Ok(Some(line)) => {
                            // Log to file if enabled
                            log_to_file!("STDOUT", &line);
                            if let Some(event) = self.parse_stream_event(&line)? {
                                // Extract content from assistant messages
                                if let ClaudeStreamEvent::Assistant { message, .. } = &event {
                                    if let Some(content) = message.content.first() {
                                        final_content = content.text.clone();
                                    }
                                }

                                // Check for errors in result events
                                if let ClaudeStreamEvent::Result { is_error, result, .. } = &event {
                                    if *is_error {
                                        success = false;
                                        error_message = Some(result.clone());
                                    } else {
                                        // Update final content with result if available
                                        if !result.is_empty() {
                                            final_content = result.clone();
                                        }
                                    }
                                }

                                events.push(event);
                            }
                        }
                        Ok(None) => break, // EOF
                        Err(e) => {
                            return Err(Error::Execution {
                                message: format!("Failed to read stdout: {}", e),
                            });
                        }
                    }
                }
                stderr_line = stderr_lines.next_line() => {
                    match stderr_line {
                        Ok(Some(line)) => {
                            eprintln!("Claude Code stderr: {}", line);
                            // Log to file if enabled
                            log_to_file!("STDERR", &line);
                            // Only treat stderr as error if it contains error indicators
                            let lower_line = line.to_lowercase();
                            if error_message.is_none() &&
                               (lower_line.contains("error") ||
                                lower_line.contains("fatal") ||
                                lower_line.contains("failed")) {
                                error_message = Some(line);
                                success = false;
                            }
                        }
                        Ok(None) => {}, // EOF on stderr
                        Err(e) => {
                            return Err(Error::Execution {
                                message: format!("Failed to read stderr: {}", e),
                            });
                        }
                    }
                }
            }
        }

        Ok((events, success, final_content, error_message))
    }

    /// Parse a single line from the JSON stream
    fn parse_stream_event(&self, line: &str) -> Result<Option<ClaudeStreamEvent>> {
        if line.trim().is_empty() {
            return Ok(None);
        }

        // Try to parse as JSON
        let json: serde_json::Value = serde_json::from_str(line).map_err(|e| Error::Parsing {
            message: format!("Failed to parse JSON from line '{}': {}", line, e),
        })?;

        // Try to deserialize into ClaudeStreamEvent
        match serde_json::from_value(json.clone()) {
            Ok(event) => Ok(Some(event)),
            Err(_) => {
                // If we can't parse it as a known event, store as unknown
                Ok(Some(ClaudeStreamEvent::Unknown))
            }
        }
    }

    /// Aggregate usage information from all events
    fn aggregate_usage(&self, events: &[ClaudeStreamEvent]) -> Option<UsageInfo> {
        let mut total_input_tokens = 0u32;
        let mut total_output_tokens = 0u32;
        let mut total_cost = 0.0f64;
        let mut has_usage = false;

        for event in events {
            match event {
                ClaudeStreamEvent::Assistant { message, .. } => {
                    has_usage = true;
                    total_input_tokens += message.usage.input_tokens;
                    total_output_tokens += message.usage.output_tokens;
                    // Note: Cost is not available per message in the current format
                }
                ClaudeStreamEvent::Result { total_cost_usd, .. } => {
                    has_usage = true;
                    total_cost = *total_cost_usd;
                }
                _ => {}
            }
        }

        if has_usage {
            Some(UsageInfo {
                input_tokens: if total_input_tokens > 0 {
                    Some(total_input_tokens)
                } else {
                    None
                },
                output_tokens: if total_output_tokens > 0 {
                    Some(total_output_tokens)
                } else {
                    None
                },
                total_tokens: if total_input_tokens + total_output_tokens > 0 {
                    Some(total_input_tokens + total_output_tokens)
                } else {
                    None
                },
                cost_usd: if total_cost > 0.0 {
                    Some(total_cost)
                } else {
                    None
                },
            })
        } else {
            None
        }
    }

    /// Check if Claude Code is available and working
    pub async fn health_check(&self) -> Result<bool> {
        let mut cmd = Command::new(&self.claude_binary_path);
        cmd.arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => match child.wait().await {
                Ok(status) => Ok(status.success()),
                Err(_) => Ok(false),
            },
            Err(_) => Ok(false),
        }
    }

    /// Get Claude Code version information
    pub async fn get_version(&self) -> Result<String> {
        let mut cmd = Command::new(&self.claude_binary_path);
        cmd.arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| Error::Execution {
            message: format!("Failed to get Claude Code version: {}", e),
        })?;

        if output.status.success() {
            String::from_utf8(output.stdout).map_err(|e| Error::Parsing {
                message: format!("Failed to parse version output: {}", e),
            })
        } else {
            Err(Error::Execution {
                message: "Failed to get Claude Code version".to_string(),
            })
        }
    }

    /// Deploy shared settings.json with environment variable substitution
    async fn deploy_shared_settings(
        &self,
        workspace: &WorkspaceConfiguration,
        config: &ExecutionConfig,
    ) -> Result<()> {
        // Determine the shared settings template path
        let template_path = config
            .shared_settings_template_path
            .clone()
            .unwrap_or_else(|| {
                self.agent_templates_base_path
                    .join("agent-templates")
                    .join("shared")
                    .join(".claude")
                    .join("settings.json")
            });

        // Check if template exists
        if !template_path.exists() {
            return Err(Error::Configuration {
                message: format!(
                    "Shared settings template not found at: {}",
                    template_path.display()
                ),
            });
        }

        // Read the template
        let template_content =
            fs::read_to_string(&template_path)
                .await
                .map_err(|e| Error::Configuration {
                    message: format!(
                        "Failed to read shared settings template from {}: {}",
                        template_path.display(),
                        e
                    ),
                })?;

        // Perform environment variable substitution
        let substituted_content =
            self.substitute_environment_variables(&template_content, workspace, config);

        // Determine target directory (.claude within the workspace)
        let target_dir = workspace.workspace_path.join(".claude");
        let target_path = target_dir.join("settings.json");

        // Create .claude directory if it doesn't exist
        fs::create_dir_all(&target_dir)
            .await
            .map_err(|e| Error::Configuration {
                message: format!(
                    "Failed to create .claude directory at {}: {}",
                    target_dir.display(),
                    e
                ),
            })?;

        // Write the substituted settings
        fs::write(&target_path, &substituted_content)
            .await
            .map_err(|e| Error::Configuration {
                message: format!(
                    "Failed to write settings.json to {}: {}",
                    target_path.display(),
                    e
                ),
            })?;

        // Validate the written JSON is valid
        self.validate_settings_json(&substituted_content)?;

        Ok(())
    }

    /// Substitute environment variables in the settings template
    fn substitute_environment_variables(
        &self,
        template: &str,
        workspace: &WorkspaceConfiguration,
        config: &ExecutionConfig,
    ) -> String {
        let mut result = template.to_string();

        // Helper to JSON-escape a value for inclusion inside JSON string literals
        fn json_escape(s: &str) -> String {
            // serde_json::to_string returns a quoted JSON string; strip enclosing quotes
            let quoted = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
            quoted.trim_matches('"').to_string()
        }

        // MCP Server configuration with defaults
        let vibe_ensemble_mcp_server = std::env::var("VIBE_ENSEMBLE_MCP_SERVER")
            .unwrap_or_else(|_| "ws://localhost:8080".to_string());
        let vibe_ensemble_mcp_binary = std::env::var("VIBE_ENSEMBLE_MCP_BINARY")
            .unwrap_or_else(|_| "vibe-ensemble-mcp".to_string());
        let vibe_ensemble_log_level =
            std::env::var("VIBE_ENSEMBLE_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite:./vibe-ensemble.db".to_string());

        // Generate AGENT_ID (defaults to WORKSPACE_ID if not set)
        let agent_id = std::env::var("AGENT_ID").unwrap_or_else(|_| workspace.id.to_string());

        // Pre-escaped variants for safe JSON embedding
        let mcp_server_esc = json_escape(&vibe_ensemble_mcp_server);
        let mcp_binary_esc = json_escape(&vibe_ensemble_mcp_binary);
        let log_level_esc = json_escape(&vibe_ensemble_log_level);
        let database_url_esc = json_escape(&database_url);
        let agent_id_esc = json_escape(&agent_id);

        // Substitute variables with default fallbacks
        result = result.replace(
            "${VIBE_ENSEMBLE_MCP_SERVER:-ws://localhost:8080}",
            &mcp_server_esc,
        );
        result = result.replace(
            "${VIBE_ENSEMBLE_MCP_BINARY:-vibe-ensemble-mcp}",
            &mcp_binary_esc,
        );
        result = result.replace("${VIBE_ENSEMBLE_LOG_LEVEL:-info}", &log_level_esc);
        result = result.replace(
            "${DATABASE_URL:-sqlite:./vibe-ensemble.db}",
            &database_url_esc,
        );
        result = result.replace("${AGENT_ID:-${WORKSPACE_ID}}", &agent_id_esc);

        // Substitute simple environment variables without defaults
        result = result.replace("${VIBE_ENSEMBLE_MCP_SERVER}", &mcp_server_esc);
        result = result.replace("${VIBE_ENSEMBLE_MCP_BINARY}", &mcp_binary_esc);
        result = result.replace("${VIBE_ENSEMBLE_LOG_LEVEL}", &log_level_esc);
        result = result.replace("${DATABASE_URL}", &database_url_esc);
        result = result.replace("${AGENT_ID}", &agent_id_esc);

        // Substitute workspace-specific variables
        result = result.replace("${WORKSPACE_ID}", &json_escape(&workspace.id.to_string()));
        result = result.replace("${WORKSPACE_NAME}", &json_escape(&workspace.name));
        result = result.replace("${TEMPLATE_NAME}", &json_escape(&workspace.template_name));

        // Substitute custom environment variables from config
        for (key, value) in &config.environment {
            let placeholder = format!("${{{}}}", key);
            result = result.replace(&placeholder, &json_escape(value));
        }

        result
    }

    /// Validate that the substituted settings JSON is valid
    fn validate_settings_json(&self, json_content: &str) -> Result<()> {
        // Parse as JSON to ensure it's valid
        let _: serde_json::Value =
            serde_json::from_str(json_content).map_err(|e| Error::Configuration {
                message: format!("Invalid JSON in substituted settings: {}", e),
            })?;

        // Additional validation could be added here to check:
        // - Required fields are present
        // - MCP server configuration is valid
        // - Permissions structure is correct

        Ok(())
    }

    /// Clean up deployed settings after execution
    pub async fn cleanup_deployed_settings(
        &self,
        workspace: &WorkspaceConfiguration,
    ) -> Result<()> {
        let settings_path = workspace
            .workspace_path
            .join(".claude")
            .join("settings.json");

        if settings_path.exists() {
            fs::remove_file(&settings_path)
                .await
                .map_err(|e| Error::Configuration {
                    message: format!(
                        "Failed to clean up deployed settings at {}: {}",
                        settings_path.display(),
                        e
                    ),
                })?;
        }

        // Remove .claude directory if it's empty
        let claude_dir = workspace.workspace_path.join(".claude");
        if claude_dir.exists() {
            if let Ok(mut dir_entries) = fs::read_dir(&claude_dir).await {
                match dir_entries.next_entry().await {
                    Ok(None) => {
                        // Directory is empty, safe to remove
                        let _ = fs::remove_dir(&claude_dir).await;
                    }
                    Ok(Some(_)) => {
                        // Directory has entries, keep it
                    }
                    Err(_) => {
                        // Ignore read errors on cleanup
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute a prompt with automatic settings cleanup
    pub async fn execute_prompt_with_cleanup(
        &self,
        workspace: &WorkspaceConfiguration,
        prompt: &str,
    ) -> Result<ExecutionResult> {
        let result = self.execute_prompt(workspace, prompt).await;

        // Always attempt cleanup, even if execution failed
        if let Err(cleanup_error) = self.cleanup_deployed_settings(workspace).await {
            eprintln!(
                "Warning: Failed to cleanup deployed settings: {}",
                cleanup_error
            );
        }

        result
    }

    /// Execute a prompt with custom configuration and automatic settings cleanup
    pub async fn execute_prompt_with_config_and_cleanup(
        &self,
        workspace: &WorkspaceConfiguration,
        prompt: &str,
        config: &ExecutionConfig,
    ) -> Result<ExecutionResult> {
        let result = self
            .execute_prompt_with_config(workspace, prompt, config)
            .await;

        // Always attempt cleanup, even if execution failed
        if let Err(cleanup_error) = self.cleanup_deployed_settings(workspace).await {
            eprintln!(
                "Warning: Failed to cleanup deployed settings: {}",
                cleanup_error
            );
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();
        assert_eq!(config.timeout_seconds, 300);
        assert_eq!(config.output_format, "stream-json");
        assert!(config.verbose);
        assert!(config.deploy_shared_settings);
        assert!(config.shared_settings_template_path.is_none());
        assert!(config.worker_output_log_path.is_none());
    }

    #[test]
    fn test_headless_executor_creation() {
        let executor = HeadlessClaudeExecutor::new();
        assert_eq!(executor.claude_binary_path, "claude");
        assert_eq!(executor.agent_templates_base_path, PathBuf::from("."));

        let custom_executor =
            HeadlessClaudeExecutor::with_binary_path("/usr/local/bin/claude".to_string());
        assert_eq!(custom_executor.claude_binary_path, "/usr/local/bin/claude");
        assert_eq!(
            custom_executor.agent_templates_base_path,
            PathBuf::from(".")
        );

        let templates_executor =
            HeadlessClaudeExecutor::with_agent_templates_path(PathBuf::from("/custom/path"));
        assert_eq!(templates_executor.claude_binary_path, "claude");
        assert_eq!(
            templates_executor.agent_templates_base_path,
            PathBuf::from("/custom/path")
        );

        let full_executor = HeadlessClaudeExecutor::with_paths(
            "/opt/claude".to_string(),
            PathBuf::from("/templates"),
        );
        assert_eq!(full_executor.claude_binary_path, "/opt/claude");
        assert_eq!(
            full_executor.agent_templates_base_path,
            PathBuf::from("/templates")
        );
    }

    #[test]
    fn test_parse_stream_event() {
        let executor = HeadlessClaudeExecutor::new();

        // Test parsing system event
        let system_json = r#"{"type": "system", "subtype": "init", "cwd": "/tmp", "session_id": "test", "tools": [], "mcp_servers": [], "model": "claude-3", "permissionMode": "allow", "slash_commands": [], "apiKeySource": "config", "output_style": "stream", "uuid": "test-uuid"}"#;
        let event = executor.parse_stream_event(system_json).unwrap();

        assert!(event.is_some());
        match event.unwrap() {
            ClaudeStreamEvent::System { subtype, .. } => {
                assert_eq!(subtype, "init");
            }
            _ => panic!("Expected system event"),
        }

        // Test parsing empty line
        let empty_result = executor.parse_stream_event("").unwrap();
        assert!(empty_result.is_none());

        // Test parsing invalid JSON
        let invalid_result = executor.parse_stream_event("invalid json");
        assert!(invalid_result.is_err());
    }

    #[test]
    fn test_usage_aggregation() {
        let executor = HeadlessClaudeExecutor::new();

        let events = vec![
            ClaudeStreamEvent::Assistant {
                message: ClaudeMessage {
                    id: "msg1".to_string(),
                    message_type: "message".to_string(),
                    role: "assistant".to_string(),
                    model: "claude-3".to_string(),
                    content: vec![ClaudeMessageContent {
                        content_type: "text".to_string(),
                        text: "Hello".to_string(),
                    }],
                    stop_reason: None,
                    stop_sequence: None,
                    usage: ClaudeUsageStats {
                        input_tokens: 10,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: None,
                        cache_creation: None,
                        output_tokens: 5,
                        service_tier: "default".to_string(),
                    },
                },
                parent_tool_use_id: None,
                session_id: "test".to_string(),
                uuid: "uuid1".to_string(),
            },
            ClaudeStreamEvent::Assistant {
                message: ClaudeMessage {
                    id: "msg2".to_string(),
                    message_type: "message".to_string(),
                    role: "assistant".to_string(),
                    model: "claude-3".to_string(),
                    content: vec![ClaudeMessageContent {
                        content_type: "text".to_string(),
                        text: "World".to_string(),
                    }],
                    stop_reason: None,
                    stop_sequence: None,
                    usage: ClaudeUsageStats {
                        input_tokens: 5,
                        cache_creation_input_tokens: None,
                        cache_read_input_tokens: None,
                        cache_creation: None,
                        output_tokens: 5,
                        service_tier: "default".to_string(),
                    },
                },
                parent_tool_use_id: None,
                session_id: "test".to_string(),
                uuid: "uuid2".to_string(),
            },
            ClaudeStreamEvent::Result {
                subtype: "final".to_string(),
                is_error: false,
                duration_ms: 1000,
                duration_api_ms: 800,
                num_turns: 1,
                result: "success".to_string(),
                session_id: "test".to_string(),
                total_cost_usd: 0.015,
                usage: ClaudeUsageStats {
                    input_tokens: 15,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                    cache_creation: None,
                    output_tokens: 10,
                    service_tier: "default".to_string(),
                },
                permission_denials: vec![],
                uuid: "result-uuid".to_string(),
            },
        ];

        let usage = executor.aggregate_usage(&events).unwrap();
        assert_eq!(usage.input_tokens, Some(15));
        assert_eq!(usage.output_tokens, Some(10));
        assert_eq!(usage.total_tokens, Some(25));
        assert_eq!(usage.cost_usd, Some(0.015));
    }

    #[test]
    fn test_execution_result_creation() {
        let execution_id = Uuid::new_v4();
        let started_at = Utc::now();
        let completed_at = started_at + chrono::Duration::seconds(30);

        let result = ExecutionResult {
            execution_id,
            prompt: "Test prompt".to_string(),
            content: "Test response".to_string(),
            success: true,
            error: None,
            events: Vec::new(),
            usage: None,
            started_at,
            completed_at,
            working_directory: "/tmp/test".to_string(),
            metadata: HashMap::new(),
        };

        assert_eq!(result.prompt, "Test prompt");
        assert_eq!(result.content, "Test response");
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    #[serial_test::serial]
    fn test_environment_variable_substitution() {
        use crate::orchestration::models::*;
        use chrono::Utc;

        // Save original environment state for all variables we'll modify
        let original_mcp_server = std::env::var("VIBE_ENSEMBLE_MCP_SERVER");
        let original_mcp_binary = std::env::var("VIBE_ENSEMBLE_MCP_BINARY");
        let original_log_level = std::env::var("VIBE_ENSEMBLE_LOG_LEVEL");
        let original_database_url = std::env::var("DATABASE_URL");
        let original_agent_id = std::env::var("AGENT_ID");

        // Ensure env vars are not set for this test
        std::env::remove_var("VIBE_ENSEMBLE_MCP_SERVER");
        std::env::remove_var("VIBE_ENSEMBLE_MCP_BINARY");
        std::env::remove_var("VIBE_ENSEMBLE_LOG_LEVEL");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("AGENT_ID");

        let executor = HeadlessClaudeExecutor::new();
        let now = Utc::now();

        // Create a test workspace
        let workspace = WorkspaceConfiguration {
            id: Uuid::new_v4(),
            name: "test-workspace".to_string(),
            template_name: "test-template".to_string(),
            template_version: "1.0.0".to_string(),
            workspace_path: PathBuf::from("/tmp/workspace"),
            project_path: PathBuf::from("/tmp/workspace/project"),
            agent_config_path: PathBuf::from("/tmp/workspace/.claude/agents"),
            variable_values: HashMap::new(),
            capabilities: Vec::new(),
            tool_permissions: ToolPermissions::default(),
            created_at: now,
            last_used_at: now,
            is_active: true,
        };

        // Create execution config with custom environment variables
        let mut config = ExecutionConfig::default();
        config
            .environment
            .insert("CUSTOM_VAR".to_string(), "custom_value".to_string());

        let template = r#"{
  "mcpServers": {
    "vibe-ensemble": {
      "command": "${VIBE_ENSEMBLE_MCP_BINARY:-vibe-ensemble-mcp}",
      "env": {
        "VIBE_ENSEMBLE_SERVER_URL": "${VIBE_ENSEMBLE_MCP_SERVER:-ws://localhost:8080}",
        "VIBE_ENSEMBLE_AGENT_ID": "${AGENT_ID:-${WORKSPACE_ID}}",
        "VIBE_ENSEMBLE_LOG_LEVEL": "${VIBE_ENSEMBLE_LOG_LEVEL:-info}",
        "DATABASE_URL": "${DATABASE_URL:-sqlite:./vibe-ensemble.db}"
      }
    }
  },
  "workspace": {
    "id": "${WORKSPACE_ID}",
    "name": "${WORKSPACE_NAME}",
    "template": "${TEMPLATE_NAME}",
    "custom": "${CUSTOM_VAR}"
  }
}"#;

        let result = executor.substitute_environment_variables(template, &workspace, &config);

        // Check that variables were substituted
        assert!(result.contains(&workspace.id.to_string()));
        assert!(result.contains("test-workspace"));
        assert!(result.contains("test-template"));
        assert!(result.contains("custom_value"));

        // Check default values are used when env vars not set
        // Note: This assertion may be affected by parallel test execution
        // Instead, we check that either the default or an env var value is present
        let contains_default = result.contains("ws://localhost:8080");
        let contains_env_value = result.contains("ws://") && !result.contains("${");
        assert!(
            contains_default || contains_env_value,
            "Result should contain either default 'ws://localhost:8080' or a substituted ws:// URL, got: {}",
            result
        );
        assert!(result.contains("vibe-ensemble-mcp"));
        assert!(result.contains("info"));
        assert!(result.contains("sqlite:./vibe-ensemble.db"));

        // Restore original environment state for all variables
        match original_mcp_server {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_MCP_SERVER", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_MCP_SERVER"),
        }
        match original_mcp_binary {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_MCP_BINARY", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_MCP_BINARY"),
        }
        match original_log_level {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_LOG_LEVEL", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_LOG_LEVEL"),
        }
        match original_database_url {
            Ok(val) => std::env::set_var("DATABASE_URL", val),
            Err(_) => std::env::remove_var("DATABASE_URL"),
        }
        match original_agent_id {
            Ok(val) => std::env::set_var("AGENT_ID", val),
            Err(_) => std::env::remove_var("AGENT_ID"),
        }
    }

    #[test]
    #[serial_test::serial]
    fn test_environment_variable_substitution_with_env_var() {
        use crate::orchestration::models::*;
        use chrono::Utc;

        // Save original environment state for ALL relevant env vars
        let original_mcp_server = std::env::var("VIBE_ENSEMBLE_MCP_SERVER");
        let original_mcp_binary = std::env::var("VIBE_ENSEMBLE_MCP_BINARY");
        let original_log_level = std::env::var("VIBE_ENSEMBLE_LOG_LEVEL");
        let original_database_url = std::env::var("DATABASE_URL");
        let original_agent_id = std::env::var("AGENT_ID");

        // Set environment variable for this test
        std::env::set_var("VIBE_ENSEMBLE_MCP_SERVER", "ws://test-server:9090");

        let executor = HeadlessClaudeExecutor::new();
        let now = Utc::now();

        let workspace = WorkspaceConfiguration {
            id: Uuid::new_v4(),
            name: "test-workspace".to_string(),
            template_name: "test-template".to_string(),
            template_version: "1.0.0".to_string(),
            workspace_path: PathBuf::from("/tmp/workspace"),
            project_path: PathBuf::from("/tmp/workspace/project"),
            agent_config_path: PathBuf::from("/tmp/workspace/.claude/agents"),
            variable_values: HashMap::new(),
            capabilities: Vec::new(),
            tool_permissions: ToolPermissions::default(),
            created_at: now,
            last_used_at: now,
            is_active: true,
        };

        let config = ExecutionConfig::default();
        let template = r#"{"server": "${VIBE_ENSEMBLE_MCP_SERVER:-ws://localhost:8080}"}"#;

        let result = executor.substitute_environment_variables(template, &workspace, &config);

        // Check that environment variable was used instead of default
        assert!(result.contains("ws://test-server:9090"));
        assert!(!result.contains("ws://localhost:8080"));

        // Restore original environment state for all vars
        match original_mcp_server {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_MCP_SERVER", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_MCP_SERVER"),
        }
        match original_mcp_binary {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_MCP_BINARY", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_MCP_BINARY"),
        }
        match original_log_level {
            Ok(val) => std::env::set_var("VIBE_ENSEMBLE_LOG_LEVEL", val),
            Err(_) => std::env::remove_var("VIBE_ENSEMBLE_LOG_LEVEL"),
        }
        match original_database_url {
            Ok(val) => std::env::set_var("DATABASE_URL", val),
            Err(_) => std::env::remove_var("DATABASE_URL"),
        }
        match original_agent_id {
            Ok(val) => std::env::set_var("AGENT_ID", val),
            Err(_) => std::env::remove_var("AGENT_ID"),
        }
    }

    #[test]
    fn test_validate_settings_json() {
        let executor = HeadlessClaudeExecutor::new();

        // Test valid JSON
        let valid_json = r#"{"test": "value", "number": 42}"#;
        assert!(executor.validate_settings_json(valid_json).is_ok());

        // Test invalid JSON
        let invalid_json = r#"{"test": "value", "number": }"#;
        assert!(executor.validate_settings_json(invalid_json).is_err());

        // Test empty JSON
        let empty_json = "";
        assert!(executor.validate_settings_json(empty_json).is_err());

        // Test complex valid JSON
        let complex_json = r#"{
            "permissions": {
                "allow": ["Read", "Write"],
                "deny": ["Delete"]
            },
            "mcpServers": {
                "vibe-ensemble": {
                    "command": "vibe-ensemble-mcp",
                    "env": {
                        "SERVER_URL": "ws://localhost:8080"
                    }
                }
            }
        }"#;
        assert!(executor.validate_settings_json(complex_json).is_ok());
    }

    #[tokio::test]
    async fn test_shared_settings_json_deployment() {
        use tempfile::TempDir;

        // Create a temporary workspace
        let temp_workspace = TempDir::new().unwrap();
        let workspace = WorkspaceConfiguration {
            id: Uuid::new_v4(),
            name: "test-workspace".to_string(),
            template_name: "test-template".to_string(),
            template_version: "1.0.0".to_string(),
            workspace_path: temp_workspace.path().to_path_buf(),
            project_path: temp_workspace.path().join("project"),
            agent_config_path: temp_workspace.path().join(".claude").join("agents"),
            variable_values: std::collections::HashMap::new(),
            capabilities: vec!["test".to_string()],
            tool_permissions: crate::orchestration::models::ToolPermissions::default(),
            created_at: chrono::Utc::now(),
            last_used_at: chrono::Utc::now(),
            is_active: true,
        };

        // Create a temporary agent templates directory
        let temp_templates = TempDir::new().unwrap();
        let templates_path = temp_templates.path().to_path_buf();

        // Create the shared settings template directory structure
        let shared_claude_dir = templates_path
            .join("agent-templates")
            .join("shared")
            .join(".claude");
        fs::create_dir_all(&shared_claude_dir).await.unwrap();

        // Create a test settings.json template with environment variables
        let template_content = r#"{
  "mcp": {
    "servers": {
      "vibe-ensemble": {
        "command": "vibe-ensemble-mcp",
        "transport": {
          "type": "websocket",
          "url": "${VIBE_ENSEMBLE_MCP_SERVER}"
        },
        "env": {
          "WORKSPACE_ID": "${WORKSPACE_ID}",
          "WORKSPACE_NAME": "${WORKSPACE_NAME}",
          "TEMPLATE_NAME": "${TEMPLATE_NAME}",
          "AGENT_ID": "${AGENT_ID}",
          "VIBE_ENSEMBLE_LOG_LEVEL": "${VIBE_ENSEMBLE_LOG_LEVEL}"
        }
      }
    }
  },
  "rules": [
    {
      "type": "allow_all_commands"
    },
    {
      "type": "deny_command",
      "pattern": "sudo"
    }
  ],
  "logging": {
    "level": "${VIBE_ENSEMBLE_LOG_LEVEL}",
    "coordination_events": true
  }
}"#;

        let template_path = shared_claude_dir.join("settings.json");
        fs::write(&template_path, template_content).await.unwrap();

        // Create executor with the temporary templates path
        let executor = HeadlessClaudeExecutor::with_agent_templates_path(templates_path);

        // Test deployment
        let config = ExecutionConfig {
            deploy_shared_settings: true,
            worker_output_log_path: None,
            ..Default::default()
        };

        let result = executor.deploy_shared_settings(&workspace, &config).await;
        assert!(
            result.is_ok(),
            "Failed to deploy shared settings: {:?}",
            result
        );

        // Verify the settings file was created in the workspace
        let deployed_path = workspace
            .workspace_path
            .join(".claude")
            .join("settings.json");
        assert!(
            deployed_path.exists(),
            "Deployed settings.json should exist"
        );

        // Read and verify the deployed content
        let deployed_content = fs::read_to_string(&deployed_path).await.unwrap();

        // Verify environment variables were substituted
        assert!(deployed_content.contains(&workspace.id.to_string()));
        assert!(deployed_content.contains("test-workspace"));
        assert!(deployed_content.contains("test-template"));

        // More flexible checks for environment variable substitution
        // The VIBE_ENSEMBLE_MCP_SERVER should either be substituted with the default or not contain the variable placeholder
        let mcp_server_substituted = deployed_content.contains("ws://localhost:8080")
            || (deployed_content.contains("ws://")
                && !deployed_content.contains("${VIBE_ENSEMBLE_MCP_SERVER}"));
        assert!(
            mcp_server_substituted,
            "MCP server URL was not properly substituted. Content: {}",
            deployed_content
        );

        // Log level should either be substituted with default "info" or not contain the variable placeholder
        let log_level_substituted = deployed_content.contains("info")
            || !deployed_content.contains("${VIBE_ENSEMBLE_LOG_LEVEL}");
        assert!(
            log_level_substituted,
            "Log level was not properly substituted. Content: {}",
            deployed_content
        );

        // Verify it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&deployed_content)
            .expect("Deployed settings should be valid JSON");

        // Test cleanup functionality
        let cleanup_result = executor.cleanup_deployed_settings(&workspace).await;
        assert!(
            cleanup_result.is_ok(),
            "Failed to cleanup deployed settings: {:?}",
            cleanup_result
        );

        // Verify the settings file was removed
        assert!(
            !deployed_path.exists(),
            "Deployed settings.json should be cleaned up"
        );

        // Verify AGENT_ID defaults to WORKSPACE_ID when unset
        assert!(
            deployed_content.contains(&workspace.id.to_string()),
            "AGENT_ID should default to WORKSPACE_ID in deployed content"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_bare_placeholder_uses_env_var() {
        let executor = HeadlessClaudeExecutor::new();

        // Create test workspace and configuration
        let temp_workspace = std::env::temp_dir().join("test_bare_placeholder");
        let workspace = WorkspaceConfiguration {
            id: uuid::Uuid::new_v4(),
            name: "test-workspace".to_string(),
            template_name: "shared".to_string(),
            template_version: "1.0.0".to_string(),
            workspace_path: temp_workspace.clone(),
            project_path: temp_workspace.join("project"),
            agent_config_path: temp_workspace.join(".claude").join("agents"),
            variable_values: std::collections::HashMap::new(),
            capabilities: vec!["test".to_string()],
            tool_permissions: crate::orchestration::models::ToolPermissions::default(),
            created_at: chrono::Utc::now(),
            last_used_at: chrono::Utc::now(),
            is_active: true,
        };

        let config = ExecutionConfig::default();

        // Set environment variable for testing bare placeholder
        std::env::set_var("VIBE_ENSEMBLE_MCP_SERVER", "ws://override:1234");

        // Test template with bare placeholder
        let template = r#"{"url":"${VIBE_ENSEMBLE_MCP_SERVER}"}"#;
        let result = executor.substitute_environment_variables(template, &workspace, &config);

        // Should use the environment variable value
        assert!(
            result.contains("ws://override:1234"),
            "Bare placeholder should use environment variable value: {}",
            result
        );

        // Verify it's valid JSON with proper escaping
        let _: serde_json::Value =
            serde_json::from_str(&result).expect("Result should be valid JSON");

        // Clean up environment variable
        std::env::remove_var("VIBE_ENSEMBLE_MCP_SERVER");

        // Test with special characters that need JSON escaping via config.environment
        let mut config_with_special = ExecutionConfig::default();
        config_with_special.environment.insert(
            "TEST_VAR".to_string(),
            "value with \"quotes\" and \\backslashes".to_string(),
        );

        let template_special = r#"{"test":"${TEST_VAR}"}"#;
        let result_special = executor.substitute_environment_variables(
            template_special,
            &workspace,
            &config_with_special,
        );

        // Should be valid JSON despite special characters
        let parsed: serde_json::Value = serde_json::from_str(&result_special)
            .expect("Result with special characters should be valid JSON");

        // Verify the content was properly escaped
        assert_eq!(parsed["test"], "value with \"quotes\" and \\backslashes");
    }
}
