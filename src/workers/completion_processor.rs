use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerOutput {
    pub ticket_id: Option<String>,
    pub outcome: WorkerOutcome,
    pub comment: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerOutcome {
    NextStage,
    PrevStage,
    CoordinatorAttention,
}
