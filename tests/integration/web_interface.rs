//! Web interface integration tests for vibe-ensemble-mcp
//!
//! Tests the HTTP API endpoints and web interface functionality.

use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, StatusCode, Method, header},
    response::Response,
};
use tower::ServiceExt; // for `oneshot` and `ready`
use serde_json::{json, Value};
use uuid::Uuid;

use vibe_ensemble_web::{create_app, WebConfig};
use vibe_ensemble_storage::StorageManager;
use vibe_ensemble_security::{AuthenticationService, UserCredentials};

use crate::common::{
    database::DatabaseTestHelper,
    fixtures::{TestScenarios, TestDataFactory},
};

/// Test web application setup and basic routing
#[tokio::test]
async fn test_web_app_setup() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager, config).await.unwrap();
    
    // Test health check endpoint
    let request = Request::builder()
        .uri("/health")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();
    
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let health_response: Value = serde_json::from_slice(&body).unwrap();
    
    assert_eq!(health_response["status"], "healthy");
    assert!(health_response["timestamp"].is_string());
}

/// Test API authentication endpoints
#[tokio::test]
async fn test_auth_endpoints() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    // Test user registration
    let register_payload = json!({
        "username": "test_web_user",
        "password": "SecureP@ssw0rd123!"
    });
    
    let register_request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(register_payload.to_string()))
        .unwrap();
    
    let register_response = app.clone().oneshot(register_request).await.unwrap();
    assert_eq!(register_response.status(), StatusCode::CREATED);
    
    let register_body = axum::body::to_bytes(register_response.into_body(), usize::MAX).await.unwrap();
    let register_result: Value = serde_json::from_slice(&register_body).unwrap();
    assert!(register_result["user_id"].is_string());
    
    // Test user login
    let login_payload = json!({
        "username": "test_web_user",
        "password": "SecureP@ssw0rd123!"
    });
    
    let login_request = Request::builder()
        .uri("/api/auth/login")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(login_payload.to_string()))
        .unwrap();
    
    let login_response = app.clone().oneshot(login_request).await.unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    
    let login_body = axum::body::to_bytes(login_response.into_body(), usize::MAX).await.unwrap();
    let login_result: Value = serde_json::from_slice(&login_body).unwrap();
    let token = login_result["token"].as_str().unwrap();
    assert!(!token.is_empty());
    
    // Test protected endpoint with valid token
    let protected_request = Request::builder()
        .uri("/api/profile")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", token))
        .body(Body::empty())
        .unwrap();
    
    let protected_response = app.clone().oneshot(protected_request).await.unwrap();
    assert_eq!(protected_response.status(), StatusCode::OK);
    
    // Test protected endpoint without token
    let unauthorized_request = Request::builder()
        .uri("/api/profile")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();
    
    let unauthorized_response = app.oneshot(unauthorized_request).await.unwrap();
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);
}

/// Test agent management API endpoints
#[tokio::test]
async fn test_agent_api() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    // Create and authenticate user first
    let auth_token = create_authenticated_user(&app).await;
    
    // Test GET /api/agents (list agents)
    let list_request = Request::builder()
        .uri("/api/agents")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let list_response = app.clone().oneshot(list_request).await.unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    
    let list_body = axum::body::to_bytes(list_response.into_body(), usize::MAX).await.unwrap();
    let agents_list: Value = serde_json::from_slice(&list_body).unwrap();
    assert!(agents_list["agents"].is_array());
    
    // Test POST /api/agents (create agent)
    let agent_payload = json!({
        "name": "test-web-agent",
        "capabilities": ["testing", "web-api"],
        "connection_metadata": {
            "host": "localhost",
            "port": 8080,
            "protocol": "http"
        }
    });
    
    let create_request = Request::builder()
        .uri("/api/agents")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::from(agent_payload.to_string()))
        .unwrap();
    
    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let create_result: Value = serde_json::from_slice(&create_body).unwrap();
    let agent_id = create_result["agent_id"].as_str().unwrap();
    
    // Test GET /api/agents/{id} (get specific agent)
    let get_request = Request::builder()
        .uri(&format!("/api/agents/{}", agent_id))
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let get_response = app.clone().oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    
    let get_body = axum::body::to_bytes(get_response.into_body(), usize::MAX).await.unwrap();
    let agent_data: Value = serde_json::from_slice(&get_body).unwrap();
    assert_eq!(agent_data["name"], "test-web-agent");
    assert_eq!(agent_data["capabilities"].as_array().unwrap().len(), 2);
    
    // Test PUT /api/agents/{id} (update agent)
    let update_payload = json!({
        "name": "updated-web-agent",
        "capabilities": ["testing", "web-api", "updated"]
    });
    
    let update_request = Request::builder()
        .uri(&format!("/api/agents/{}", agent_id))
        .method(Method::PUT)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::from(update_payload.to_string()))
        .unwrap();
    
    let update_response = app.clone().oneshot(update_request).await.unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    
    // Test DELETE /api/agents/{id} (delete agent)
    let delete_request = Request::builder()
        .uri(&format!("/api/agents/{}", agent_id))
        .method(Method::DELETE)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let delete_response = app.oneshot(delete_request).await.unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);
}

/// Test issue management API endpoints
#[tokio::test]
async fn test_issues_api() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    let auth_token = create_authenticated_user(&app).await;
    
    // Test GET /api/issues (list issues)
    let list_request = Request::builder()
        .uri("/api/issues")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let list_response = app.clone().oneshot(list_request).await.unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    
    // Test POST /api/issues (create issue)
    let issue_payload = json!({
        "title": "Web API Test Issue",
        "description": "Testing issue creation via web API",
        "priority": "medium"
    });
    
    let create_request = Request::builder()
        .uri("/api/issues")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::from(issue_payload.to_string()))
        .unwrap();
    
    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let create_result: Value = serde_json::from_slice(&create_body).unwrap();
    let issue_id = create_result["issue_id"].as_str().unwrap();
    
    // Test GET /api/issues/{id}
    let get_request = Request::builder()
        .uri(&format!("/api/issues/{}", issue_id))
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let get_response = app.clone().oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    
    // Test PATCH /api/issues/{id}/status (update status)
    let status_payload = json!({
        "status": "in_progress"
    });
    
    let status_request = Request::builder()
        .uri(&format!("/api/issues/{}/status", issue_id))
        .method(Method::PATCH)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::from(status_payload.to_string()))
        .unwrap();
    
    let status_response = app.oneshot(status_request).await.unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
}

/// Test knowledge management API endpoints
#[tokio::test]
async fn test_knowledge_api() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    let auth_token = create_authenticated_user(&app).await;
    
    // Test POST /api/knowledge (create knowledge)
    let knowledge_payload = json!({
        "title": "Web API Testing Guide",
        "content": "Comprehensive guide for testing web APIs in Rust",
        "knowledge_type": "technical_documentation",
        "access_level": "team_visible",
        "tags": ["testing", "web", "api", "rust"]
    });
    
    let create_request = Request::builder()
        .uri("/api/knowledge")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::from(knowledge_payload.to_string()))
        .unwrap();
    
    let create_response = app.clone().oneshot(create_request).await.unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    
    let create_body = axum::body::to_bytes(create_response.into_body(), usize::MAX).await.unwrap();
    let create_result: Value = serde_json::from_slice(&create_body).unwrap();
    let knowledge_id = create_result["knowledge_id"].as_str().unwrap();
    
    // Test GET /api/knowledge (list knowledge)
    let list_request = Request::builder()
        .uri("/api/knowledge")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let list_response = app.clone().oneshot(list_request).await.unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    
    // Test GET /api/knowledge/search?q=testing
    let search_request = Request::builder()
        .uri("/api/knowledge/search?q=testing")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let search_response = app.clone().oneshot(search_request).await.unwrap();
    assert_eq!(search_response.status(), StatusCode::OK);
    
    let search_body = axum::body::to_bytes(search_response.into_body(), usize::MAX).await.unwrap();
    let search_results: Value = serde_json::from_slice(&search_body).unwrap();
    assert!(search_results["results"].is_array());
    assert!(search_results["results"].as_array().unwrap().len() > 0);
    
    // Test GET /api/knowledge/{id}
    let get_request = Request::builder()
        .uri(&format!("/api/knowledge/{}", knowledge_id))
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let get_response = app.oneshot(get_request).await.unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
}

/// Test WebSocket functionality
#[tokio::test]
async fn test_websocket_api() {
    use tokio_tungstenite::{connect_async, tungstenite::Message};
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    // Start server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    // Give server time to start
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // Connect WebSocket client
    let ws_url = format!("ws://127.0.0.1:{}/ws", addr.port());
    let (ws_stream, _) = connect_async(&ws_url).await.unwrap();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    
    // Send authentication message
    let auth_msg = json!({
        "type": "authenticate",
        "token": "test-token" // In real app, this would be a valid JWT
    });
    
    ws_sender.send(Message::Text(auth_msg.to_string())).await.unwrap();
    
    // Send subscription message
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "agent_updates"
    });
    
    ws_sender.send(Message::Text(subscribe_msg.to_string())).await.unwrap();
    
    // Receive responses
    let timeout = std::time::Duration::from_secs(5);
    let response = tokio::time::timeout(timeout, ws_receiver.next()).await;
    
    if let Ok(Some(Ok(Message::Text(text)))) = response {
        let response_data: Value = serde_json::from_str(&text).unwrap();
        assert!(response_data["type"].is_string());
    }
    
    server_handle.abort();
}

/// Test API error handling
#[tokio::test]
async fn test_api_error_handling() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    // Test 404 for non-existent endpoint
    let not_found_request = Request::builder()
        .uri("/api/nonexistent")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();
    
    let not_found_response = app.clone().oneshot(not_found_request).await.unwrap();
    assert_eq!(not_found_response.status(), StatusCode::NOT_FOUND);
    
    // Test 400 for invalid JSON
    let invalid_json_request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("invalid json"))
        .unwrap();
    
    let invalid_json_response = app.clone().oneshot(invalid_json_request).await.unwrap();
    assert_eq!(invalid_json_response.status(), StatusCode::BAD_REQUEST);
    
    // Test 401 for unauthorized access
    let unauthorized_request = Request::builder()
        .uri("/api/agents")
        .method(Method::GET)
        .body(Body::empty())
        .unwrap();
    
    let unauthorized_response = app.clone().oneshot(unauthorized_request).await.unwrap();
    assert_eq!(unauthorized_response.status(), StatusCode::UNAUTHORIZED);
    
    // Test 404 for non-existent resource
    let auth_token = create_authenticated_user(&app).await;
    
    let missing_resource_request = Request::builder()
        .uri("/api/agents/00000000-0000-0000-0000-000000000000")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let missing_resource_response = app.oneshot(missing_resource_request).await.unwrap();
    assert_eq!(missing_resource_response.status(), StatusCode::NOT_FOUND);
}

/// Test API rate limiting
#[tokio::test]
async fn test_api_rate_limiting() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let mut config = WebConfig::default();
    config.rate_limit_requests_per_minute = 5; // Very low limit for testing
    
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    let auth_token = create_authenticated_user(&app).await;
    
    // Make requests up to the limit
    for i in 0..5 {
        let request = Request::builder()
            .uri("/api/agents")
            .method(Method::GET)
            .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
            .body(Body::empty())
            .unwrap();
        
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK, "Request {} should succeed", i);
    }
    
    // Next request should be rate limited
    let rate_limited_request = Request::builder()
        .uri("/api/agents")
        .method(Method::GET)
        .header(header::AUTHORIZATION, format!("Bearer {}", auth_token))
        .body(Body::empty())
        .unwrap();
    
    let rate_limited_response = app.oneshot(rate_limited_request).await.unwrap();
    assert_eq!(rate_limited_response.status(), StatusCode::TOO_MANY_REQUESTS);
}

/// Test CORS headers
#[tokio::test]
async fn test_cors_headers() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    
    // Test preflight request
    let preflight_request = Request::builder()
        .uri("/api/agents")
        .method(Method::OPTIONS)
        .header(header::ORIGIN, "https://example.com")
        .header("Access-Control-Request-Method", "GET")
        .body(Body::empty())
        .unwrap();
    
    let preflight_response = app.clone().oneshot(preflight_request).await.unwrap();
    assert_eq!(preflight_response.status(), StatusCode::OK);
    
    let headers = preflight_response.headers();
    assert!(headers.contains_key("access-control-allow-origin"));
    assert!(headers.contains_key("access-control-allow-methods"));
    assert!(headers.contains_key("access-control-allow-headers"));
    
    // Test actual CORS request
    let cors_request = Request::builder()
        .uri("/health")
        .method(Method::GET)
        .header(header::ORIGIN, "https://example.com")
        .body(Body::empty())
        .unwrap();
    
    let cors_response = app.oneshot(cors_request).await.unwrap();
    assert_eq!(cors_response.status(), StatusCode::OK);
    
    let cors_headers = cors_response.headers();
    assert!(cors_headers.contains_key("access-control-allow-origin"));
}

/// Helper function to create an authenticated user and return the auth token
async fn create_authenticated_user(app: &axum::Router) -> String {
    // Register user
    let register_payload = json!({
        "username": format!("test_user_{}", Uuid::new_v4()),
        "password": "SecureP@ssw0rd123!"
    });
    
    let register_request = Request::builder()
        .uri("/api/auth/register")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(register_payload.to_string()))
        .unwrap();
    
    let register_response = app.clone().oneshot(register_request).await.unwrap();
    assert_eq!(register_response.status(), StatusCode::CREATED);
    
    // Login user
    let login_request = Request::builder()
        .uri("/api/auth/login")
        .method(Method::POST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(register_payload.to_string()))
        .unwrap();
    
    let login_response = app.clone().oneshot(login_request).await.unwrap();
    assert_eq!(login_response.status(), StatusCode::OK);
    
    let login_body = axum::body::to_bytes(login_response.into_body(), usize::MAX).await.unwrap();
    let login_result: Value = serde_json::from_slice(&login_body).unwrap();
    
    login_result["token"].as_str().unwrap().to_string()
}

/// Test concurrent API requests
#[tokio::test]
async fn test_concurrent_api_requests() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let config = WebConfig::default();
    let app = create_app(storage_manager.clone(), config).await.unwrap();
    let auth_token = create_authenticated_user(&app).await;
    
    let concurrent_requests = 20;
    let mut handles = Vec::new();
    
    for i in 0..concurrent_requests {
        let app_clone = app.clone();
        let token_clone = auth_token.clone();
        
        let handle = tokio::spawn(async move {
            let request = Request::builder()
                .uri("/api/agents")
                .method(Method::GET)
                .header(header::AUTHORIZATION, format!("Bearer {}", token_clone))
                .body(Body::empty())
                .unwrap();
            
            let response = app_clone.oneshot(request).await.unwrap();
            (i, response.status())
        });
        
        handles.push(handle);
    }
    
    // Wait for all requests to complete
    let mut successful_requests = 0;
    for handle in handles {
        let (request_id, status) = handle.await.unwrap();
        if status == StatusCode::OK {
            successful_requests += 1;
        } else {
            println!("Request {} failed with status: {:?}", request_id, status);
        }
    }
    
    // Should handle concurrent requests successfully
    assert!(successful_requests >= concurrent_requests * 80 / 100); // At least 80% success rate
}