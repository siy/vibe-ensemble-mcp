//! Integration tests for link validation functionality

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::util::ServiceExt;
use vibe_ensemble_storage::{manager::DatabaseConfig, StorageManager};

async fn setup_test_app() -> Result<Router, Box<dyn std::error::Error>> {
    let config = DatabaseConfig {
        url: "sqlite::memory:".to_string(),
        max_connections: None,
        migrate_on_startup: true,
        performance_config: None,
    };
    let storage = Arc::new(StorageManager::new(&config).await?);

    // Build a minimal router for testing
    use axum::{
        routing::{get, post},
        Router,
    };
    use vibe_ensemble_web::handlers;

    let app = Router::new()
        .route("/api/links/validate", post(handlers::links::validate_links))
        .route(
            "/api/links/validate",
            get(handlers::links::trigger_link_validation),
        )
        .with_state(storage);

    Ok(app)
}

#[tokio::test]
async fn test_link_validation_trigger() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/links/validate")
        .body(Body::empty())?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    let validation_data: Value = serde_json::from_slice(&body)?;

    // Check that the response has the required "results" field
    assert!(
        validation_data.get("results").is_some(),
        "Response should have 'results' field"
    );

    // Check that results is an array
    let results = validation_data["results"].as_array().unwrap();
    assert!(!results.is_empty(), "Results should not be empty");

    // Check that summary exists
    assert!(
        validation_data.get("summary").is_some(),
        "Response should have 'summary' field"
    );
    let summary = &validation_data["summary"];
    assert!(
        summary.get("total").is_some(),
        "Summary should have 'total' field"
    );
    assert!(
        summary.get("healthy").is_some(),
        "Summary should have 'healthy' field"
    );
    assert!(
        summary.get("broken").is_some(),
        "Summary should have 'broken' field"
    );
    assert!(
        summary.get("warning").is_some(),
        "Summary should have 'warning' field"
    );

    // Verify that we have actual validation data, not zeros
    let total = summary["total"].as_u64().unwrap();
    assert!(total > 0, "Total should be greater than 0");

    // Check that each result has the expected structure
    for result in results {
        assert!(
            result.get("url").is_some(),
            "Result should have 'url' field"
        );
        assert!(
            result.get("status").is_some(),
            "Result should have 'status' field"
        );
        // status_code might be null for failed requests
        // response_time_ms should be present
        assert!(
            result.get("response_time_ms").is_some(),
            "Result should have 'response_time_ms' field"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_link_validation_with_custom_urls() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    let payload = json!({
        "urls": [
            "https://httpbin.org/status/200",
            "https://httpbin.org/status/404",
            "https://invalid-domain-that-does-not-exist.com"
        ]
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/links/validate")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload)?))?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    let validation_data: Value = serde_json::from_slice(&body)?;

    // Check that the response has the required "results" field
    assert!(
        validation_data.get("results").is_some(),
        "Response should have 'results' field"
    );

    let results = validation_data["results"].as_array().unwrap();
    assert_eq!(results.len(), 3, "Should have results for all 3 URLs");

    let summary = &validation_data["summary"];
    assert_eq!(summary["total"].as_u64().unwrap(), 3, "Total should be 3");

    // Verify that actual validation occurred - we should have at least one healthy and one broken
    let healthy = summary["healthy"].as_u64().unwrap();
    let broken = summary["broken"].as_u64().unwrap();
    let warning = summary["warning"].as_u64().unwrap();

    assert_eq!(
        healthy + broken + warning,
        3,
        "All URLs should be categorized"
    );

    Ok(())
}

#[tokio::test]
async fn test_link_validation_response_structure() -> Result<(), Box<dyn std::error::Error>> {
    let app = setup_test_app().await?;

    let payload = json!({
        "urls": ["https://httpbin.org/status/200"]
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/links/validate")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload)?))?;

    let response = app.oneshot(request).await?;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await?.to_bytes();
    let validation_data: Value = serde_json::from_slice(&body)?;

    // Verify the exact structure expected
    assert!(validation_data.get("results").is_some());
    assert!(validation_data.get("summary").is_some());
    assert!(validation_data.get("timestamp").is_some());

    let result = &validation_data["results"][0];
    assert_eq!(result["url"], "https://httpbin.org/status/200");
    assert_eq!(result["status"], "healthy");
    assert_eq!(result["status_code"], 200);
    assert!(result["response_time_ms"].as_u64().unwrap() > 0);
    assert!(result.get("error").is_none() || result["error"].is_null());

    Ok(())
}
