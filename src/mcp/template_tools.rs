use async_trait::async_trait;
use serde_json::{json, Value};

use super::tools::{create_json_error_response, create_json_success_response, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{configure, error::Result, server::AppState};

pub struct ListWorkerTemplatesOol;

#[async_trait]
impl ToolHandler for ListWorkerTemplatesOol {
    async fn call(&self, _state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        let templates = configure::list_worker_templates();

        let response = json!({
            "templates": templates,
            "location": ".claude/worker-templates/",
            "usage": "Use load_worker_template to get the content of a specific template"
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_worker_templates".to_string(),
            description: "List all available worker templates that can be used as system prompts".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

pub struct LoadWorkerTemplateTool;

#[async_trait]
impl ToolHandler for LoadWorkerTemplateTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let template_name: String = arguments
            .as_ref()
            .and_then(|args| args.get("template_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: template_name"))?
            .to_string();

        // Get stored working directory (use simple key for now)
        let working_directory = state.coordinator_directories.get("coordinator").map(|entry| entry.value().clone());

        // Ensure templates exist on disk first
        if let Err(e) = configure::ensure_worker_templates_exist_in_directory(working_directory.as_deref()) {
            return Ok(create_json_error_response(&format!(
                "Failed to ensure templates exist: {}",
                e
            )));
        }

        match configure::load_worker_template_from_directory(&template_name, working_directory.as_deref()) {
            Ok(content) => {
                let base_dir = working_directory.as_deref().unwrap_or(".");
                let template_path = format!("{}/.claude/worker-templates/{}.md", base_dir, template_name);
                let response = json!({
                    "template_name": template_name,
                    "content": content,
                    "source": if std::path::Path::new(&template_path).exists() {
                        "disk"
                    } else {
                        "embedded"
                    }
                });
                Ok(create_json_success_response(response))
            }
            Err(e) => Ok(create_json_error_response(&format!(
                "Failed to load template '{}': {}",
                template_name, e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "load_worker_template".to_string(),
            description: "Load a worker template from disk (with fallback to embedded version). Use the content as system_prompt when creating worker types.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "template_name": {
                        "type": "string",
                        "description": "Name of the template to load (e.g., 'planning', 'implementation', 'testing')",
                        "enum": ["planning", "design", "implementation", "testing", "review", "deployment", "research", "documentation"]
                    }
                },
                "required": ["template_name"]
            }),
        }
    }
}

pub struct EnsureWorkerTemplatesExistTool;

#[async_trait]
impl ToolHandler for EnsureWorkerTemplatesExistTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let working_directory: Option<String> = arguments
            .as_ref()
            .and_then(|args| args.get("working_directory"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Store the working directory if provided (associate with client_id from WebSocket context)
        if let Some(working_dir) = &working_directory {
            // For now, use a simple key. In a real implementation, you'd get the client_id from WebSocket context
            let client_key = "coordinator"; // TODO: Get actual client_id from WebSocket context
            state.coordinator_directories.insert(client_key.to_string(), working_dir.clone());
        }

        match configure::ensure_worker_templates_exist_in_directory(working_directory.as_deref()) {
            Ok(()) => {
                let response = json!({
                    "message": "All worker templates verified and created if missing",
                    "location": ".claude/worker-templates/"
                });
                Ok(create_json_success_response(response))
            }
            Err(e) => Ok(create_json_error_response(&format!(
                "Failed to ensure worker templates exist: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "ensure_worker_templates_exist".to_string(),
            description: "Ensure all worker templates exist on disk, creating any missing ones. Optionally specify working directory.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "working_directory": {
                        "type": "string",
                        "description": "Working directory where .claude/worker-templates/ should be located (optional, defaults to current directory)"
                    }
                },
                "required": []
            }),
        }
    }
}