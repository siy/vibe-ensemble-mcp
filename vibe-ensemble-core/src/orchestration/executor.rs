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
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
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
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            timeout_seconds: 300, // 5 minutes
            verbose: true,
            environment: HashMap::new(),
            working_directory: None,
            output_format: "stream-json".to_string(),
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
        }
    }

    /// Create a new executor with custom binary path
    pub fn with_binary_path(binary_path: String) -> Self {
        Self {
            claude_binary_path: binary_path,
            default_config: ExecutionConfig::default(),
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

        // Execute command
        let mut child = cmd.spawn().map_err(|e| Error::Execution {
            message: format!("Failed to spawn Claude Code process: {}", e),
        })?;

        let stdout = child.stdout.take().ok_or_else(|| Error::Execution {
            message: "Failed to capture stdout".to_string(),
        })?;

        let stderr = child.stderr.take().ok_or_else(|| Error::Execution {
            message: "Failed to capture stderr".to_string(),
        })?;

        // Setup timeout
        let timeout = tokio::time::timeout(
            std::time::Duration::from_secs(config.timeout_seconds),
            self.process_stream(stdout, stderr),
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
    ) -> Result<(Vec<ClaudeStreamEvent>, bool, String, Option<String>)> {
        let mut events = Vec::new();
        let mut success = true;
        let mut final_content = String::new();
        let mut error_message = None;

        let stdout_reader = BufReader::new(stdout);
        let mut stdout_lines = stdout_reader.lines();

        let stderr_reader = BufReader::new(stderr);
        let mut stderr_lines = stderr_reader.lines();

        // Read from both stdout and stderr concurrently
        loop {
            tokio::select! {
                stdout_line = stdout_lines.next_line() => {
                    match stdout_line {
                        Ok(Some(line)) => {
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
                            // Log stderr but don't fail on it
                            eprintln!("Claude Code stderr: {}", line);
                            if error_message.is_none() && !line.trim().is_empty() {
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
    }

    #[test]
    fn test_headless_executor_creation() {
        let executor = HeadlessClaudeExecutor::new();
        assert_eq!(executor.claude_binary_path, "claude");

        let custom_executor =
            HeadlessClaudeExecutor::with_binary_path("/usr/local/bin/claude".to_string());
        assert_eq!(custom_executor.claude_binary_path, "/usr/local/bin/claude");
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
}
