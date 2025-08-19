//! Error types for monitoring and observability

use thiserror::Error;

/// Result type for monitoring operations
pub type Result<T> = std::result::Result<T, MonitoringError>;

/// Monitoring and observability error types
#[derive(Error, Debug)]
pub enum MonitoringError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Metrics collection error
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Tracing setup error
    #[error("Tracing setup error: {0}")]
    Tracing(String),

    /// Health check error
    #[error("Health check error: {0}")]
    HealthCheck(String),

    /// OpenTelemetry error
    #[error("OpenTelemetry error: {0}")]
    OpenTelemetry(String),

    /// Server error
    #[error("Monitoring server error: {0}")]
    Server(String),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Anyhow error wrapper
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<opentelemetry::trace::TraceError> for MonitoringError {
    fn from(err: opentelemetry::trace::TraceError) -> Self {
        MonitoringError::OpenTelemetry(err.to_string())
    }
}

impl From<opentelemetry::metrics::MetricsError> for MonitoringError {
    fn from(err: opentelemetry::metrics::MetricsError) -> Self {
        MonitoringError::OpenTelemetry(err.to_string())
    }
}
