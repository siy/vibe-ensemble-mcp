//! Metrics collection and Prometheus export

use crate::{config::MetricsConfig, error::Result, MonitoringError};
use metrics::{Counter, Gauge, Histogram, Unit};
use metrics_exporter_prometheus::PrometheusBuilder;
use metrics_util::registry::{AtomicStorage, Registry};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use sysinfo::{System, SystemExt};
use tokio::time;
use tracing::{error, info, warn};
#[cfg(feature = "storage")]
use vibe_ensemble_core::{agent::Agent, issue::Issue};
#[cfg(feature = "storage")]
use vibe_ensemble_storage::StorageManager;

/// System metrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// CPU usage percentage
    pub cpu_usage: f64,
    /// Memory usage in bytes
    pub memory_used: u64,
    /// Total memory in bytes
    pub memory_total: u64,
    /// Memory usage percentage
    pub memory_percentage: f64,
    /// Disk usage in bytes
    pub disk_used: u64,
    /// Total disk space in bytes
    pub disk_total: u64,
    /// Network bytes received
    pub network_rx_bytes: u64,
    /// Network bytes transmitted
    pub network_tx_bytes: u64,
    /// System load average
    pub load_average: f64,
    /// Number of running processes
    pub process_count: usize,
}

/// Business metrics for the MCP system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessMetrics {
    /// Total number of agents
    pub total_agents: u64,
    /// Active agents count
    pub active_agents: u64,
    /// Total issues
    pub total_issues: u64,
    /// Open issues
    pub open_issues: u64,
    /// Closed issues
    pub closed_issues: u64,
    /// Messages sent in last period
    pub messages_sent: u64,
    /// Average issue resolution time
    pub avg_resolution_time_hours: f64,
    /// Knowledge items count
    pub knowledge_items: u64,
}

/// Metrics collector and manager
pub struct MetricsCollector {
    config: MetricsConfig,
    system: System,
    #[cfg(feature = "storage")]
    storage: Option<Arc<StorageManager>>,
    registry: Registry<String, AtomicStorage>,
    collection_start: Instant,
    last_system_metrics: Arc<RwLock<Option<SystemMetrics>>>,
    last_business_metrics: Arc<RwLock<Option<BusinessMetrics>>>,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(config: MetricsConfig) -> Self {
        let registry = Registry::new(AtomicStorage);

        Self {
            config,
            system: System::new_all(),
            storage: None,
            registry,
            collection_start: Instant::now(),
            last_system_metrics: Arc::new(RwLock::new(None)),
            last_business_metrics: Arc::new(RwLock::new(None)),
        }
    }

    /// Set storage manager for business metrics
    pub fn with_storage(mut self, storage: Arc<StorageManager>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Initialize metrics collection
    pub async fn initialize(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Metrics collection disabled");
            return Ok(());
        }

        info!("Initializing metrics collection");

        // Set up Prometheus exporter
        let builder = PrometheusBuilder::new().with_http_listener(
            format!("{}:{}", self.config.host, self.config.port)
                .parse()
                .map_err(|e| MonitoringError::Config(format!("Invalid metrics address: {}", e)))?,
        );

        builder
            .install()
            .map_err(|e| MonitoringError::Metrics(format!("Failed to setup Prometheus: {}", e)))?;

        // Register custom metrics
        self.register_metrics();

        // Start collection background task
        if self.config.system_metrics || self.config.business_metrics {
            self.start_collection_task().await;
        }

        info!(
            "Metrics collection initialized on {}:{}",
            self.config.host, self.config.port
        );
        Ok(())
    }

    /// Register custom metrics
    fn register_metrics(&self) {
        // System metrics
        metrics::describe_gauge!(
            "system_cpu_usage_percent",
            Unit::Percent,
            "CPU usage percentage"
        );
        metrics::describe_gauge!(
            "system_memory_used_bytes",
            Unit::Bytes,
            "Memory usage in bytes"
        );
        metrics::describe_gauge!(
            "system_memory_total_bytes",
            Unit::Bytes,
            "Total memory in bytes"
        );
        metrics::describe_gauge!(
            "system_memory_usage_percent",
            Unit::Percent,
            "Memory usage percentage"
        );
        metrics::describe_gauge!("system_load_average", "System load average");
        metrics::describe_gauge!("system_process_count", "Number of running processes");

        // Business metrics
        metrics::describe_gauge!("agents_total", "Total number of agents");
        metrics::describe_gauge!("agents_active", "Number of active agents");
        metrics::describe_gauge!("issues_total", "Total number of issues");
        metrics::describe_gauge!("issues_open", "Number of open issues");
        metrics::describe_gauge!("issues_closed", "Number of closed issues");
        metrics::describe_counter!("messages_sent_total", "Total messages sent");
        metrics::describe_histogram!(
            "issue_resolution_time_hours",
            Unit::Seconds,
            "Issue resolution time in hours"
        );
        metrics::describe_gauge!("knowledge_items_total", "Total knowledge items");

        // Request metrics
        metrics::describe_counter!("http_requests_total", "Total HTTP requests");
        metrics::describe_histogram!(
            "http_request_duration_ms",
            Unit::Milliseconds,
            "HTTP request duration"
        );
        metrics::describe_counter!("http_errors_total", "Total HTTP errors");

        // Operation metrics
        metrics::describe_counter!("operation_total", "Total operations performed");
        metrics::describe_histogram!(
            "operation_duration_ms",
            Unit::Milliseconds,
            "Operation duration"
        );
        metrics::describe_counter!("operation_errors_total", "Total operation errors");
    }

    /// Start background metrics collection task
    async fn start_collection_task(&self) {
        let config = self.config.clone();
        let storage = self.storage.clone();
        let system_metrics_ref = self.last_system_metrics.clone();
        let business_metrics_ref = self.last_business_metrics.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(config.collection_interval));
            let mut system = System::new_all();

            loop {
                interval.tick().await;

                // Collect system metrics
                if config.system_metrics {
                    if let Err(e) =
                        Self::collect_system_metrics(&mut system, &system_metrics_ref).await
                    {
                        error!("Failed to collect system metrics: {}", e);
                    }
                }

                // Collect business metrics
                if config.business_metrics {
                    if let Some(storage) = &storage {
                        if let Err(e) =
                            Self::collect_business_metrics(storage, &business_metrics_ref).await
                        {
                            error!("Failed to collect business metrics: {}", e);
                        }
                    } else {
                        warn!("Business metrics enabled but no storage manager available");
                    }
                }
            }
        });

        info!("Started metrics collection background task");
    }

    /// Collect system metrics
    async fn collect_system_metrics(
        system: &mut System,
        metrics_ref: &Arc<RwLock<Option<SystemMetrics>>>,
    ) -> Result<()> {
        // Refresh system information
        system.refresh_all();

        // Calculate CPU usage
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;

        // Memory metrics
        let memory_used = system.used_memory();
        let memory_total = system.total_memory();
        let memory_percentage = if memory_total > 0 {
            (memory_used as f64 / memory_total as f64) * 100.0
        } else {
            0.0
        };

        // Load average (Unix-like systems)
        let load_average = system.load_average().one;

        // Process count
        let process_count = system.processes().len();

        let system_metrics = SystemMetrics {
            cpu_usage,
            memory_used,
            memory_total,
            memory_percentage,
            disk_used: 0,        // TODO: Implement disk metrics
            disk_total: 0,       // TODO: Implement disk metrics
            network_rx_bytes: 0, // TODO: Implement network metrics
            network_tx_bytes: 0, // TODO: Implement network metrics
            load_average,
            process_count,
        };

        // Update Prometheus metrics
        metrics::gauge!("system_cpu_usage_percent").set(cpu_usage);
        metrics::gauge!("system_memory_used_bytes").set(memory_used as f64);
        metrics::gauge!("system_memory_total_bytes").set(memory_total as f64);
        metrics::gauge!("system_memory_usage_percent").set(memory_percentage);
        metrics::gauge!("system_load_average").set(load_average);
        metrics::gauge!("system_process_count").set(process_count as f64);

        // Store metrics for API access
        *metrics_ref.write().unwrap() = Some(system_metrics);

        Ok(())
    }

    /// Collect business metrics from storage
    async fn collect_business_metrics(
        storage: &StorageManager,
        metrics_ref: &Arc<RwLock<Option<BusinessMetrics>>>,
    ) -> Result<()> {
        let agent_service = storage.agent_service();
        let issue_service = storage.issue_service();
        let message_service = storage.message_service();
        let knowledge_service = storage.knowledge_service();

        // Get agent metrics
        let agents = agent_service
            .list_agents()
            .await
            .map_err(|e| MonitoringError::Metrics(format!("Failed to get agents: {}", e)))?;

        let total_agents = agents.len() as u64;
        let active_agents = agents.iter().filter(|a| a.is_healthy()).count() as u64;

        // Get issue metrics
        let issues = issue_service
            .list_issues()
            .await
            .map_err(|e| MonitoringError::Metrics(format!("Failed to get issues: {}", e)))?;

        let total_issues = issues.len() as u64;
        let open_issues = issues.iter().filter(|i| i.is_open()).count() as u64;
        let closed_issues = total_issues - open_issues;

        // Calculate average resolution time
        let closed_with_resolution: Vec<_> = issues
            .iter()
            .filter(|i| i.is_closed() && i.resolved_at.is_some())
            .collect();

        let avg_resolution_time_hours = if !closed_with_resolution.is_empty() {
            let total_hours: i64 = closed_with_resolution
                .iter()
                .map(|issue| {
                    let resolution_time = issue.resolved_at.unwrap() - issue.created_at;
                    resolution_time.num_hours()
                })
                .sum();
            total_hours as f64 / closed_with_resolution.len() as f64
        } else {
            0.0
        };

        // Get message count (approximate - last hour)
        let recent_messages = message_service
            .list_recent_messages(chrono::Duration::hours(1))
            .await
            .map_err(|e| MonitoringError::Metrics(format!("Failed to get messages: {}", e)))?;
        let messages_sent = recent_messages.len() as u64;

        // Get knowledge items
        let knowledge_items = knowledge_service
            .list_knowledge_items()
            .await
            .map_err(|e| MonitoringError::Metrics(format!("Failed to get knowledge items: {}", e)))?
            .len() as u64;

        let business_metrics = BusinessMetrics {
            total_agents,
            active_agents,
            total_issues,
            open_issues,
            closed_issues,
            messages_sent,
            avg_resolution_time_hours,
            knowledge_items,
        };

        // Update Prometheus metrics
        metrics::gauge!("agents_total").set(total_agents as f64);
        metrics::gauge!("agents_active").set(active_agents as f64);
        metrics::gauge!("issues_total").set(total_issues as f64);
        metrics::gauge!("issues_open").set(open_issues as f64);
        metrics::gauge!("issues_closed").set(closed_issues as f64);
        metrics::counter!("messages_sent_total").increment(messages_sent);
        metrics::gauge!("knowledge_items_total").set(knowledge_items as f64);

        // Store metrics for API access
        *metrics_ref.write().unwrap() = Some(business_metrics);

        Ok(())
    }

    /// Get current system metrics
    pub fn get_system_metrics(&self) -> Option<SystemMetrics> {
        self.last_system_metrics.read().unwrap().clone()
    }

    /// Get current business metrics
    pub fn get_business_metrics(&self) -> Option<BusinessMetrics> {
        self.last_business_metrics.read().unwrap().clone()
    }

    /// Record HTTP request metrics
    pub fn record_http_request(&self, method: &str, path: &str, status: u16, duration: Duration) {
        let labels = [
            ("method", method),
            ("path", path),
            ("status", &status.to_string()),
        ];

        metrics::counter!("http_requests_total", &labels).increment(1);
        metrics::histogram!("http_request_duration_ms", &labels)
            .record(duration.as_millis() as f64);

        if status >= 400 {
            metrics::counter!("http_errors_total", &labels).increment(1);
        }
    }

    /// Record operation metrics
    pub fn record_operation(&self, operation: &str, duration: Duration, success: bool) {
        let labels = [
            ("operation", operation),
            ("success", if success { "true" } else { "false" }),
        ];

        metrics::counter!("operation_total", &labels).increment(1);
        metrics::histogram!("operation_duration_ms", &labels).record(duration.as_millis() as f64);

        if !success {
            metrics::counter!("operation_errors_total", &[("operation", operation)]).increment(1);
        }
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.collection_start.elapsed().as_secs()
    }
}

/// Helper functions for metrics collection
impl MetricsCollector {
    /// Create a timer for measuring operation duration
    pub fn start_timer(&self) -> MetricsTimer {
        MetricsTimer::new()
    }
}

/// Timer for measuring operation duration
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Stop the timer and return duration
    pub fn stop(self) -> Duration {
        self.start.elapsed()
    }

    /// Stop the timer and record metrics
    pub fn stop_and_record(self, operation: &str, success: bool) -> Duration {
        let duration = self.stop();

        // Record via global metrics (assuming MetricsCollector is globally available)
        let labels = [
            ("operation", operation),
            ("success", if success { "true" } else { "false" }),
        ];

        metrics::counter!("operation_total", &labels).increment(1);
        metrics::histogram!("operation_duration_ms", &labels).record(duration.as_millis() as f64);

        if !success {
            metrics::counter!("operation_errors_total", &[("operation", operation)]).increment(1);
        }

        duration
    }
}
