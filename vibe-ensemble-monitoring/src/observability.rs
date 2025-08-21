//! Comprehensive observability service combining all monitoring components

use crate::{
    config::MonitoringConfig,
    error::Result,
    health::{HealthCheck, HealthReport, HealthStatus},
    metrics::{BusinessMetrics, MetricsCollector, SystemMetrics},
    tracing_setup::TracingSetup,
    MonitoringError,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use vibe_ensemble_core; // Import core types as needed
use vibe_ensemble_storage::StorageManager;

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Alert condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub severity: AlertSeverity,
    pub title: String,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub source: String,
    pub resolved: bool,
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Usage analytics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageAnalytics {
    /// Agent activity patterns
    pub agent_activity: HashMap<String, u64>,
    /// Issue creation patterns (by hour)
    pub issue_creation_patterns: HashMap<u8, u64>,
    /// Most active knowledge areas
    pub knowledge_usage: HashMap<String, u64>,
    /// Performance trends
    pub performance_trends: Vec<PerformanceTrend>,
    /// Error patterns
    pub error_patterns: HashMap<String, u64>,
}

/// Performance trend data point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceTrend {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub avg_response_time_ms: f64,
    pub requests_per_minute: f64,
    pub error_rate_percent: f64,
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
}

/// Comprehensive observability dashboard data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityDashboard {
    pub health: HealthReport,
    pub system_metrics: Option<SystemMetrics>,
    pub business_metrics: Option<BusinessMetrics>,
    pub active_alerts: Vec<Alert>,
    pub usage_analytics: UsageAnalytics,
    pub uptime_seconds: u64,
    pub version: String,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Main observability service coordinating all monitoring components
pub struct ObservabilityService {
    config: MonitoringConfig,
    tracing_setup: Option<TracingSetup>,
    metrics_collector: Option<MetricsCollector>,
    health_check: Option<HealthCheck>,
    storage: Option<Arc<StorageManager>>,
    start_time: Instant,
    alerts: Arc<RwLock<Vec<Alert>>>,
    performance_history: Arc<RwLock<Vec<PerformanceTrend>>>,
}

impl ObservabilityService {
    /// Create new observability service
    pub fn new(config: MonitoringConfig) -> Self {
        Self {
            config,
            tracing_setup: None,
            metrics_collector: None,
            health_check: None,
            storage: None,
            start_time: Instant::now(),
            alerts: Arc::new(RwLock::new(Vec::new())),
            performance_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Set storage manager for business metrics and analytics
    pub fn with_storage(mut self, storage: Arc<StorageManager>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Initialize all observability components
    #[instrument(skip(self))]
    pub async fn initialize(&mut self) -> Result<()> {
        info!("Initializing observability service");

        // Initialize tracing
        if self.config.tracing.enabled {
            let mut tracing_setup = TracingSetup::new(self.config.tracing.clone());
            tracing_setup.initialize()?;
            self.tracing_setup = Some(tracing_setup);
            info!("Tracing initialized");
        }

        // Initialize metrics collection
        if self.config.metrics.enabled {
            let mut metrics_collector = if let Some(storage) = &self.storage {
                MetricsCollector::new(self.config.metrics.clone()).with_storage(storage.clone())
            } else {
                MetricsCollector::new(self.config.metrics.clone())
            };

            metrics_collector.initialize().await?;
            self.metrics_collector = Some(metrics_collector);
            info!("Metrics collection initialized");
        }

        // Initialize health checks
        if self.config.health.enabled {
            let health_check =
                HealthCheck::new(self.config.health.clone()).with_defaults(self.storage.clone());

            // Start periodic health checks
            let _handle = health_check.start_periodic_checks();

            self.health_check = Some(health_check);
            info!("Health checks initialized");
        }

        // Start alerting if enabled
        if self.config.alerting.enabled {
            self.start_alerting_system().await;
            info!("Alerting system initialized");
        }

        // Start analytics collection
        self.start_analytics_collection().await;

        info!("Observability service initialized successfully");
        Ok(())
    }

    /// Start alerting system background task
    async fn start_alerting_system(&self) {
        let config = self.config.alerting.clone();
        let metrics_collector = self
            .metrics_collector
            .as_ref()
            .map(|m| Arc::new(std::ptr::addr_of!(*m)));
        let alerts = self.alerts.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(config.check_interval));

            loop {
                interval.tick().await;

                // Check various alert conditions
                let mut new_alerts = Vec::new();

                // Memory usage alert
                if let Some(system_metrics) = metrics_collector
                    .as_ref()
                    .and_then(|m| unsafe { m.as_ref() }.map(|m| m.get_system_metrics()))
                    .flatten()
                {
                    if system_metrics.memory_percentage > config.memory_threshold {
                        new_alerts.push(Alert {
                            id: format!("memory-{}", chrono::Utc::now().timestamp()),
                            severity: AlertSeverity::Warning,
                            title: "High Memory Usage".to_string(),
                            message: format!(
                                "Memory usage is {:.1}%",
                                system_metrics.memory_percentage
                            ),
                            timestamp: chrono::Utc::now(),
                            source: "system-monitor".to_string(),
                            resolved: false,
                            metadata: {
                                let mut map = HashMap::new();
                                map.insert(
                                    "memory_percent".to_string(),
                                    system_metrics.memory_percentage.into(),
                                );
                                map.insert("threshold".to_string(), config.memory_threshold.into());
                                map
                            },
                        });
                    }

                    // CPU usage alert
                    if system_metrics.cpu_usage > config.cpu_threshold {
                        new_alerts.push(Alert {
                            id: format!("cpu-{}", chrono::Utc::now().timestamp()),
                            severity: AlertSeverity::Warning,
                            title: "High CPU Usage".to_string(),
                            message: format!("CPU usage is {:.1}%", system_metrics.cpu_usage),
                            timestamp: chrono::Utc::now(),
                            source: "system-monitor".to_string(),
                            resolved: false,
                            metadata: {
                                let mut map = HashMap::new();
                                map.insert(
                                    "cpu_percent".to_string(),
                                    system_metrics.cpu_usage.into(),
                                );
                                map.insert("threshold".to_string(), config.cpu_threshold.into());
                                map
                            },
                        });
                    }
                }

                // Add new alerts
                if !new_alerts.is_empty() {
                    let mut alerts_lock = alerts.write().await;
                    for alert in new_alerts {
                        warn!("Alert triggered: {} - {}", alert.title, alert.message);
                        alerts_lock.push(alert);
                    }

                    // Keep only last 100 alerts to prevent memory growth
                    if alerts_lock.len() > 100 {
                        alerts_lock.drain(0..(alerts_lock.len() - 100));
                    }
                }
            }
        });
    }

    /// Start analytics collection background task
    async fn start_analytics_collection(&self) {
        let storage = self.storage.clone();
        let performance_history = self.performance_history.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes

            loop {
                interval.tick().await;

                if let Some(storage) = &storage {
                    // Collect performance trends
                    let trend = PerformanceTrend {
                        timestamp: chrono::Utc::now(),
                        avg_response_time_ms: 0.0, // TODO: Calculate from metrics
                        requests_per_minute: 0.0,  // TODO: Calculate from metrics
                        error_rate_percent: 0.0,   // TODO: Calculate from metrics
                        cpu_usage_percent: 0.0,    // TODO: Get from system metrics
                        memory_usage_percent: 0.0, // TODO: Get from system metrics
                    };

                    let mut history = performance_history.write().await;
                    history.push(trend);

                    // Keep only last 24 hours of data (288 data points at 5-minute intervals)
                    if history.len() > 288 {
                        history.drain(0..(history.len() - 288));
                    }
                }
            }
        });
    }

    /// Get comprehensive observability dashboard data
    #[instrument(skip(self))]
    pub async fn get_dashboard(&self) -> Result<ObservabilityDashboard> {
        let health = if let Some(health_check) = &self.health_check {
            health_check.get_cached_health().await.unwrap_or_else(|| {
                // Return a basic health report if cached one is not available
                use crate::health::HealthCheckResult;
                let mut checks = HashMap::new();
                checks.insert(
                    "basic".to_string(),
                    HealthCheckResult {
                        name: "basic".to_string(),
                        status: HealthStatus::Healthy,
                        message: Some("Service running".to_string()),
                        duration_ms: 0,
                        timestamp: chrono::Utc::now(),
                        metadata: HashMap::new(),
                    },
                );

                crate::health::HealthReport {
                    status: HealthStatus::Healthy,
                    checks,
                    total_duration_ms: 0,
                    timestamp: chrono::Utc::now(),
                    uptime_seconds: self.start_time.elapsed().as_secs(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                }
            })
        } else {
            return Err(MonitoringError::Config(
                "Health check not initialized".to_string(),
            ));
        };

        let system_metrics = self
            .metrics_collector
            .as_ref()
            .and_then(|m| m.get_system_metrics());
        let business_metrics = self
            .metrics_collector
            .as_ref()
            .and_then(|m| m.get_business_metrics());

        let active_alerts = self
            .alerts
            .read()
            .await
            .iter()
            .filter(|alert| !alert.resolved)
            .cloned()
            .collect();

        let usage_analytics = self.collect_usage_analytics().await?;

        Ok(ObservabilityDashboard {
            health,
            system_metrics,
            business_metrics,
            active_alerts,
            usage_analytics,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            last_updated: chrono::Utc::now(),
        })
    }

    /// Collect usage analytics
    async fn collect_usage_analytics(&self) -> Result<UsageAnalytics> {
        if let Some(storage) = &self.storage {
            // Get agent activity
            let agents =
                storage.agent_service().list_agents().await.map_err(|e| {
                    MonitoringError::Metrics(format!("Failed to get agents: {}", e))
                })?;

            let agent_activity: HashMap<String, u64> = agents
                .into_iter()
                .map(|agent| (agent.id.to_string(), 1)) // Simplified - would track actual activity
                .collect();

            // Get issue creation patterns (by hour of day)
            let issues =
                storage.issue_service().list_issues().await.map_err(|e| {
                    MonitoringError::Metrics(format!("Failed to get issues: {}", e))
                })?;

            let mut issue_creation_patterns = HashMap::new();
            for issue in issues {
                let hour = issue.created_at.hour() as u8;
                *issue_creation_patterns.entry(hour).or_insert(0) += 1;
            }

            // Get knowledge usage
            let knowledge_items = storage
                .knowledge_service()
                .list_knowledge_items()
                .await
                .map_err(|e| MonitoringError::Metrics(format!("Failed to get knowledge: {}", e)))?;

            let knowledge_usage: HashMap<String, u64> = knowledge_items
                .into_iter()
                .map(|item| (item.title, 1)) // Simplified - would track actual usage
                .collect();

            // Get performance trends
            let performance_trends = self.performance_history.read().await.clone();

            // Simple error patterns (in a real system, this would be more sophisticated)
            let error_patterns = HashMap::new();

            Ok(UsageAnalytics {
                agent_activity,
                issue_creation_patterns,
                knowledge_usage,
                performance_trends,
                error_patterns,
            })
        } else {
            // Return empty analytics if no storage
            Ok(UsageAnalytics {
                agent_activity: HashMap::new(),
                issue_creation_patterns: HashMap::new(),
                knowledge_usage: HashMap::new(),
                performance_trends: Vec::new(),
                error_patterns: HashMap::new(),
            })
        }
    }

    /// Record an agent operation for tracing and metrics
    #[instrument(skip(self))]
    pub async fn record_agent_operation(
        &self,
        agent_id: &str,
        operation: &str,
        duration: Duration,
        success: bool,
    ) {
        // Record metrics if available
        if let Some(metrics) = &self.metrics_collector {
            metrics.record_operation(operation, duration, success);
        }

        // Add tracing context
        tracing::info!(
            agent_id = agent_id,
            operation = operation,
            duration_ms = duration.as_millis(),
            success = success,
            "Agent operation completed"
        );
    }

    /// Record an error for tracking and alerting
    #[instrument(skip(self, error))]
    pub async fn record_error(&self, error: &str, context: HashMap<String, serde_json::Value>) {
        error!(error = error, context = ?context, "Error recorded");

        // Create alert for critical errors
        let alert = Alert {
            id: format!("error-{}", chrono::Utc::now().timestamp()),
            severity: AlertSeverity::Error,
            title: "Error Detected".to_string(),
            message: error.to_string(),
            timestamp: chrono::Utc::now(),
            source: "error-tracker".to_string(),
            resolved: false,
            metadata: context,
        };

        self.alerts.write().await.push(alert);
    }

    /// Get current alerts
    pub async fn get_alerts(&self) -> Vec<Alert> {
        self.alerts.read().await.clone()
    }

    /// Resolve an alert
    pub async fn resolve_alert(&self, alert_id: &str) -> Result<()> {
        let mut alerts = self.alerts.write().await;
        if let Some(alert) = alerts.iter_mut().find(|a| a.id == alert_id) {
            alert.resolved = true;
            info!("Alert {} resolved", alert_id);
            Ok(())
        } else {
            Err(MonitoringError::Config(format!(
                "Alert {} not found",
                alert_id
            )))
        }
    }

    /// Graceful shutdown of all observability components
    #[instrument(skip(self))]
    pub async fn shutdown(&self) {
        info!("Shutting down observability service");

        if let Some(tracing_setup) = &self.tracing_setup {
            tracing_setup.shutdown().await;
        }

        info!("Observability service shutdown complete");
    }
}
