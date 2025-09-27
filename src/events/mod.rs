/// Typed event system for end-to-end type safety
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod emitter;

/// Strongly typed event payload - replaces String-based broadcasts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    pub event_type: EventType,
    pub timestamp: DateTime<Utc>,
    pub data: EventData,
}

/// Event types in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    TicketCreated,
    TicketUpdated,
    TicketStageChanged,
    TicketClosed,
    TicketUnblocked,
    WorkerStarted,
    WorkerCompleted,
    WorkerFailed,
    WorkerStopped,
    WorkerTypeCreated,
    WorkerTypeUpdated,
    WorkerTypeDeleted,
    ProjectCreated,
    StageCompleted,
    TaskAssigned,
    QueueUpdated,
    SystemInit,
    SystemMessage,
    EndpointDiscovery,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::TicketCreated => write!(f, "ticket_created"),
            EventType::TicketUpdated => write!(f, "ticket_updated"),
            EventType::TicketStageChanged => write!(f, "ticket_stage_updated"),
            EventType::TicketClosed => write!(f, "ticket_closed"),
            EventType::TicketUnblocked => write!(f, "ticket_unblocked"),
            EventType::WorkerStarted => write!(f, "worker_started"),
            EventType::WorkerCompleted => write!(f, "worker_completed"),
            EventType::WorkerFailed => write!(f, "worker_failed"),
            EventType::WorkerStopped => write!(f, "worker_stopped"),
            EventType::WorkerTypeCreated => write!(f, "worker_type_created"),
            EventType::WorkerTypeUpdated => write!(f, "worker_type_updated"),
            EventType::WorkerTypeDeleted => write!(f, "worker_type_deleted"),
            EventType::ProjectCreated => write!(f, "project_created"),
            EventType::StageCompleted => write!(f, "stage_completed"),
            EventType::TaskAssigned => write!(f, "task_assigned"),
            EventType::QueueUpdated => write!(f, "queue_updated"),
            EventType::SystemInit => write!(f, "system_init"),
            EventType::SystemMessage => write!(f, "system_message"),
            EventType::EndpointDiscovery => write!(f, "endpoint_discovery"),
        }
    }
}

/// Event data - strongly typed per event type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    Ticket(TicketEventData),
    Worker(WorkerEventData),
    Queue(QueueEventData),
    System(SystemEventData),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketEventData {
    pub ticket_id: String,
    pub project_id: String,
    pub stage: Option<String>,
    pub state: Option<String>,
    pub change_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerEventData {
    pub worker_id: String,
    pub worker_type: String,
    pub project_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEventData {
    pub queue_name: String,
    pub project_id: String,
    pub worker_type: String,
    pub task_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEventData {
    pub component: String,
    pub message: String,
    pub metadata: Option<Value>,
}

impl EventPayload {
    /// Create a new ticket created event with full ticket data
    pub fn ticket_created_with_data(
        ticket_id: &str,
        project_id: &str,
        _title: &str,
        current_stage: &str,
    ) -> Self {
        Self {
            event_type: EventType::TicketCreated,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: Some(current_stage.to_string()),
                state: Some("open".to_string()),
                change_type: "created".to_string(),
            }),
        }
    }

    /// Create a new ticket event
    pub fn ticket_created(ticket_id: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::TicketCreated,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: None,
                state: Some("open".to_string()),
                change_type: "created".to_string(),
            }),
        }
    }

    /// Create a ticket updated event
    pub fn ticket_updated(ticket_id: &str, project_id: &str, change_type: &str) -> Self {
        Self {
            event_type: EventType::TicketUpdated,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: None,
                state: None,
                change_type: change_type.to_string(),
            }),
        }
    }

    /// Create a ticket closed event
    pub fn ticket_closed(ticket_id: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::TicketClosed,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: None,
                state: Some("closed".to_string()),
                change_type: "closed".to_string(),
            }),
        }
    }

    /// Create a ticket unblocked event
    pub fn ticket_unblocked(ticket_id: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::TicketUnblocked,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: None,
                state: Some("open".to_string()),
                change_type: "unblocked".to_string(),
            }),
        }
    }

    /// Create a ticket stage change event
    pub fn ticket_stage_changed(
        ticket_id: &str,
        project_id: &str,
        old_stage: &str,
        new_stage: &str,
    ) -> Self {
        Self {
            event_type: EventType::TicketStageChanged,
            timestamp: Utc::now(),
            data: EventData::Ticket(TicketEventData {
                ticket_id: ticket_id.to_string(),
                project_id: project_id.to_string(),
                stage: Some(new_stage.to_string()),
                state: None,
                change_type: format!("{} -> {}", old_stage, new_stage),
            }),
        }
    }

    /// Create a worker spawned event
    pub fn worker_spawned(worker_id: &str, worker_type: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::WorkerStarted,
            timestamp: Utc::now(),
            data: EventData::Worker(WorkerEventData {
                worker_id: worker_id.to_string(),
                worker_type: worker_type.to_string(),
                project_id: project_id.to_string(),
                status: "spawning".to_string(),
            }),
        }
    }

    /// Create a worker started event
    pub fn worker_started(worker_id: &str, worker_type: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::WorkerStarted,
            timestamp: Utc::now(),
            data: EventData::Worker(WorkerEventData {
                worker_id: worker_id.to_string(),
                worker_type: worker_type.to_string(),
                project_id: project_id.to_string(),
                status: "started".to_string(),
            }),
        }
    }

    /// Create a worker completed event
    pub fn worker_completed(worker_id: &str, worker_type: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::WorkerCompleted,
            timestamp: Utc::now(),
            data: EventData::Worker(WorkerEventData {
                worker_id: worker_id.to_string(),
                worker_type: worker_type.to_string(),
                project_id: project_id.to_string(),
                status: "completed".to_string(),
            }),
        }
    }

    /// Create a worker failed event
    pub fn worker_failed(worker_id: &str, worker_type: &str, project_id: &str) -> Self {
        Self {
            event_type: EventType::WorkerFailed,
            timestamp: Utc::now(),
            data: EventData::Worker(WorkerEventData {
                worker_id: worker_id.to_string(),
                worker_type: worker_type.to_string(),
                project_id: project_id.to_string(),
                status: "failed".to_string(),
            }),
        }
    }

    /// Create a queue update event
    pub fn queue_updated(
        queue_name: &str,
        project_id: &str,
        worker_type: &str,
        task_count: usize,
    ) -> Self {
        Self {
            event_type: EventType::QueueUpdated,
            timestamp: Utc::now(),
            data: EventData::Queue(QueueEventData {
                queue_name: queue_name.to_string(),
                project_id: project_id.to_string(),
                worker_type: worker_type.to_string(),
                task_count,
            }),
        }
    }

    /// Create system init event
    pub fn system_init() -> Self {
        Self {
            event_type: EventType::SystemInit,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "mcp_server".to_string(),
                message: "MCP server initialized".to_string(),
                metadata: None,
            }),
        }
    }

    /// Create system message event for generic system/info messages
    pub fn system_message(component: &str, message: &str, metadata: Option<Value>) -> Self {
        Self {
            event_type: EventType::SystemMessage,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: component.to_string(),
                message: message.to_string(),
                metadata,
            }),
        }
    }

    /// Create endpoint discovery event
    pub fn endpoint_discovery(http_url: &str, sse_url: &str) -> Self {
        Self {
            event_type: EventType::EndpointDiscovery,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "transport".to_string(),
                message: "Available endpoints".to_string(),
                metadata: Some(serde_json::json!({
                    "http": http_url,
                    "sse": sse_url
                })),
            }),
        }
    }

    /// Create a worker stopped event
    pub fn worker_stopped(worker_id: &str, reason: &str) -> Self {
        Self {
            event_type: EventType::WorkerStopped,
            timestamp: Utc::now(),
            data: EventData::Worker(WorkerEventData {
                worker_id: worker_id.to_string(),
                worker_type: "unknown".to_string(),
                project_id: "unknown".to_string(),
                status: reason.to_string(),
            }),
        }
    }

    /// Create a worker type created event
    pub fn worker_type_created(project_id: &str, worker_type: &str) -> Self {
        Self {
            event_type: EventType::WorkerTypeCreated,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "worker_type".to_string(),
                message: format!(
                    "Worker type '{}' created in project '{}'",
                    worker_type, project_id
                ),
                metadata: Some(serde_json::json!({
                    "project_id": project_id,
                    "worker_type": worker_type
                })),
            }),
        }
    }

    /// Create a worker type updated event
    pub fn worker_type_updated(project_id: &str, worker_type: &str) -> Self {
        Self {
            event_type: EventType::WorkerTypeUpdated,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "worker_type".to_string(),
                message: format!(
                    "Worker type '{}' updated in project '{}'",
                    worker_type, project_id
                ),
                metadata: Some(serde_json::json!({
                    "project_id": project_id,
                    "worker_type": worker_type
                })),
            }),
        }
    }

    /// Create a worker type deleted event
    pub fn worker_type_deleted(project_id: &str, worker_type: &str) -> Self {
        Self {
            event_type: EventType::WorkerTypeDeleted,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "worker_type".to_string(),
                message: format!(
                    "Worker type '{}' deleted from project '{}'",
                    worker_type, project_id
                ),
                metadata: Some(serde_json::json!({
                    "project_id": project_id,
                    "worker_type": worker_type
                })),
            }),
        }
    }

    /// Create a project created event
    pub fn project_created(project_id: &str) -> Self {
        Self {
            event_type: EventType::ProjectCreated,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "project".to_string(),
                message: format!("Project '{}' created", project_id),
                metadata: Some(serde_json::json!({
                    "project_id": project_id
                })),
            }),
        }
    }

    /// Create a stage completed event
    pub fn stage_completed(ticket_id: &str, stage: &str, worker_id: &str) -> Self {
        Self {
            event_type: EventType::StageCompleted,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "stage".to_string(),
                message: format!("Stage '{}' completed for ticket '{}'", stage, ticket_id),
                metadata: Some(serde_json::json!({
                    "ticket_id": ticket_id,
                    "stage": stage,
                    "worker_id": worker_id
                })),
            }),
        }
    }

    /// Create a task assigned event
    pub fn task_assigned(ticket_id: &str, queue_name: &str) -> Self {
        Self {
            event_type: EventType::TaskAssigned,
            timestamp: Utc::now(),
            data: EventData::System(SystemEventData {
                component: "queue".to_string(),
                message: format!("Task assigned to queue '{}'", queue_name),
                metadata: Some(serde_json::json!({
                    "ticket_id": ticket_id,
                    "queue_name": queue_name
                })),
            }),
        }
    }

    /// Convert to JSON-RPC notification format - uses sampling/createMessage for Claude processing
    pub fn to_jsonrpc_notification(&self) -> Value {
        use crate::mcp::JsonRpcEnvelopes;

        // Use sampling/createMessage format to trigger Claude to process realtime events
        JsonRpcEnvelopes::sampling_create_message()
    }
}
