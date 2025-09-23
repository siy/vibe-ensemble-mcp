use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// Default permission mode for workers
pub const DEFAULT_PERMISSION_MODE: &str = "acceptEdits";

/// Common allow-list for minimal starting permissions
pub const MINIMAL_ALLOW_LIST: &[&str] = &["mcp__*"];

/// Balanced allow-list for more permissive setups
pub const BALANCED_ALLOW_LIST: &[&str] = &["Read", "Write", "Edit", "MultiEdit", "Bash", "mcp__*"];

/// Default deny-list for security
pub const COMMON_DENY_LIST: &[&str] = &["WebFetch", "WebSearch"];

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
    DEFAULT_PERMISSION_MODE.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeSettings {
    #[serde(default)]
    pub permissions: ClaudePermissions,
    // Additional fields that might be present in worker-permissions.json
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "enableAllProjectMcpServers"
    )]
    pub enable_all_project_mcp_servers: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        rename = "enabledMcpjsonServers"
    )]
    pub enabled_mcpjson_servers: Vec<String>,
}

impl Default for ClaudePermissions {
    fn default() -> Self {
        Self {
            allow: MINIMAL_ALLOW_LIST.iter().map(|s| s.to_string()).collect(),
            deny: COMMON_DENY_LIST.iter().map(|s| s.to_string()).collect(),
            ask: vec![],
            additional_directories: vec![],
            default_mode: default_mode(),
        }
    }
}

impl ClaudePermissions {
    /// Create minimal permissions for workers
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create balanced permissions for development
    pub fn balanced() -> Self {
        Self {
            allow: BALANCED_ALLOW_LIST.iter().map(|s| s.to_string()).collect(),
            deny: COMMON_DENY_LIST.iter().map(|s| s.to_string()).collect(),
            ask: vec![],
            additional_directories: vec![],
            default_mode: default_mode(),
        }
    }
}

/// Permission modes supported by the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
pub enum PermissionMode {
    /// No restrictions - workers run with --dangerously-skip-permissions
    Bypass,
    /// Use permissions from .claude/settings.local.json
    Inherit,
    /// Use permissions from .vibe-ensemble-mcp/worker-permissions.json
    File,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Bypass => write!(f, "bypass"),
            PermissionMode::Inherit => write!(f, "inherit"),
            PermissionMode::File => write!(f, "file"),
        }
    }
}

impl FromStr for PermissionMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "bypass" => Ok(PermissionMode::Bypass),
            "inherit" => Ok(PermissionMode::Inherit),
            "file" => Ok(PermissionMode::File),
            _ => Err(anyhow::anyhow!(
                "Invalid permission mode '{}'. Valid options: bypass, inherit, file",
                s
            )),
        }
    }
}

impl PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PermissionMode::Bypass => "bypass",
            PermissionMode::Inherit => "inherit",
            PermissionMode::File => "file",
        }
    }
}

/// Load permissions from .claude/settings.local.json
pub fn load_inherit_permissions(project_path: &str) -> Result<ClaudePermissions> {
    let settings_path = Path::new(project_path).join(".claude/settings.local.json");

    if !settings_path.exists() {
        return Ok(ClaudePermissions::default());
    }

    let content = fs::read_to_string(&settings_path)
        .with_context(|| format!("Failed to read settings file: {}", settings_path.display()))?;

    let settings: ClaudeSettings = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse Claude settings from {}",
            settings_path.display()
        )
    })?;

    Ok(settings.permissions)
}

/// Load permissions from .vibe-ensemble-mcp/worker-permissions.json
pub fn load_file_permissions(project_path: &str) -> Result<ClaudePermissions> {
    use tracing::{debug, info, warn};

    let permissions_path =
        Path::new(project_path).join(".vibe-ensemble-mcp/worker-permissions.json");

    if !permissions_path.exists() {
        debug!(
            "Worker permissions file does not exist, using defaults: {}",
            permissions_path.display()
        );
        return Ok(ClaudePermissions::default());
    }

    let content = fs::read_to_string(&permissions_path).with_context(|| {
        format!(
            "Failed to read permissions file: {}",
            permissions_path.display()
        )
    })?;

    debug!(
        "Read worker permissions file: {} ({} bytes)",
        permissions_path.display(),
        content.len()
    );

    // Try to parse as the expected structure first
    match serde_json::from_str::<ClaudeSettings>(&content) {
        Ok(settings) => {
            info!(
                "Successfully parsed worker permissions from {}: {} allowed, {} denied tools",
                permissions_path.display(),
                settings.permissions.allow.len(),
                settings.permissions.deny.len()
            );
            debug!("Allowed tools: {:?}", settings.permissions.allow);
            Ok(settings.permissions)
        }
        Err(e) => {
            warn!(
                "Failed to parse as ClaudeSettings, trying direct permissions parsing: {}",
                e
            );

            // Try to parse just the permissions section directly
            let json_value: serde_json::Value =
                serde_json::from_str(&content).with_context(|| {
                    format!("Failed to parse JSON from {}", permissions_path.display())
                })?;

            if let Some(permissions_obj) = json_value.get("permissions") {
                match serde_json::from_value::<ClaudePermissions>(permissions_obj.clone()) {
                    Ok(permissions) => {
                        info!("Successfully extracted permissions section: {} allowed, {} denied tools",
                              permissions.allow.len(),
                              permissions.deny.len());
                        debug!("Allowed tools: {:?}", permissions.allow);
                        Ok(permissions)
                    }
                    Err(e2) => {
                        warn!("Failed to parse permissions section: {}", e2);
                        debug!("Permissions JSON value: {:?}", permissions_obj);
                        Err(anyhow::anyhow!(
                            "Failed to parse permissions from {}: {}",
                            permissions_path.display(),
                            e2
                        ))
                    }
                }
            } else {
                Err(anyhow::anyhow!(
                    "No 'permissions' section found in {}",
                    permissions_path.display()
                ))
            }
        }
    }
}

/// Permission policy that clarifies intent at call sites
#[derive(Debug, Clone)]
pub enum PermissionPolicy {
    /// Bypass all permissions - no restrictions
    Bypass,
    /// Apply specific permission rules
    Enforce(ClaudePermissions),
}

impl PermissionPolicy {
    /// Check if this policy bypasses all permissions
    pub fn is_bypass(&self) -> bool {
        matches!(self, PermissionPolicy::Bypass)
    }

    /// Get permissions if any are enforced, None if bypass mode
    pub fn permissions(&self) -> Option<&ClaudePermissions> {
        match self {
            PermissionPolicy::Bypass => None,
            PermissionPolicy::Enforce(perms) => Some(perms),
        }
    }
}

/// Load permission policy based on the permission mode
pub fn load_permission_policy(
    mode: PermissionMode,
    project_path: &str,
) -> Result<PermissionPolicy> {
    match mode {
        PermissionMode::Bypass => Ok(PermissionPolicy::Bypass),
        PermissionMode::Inherit => Ok(PermissionPolicy::Enforce(load_inherit_permissions(
            project_path,
        )?)),
        PermissionMode::File => Ok(PermissionPolicy::Enforce(load_file_permissions(
            project_path,
        )?)),
    }
}

/// Create project-specific worker permissions file if it doesn't exist
pub fn create_project_permissions(project_path: &str) -> Result<()> {
    use tracing::{debug, info};

    let vibe_dir = std::path::Path::new(project_path).join(".vibe-ensemble-mcp");
    let permissions_file = vibe_dir.join("worker-permissions.json");

    // Only create if doesn't exist (preserve existing)
    if permissions_file.exists() {
        debug!(
            "Worker permissions file already exists at: {}",
            permissions_file.display()
        );
        return Ok(());
    }

    // Create .vibe-ensemble-mcp directory if it doesn't exist
    if !vibe_dir.exists() {
        debug!(
            "Creating .vibe-ensemble-mcp directory: {}",
            vibe_dir.display()
        );
        fs::create_dir_all(&vibe_dir).with_context(|| {
            format!(
                "Failed to create .vibe-ensemble-mcp directory: {}",
                vibe_dir.display()
            )
        })?;
    }

    // Create default permissions with comprehensive MCP tool access
    let default_permissions = ClaudeSettings {
        permissions: ClaudePermissions {
            allow: vec![
                // All vibe-ensemble-mcp tools
                "mcp__vibe-ensemble-mcp__create_project".to_string(),
                "mcp__vibe-ensemble-mcp__list_projects".to_string(),
                "mcp__vibe-ensemble-mcp__get_project".to_string(),
                "mcp__vibe-ensemble-mcp__update_project".to_string(),
                "mcp__vibe-ensemble-mcp__delete_project".to_string(),
                "mcp__vibe-ensemble-mcp__create_worker_type".to_string(),
                "mcp__vibe-ensemble-mcp__list_worker_types".to_string(),
                "mcp__vibe-ensemble-mcp__get_worker_type".to_string(),
                "mcp__vibe-ensemble-mcp__update_worker_type".to_string(),
                "mcp__vibe-ensemble-mcp__delete_worker_type".to_string(),
                "mcp__vibe-ensemble-mcp__create_ticket".to_string(),
                "mcp__vibe-ensemble-mcp__get_ticket".to_string(),
                "mcp__vibe-ensemble-mcp__list_tickets".to_string(),
                "mcp__vibe-ensemble-mcp__add_ticket_comment".to_string(),
                "mcp__vibe-ensemble-mcp__close_ticket".to_string(),
                "mcp__vibe-ensemble-mcp__resume_ticket_processing".to_string(),
                "mcp__vibe-ensemble-mcp__add_ticket_dependency".to_string(),
                "mcp__vibe-ensemble-mcp__remove_ticket_dependency".to_string(),
                "mcp__vibe-ensemble-mcp__get_dependency_graph".to_string(),
                "mcp__vibe-ensemble-mcp__list_ready_tickets".to_string(),
                "mcp__vibe-ensemble-mcp__list_blocked_tickets".to_string(),
                "mcp__vibe-ensemble-mcp__list_events".to_string(),
                "mcp__vibe-ensemble-mcp__resolve_event".to_string(),
                "mcp__vibe-ensemble-mcp__get_tickets_by_stage".to_string(),
                "mcp__vibe-ensemble-mcp__get_permission_model".to_string(),
                "mcp__vibe-ensemble-mcp__list_client_tools".to_string(),
                "mcp__vibe-ensemble-mcp__call_client_tool".to_string(),
                "mcp__vibe-ensemble-mcp__list_connected_clients".to_string(),
                "mcp__vibe-ensemble-mcp__list_pending_requests".to_string(),
                "mcp__vibe-ensemble-mcp__execute_workflow".to_string(),
                "mcp__vibe-ensemble-mcp__parallel_call".to_string(),
                "mcp__vibe-ensemble-mcp__broadcast_to_clients".to_string(),
                "mcp__vibe-ensemble-mcp__collaborative_sync".to_string(),
                "mcp__vibe-ensemble-mcp__poll_client_status".to_string(),
                "mcp__vibe-ensemble-mcp__client_group_manager".to_string(),
                "mcp__vibe-ensemble-mcp__client_health_monitor".to_string(),
                "mcp__vibe-ensemble-mcp__validate_websocket_integration".to_string(),
                "mcp__vibe-ensemble-mcp__test_websocket_compatibility".to_string(),
                // Template management tools
                "mcp__vibe-ensemble-mcp__list_worker_templates".to_string(),
                "mcp__vibe-ensemble-mcp__load_worker_template".to_string(),
                "mcp__vibe-ensemble-mcp__ensure_worker_templates_exist".to_string(),
                // Essential Claude Code tools
                "TodoWrite".to_string(),
                "Bash".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
                "MultiEdit".to_string(),
                "Glob".to_string(),
                "Grep".to_string(),
            ],
            deny: vec!["WebFetch".to_string(), "WebSearch".to_string()],
            ask: vec![],
            additional_directories: vec![],
            default_mode: default_mode(),
        },
        enable_all_project_mcp_servers: Some(true),
        enabled_mcpjson_servers: vec!["vibe-ensemble-mcp".to_string()],
    };

    // Serialize to pretty JSON
    let permissions_content = serde_json::to_string_pretty(&default_permissions)
        .with_context(|| "Failed to serialize default permissions to JSON")?;

    // Write to file
    fs::write(&permissions_file, permissions_content).with_context(|| {
        format!(
            "Failed to write permissions file: {}",
            permissions_file.display()
        )
    })?;

    info!(
        "Created worker permissions file: {}",
        permissions_file.display()
    );
    debug!(
        "Generated permissions with {} allowed tools, {} denied tools",
        default_permissions.permissions.allow.len(),
        default_permissions.permissions.deny.len()
    );

    Ok(())
}
