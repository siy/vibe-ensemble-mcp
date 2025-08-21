//! HTTP server for monitoring endpoints

use crate::{
    health::{HealthReport, HealthStatus},
    metrics::{BusinessMetrics, SystemMetrics},
    observability::{Alert, ObservabilityDashboard, ObservabilityService, UsageAnalytics},
    MonitoringError, Result,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, instrument};

/// Monitoring server for exposing metrics and health endpoints
pub struct MonitoringServer {
    observability: Arc<ObservabilityService>,
    host: String,
    port: u16,
}

impl MonitoringServer {
    /// Create new monitoring server
    pub fn new(observability: Arc<ObservabilityService>, host: String, port: u16) -> Self {
        Self {
            observability,
            host,
            port,
        }
    }

    /// Build the router with all monitoring endpoints
    fn build_router(&self) -> Router {
        Router::new()
            // Health check endpoints
            .route("/health", get(health_check))
            .route("/health/live", get(liveness_check))
            .route("/health/ready", get(readiness_check))
            // Metrics endpoints
            .route("/metrics", get(prometheus_metrics))
            .route("/api/metrics/system", get(system_metrics))
            .route("/api/metrics/business", get(business_metrics))
            // Dashboard and analytics
            .route("/api/dashboard", get(dashboard))
            .route("/api/analytics", get(analytics))
            // Alerts
            .route("/api/alerts", get(list_alerts))
            .route("/api/alerts/:id/resolve", post(resolve_alert))
            // Version and info
            .route("/api/info", get(service_info))
            // Debugging endpoints
            .route("/debug/config", get(debug_config))
            .with_state(self.observability.clone())
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
    }

    /// Run the monitoring server
    #[instrument(skip(self))]
    pub async fn run(self) -> Result<()> {
        let app = self.build_router();
        let addr = format!("{}:{}", self.host, self.port);

        info!("Starting monitoring server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| MonitoringError::Server(format!("Failed to bind to {}: {}", addr, e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| MonitoringError::Server(format!("Server error: {}", e)))?;

        Ok(())
    }
}

/// Health check endpoint - returns overall health status
#[instrument(skip(observability))]
async fn health_check(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<HealthReport>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => {
            let status_code = match dashboard.health.status {
                HealthStatus::Healthy => StatusCode::OK,
                HealthStatus::Degraded => StatusCode::OK, // Still OK but with warnings
                HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
            };

            // Return with appropriate status code
            match status_code {
                StatusCode::OK => Ok(Json(dashboard.health)),
                _ => Err(status_code),
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Liveness probe - basic check that service is running
#[instrument(skip(observability))]
async fn liveness_check(State(observability): State<Arc<ObservabilityService>>) -> Json<Value> {
    Json(json!({
        "status": "alive",
        "timestamp": chrono::Utc::now(),
        "uptime_seconds": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }))
}

/// Readiness probe - checks if service is ready to accept traffic
#[instrument(skip(observability))]
async fn readiness_check(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<Value>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => {
            let ready = dashboard.health.status != HealthStatus::Unhealthy;

            if ready {
                Ok(Json(json!({
                    "status": "ready",
                    "timestamp": chrono::Utc::now(),
                    "checks": dashboard.health.checks
                })))
            } else {
                Err(StatusCode::SERVICE_UNAVAILABLE)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Prometheus metrics endpoint
#[instrument]
async fn prometheus_metrics() -> String {
    // In a real implementation, this would return Prometheus formatted metrics
    // For now, we'll return a placeholder
    format!(
        "# HELP vibe_ensemble_up Service up indicator\n\
         # TYPE vibe_ensemble_up gauge\n\
         vibe_ensemble_up 1\n\
         # HELP vibe_ensemble_uptime_seconds Service uptime in seconds\n\
         # TYPE vibe_ensemble_uptime_seconds counter\n\
         vibe_ensemble_uptime_seconds {}\n",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    )
}

/// System metrics API endpoint
#[instrument(skip(observability))]
async fn system_metrics(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<SystemMetrics>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => {
            if let Some(metrics) = dashboard.system_metrics {
                Ok(Json(metrics))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Business metrics API endpoint
#[instrument(skip(observability))]
async fn business_metrics(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<BusinessMetrics>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => {
            if let Some(metrics) = dashboard.business_metrics {
                Ok(Json(metrics))
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Dashboard data endpoint
#[instrument(skip(observability))]
async fn dashboard(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<ObservabilityDashboard>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => Ok(Json(dashboard)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// Analytics endpoint
#[instrument(skip(observability))]
async fn analytics(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<UsageAnalytics>, StatusCode> {
    match observability.get_dashboard().await {
        Ok(dashboard) => Ok(Json(dashboard.usage_analytics)),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

/// List alerts endpoint
#[instrument(skip(observability))]
async fn list_alerts(
    State(observability): State<Arc<ObservabilityService>>,
) -> Result<Json<Vec<Alert>>, StatusCode> {
    let alerts = observability.get_alerts().await;
    Ok(Json(alerts))
}

/// Resolve alert endpoint
#[instrument(skip(observability))]
async fn resolve_alert(
    State(observability): State<Arc<ObservabilityService>>,
    Path(alert_id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    match observability.resolve_alert(&alert_id).await {
        Ok(()) => Ok(Json(json!({
            "status": "resolved",
            "alert_id": alert_id,
            "timestamp": chrono::Utc::now()
        }))),
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

/// Service information endpoint
#[instrument]
async fn service_info() -> Json<Value> {
    Json(json!({
        "name": "vibe-ensemble-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
        "authors": env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<_>>(),
        "repository": env!("CARGO_PKG_REPOSITORY"),
        "rust_version": env!("CARGO_PKG_RUST_VERSION"),
        "build_timestamp": chrono::Utc::now(),
        "features": {
            "tracing": true,
            "metrics": true,
            "health_checks": true,
            "alerting": true,
            "analytics": true
        }
    }))
}

/// Debug configuration endpoint (be careful about sensitive data)
#[instrument]
async fn debug_config() -> Json<Value> {
    Json(json!({
        "environment": std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()),
        "log_level": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        "features": {
            "system_metrics": cfg!(feature = "system-metrics"),
        },
        "timestamp": chrono::Utc::now()
    }))
}
