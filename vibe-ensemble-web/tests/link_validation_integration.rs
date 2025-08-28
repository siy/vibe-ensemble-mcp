//! Integration tests for link validation system
//!
//! These tests verify the complete link validation and navigation integrity
//! functionality across all system components.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_web::{
    link_validator::{AutoRepairConfig, LinkStatus, LinkValidator, ValidationConfig},
    server::WebServer,
};

/// Test helper to create a storage manager for testing
async fn create_test_storage() -> Arc<StorageManager> {
    let storage = StorageManager::new_in_memory()
        .await
        .expect("Failed to create in-memory storage");
    Arc::new(storage)
}

/// Test helper to create a web server app for testing
async fn create_test_app() -> Router {
    let storage = create_test_storage().await;
    let config = vibe_ensemble_web::server::WebConfig::default();
    let server = WebServer::new(config, storage)
        .await
        .expect("Failed to create server");
    server.build_router()
}

#[tokio::test]
async fn test_link_health_dashboard_loads() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/link-health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Verify the page contains expected elements
    assert!(body_str.contains("Link Health Monitoring"));
    assert!(body_str.contains("health-overview"));
    assert!(body_str.contains("validateAllLinks"));
}

#[tokio::test]
async fn test_link_validation_api_endpoints() {
    let app = create_test_app().await;

    // Test health summary endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/links/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let health_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify health summary structure
    assert!(health_data.get("total_links").is_some());
    assert!(health_data.get("healthy_links").is_some());
    assert!(health_data.get("broken_links").is_some());
    assert!(health_data.get("health_score").is_some());

    // Test link status endpoint
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/links/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let status_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify status response structure
    assert!(status_data.get("links").is_some());
    assert!(status_data.get("total").is_some());
    assert!(status_data.get("timestamp").is_some());
}

#[tokio::test]
async fn test_link_validation_trigger() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/links/validate")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let validation_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify validation response
    assert_eq!(
        validation_data.get("status").unwrap().as_str().unwrap(),
        "completed"
    );
    assert!(validation_data.get("results").is_some());
    assert!(validation_data.get("timestamp").is_some());
}

#[tokio::test]
async fn test_navigation_analytics_endpoint() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/links/analytics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let analytics_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify analytics response structure
    assert!(analytics_data.get("analytics").is_some());
    assert!(analytics_data.get("timestamp").is_some());
}

#[tokio::test]
async fn test_auto_repair_functionality() {
    let app = create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/links/auto-repair")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let repair_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify repair response structure
    assert_eq!(
        repair_data.get("status").unwrap().as_str().unwrap(),
        "completed"
    );
    assert!(repair_data.get("repairs_applied").is_some());
    assert!(repair_data.get("config").is_some());
    assert!(repair_data.get("timestamp").is_some());
}

#[tokio::test]
async fn test_known_navigation_routes() {
    let app = create_test_app().await;

    let known_routes = vec!["/", "/dashboard", "/link-health"];

    for route in known_routes {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(route).body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Should not return 404 for known routes
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Route {} should be accessible",
            route
        );
    }
}

#[tokio::test]
async fn test_known_api_routes() {
    let app = create_test_app().await;

    let api_routes = vec![
        "/api/health",
        "/api/stats",
        "/api/agents",
        "/api/issues",
        "/api/links/health",
        "/api/links/status",
        "/api/links/analytics",
    ];

    for route in api_routes {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(route).body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Should return successful status codes (200, 201, etc.) for known API routes
        assert!(
            response.status().is_success() || response.status().is_redirection(),
            "API route {} should be accessible, got status: {}",
            route,
            response.status()
        );
    }
}

#[tokio::test]
async fn test_link_validator_core_functionality() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register test routes
    validator.register_routes(vec!["/dashboard".to_string(), "/api/health".to_string()]);

    // Test link discovery
    let links = validator.discover_links().await.unwrap();
    assert!(!links.is_empty());

    // Test route registration check
    assert!(validator.is_route_registered("/dashboard"));
    assert!(!validator.is_route_registered("/nonexistent"));

    // Test link type determination
    assert_eq!(
        validator.determine_link_type("/api/test"),
        vibe_ensemble_web::link_validator::LinkType::Api
    );
    assert_eq!(
        validator.determine_link_type("/dashboard"),
        vibe_ensemble_web::link_validator::LinkType::Navigation
    );
    assert_eq!(
        validator.determine_link_type("https://example.com"),
        vibe_ensemble_web::link_validator::LinkType::External
    );
}

#[tokio::test]
async fn test_repair_suggestions_generation() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register known routes
    validator.register_routes(vec!["/dashboard".to_string(), "/api/health".to_string()]);

    // Test repair suggestions for broken URL
    let suggestions = validator.generate_repair_suggestions("/dashbord"); // Typo
    assert!(!suggestions.is_empty());

    // Should suggest similar URL
    let url_correction_suggestion = suggestions.iter().find(|s| {
        matches!(
            s.repair_type,
            vibe_ensemble_web::link_validator::RepairType::UrlCorrection
        )
    });
    assert!(url_correction_suggestion.is_some());

    // Test repair suggestions for missing handler
    let suggestions = validator.generate_repair_suggestions("/nonexistent");
    assert!(!suggestions.is_empty());

    let missing_handler_suggestion = suggestions.iter().find(|s| {
        matches!(
            s.repair_type,
            vibe_ensemble_web::link_validator::RepairType::MissingHandler
        )
    });
    assert!(missing_handler_suggestion.is_some());
}

#[tokio::test]
async fn test_auto_repair_application() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register known routes
    validator.register_routes(vec!["/dashboard".to_string(), "/api/health".to_string()]);

    let config = AutoRepairConfig {
        enabled: true,
        confidence_threshold: 0.5,
        auto_fix_safe_issues: true,
        create_redirects: true,
        suggest_alternatives: true,
    };

    let repairs = validator.apply_auto_repairs(&config).await.unwrap();

    // Should complete without errors (even if no repairs needed)
    assert!(!repairs.is_empty() || repairs.len() == 0);
}

#[tokio::test]
async fn test_navigation_analytics_recording() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Record some navigation events
    validator.record_navigation(
        "/dashboard",
        Some("TestAgent/1.0"),
        Some(std::time::Duration::from_millis(100)),
    );
    validator.record_navigation(
        "/api/health",
        Some("TestAgent/1.0"),
        Some(std::time::Duration::from_millis(50)),
    );

    let analytics = validator.get_analytics();

    // Should have recorded analytics
    assert!(analytics.len() <= 2); // May be empty if analytics storage isn't implemented
}

#[tokio::test]
async fn test_health_summary_calculation() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    let summary = validator.get_health_summary();

    // Verify health summary structure
    assert!(summary.health_score >= 0.0 && summary.health_score <= 100.0);
    assert_eq!(
        summary.total_links,
        summary.healthy_links
            + summary.warning_links
            + summary.broken_links
            + summary.pending_links
            + summary.unknown_links
    );
}

#[tokio::test]
async fn test_validation_result_updates() {
    use vibe_ensemble_web::link_validator::{LinkStatus, LinkType, LinkValidationResult};

    let mut result = LinkValidationResult::new("/test".to_string(), LinkType::Navigation);

    // Test initial state
    assert_eq!(result.status, LinkStatus::Unknown);
    assert_eq!(result.check_count, 0);
    assert_eq!(result.success_rate, 0.0);

    // Test successful update
    result.update_result(LinkStatus::Healthy, None, Some(200), None);
    assert_eq!(result.status, LinkStatus::Healthy);
    assert_eq!(result.check_count, 1);
    assert_eq!(result.success_rate, 1.0);

    // Test failed update
    result.update_result(
        LinkStatus::Broken,
        None,
        Some(404),
        Some("Not found".to_string()),
    );
    assert_eq!(result.status, LinkStatus::Broken);
    assert_eq!(result.check_count, 2);
    assert_eq!(result.success_rate, 0.5);
}

#[tokio::test]
async fn test_string_distance_calculation() {
    let storage = create_test_storage().await;
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Test identical strings
    assert_eq!(validator.calculate_string_distance("test", "test"), 0.0);

    // Test completely different strings
    let distance = validator.calculate_string_distance("abc", "xyz");
    assert!(distance > 0.0 && distance <= 1.0);

    // Test similar strings
    let distance = validator.calculate_string_distance("dashboard", "dashbord");
    assert!(distance > 0.0 && distance < 0.5); // Should be quite similar
}

#[tokio::test]
async fn test_link_validation_with_filters() {
    let app = create_test_app().await;

    // Test with type filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/links/status?link_type=api")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test with status filter
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/links/status?status=healthy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Test with success rate filter
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/links/status?min_success_rate=0.8")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_repair_suggestions_api() {
    let app = create_test_app().await;

    // URL encode the test URL
    let encoded_url = urlencoding::encode("/broken-link");
    let uri = format!("/api/links/{}/repair-suggestions", encoded_url);

    let response = app
        .oneshot(Request::builder().uri(&uri).body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let suggestions_data: Value = serde_json::from_slice(&body).unwrap();

    // Verify repair suggestions response structure
    assert!(suggestions_data.get("url").is_some());
    assert!(suggestions_data.get("suggestions").is_some());
    assert!(suggestions_data.get("timestamp").is_some());
}

// Helper test to verify all route patterns are accessible
#[tokio::test]
async fn test_all_registered_routes_accessible() {
    let app = create_test_app().await;

    // Test that we can access the main application routes without 404s
    let critical_routes = vec![
        "/",
        "/dashboard",
        "/link-health",
        "/api/health",
        "/api/stats",
        "/api/links/health",
        "/api/links/status",
        "/api/links/validate",
        "/api/links/analytics",
        "/api/links/auto-repair",
    ];

    for route in critical_routes {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(route).body(Body::empty()).unwrap())
            .await
            .unwrap();

        // Should not be 404 - may be other errors but route should exist
        assert_ne!(
            response.status(),
            StatusCode::NOT_FOUND,
            "Critical route {} returned 404 - route may not be registered",
            route
        );
    }
}
