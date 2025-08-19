//! Tracing and logging setup with distributed tracing support

use crate::{config::TracingConfig, error::Result, MonitoringError};
use opentelemetry::{global, KeyValue};
use opentelemetry_sdk::{trace, Resource};
use std::collections::HashMap;
use tracing::{info, warn};
use tracing_subscriber::{
    fmt::format::FmtSpan, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry,
};

/// Tracing setup and management
pub struct TracingSetup {
    config: TracingConfig,
    _tracer: Option<opentelemetry_sdk::trace::Tracer>,
}

impl TracingSetup {
    /// Create new tracing setup
    pub fn new(config: TracingConfig) -> Self {
        Self {
            config,
            _tracer: None,
        }
    }

    /// Initialize distributed tracing with OpenTelemetry
    pub fn initialize(&mut self) -> Result<()> {
        if !self.config.enabled {
            info!("Tracing disabled in configuration");
            return Ok(());
        }

        info!("Initializing distributed tracing");

        // Create resource with service information
        let resource = Resource::new(vec![
            KeyValue::new("service.name", self.config.service_name.clone()),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
        ]);

        // Set up tracer provider
        let tracer_provider = if let Some(jaeger_endpoint) = &self.config.jaeger_endpoint {
            info!("Setting up Jaeger tracing to {}", jaeger_endpoint);
            self.setup_jaeger_tracer(resource, jaeger_endpoint)?
        } else {
            info!("Setting up console tracing (no Jaeger endpoint configured)");
            self.setup_console_tracer(resource)?
        };

        // Set global tracer provider
        global::set_tracer_provider(tracer_provider.clone());

        // Get tracer instance
        let tracer = tracer_provider.tracer(self.config.service_name.clone());
        self._tracer = Some(tracer);

        // Set up subscriber with tracing layers
        self.setup_subscriber()?;

        info!("Distributed tracing initialized successfully");
        Ok(())
    }

    /// Set up Jaeger tracer
    fn setup_jaeger_tracer(
        &self,
        resource: Resource,
        jaeger_endpoint: &str,
    ) -> Result<opentelemetry_sdk::trace::TracerProvider> {
        use opentelemetry_jaeger::JaegerPipeline;

        let tracer_provider = JaegerPipeline::new()
            .with_service_name(&self.config.service_name)
            .with_trace_config(
                trace::Config::default()
                    .with_sampler(trace::Sampler::TraceIdRatioBased(
                        self.config.sampling_ratio,
                    ))
                    .with_resource(resource),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .map_err(|e| MonitoringError::Tracing(format!("Failed to setup Jaeger: {}", e)))?;

        Ok(tracer_provider)
    }

    /// Set up console tracer for local development
    fn setup_console_tracer(
        &self,
        resource: Resource,
    ) -> Result<opentelemetry_sdk::trace::TracerProvider> {
        let tracer_provider = trace::TracerProvider::builder()
            .with_config(
                trace::Config::default()
                    .with_sampler(trace::Sampler::TraceIdRatioBased(
                        self.config.sampling_ratio,
                    ))
                    .with_resource(resource),
            )
            .build();

        Ok(tracer_provider)
    }

    /// Set up tracing subscriber with appropriate layers
    fn setup_subscriber(&self) -> Result<()> {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let registry = Registry::default().with(env_filter);

        if self.config.json_logs {
            // JSON formatted logs for production
            let json_layer = tracing_subscriber::fmt::layer()
                .json()
                .with_span_events(FmtSpan::CLOSE)
                .with_current_span(true)
                .with_span_list(true);

            registry
                .with(json_layer)
                .with(tracing_opentelemetry::layer())
                .try_init()
                .map_err(|e| {
                    MonitoringError::Tracing(format!("Failed to init subscriber: {}", e))
                })?;
        } else {
            // Human-readable logs for development
            let fmt_layer = tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_thread_ids(true)
                .with_span_events(FmtSpan::CLOSE);

            registry
                .with(fmt_layer)
                .with(tracing_opentelemetry::layer())
                .try_init()
                .map_err(|e| {
                    MonitoringError::Tracing(format!("Failed to init subscriber: {}", e))
                })?;
        }

        Ok(())
    }

    /// Create a trace context for multi-agent operations
    pub fn create_agent_trace_context(
        &self,
        agent_id: &str,
        operation: &str,
    ) -> HashMap<String, String> {
        let mut context = HashMap::new();

        // Add trace metadata
        context.insert("agent_id".to_string(), agent_id.to_string());
        context.insert("operation".to_string(), operation.to_string());
        context.insert("service".to_string(), self.config.service_name.clone());
        context.insert("timestamp".to_string(), chrono::Utc::now().to_rfc3339());

        // Add trace ID if available
        if let Some(span) = tracing::Span::current().id() {
            context.insert("span_id".to_string(), format!("{:?}", span));
        }

        context
    }

    /// Shutdown tracing gracefully
    pub async fn shutdown(&self) {
        if self.config.enabled {
            info!("Shutting down tracing");
            global::shutdown_tracer_provider();
        }
    }
}

impl Drop for TracingSetup {
    fn drop(&mut self) {
        if self.config.enabled {
            warn!("TracingSetup dropped - consider calling shutdown() explicitly");
        }
    }
}

/// Utility macro for creating traced spans with agent context
#[macro_export]
macro_rules! trace_agent_operation {
    ($agent_id:expr, $operation:expr, $($field:tt)*) => {
        tracing::info_span!(
            "agent_operation",
            agent_id = $agent_id,
            operation = $operation,
            $($field)*
        )
    };
}

/// Utility macro for timing operations
#[macro_export]
macro_rules! time_operation {
    ($name:expr, $block:expr) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let elapsed = start.elapsed();

        tracing::info!(
            operation = $name,
            duration_ms = elapsed.as_millis(),
            "Operation completed"
        );

        // Record metrics
        metrics::histogram!("operation_duration_ms").record(elapsed.as_millis() as f64);
        metrics::counter!("operation_total").increment(1);

        result
    }};
}
