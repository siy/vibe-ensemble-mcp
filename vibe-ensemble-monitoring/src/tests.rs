//! Tests for monitoring and observability components

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{AlertingConfig, HealthConfig, MetricsConfig, MonitoringConfig, TracingConfig},
        health::{HealthCheck, HealthStatus},
        metrics::MetricsCollector,
        observability::{AlertSeverity, ObservabilityService},
        tracing_setup::TracingSetup,
    };
    use tokio;

    /// Test that monitoring configuration is created correctly
    #[test]
    fn test_monitoring_config_creation() {
        let config = MonitoringConfig::default();

        assert!(config.metrics.enabled);
        assert_eq!(config.metrics.port, 9090);
        assert!(config.tracing.enabled);
        assert!(config.health.enabled);
        assert!(config.alerting.enabled);
    }

    /// Test metrics collector initialization
    #[tokio::test]
    async fn test_metrics_collector_initialization() {
        let metrics_config = MetricsConfig {
            enabled: false, // Disable to avoid port conflicts
            host: "127.0.0.1".to_string(),
            port: 9091,
            collection_interval: 5,
            system_metrics: true,
            business_metrics: false,
        };

        let mut collector = MetricsCollector::new(metrics_config);
        let result = collector.initialize().await;

        // Should succeed even with disabled metrics
        assert!(result.is_ok());
    }

    /// Test tracing setup initialization
    #[tokio::test]
    async fn test_tracing_setup() {
        let tracing_config = TracingConfig {
            enabled: false, // Disable to avoid conflicts
            service_name: "test-service".to_string(),
            jaeger_endpoint: None,
            sampling_ratio: 1.0,
            max_spans: 1000,
            json_logs: false,
        };

        let mut tracing_setup = TracingSetup::new(tracing_config);
        let result = tracing_setup.initialize();

        // Should succeed when disabled
        assert!(result.is_ok());
    }

    /// Test health check creation and basic functionality
    #[tokio::test]
    async fn test_health_check_basic() {
        let health_config = HealthConfig {
            enabled: true,
            host: "127.0.0.1".to_string(),
            port: 8091,
            timeout: 5,
            readiness_enabled: true,
            liveness_enabled: true,
        };

        let health_check = HealthCheck::new(health_config);

        // Test liveness check (should always pass)
        let liveness_result = health_check.check_liveness().await;
        assert!(liveness_result.is_ok());

        let report = liveness_result.unwrap();
        assert_eq!(report.status, HealthStatus::Healthy);
        assert!(report.checks.contains_key("liveness"));
    }

    /// Test observability service initialization without storage
    #[tokio::test]
    async fn test_observability_service_no_storage() {
        let monitoring_config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false, // Disable to avoid conflicts
                host: "127.0.0.1".to_string(),
                port: 9092,
                collection_interval: 5,
                system_metrics: true,
                business_metrics: false,
            },
            tracing: TracingConfig {
                enabled: false, // Disable to avoid conflicts
                service_name: "test-service".to_string(),
                jaeger_endpoint: None,
                sampling_ratio: 1.0,
                max_spans: 1000,
                json_logs: false,
            },
            health: HealthConfig {
                enabled: false, // Disable to avoid conflicts
                host: "127.0.0.1".to_string(),
                port: 8092,
                timeout: 5,
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: false, // Disable to avoid conflicts
                error_rate_threshold: 5.0,
                response_time_threshold: 1000,
                memory_threshold: 85.0,
                cpu_threshold: 80.0,
                check_interval: 60,
            },
        };

        let mut observability = ObservabilityService::new(monitoring_config);
        let result = observability.initialize().await;

        // Should succeed with all components disabled
        assert!(result.is_ok());
    }

    /// Test alert creation and management
    #[tokio::test]
    async fn test_alert_management() {
        let monitoring_config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 9093,
                collection_interval: 5,
                system_metrics: false,
                business_metrics: false,
            },
            tracing: TracingConfig {
                enabled: false,
                service_name: "test-service".to_string(),
                jaeger_endpoint: None,
                sampling_ratio: 1.0,
                max_spans: 1000,
                json_logs: false,
            },
            health: HealthConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 8093,
                timeout: 5,
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: false,
                error_rate_threshold: 5.0,
                response_time_threshold: 1000,
                memory_threshold: 85.0,
                cpu_threshold: 80.0,
                check_interval: 60,
            },
        };

        let mut observability = ObservabilityService::new(monitoring_config);
        observability.initialize().await.unwrap();

        // Test error recording which should create an alert
        let mut context = std::collections::HashMap::new();
        context.insert(
            "component".to_string(),
            serde_json::Value::String("test".to_string()),
        );

        observability
            .record_error("Test error message", context)
            .await;

        // Check that alert was created
        let alerts = observability.get_alerts().await;
        assert!(!alerts.is_empty());

        let alert = &alerts[0];
        assert_eq!(alert.severity, AlertSeverity::Error);
        assert!(alert.message.contains("Test error message"));
        assert!(!alert.resolved);

        // Test alert resolution
        let alert_id = alert.id.clone();
        let result = observability.resolve_alert(&alert_id).await;
        assert!(result.is_ok());

        let alerts_after = observability.get_alerts().await;
        let resolved_alert = alerts_after.iter().find(|a| a.id == alert_id).unwrap();
        assert!(resolved_alert.resolved);
    }

    /// Test dashboard data collection
    #[tokio::test]
    async fn test_dashboard_data() {
        let monitoring_config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 9094,
                collection_interval: 5,
                system_metrics: false,
                business_metrics: false,
            },
            tracing: TracingConfig {
                enabled: false,
                service_name: "test-service".to_string(),
                jaeger_endpoint: None,
                sampling_ratio: 1.0,
                max_spans: 1000,
                json_logs: false,
            },
            health: HealthConfig {
                enabled: true, // Enable health checks for testing
                host: "127.0.0.1".to_string(),
                port: 8094,
                timeout: 5,
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: false,
                error_rate_threshold: 5.0,
                response_time_threshold: 1000,
                memory_threshold: 85.0,
                cpu_threshold: 80.0,
                check_interval: 60,
            },
        };

        let mut observability = ObservabilityService::new(monitoring_config);
        observability.initialize().await.unwrap();

        // Get dashboard data
        let dashboard = observability.get_dashboard().await;
        assert!(dashboard.is_ok());

        let dashboard_data = dashboard.unwrap();
        assert!(dashboard_data.uptime_seconds > 0);
        assert!(!dashboard_data.version.is_empty());

        // Health should be available since we enabled it
        assert_ne!(dashboard_data.health.status, HealthStatus::Unhealthy);
    }

    /// Test operation recording
    #[tokio::test]
    async fn test_operation_recording() {
        let monitoring_config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 9095,
                collection_interval: 5,
                system_metrics: false,
                business_metrics: false,
            },
            tracing: TracingConfig {
                enabled: false,
                service_name: "test-service".to_string(),
                jaeger_endpoint: None,
                sampling_ratio: 1.0,
                max_spans: 1000,
                json_logs: false,
            },
            health: HealthConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 8095,
                timeout: 5,
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: false,
                error_rate_threshold: 5.0,
                response_time_threshold: 1000,
                memory_threshold: 85.0,
                cpu_threshold: 80.0,
                check_interval: 60,
            },
        };

        let mut observability = ObservabilityService::new(monitoring_config);
        observability.initialize().await.unwrap();

        // Record some operations
        let duration = std::time::Duration::from_millis(100);
        observability
            .record_agent_operation("test-agent-1", "test-operation", duration, true)
            .await;
        observability
            .record_agent_operation("test-agent-2", "test-operation", duration, false)
            .await;

        // Operations should be recorded (we can't easily test metrics without a metrics backend)
        // But at least the function calls should not panic
    }

    /// Test configuration validation
    #[test]
    fn test_config_validation() {
        // Test that invalid configurations are handled properly
        let config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: true,
                host: "invalid-host".to_string(), // This should still work in our implementation
                port: 65536,                      // Invalid port, but u16 limits this
                collection_interval: 0,           // Could be problematic but we don't validate it
                system_metrics: true,
                business_metrics: true,
            },
            tracing: TracingConfig {
                enabled: true,
                service_name: "".to_string(), // Empty service name
                jaeger_endpoint: Some("invalid-endpoint".to_string()),
                sampling_ratio: 2.0, // Invalid sampling ratio > 1.0
                max_spans: 0,        // Zero max spans
                json_logs: false,
            },
            health: HealthConfig {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 8096,
                timeout: 0, // Zero timeout
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: true,
                error_rate_threshold: -1.0, // Negative threshold
                response_time_threshold: 0, // Zero threshold
                memory_threshold: 200.0,    // > 100%
                cpu_threshold: 200.0,       // > 100%
                check_interval: 0,          // Zero interval
            },
        };

        // The config should still be created (we don't validate in constructor)
        // In a production system, you'd want validation
        assert!(config.metrics.enabled);
        assert!(config.tracing.enabled);
        assert!(config.health.enabled);
        assert!(config.alerting.enabled);
    }

    /// Test graceful shutdown
    #[tokio::test]
    async fn test_graceful_shutdown() {
        let monitoring_config = MonitoringConfig {
            metrics: MetricsConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 9096,
                collection_interval: 5,
                system_metrics: false,
                business_metrics: false,
            },
            tracing: TracingConfig {
                enabled: false,
                service_name: "test-service".to_string(),
                jaeger_endpoint: None,
                sampling_ratio: 1.0,
                max_spans: 1000,
                json_logs: false,
            },
            health: HealthConfig {
                enabled: false,
                host: "127.0.0.1".to_string(),
                port: 8097,
                timeout: 5,
                readiness_enabled: true,
                liveness_enabled: true,
            },
            alerting: AlertingConfig {
                enabled: false,
                error_rate_threshold: 5.0,
                response_time_threshold: 1000,
                memory_threshold: 85.0,
                cpu_threshold: 80.0,
                check_interval: 60,
            },
        };

        let mut observability = ObservabilityService::new(monitoring_config);
        observability.initialize().await.unwrap();

        // Test shutdown (should not panic)
        observability.shutdown().await;
    }
}
