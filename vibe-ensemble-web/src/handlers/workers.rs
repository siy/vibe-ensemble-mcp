//! Worker management handlers

use askama::Template;
use axum::{
    extract::{Path, Query, State, WebSocketUpgrade},
    response::{Html, Json},
    Json as JsonExtractor,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use vibe_ensemble_core::orchestration::worker_manager::{WorkerInfo, WorkerStatus, OutputType};
use vibe_ensemble_storage::StorageManager;

use crate::{templates::WorkersTemplate, Result};

/// Worker dashboard template data
#[derive(Debug, Serialize)]
pub struct WorkerDashboardData {
    pub workers: Vec<WorkerInfo>,
    pub total_workers: usize,
    pub active_workers: usize,
    pub connected_workers: usize,
    pub system_metrics: Option<crate::templates::SystemMetrics>,
}

/// Worker spawn request parameters
#[derive(Debug, Deserialize)]
pub struct WorkerSpawnRequest {
    pub prompt: String,
    pub capabilities: Vec<String>,
    pub working_directory: Option<String>,
}

/// Worker spawn response
#[derive(Debug, Serialize)]
pub struct WorkerSpawnResponse {
    pub worker_id: Uuid,
    pub status: String,
    pub message: String,
}

/// Worker shutdown request parameters
#[derive(Debug, Deserialize)]
pub struct WorkerShutdownRequest {
    pub graceful: bool,
}

/// Worker shutdown response
#[derive(Debug, Serialize)]
pub struct WorkerShutdownResponse {
    pub worker_id: Uuid,
    pub status: String,
    pub message: String,
}

/// Worker output query parameters
#[derive(Debug, Deserialize)]
pub struct WorkerOutputQuery {
    pub limit: Option<usize>,
    pub follow: Option<bool>,
}

/// Worker output line for API responses
#[derive(Debug, Clone, Serialize)]
pub struct WorkerOutputLine {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub output_type: OutputType,
    pub content: String,
}

/// Workers dashboard page handler
pub async fn dashboard(State(_storage): State<Arc<StorageManager>>) -> Result<Html<String>> {
    // For now, return a basic template since we don't have direct access to WorkerManager
    // This will be populated via WebSocket connections and API calls
    let workers: Vec<WorkerInfo> = Vec::new();

    let dashboard_data = WorkerDashboardData {
        total_workers: workers.len(),
        active_workers: workers
            .iter()
            .filter(|w| matches!(w.status, WorkerStatus::Connected | WorkerStatus::Running))
            .count(),
        connected_workers: workers
            .iter()
            .filter(|w| matches!(w.status, WorkerStatus::Connected))
            .count(),
        workers,
        system_metrics: None,
    };

    let template = WorkersTemplate::new(dashboard_data);
    let rendered = template
        .render()
        .map_err(|e| crate::Error::Internal(anyhow::anyhow!("Template render error: {}", e)))?;

    Ok(Html(rendered))
}

/// List all workers API endpoint
pub async fn list_workers_api(
    State(_storage): State<Arc<StorageManager>>,
) -> Result<Json<Vec<WorkerInfo>>> {
    // This would typically call the MCP server or worker manager directly
    // For now, return empty list - will be populated via WebSocket updates
    Ok(Json(Vec::new()))
}

/// Get specific worker status API endpoint
pub async fn get_worker_status_api(
    State(_storage): State<Arc<StorageManager>>,
    Path(_worker_id): Path<Uuid>,
) -> Result<Json<Option<WorkerInfo>>> {
    // This would typically call the worker manager
    // For now, return None - will be implemented with proper worker manager integration
    Ok(Json(None))
}

/// Spawn new worker API endpoint
pub async fn spawn_worker_api(
    State(_storage): State<Arc<StorageManager>>,
    JsonExtractor(request): JsonExtractor<WorkerSpawnRequest>,
) -> Result<Json<WorkerSpawnResponse>> {
    // This would typically call the worker manager to spawn a new worker
    // For now, return a mock response
    let worker_id = Uuid::new_v4();

    let response = WorkerSpawnResponse {
        worker_id,
        status: "spawning".to_string(),
        message: format!(
            "Worker spawn initiated with prompt: {} and capabilities: {:?}",
            request.prompt.chars().take(50).collect::<String>(),
            request.capabilities
        ),
    };

    Ok(Json(response))
}

/// Shutdown worker API endpoint
pub async fn shutdown_worker_api(
    State(_storage): State<Arc<StorageManager>>,
    Path(worker_id): Path<Uuid>,
    JsonExtractor(request): JsonExtractor<WorkerShutdownRequest>,
) -> Result<Json<WorkerShutdownResponse>> {
    // This would typically call the worker manager to shutdown the worker
    // For now, return a mock response

    let response = WorkerShutdownResponse {
        worker_id,
        status: if request.graceful {
            "shutting_down"
        } else {
            "force_killing"
        }
        .to_string(),
        message: format!(
            "Worker {} {} shutdown initiated",
            worker_id,
            if request.graceful {
                "graceful"
            } else {
                "force"
            }
        ),
    };

    Ok(Json(response))
}

/// Get worker output API endpoint
pub async fn get_worker_output_api(
    State(_storage): State<Arc<StorageManager>>,
    Path(_worker_id): Path<Uuid>,
    Query(_params): Query<WorkerOutputQuery>,
) -> Result<Json<Vec<WorkerOutputLine>>> {
    // This would typically call the worker manager to get output
    // For now, return empty output
    Ok(Json(Vec::new()))
}

/// WebSocket handler for real-time worker updates
pub async fn worker_websocket_handler(
    ws: WebSocketUpgrade,
    State(_storage): State<Arc<StorageManager>>,
) -> axum::response::Response {
    ws.on_upgrade(handle_worker_websocket)
}

/// Handle worker WebSocket connection
async fn handle_worker_websocket(mut socket: axum::extract::ws::WebSocket) {
    use axum::extract::ws::Message;
    use tokio::time::{interval, Duration};

    // Send initial worker status
    let initial_status = serde_json::json!({
        "type": "worker_status_update",
        "workers": [],
        "timestamp": chrono::Utc::now()
    });

    if socket
        .send(Message::Text(initial_status.to_string()))
        .await
        .is_err()
    {
        return;
    }

    // Set up periodic updates
    let mut interval = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                // Send periodic worker status updates
                let status_update = serde_json::json!({
                    "type": "worker_status_update",
                    "workers": [],
                    "timestamp": chrono::Utc::now()
                });

                if socket.send(Message::Text(status_update.to_string())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // Handle incoming WebSocket messages (e.g., subscribe to specific worker)
                        if let Ok(request) = serde_json::from_str::<serde_json::Value>(&text) {
                            if let Some(msg_type) = request.get("type").and_then(|v| v.as_str()) {
                                match msg_type {
                                    "subscribe_worker" => {
                                        if let Some(_worker_id) = request.get("worker_id").and_then(|v| v.as_str()) {
                                            // Subscribe to specific worker output
                                            // This will be implemented with proper WorkerManager integration
                                        }
                                    }
                                    "unsubscribe_worker" => {
                                        if let Some(_worker_id) = request.get("worker_id").and_then(|v| v.as_str()) {
                                            // Unsubscribe from worker output
                                            // This will be implemented with proper WorkerManager integration
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(_)) => break,
                    None => break,
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_dashboard_data_serialization() {
        let data = WorkerDashboardData {
            workers: Vec::new(),
            total_workers: 0,
            active_workers: 0,
            connected_workers: 0,
            system_metrics: None,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("workers"));
        assert!(json.contains("total_workers"));
    }

    #[test]
    fn test_worker_spawn_request_deserialization() {
        let json = r#"{
            "prompt": "Test worker prompt",
            "capabilities": ["test", "debug"],
            "working_directory": "/tmp"
        }"#;

        let request: WorkerSpawnRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.prompt, "Test worker prompt");
        assert_eq!(request.capabilities, vec!["test", "debug"]);
        assert_eq!(request.working_directory, Some("/tmp".to_string()));
    }
}
