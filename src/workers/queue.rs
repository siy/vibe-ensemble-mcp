use anyhow::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, debug};
use uuid::Uuid;

use super::types::{TaskItem, QueueRegistry};

pub struct QueueManager {
    queues: QueueRegistry,
}

impl QueueManager {
    pub fn new() -> Self {
        Self {
            queues: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_queue(&self, queue_name: &str) -> Result<()> {
        info!("Creating queue: {}", queue_name);
        
        let mut queues = self.queues.write().await;
        if !queues.contains_key(queue_name) {
            queues.insert(queue_name.to_string(), RwLock::new(Vec::new()));
            info!("Queue '{}' created", queue_name);
        } else {
            debug!("Queue '{}' already exists", queue_name);
        }
        
        Ok(())
    }

    pub async fn delete_queue(&self, queue_name: &str) -> Result<bool> {
        info!("Deleting queue: {}", queue_name);
        
        let mut queues = self.queues.write().await;
        let removed = queues.remove(queue_name).is_some();
        
        if removed {
            info!("Queue '{}' deleted", queue_name);
        }
        
        Ok(removed)
    }

    pub async fn add_task(&self, queue_name: &str, ticket_id: &str) -> Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let task = TaskItem {
            task_id: task_id.clone(),
            ticket_id: ticket_id.to_string(),
            created_at: chrono::Utc::now(),
        };

        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let mut queue_items = queue.write().await;
            queue_items.push(task);
            info!("Task {} added to queue {}", task_id, queue_name);
            Ok(task_id)
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }

    pub async fn get_next_task(&self, queue_name: &str) -> Result<Option<TaskItem>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let mut queue_items = queue.write().await;
            if queue_items.is_empty() {
                Ok(None)
            } else {
                let task = queue_items.remove(0);
                debug!("Task {} retrieved from queue {}", task.task_id, queue_name);
                Ok(Some(task))
            }
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }

    pub async fn get_queue_status(&self, queue_name: &str) -> Result<Option<QueueStatus>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let queue_items = queue.read().await;
            Ok(Some(QueueStatus {
                queue_name: queue_name.to_string(),
                task_count: queue_items.len(),
                tasks: queue_items.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_queues(&self) -> Result<Vec<QueueStatus>> {
        let queues = self.queues.read().await;
        let mut result = Vec::new();
        
        for (queue_name, queue) in queues.iter() {
            let queue_items = queue.read().await;
            result.push(QueueStatus {
                queue_name: queue_name.clone(),
                task_count: queue_items.len(),
                tasks: queue_items.clone(),
            });
        }
        
        Ok(result)
    }

    pub async fn get_queue_tasks(&self, queue_name: &str) -> Result<Vec<TaskItem>> {
        let queues = self.queues.read().await;
        if let Some(queue) = queues.get(queue_name) {
            let queue_items = queue.read().await;
            Ok(queue_items.clone())
        } else {
            Err(anyhow::anyhow!("Queue '{}' not found", queue_name))
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct QueueStatus {
    pub queue_name: String,
    pub task_count: usize,
    pub tasks: Vec<TaskItem>,
}