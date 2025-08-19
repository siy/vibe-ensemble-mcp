//! Vibe Ensemble Monitoring and Observability
//!
//! This crate provides comprehensive monitoring and observability capabilities including:
//! - Structured logging with trace correlation
//! - Metrics collection with Prometheus compatibility
//! - Distributed tracing for multi-agent operations
//! - Performance monitoring and alerting
//! - Health checks and readiness probes
//! - Error tracking and aggregation
//! - Usage analytics and reporting

pub mod config;
pub mod error;
pub mod health;
pub mod metrics;
pub mod observability;
pub mod server;
pub mod tracing_setup;

#[cfg(test)]
mod tests;

pub use config::MonitoringConfig;
pub use error::{MonitoringError, Result};
pub use health::{HealthCheck, HealthStatus};
pub use metrics::MetricsCollector;
pub use observability::ObservabilityService;
pub use server::MonitoringServer;
pub use tracing_setup::TracingSetup;
