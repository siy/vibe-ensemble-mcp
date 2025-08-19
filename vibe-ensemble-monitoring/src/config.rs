//! Configuration for monitoring and observability

use serde::{Deserialize, Serialize};

/// Complete monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Metrics collection configuration
    pub metrics: MetricsConfig,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// Health check configuration
    pub health: HealthConfig,
    /// Alerting configuration
    pub alerting: AlertingConfig,
}

/// Metrics collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Prometheus endpoint host
    pub host: String,
    /// Prometheus endpoint port
    pub port: u16,
    /// Metrics collection interval in seconds
    pub collection_interval: u64,
    /// Enable system metrics (CPU, memory, etc.)
    pub system_metrics: bool,
    /// Enable business metrics
    pub business_metrics: bool,
}

/// Tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable distributed tracing
    pub enabled: bool,
    /// Service name for tracing
    pub service_name: String,
    /// Jaeger endpoint URL
    pub jaeger_endpoint: Option<String>,
    /// Trace sampling ratio (0.0 to 1.0)
    pub sampling_ratio: f64,
    /// Maximum trace spans to keep in memory
    pub max_spans: usize,
    /// Enable JSON logging format
    pub json_logs: bool,
}

/// Health check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthConfig {
    /// Enable health checks
    pub enabled: bool,
    /// Health check endpoint host
    pub host: String,
    /// Health check endpoint port
    pub port: u16,
    /// Health check timeout in seconds
    pub timeout: u64,
    /// Enable readiness probes
    pub readiness_enabled: bool,
    /// Enable liveness probes
    pub liveness_enabled: bool,
}

/// Alerting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    /// Enable alerting
    pub enabled: bool,
    /// Error rate threshold (percentage)
    pub error_rate_threshold: f64,
    /// Response time threshold in milliseconds
    pub response_time_threshold: u64,
    /// Memory usage threshold (percentage)
    pub memory_threshold: f64,
    /// CPU usage threshold (percentage)
    pub cpu_threshold: f64,
    /// Alert check interval in seconds
    pub check_interval: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            metrics: MetricsConfig::default(),
            tracing: TracingConfig::default(),
            health: HealthConfig::default(),
            alerting: AlertingConfig::default(),
        }
    }
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 9090,
            collection_interval: 15,
            system_metrics: true,
            business_metrics: true,
        }
    }
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "vibe-ensemble-mcp".to_string(),
            jaeger_endpoint: None,
            sampling_ratio: 1.0,
            max_spans: 10000,
            json_logs: false,
        }
    }
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 8080,
            timeout: 30,
            readiness_enabled: true,
            liveness_enabled: true,
        }
    }
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            error_rate_threshold: 5.0,
            response_time_threshold: 1000,
            memory_threshold: 85.0,
            cpu_threshold: 80.0,
            check_interval: 60,
        }
    }
}
