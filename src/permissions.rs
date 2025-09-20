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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionMode {
    /// No restrictions - workers run with --dangerously-skip-permissions
    Bypass,
    /// Use permissions from .claude/settings.local.json
    Inherit,
    /// Use permissions from .vibe-ensemble-mcp/worker-permissions.json
    File,
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
    let permissions_path =
        Path::new(project_path).join(".vibe-ensemble-mcp/worker-permissions.json");

    if !permissions_path.exists() {
        return Ok(ClaudePermissions::default());
    }

    let content = fs::read_to_string(&permissions_path).with_context(|| {
        format!(
            "Failed to read permissions file: {}",
            permissions_path.display()
        )
    })?;

    let settings: ClaudeSettings = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse worker permissions from {}",
            permissions_path.display()
        )
    })?;

    Ok(settings.permissions)
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

/// Load permissions based on the permission mode (deprecated - use load_permission_policy)
pub fn load_permissions(
    mode: PermissionMode,
    project_path: &str,
) -> Result<Option<ClaudePermissions>> {
    let policy = load_permission_policy(mode, project_path)?;
    Ok(policy.permissions().cloned())
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
