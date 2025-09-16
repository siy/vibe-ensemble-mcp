use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use tracing::{debug, info};

use super::tools::{
    create_error_response, create_success_response, extract_optional_param, extract_param,
    ToolHandler,
};
use super::types::{CallToolResponse, Tool};
use crate::{
    database::projects::{CreateProjectRequest, Project, UpdateProjectRequest},
    error::Result,
    server::AppState,
};

pub struct CreateProjectTool;

#[async_trait]
impl ToolHandler for CreateProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;
        let path: String = extract_param(&arguments, "path")?;
        let short_description: Option<String> = extract_optional_param(&arguments, "description")?;
        let project_rules: Option<String> = extract_optional_param(&arguments, "project_rules")?;
        let project_patterns: Option<String> =
            extract_optional_param(&arguments, "project_patterns")?;

        // Create the project directory if it doesn't exist
        debug!("Checking if project directory exists: {}", path);
        if !std::path::Path::new(&path).exists() {
            info!("Creating project directory: {}", path);
            if let Err(e) = fs::create_dir_all(&path) {
                return Ok(create_error_response(&format!(
                    "Failed to create project directory '{}': {}",
                    path, e
                )));
            }
            info!("✓ Successfully created project directory: {}", path);
        } else {
            debug!("Project directory already exists: {}", path);
        }

        let request = CreateProjectRequest {
            repository_name: repository_name.clone(),
            path,
            short_description,
            project_rules,
            project_patterns,
        };

        match Project::create(&state.db, request).await {
            Ok(project) => {
                let response = json!({
                    "repository_name": project.repository_name,
                    "path": project.path,
                    "description": project.short_description,
                    "created_at": project.created_at
                });
                
                // Broadcast project_created event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": "vibe-ensemble://projects",
                        "event": {
                            "type": "project_created",
                            "project": {
                                "repository_name": project.repository_name,
                                "path": project.path,
                                "description": project.short_description,
                                "created_at": project.created_at
                            },
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast project_created event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast project_created event for: {}", project.repository_name);
                }
                
                Ok(create_success_response(&format!(
                    "Project created successfully: {}",
                    response
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to create project: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_project".to_string(),
            description: "Create a new project with repository name and path".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    },
                    "path": {
                        "type": "string",
                        "description": "Local path to the project directory"
                    },
                    "description": {
                        "type": "string",
                        "description": "Optional short description of the project"
                    }
                },
                "required": ["repository_name", "path"]
            }),
        }
    }
}

pub struct ListProjectsTool;

#[async_trait]
impl ToolHandler for ListProjectsTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        match Project::list_all(&state.db).await {
            Ok(projects) => {
                let projects_json = serde_json::to_string_pretty(&projects)?;
                Ok(create_success_response(&format!(
                    "Projects:\n{}",
                    projects_json
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to list projects: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_projects".to_string(),
            description: "List all projects".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
        }
    }
}

pub struct GetProjectTool;

#[async_trait]
impl ToolHandler for GetProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;

        match Project::get_by_name(&state.db, &repository_name).await {
            Ok(Some(project)) => {
                let project_json = serde_json::to_string_pretty(&project)?;
                Ok(create_success_response(&format!(
                    "Project:\n{}",
                    project_json
                )))
            }
            Ok(None) => Ok(create_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to get project: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_project".to_string(),
            description: "Get project details by repository name".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}

pub struct UpdateProjectTool;

#[async_trait]
impl ToolHandler for UpdateProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;
        let path: Option<String> = extract_optional_param(&arguments, "path")?;
        let short_description: Option<String> = extract_optional_param(&arguments, "description")?;
        let project_rules: Option<String> = extract_optional_param(&arguments, "project_rules")?;
        let project_patterns: Option<String> =
            extract_optional_param(&arguments, "project_patterns")?;

        let request = UpdateProjectRequest {
            path,
            short_description,
            project_rules,
            project_patterns,
        };

        match Project::update(&state.db, &repository_name, request).await {
            Ok(Some(project)) => {
                let project_json = serde_json::to_string_pretty(&project)?;
                Ok(create_success_response(&format!(
                    "Project updated:\n{}",
                    project_json
                )))
            }
            Ok(None) => Ok(create_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to update project: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "update_project".to_string(),
            description: "Update project details".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    },
                    "path": {
                        "type": "string",
                        "description": "New path to the project directory"
                    },
                    "description": {
                        "type": "string",
                        "description": "New short description of the project"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}

pub struct DeleteProjectTool;

#[async_trait]
impl ToolHandler for DeleteProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;

        match Project::delete(&state.db, &repository_name).await {
            Ok(true) => Ok(create_success_response(&format!(
                "Project '{}' deleted successfully",
                repository_name
            ))),
            Ok(false) => Ok(create_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to delete project: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_project".to_string(),
            description: "Delete a project by repository name".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "repository_name": {
                        "type": "string",
                        "description": "Repository name in org/repo format"
                    }
                },
                "required": ["repository_name"]
            }),
        }
    }
}
