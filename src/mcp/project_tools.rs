use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::process::Command;
use tracing::{debug, info, warn};

use super::pagination::extract_cursor;
use super::tools::{
    create_json_error_response, create_json_success_response, extract_optional_param,
    extract_param, ToolHandler,
};
use super::types::{CallToolResponse, Tool};
use crate::{
    database::projects::{CreateProjectRequest, Project, UpdateProjectRequest},
    error::Result,
    permissions::create_project_permissions,
    server::AppState,
};

/// Initialize git repository and validate branch status
fn initialize_git_repository(project_path: &str) -> Result<String> {
    let path = std::path::Path::new(project_path);

    // Check if already a git repository
    let git_dir = path.join(".git");
    if git_dir.exists() {
        debug!("Git repository already exists at: {}", project_path);

        // Check current branch
        let output = Command::new("git")
            .current_dir(project_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .map_err(|e| {
                crate::error::AppError::BadRequest(format!("Failed to check git branch: {}", e))
            })?;

        if !output.status.success() {
            return Err(crate::error::AppError::BadRequest(
                "Failed to determine current git branch".to_string(),
            ));
        }

        let current_branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        debug!("Current git branch: {}", current_branch);

        // Validate branch is main or develop
        if current_branch != "main" && current_branch != "develop" {
            return Err(crate::error::AppError::BadRequest(format!(
                "Project is on branch '{}' but should be on 'main' or 'develop'. \
                Please coordinate with user to determine correct base branch before proceeding.",
                current_branch
            )));
        }

        return Ok(format!(
            "Using existing git repository on branch '{}'",
            current_branch
        ));
    }

    // Initialize new git repository
    info!("Initializing git repository at: {}", project_path);

    let init_output = Command::new("git")
        .current_dir(project_path)
        .args(["init"])
        .output()
        .map_err(|e| {
            crate::error::AppError::BadRequest(format!(
                "Failed to initialize git repository: {}",
                e
            ))
        })?;

    if !init_output.status.success() {
        return Err(crate::error::AppError::BadRequest(
            "Failed to initialize git repository".to_string(),
        ));
    }

    // Create initial .gitignore
    let gitignore_content = r#"# Common ignore patterns
.DS_Store
.vscode/
.idea/
*.log
*.tmp
.env
.env.local

# vibe-ensemble-mcp specific
.vibe-ensemble-mcp/
"#;

    let gitignore_path = path.join(".gitignore");
    if let Err(e) = fs::write(&gitignore_path, gitignore_content) {
        warn!("Failed to create .gitignore: {}", e);
    }

    // Stage and commit initial files
    let add_output = Command::new("git")
        .current_dir(project_path)
        .args(["add", "."])
        .output()
        .map_err(|e| {
            crate::error::AppError::BadRequest(format!("Failed to stage initial files: {}", e))
        })?;

    if !add_output.status.success() {
        warn!("Failed to stage initial files for git commit");
    }

    let commit_output = Command::new("git")
        .current_dir(project_path)
        .args([
            "commit",
            "-m",
            "chore: initialize project with vibe-ensemble-mcp",
        ])
        .output()
        .map_err(|e| {
            crate::error::AppError::BadRequest(format!("Failed to create initial commit: {}", e))
        })?;

    if !commit_output.status.success() {
        warn!("Failed to create initial git commit");
    }

    Ok("Initialized new git repository with initial commit".to_string())
}

pub struct CreateProjectTool;

#[async_trait]
impl ToolHandler for CreateProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let repository_name: String = extract_param(&arguments, "repository_name")?;
        let path: String = extract_param(&arguments, "path")?;
        let short_description: Option<String> = extract_optional_param(&arguments, "description")?;
        let rules: Option<String> = extract_optional_param(&arguments, "rules")?;
        let patterns: Option<String> = extract_optional_param(&arguments, "patterns")?;

        // Create the project directory if it doesn't exist
        debug!("Checking if project directory exists: {}", path);
        if !std::path::Path::new(&path).exists() {
            info!("Creating project directory: {}", path);
            if let Err(e) = fs::create_dir_all(&path) {
                return Ok(create_json_error_response(&format!(
                    "Failed to create project directory '{}': {}",
                    path, e
                )));
            }
            info!("✓ Successfully created project directory: {}", path);
        } else {
            debug!("Project directory already exists: {}", path);
        }

        // Validate the project path after creation/verification
        debug!("Validating project path: {}", path);
        if let Err(e) =
            crate::workers::validation::WorkerInputValidator::validate_project_path(&path)
        {
            return Ok(create_json_error_response(&format!(
                "Project path validation failed: {}. Path must be an absolute path to an existing directory.",
                e
            )));
        }
        info!("✓ Project path validation passed");

        // Create project-specific worker permissions file if it doesn't exist
        debug!("Creating project-specific worker permissions for: {}", path);
        if let Err(e) = create_project_permissions(&path) {
            warn!("Failed to create project permissions file: {}", e);
            // Don't fail the whole project creation for this - it's not critical
        }

        // Initialize git repository
        let git_status = match initialize_git_repository(&path) {
            Ok(status) => {
                info!("Git initialization: {}", status);
                status
            }
            Err(e) => {
                return Ok(create_json_error_response(&format!(
                    "Git repository initialization failed: {}",
                    e
                )));
            }
        };

        let request = CreateProjectRequest {
            repository_name: repository_name.clone(),
            path,
            short_description,
            rules,
            patterns,
        };

        match Project::create(&state.db, request).await {
            Ok(project) => {
                let response = json!({
                    "repository_name": project.repository_name,
                    "path": project.path,
                    "description": project.short_description,
                    "created_at": project.created_at,
                    "git_status": git_status
                });

                // Emit project_created event
                let project_data = json!({
                    "repository_name": project.repository_name,
                    "path": project.path,
                    "description": project.short_description,
                    "created_at": project.created_at
                });
                if let Err(e) = state
                    .event_emitter()
                    .emit_project_created(&project_data)
                    .await
                {
                    warn!("Failed to emit project_created event: {}", e);
                }

                Ok(create_json_success_response(response))
            }
            Err(e) => Ok(create_json_error_response(&format!(
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
                    },
                    "rules": {
                        "type": "string",
                        "description": "Project-specific rules and guidelines"
                    },
                    "patterns": {
                        "type": "string",
                        "description": "Project-specific patterns and conventions"
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
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let args = arguments.unwrap_or_default();

        // Parse pagination parameters using helper
        let cursor = extract_cursor(&Some(args.clone()))?;

        match Project::list_all(&state.db).await {
            Ok(all_projects) => {
                // Apply pagination using helper
                let pagination_result = cursor.paginate(all_projects);

                // Create response with pagination info
                let response_data = json!({
                    "projects": pagination_result.items,
                    "pagination": {
                        "total": pagination_result.total,
                        "has_more": pagination_result.has_more,
                        "next_cursor": pagination_result.next_cursor
                    }
                });

                Ok(create_json_success_response(response_data))
            }
            Err(e) => Ok(create_json_error_response(&format!(
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
                "properties": {
                    "cursor": {
                        "type": "string",
                        "description": "Optional cursor for pagination"
                    }
                },
                "required": []
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
            Ok(Some(project)) => Ok(create_json_success_response(
                serde_json::to_value(&project).map_err(|e| {
                    warn!(
                        "Failed to serialize project '{}' to JSON: {}",
                        repository_name, e
                    );
                    e
                })?,
            )),
            Ok(None) => Ok(create_json_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_json_error_response(&format!(
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
        let rules: Option<String> = extract_optional_param(&arguments, "rules")?;
        let patterns: Option<String> = extract_optional_param(&arguments, "patterns")?;

        let request = UpdateProjectRequest {
            path,
            short_description,
            rules,
            patterns,
            jbct_enabled: None,
            jbct_version: None,
            jbct_url: None,
        };

        match Project::update(&state.db, &repository_name, request).await {
            Ok(Some(project)) => Ok(create_json_success_response(
                serde_json::to_value(&project).map_err(|e| {
                    warn!(
                        "Failed to serialize updated project '{}' to JSON: {}",
                        repository_name, e
                    );
                    e
                })?,
            )),
            Ok(None) => Ok(create_json_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_json_error_response(&format!(
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
                    },
                    "rules": {
                        "type": "string",
                        "description": "Project-specific rules and guidelines"
                    },
                    "patterns": {
                        "type": "string",
                        "description": "Project-specific patterns and conventions"
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
            Ok(true) => Ok(create_json_success_response(json!({
                "message": format!("Project '{}' deleted successfully", repository_name),
                "repository_name": repository_name
            }))),
            Ok(false) => Ok(create_json_error_response(&format!(
                "Project '{}' not found",
                repository_name
            ))),
            Err(e) => Ok(create_json_error_response(&format!(
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
