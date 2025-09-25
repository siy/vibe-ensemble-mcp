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
    WorkerSpawned,
    WorkerFinished,
    WorkerFailed,
    QueueUpdated,
    SystemInit,
    SystemMessage,
    EndpointDiscovery,
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
            event_type: EventType::WorkerSpawned,
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
            event_type: EventType::WorkerSpawned,
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
            event_type: EventType::WorkerFinished,
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

    /// Convert to JSON-RPC notification format - uses sampling/createMessage for Claude processing
    pub fn to_jsonrpc_notification(&self) -> Value {
        use crate::mcp::JsonRpcEnvelopes;

        // Use sampling/createMessage format to trigger Claude to process realtime events
        JsonRpcEnvelopes::sampling_create_message()
    }
}
