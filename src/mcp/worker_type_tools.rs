use async_trait::async_trait;
use serde_json::{json, Value};

use super::tools::{
    create_error_response, create_success_response, extract_optional_param, extract_param,
    ToolHandler,
};
use super::types::{CallToolResponse, Tool};
use crate::{
    database::worker_types::{CreateWorkerTypeRequest, UpdateWorkerTypeRequest, WorkerType},
    error::Result,
    server::AppState,
};

pub struct CreateWorkerTypeTool;

#[async_trait]
impl ToolHandler for CreateWorkerTypeTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;
        let system_prompt: String = extract_param(&arguments, "system_prompt")?;
        let short_description: Option<String> =
            extract_optional_param(&arguments, "short_description")?;

        let request = CreateWorkerTypeRequest {
            project_id: project_id.clone(),
            worker_type: worker_type.clone(),
            short_description: short_description.clone(),
            system_prompt: system_prompt.clone(),
        };

        match WorkerType::create(&state.db, request).await {
            Ok(worker_type_info) => {
                let response = json!({
                    "id": worker_type_info.id,
                    "project_id": worker_type_info.project_id,
                    "worker_type": worker_type_info.worker_type,
                    "short_description": worker_type_info.short_description,
                    "system_prompt": worker_type_info.system_prompt,
                    "created_at": worker_type_info.created_at,
                    "updated_at": worker_type_info.updated_at
                });
                
                // Broadcast worker_type_created event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("vibe-ensemble://worker-types/{}/{}", project_id, worker_type),
                        "event": {
                            "type": "worker_type_created",
                            "worker_type": {
                                "id": worker_type_info.id,
                                "project_id": worker_type_info.project_id,
                                "worker_type": worker_type_info.worker_type,
                                "short_description": worker_type_info.short_description,
                                "system_prompt": worker_type_info.system_prompt,
                                "created_at": worker_type_info.created_at,
                                "updated_at": worker_type_info.updated_at
                            },
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast worker_type_created event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast worker_type_created event for: {}/{}", project_id, worker_type);
                }
                
                Ok(create_success_response(&format!(
                    "Worker type '{}' created successfully for project '{}': {}",
                    worker_type, project_id, response
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to create worker type '{}' for project '{}': {}",
                worker_type, project_id, e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "create_worker_type".to_string(),
            description: "Create a new worker type with a custom system prompt for a project"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier (e.g., 'architect', 'developer', 'tester')"
                    },
                    "system_prompt": {
                        "type": "string",
                        "description": "Specialized system prompt defining the worker's role and capabilities"
                    },
                    "short_description": {
                        "type": "string",
                        "description": "Optional brief description of the worker type's purpose"
                    }
                },
                "required": ["project_id", "worker_type", "system_prompt"]
            }),
        }
    }
}

pub struct ListWorkerTypesTool;

#[async_trait]
impl ToolHandler for ListWorkerTypesTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: Option<String> = extract_optional_param(&arguments, "project_id")?;

        match WorkerType::list_by_project(&state.db, project_id.as_deref()).await {
            Ok(worker_types) => {
                let worker_types_json = serde_json::to_string_pretty(&worker_types)?;
                Ok(create_success_response(&format!(
                    "Worker types:\n{}",
                    worker_types_json
                )))
            }
            Err(e) => Ok(create_error_response(&format!(
                "Failed to list worker types: {}",
                e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "list_worker_types".to_string(),
            description: "List all worker types, optionally filtered by project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Optional project ID to filter worker types"
                    }
                }
            }),
        }
    }
}

pub struct GetWorkerTypeTool;

#[async_trait]
impl ToolHandler for GetWorkerTypeTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;

        match WorkerType::get_by_type(&state.db, &project_id, &worker_type).await {
            Ok(Some(worker_type_info)) => {
                let response = json!({
                    "id": worker_type_info.id,
                    "project_id": worker_type_info.project_id,
                    "worker_type": worker_type_info.worker_type,
                    "short_description": worker_type_info.short_description,
                    "system_prompt": worker_type_info.system_prompt,
                    "created_at": worker_type_info.created_at,
                    "updated_at": worker_type_info.updated_at
                });
                Ok(create_success_response(&format!(
                    "Worker type details: {}",
                    response
                )))
            }
            Ok(None) => Ok(create_error_response(&format!(
                "Worker type '{}' not found for project '{}'",
                worker_type, project_id
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to get worker type '{}' for project '{}': {}",
                worker_type, project_id, e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "get_worker_type".to_string(),
            description: "Get details of a specific worker type".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier to retrieve"
                    }
                },
                "required": ["project_id", "worker_type"]
            }),
        }
    }
}

pub struct UpdateWorkerTypeTool;

#[async_trait]
impl ToolHandler for UpdateWorkerTypeTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;
        let short_description: Option<String> =
            extract_optional_param(&arguments, "short_description")?;
        let system_prompt: Option<String> = extract_optional_param(&arguments, "system_prompt")?;

        if short_description.is_none() && system_prompt.is_none() {
            return Ok(create_error_response(
                "At least one of 'short_description' or 'system_prompt' must be provided for update"
            ));
        }

        let request = UpdateWorkerTypeRequest {
            short_description,
            system_prompt,
        };

        match WorkerType::update(&state.db, &project_id, &worker_type, request).await {
            Ok(Some(worker_type_info)) => {
                let response = json!({
                    "id": worker_type_info.id,
                    "project_id": worker_type_info.project_id,
                    "worker_type": worker_type_info.worker_type,
                    "short_description": worker_type_info.short_description,
                    "system_prompt": worker_type_info.system_prompt,
                    "created_at": worker_type_info.created_at,
                    "updated_at": worker_type_info.updated_at
                });
                
                // Broadcast worker_type_updated event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("vibe-ensemble://worker-types/{}/{}", project_id, worker_type),
                        "event": {
                            "type": "worker_type_updated",
                            "worker_type": {
                                "id": worker_type_info.id,
                                "project_id": worker_type_info.project_id,
                                "worker_type": worker_type_info.worker_type,
                                "short_description": worker_type_info.short_description,
                                "system_prompt": worker_type_info.system_prompt,
                                "created_at": worker_type_info.created_at,
                                "updated_at": worker_type_info.updated_at
                            },
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast worker_type_updated event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast worker_type_updated event for: {}/{}", project_id, worker_type);
                }
                
                Ok(create_success_response(&format!(
                    "Worker type '{}' updated successfully for project '{}': {}",
                    worker_type, project_id, response
                )))
            }
            Ok(None) => Ok(create_error_response(&format!(
                "Worker type '{}' not found for project '{}'",
                worker_type, project_id
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to update worker type '{}' for project '{}': {}",
                worker_type, project_id, e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "update_worker_type".to_string(),
            description: "Update an existing worker type's description or system prompt"
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier to update"
                    },
                    "short_description": {
                        "type": "string",
                        "description": "Updated description of the worker type's purpose"
                    },
                    "system_prompt": {
                        "type": "string",
                        "description": "Updated system prompt defining the worker's role and capabilities"
                    }
                },
                "required": ["project_id", "worker_type"]
            }),
        }
    }
}

pub struct DeleteWorkerTypeTool;

#[async_trait]
impl ToolHandler for DeleteWorkerTypeTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = extract_param(&arguments, "project_id")?;
        let worker_type: String = extract_param(&arguments, "worker_type")?;

        match WorkerType::delete(&state.db, &project_id, &worker_type).await {
            Ok(true) => {
                // Broadcast worker_type_deleted event
                let event = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/resources/updated",
                    "params": {
                        "uri": format!("vibe-ensemble://worker-types/{}/{}", project_id, worker_type),
                        "event": {
                            "type": "worker_type_deleted",
                            "project_id": project_id,
                            "worker_type": worker_type,
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                if let Err(e) = state.event_broadcaster.broadcast(event.to_string()) {
                    tracing::warn!("Failed to broadcast worker_type_deleted event: {}", e);
                } else {
                    tracing::debug!("Successfully broadcast worker_type_deleted event for: {}/{}", project_id, worker_type);
                }
                
                Ok(create_success_response(&format!(
                    "Worker type '{}' deleted successfully from project '{}'",
                    worker_type, project_id
                )))
            }
            Ok(false) => Ok(create_error_response(&format!(
                "Worker type '{}' not found for project '{}'",
                worker_type, project_id
            ))),
            Err(e) => Ok(create_error_response(&format!(
                "Failed to delete worker type '{}' from project '{}': {}",
                worker_type, project_id, e
            ))),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "delete_worker_type".to_string(),
            description: "Delete a worker type from a project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "Project repository name"
                    },
                    "worker_type": {
                        "type": "string",
                        "description": "Worker type identifier to delete"
                    }
                },
                "required": ["project_id", "worker_type"]
            }),
        }
    }
}
