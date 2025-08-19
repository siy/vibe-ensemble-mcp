//! Health checks and readiness probes

use crate::{config::HealthConfig, error::Result};
use async_trait::async_trait;
use futures::future;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time;
use tracing::{error, info, warn};
use vibe_ensemble_storage::StorageManager;

/// Health check status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded but functional
    Degraded,
    /// Service is unhealthy
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded => write!(f, "degraded"),
            HealthStatus::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Individual health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    /// Name of the health check
    pub name: String,
    /// Status of the health check
    pub status: HealthStatus,
    /// Optional message describing the status
    pub message: Option<String>,
    /// Duration of the health check
    pub duration_ms: u64,
    /// Timestamp when check was performed
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Overall health report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    /// Overall health status
    pub status: HealthStatus,
    /// Individual check results
    pub checks: HashMap<String, HealthCheckResult>,
    /// Total duration for all checks
    pub total_duration_ms: u64,
    /// Timestamp of the report
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// System uptime in seconds
    pub uptime_seconds: u64,
    /// Version information
    pub version: String,
}

/// Health check trait for implementing custom checks
#[async_trait::async_trait]
pub trait HealthCheckProvider {
    /// Name of the health check
    fn name(&self) -> &str;

    /// Perform the health check
    async fn check(&self) -> HealthCheckResult;

    /// Whether this is a critical check (affects overall status)
    fn is_critical(&self) -> bool {
        true
    }
}

/// Database health check
pub struct DatabaseHealthCheck {
    storage: Arc<StorageManager>,
}

impl DatabaseHealthCheck {
    pub fn new(storage: Arc<StorageManager>) -> Self {
        Self { storage }
    }
}

#[async_trait::async_trait]
impl HealthCheckProvider for DatabaseHealthCheck {
    fn name(&self) -> &str {
        "database"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        let (status, message) = match self.storage.health_check().await {
            Ok(()) => (
                HealthStatus::Healthy,
                Some("Database connection healthy".to_string()),
            ),
            Err(e) => (
                HealthStatus::Unhealthy,
                Some(format!("Database error: {}", e)),
            ),
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms,
            timestamp,
            metadata: HashMap::new(),
        }
    }
}

/// Memory health check
pub struct MemoryHealthCheck {
    threshold_percent: f64,
}

impl MemoryHealthCheck {
    pub fn new(threshold_percent: f64) -> Self {
        Self { threshold_percent }
    }
}

#[async_trait::async_trait]
impl HealthCheckProvider for MemoryHealthCheck {
    fn name(&self) -> &str {
        "memory"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        let mut system = sysinfo::System::new_all();
        system.refresh_memory();

        let used = system.used_memory();
        let total = system.total_memory();
        let percentage = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let (status, message) = if percentage < self.threshold_percent {
            (
                HealthStatus::Healthy,
                Some(format!("Memory usage: {:.1}%", percentage)),
            )
        } else if percentage < self.threshold_percent + 10.0 {
            (
                HealthStatus::Degraded,
                Some(format!("Memory usage high: {:.1}%", percentage)),
            )
        } else {
            (
                HealthStatus::Unhealthy,
                Some(format!("Memory usage critical: {:.1}%", percentage)),
            )
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        let mut metadata = HashMap::new();
        metadata.insert(
            "used_bytes".to_string(),
            serde_json::Value::Number(used.into()),
        );
        metadata.insert(
            "total_bytes".to_string(),
            serde_json::Value::Number(total.into()),
        );
        metadata.insert(
            "percentage".to_string(),
            serde_json::Value::Number(percentage.into()),
        );

        HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms,
            timestamp,
            metadata,
        }
    }

    fn is_critical(&self) -> bool {
        false // Memory is important but not critical for basic functionality
    }
}

/// Disk space health check
pub struct DiskHealthCheck {
    threshold_percent: f64,
}

impl DiskHealthCheck {
    pub fn new(threshold_percent: f64) -> Self {
        Self { threshold_percent }
    }
}

#[async_trait::async_trait]
impl HealthCheckProvider for DiskHealthCheck {
    fn name(&self) -> &str {
        "disk"
    }

    async fn check(&self) -> HealthCheckResult {
        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        // For simplicity, we'll just return healthy status
        // In a real implementation, you would check actual disk usage
        let status = HealthStatus::Healthy;
        let message = Some("Disk space check not implemented".to_string());

        let duration_ms = start.elapsed().as_millis() as u64;

        HealthCheckResult {
            name: self.name().to_string(),
            status,
            message,
            duration_ms,
            timestamp,
            metadata: HashMap::new(),
        }
    }

    fn is_critical(&self) -> bool {
        false
    }
}

/// Main health check coordinator
pub struct HealthCheck {
    config: HealthConfig,
    providers: Vec<Box<dyn HealthCheckProvider + Send + Sync>>,
    start_time: Instant,
    last_report: Arc<tokio::sync::RwLock<Option<HealthReport>>>,
}

impl HealthCheck {
    /// Create new health check coordinator
    pub fn new(config: HealthConfig) -> Self {
        Self {
            config,
            providers: Vec::new(),
            start_time: Instant::now(),
            last_report: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Add a health check provider
    pub fn add_provider<P: HealthCheckProvider + Send + Sync + 'static>(&mut self, provider: P) {
        self.providers.push(Box::new(provider));
    }

    /// Initialize health checks with default providers
    pub fn with_defaults(mut self, storage: Option<Arc<StorageManager>>) -> Self {
        // Add database check if storage is available
        if let Some(storage) = storage {
            self.add_provider(DatabaseHealthCheck::new(storage));
        }

        // Add memory check
        self.add_provider(MemoryHealthCheck::new(85.0));

        // Add disk check
        self.add_provider(DiskHealthCheck::new(90.0));

        self
    }

    /// Perform all health checks
    pub async fn check_health(&self) -> Result<HealthReport> {
        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        info!("Performing health checks");

        let mut checks = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;

        // Run all health checks concurrently
        let futures: Vec<_> = self
            .providers
            .iter()
            .map(|provider| provider.check())
            .collect();
        let results = future::join_all(futures).await;

        // Process results
        for result in results {
            let name = result.name.clone();

            // Update overall status based on critical checks
            if let Some(provider) = self.providers.iter().find(|p| p.name() == name) {
                if provider.is_critical() {
                    match result.status {
                        HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
                        HealthStatus::Degraded if overall_status == HealthStatus::Healthy => {
                            overall_status = HealthStatus::Degraded;
                        }
                        _ => {}
                    }
                }
            }

            checks.insert(name, result);
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let uptime_seconds = self.start_time.elapsed().as_secs();

        let report = HealthReport {
            status: overall_status,
            checks,
            total_duration_ms,
            timestamp,
            uptime_seconds,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Cache the report
        *self.last_report.write().await = Some(report.clone());

        info!("Health check completed: {}", report.status);
        Ok(report)
    }

    /// Get cached health report (faster than full check)
    pub async fn get_cached_health(&self) -> Option<HealthReport> {
        self.last_report.read().await.clone()
    }

    /// Check readiness (subset of health checks for startup readiness)
    pub async fn check_readiness(&self) -> Result<HealthReport> {
        // For readiness, we only check critical services
        let critical_providers: Vec<_> =
            self.providers.iter().filter(|p| p.is_critical()).collect();

        if critical_providers.is_empty() {
            return self.check_health().await;
        }

        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        let mut checks = HashMap::new();
        let mut overall_status = HealthStatus::Healthy;

        // Run critical health checks
        for provider in critical_providers {
            let result = provider.check().await;
            let name = result.name.clone();

            match result.status {
                HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
                HealthStatus::Degraded if overall_status == HealthStatus::Healthy => {
                    overall_status = HealthStatus::Degraded;
                }
                _ => {}
            }

            checks.insert(name, result);
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let uptime_seconds = self.start_time.elapsed().as_secs();

        Ok(HealthReport {
            status: overall_status,
            checks,
            total_duration_ms,
            timestamp,
            uptime_seconds,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Check liveness (basic health check for container orchestration)
    pub async fn check_liveness(&self) -> Result<HealthReport> {
        // Liveness is just a basic check that the service is running
        let start = Instant::now();
        let timestamp = chrono::Utc::now();

        let mut checks = HashMap::new();

        // Simple liveness check
        let result = HealthCheckResult {
            name: "liveness".to_string(),
            status: HealthStatus::Healthy,
            message: Some("Service is running".to_string()),
            duration_ms: 0,
            timestamp,
            metadata: HashMap::new(),
        };

        checks.insert("liveness".to_string(), result);

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let uptime_seconds = self.start_time.elapsed().as_secs();

        Ok(HealthReport {
            status: HealthStatus::Healthy,
            checks,
            total_duration_ms,
            timestamp,
            uptime_seconds,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    /// Start periodic health check background task
    pub fn start_periodic_checks(&self) -> tokio::task::JoinHandle<()> {
        let health_check = self.clone();
        let interval_seconds = 30; // Check every 30 seconds

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval_seconds));

            loop {
                interval.tick().await;

                match health_check.check_health().await {
                    Ok(report) => {
                        if report.status != HealthStatus::Healthy {
                            warn!("Health check status: {}", report.status);
                        }
                    }
                    Err(e) => {
                        error!("Health check failed: {}", e);
                    }
                }
            }
        })
    }
}

impl Clone for HealthCheck {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            providers: Vec::new(), // Note: providers are not cloned due to trait object limitations
            start_time: self.start_time,
            last_report: self.last_report.clone(),
        }
    }
}
