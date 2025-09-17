use async_trait::async_trait;
use serde_json::{json, Value};

use super::tools::{create_success_response, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{error::Result, server::AppState};

pub struct GetPermissionModelTool;

#[async_trait]
impl ToolHandler for GetPermissionModelTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        let permission_mode = &state.config.permission_mode;

        let (config_file, description, example_path) = match permission_mode.as_str() {
            "bypass" => (
                "None (bypass mode)",
                "Workers run with --dangerously-skip-permissions flag. No permission restrictions are enforced. This mode provides unrestricted access to all tools and system capabilities.",
                "N/A - No configuration file used"
            ),
            "inherit" => (
                ".claude/settings.local.json",
                "Workers inherit permissions from the project's Claude Code settings. This mode uses the same permissions as your interactive coordinator session.",
                ".claude/settings.local.json (in project root)"
            ),
            "file" => (
                ".vibe-ensemble-mcp/worker-permissions.json",
                "Workers use custom permissions from a dedicated worker configuration file. This allows fine-grained control over what tools workers can access.",
                ".vibe-ensemble-mcp/worker-permissions.json (in project root)"
            ),
            _ => (
                "Unknown mode",
                "Invalid permission mode configured",
                "N/A"
            )
        };

        let permission_structure = json!({
            "permissions": {
                "allow": ["Read", "Write", "Edit", "MultiEdit", "Bash", "mcp__*"],
                "deny": ["WebFetch", "WebSearch"],
                "ask": [],
                "additionalDirectories": ["./temp", "./build"],
                "defaultMode": "acceptEdits"
            }
        });

        let response = json!({
            "permission_mode": permission_mode,
            "config_file": config_file,
            "description": description,
            "example_config_path": example_path,
            "permission_structure_format": permission_structure,
            "common_tools": {
                "file_operations": ["Read", "Write", "Edit", "MultiEdit", "Glob", "Grep"],
                "system_commands": ["Bash"],
                "mcp_tools": ["mcp__*"],
                "web_access": ["WebFetch", "WebSearch"],
                "version_control": ["git*"]
            },
            "permission_fields_explanation": {
                "allow": "Array of tools that workers can use without restriction",
                "deny": "Array of tools that workers are prohibited from using",
                "ask": "Array of tools that require user confirmation (ignored in headless worker mode)",
                "additionalDirectories": "Additional directories workers can access beyond the project directory",
                "defaultMode": "Default permission behavior (acceptEdits or rejectEdits)"
            },
            "usage_guidance": {
                "when_worker_reports_permission_issue": [
                    "1. Identify the specific tool the worker needs from their CoordinatorAttention outcome",
                    "2. Determine if the tool should be allowed for this project type",
                    "3. Update the appropriate configuration file based on the permission_mode",
                    "4. Add the tool to the 'allow' array in the configuration file",
                    "5. Workers will pick up new permissions when they restart (no server restart needed)"
                ],
                "security_considerations": [
                    "Start with minimal permissions and add tools as needed",
                    "Use 'inherit' mode for most production cases",
                    "Only use 'bypass' mode in isolated development environments",
                    "Review worker activity logs in .vibe-ensemble-mcp/logs/ to understand tool usage"
                ]
            }
        });

        Ok(create_success_response(&serde_json::to_string_pretty(
            &response,
        )?))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_permission_model".to_string(),
            description: "Get information about the permission model in use and which files control worker access. This tool helps coordinators understand how to manage permissions when workers encounter access restrictions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}
