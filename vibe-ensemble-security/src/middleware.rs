//! Security middleware for web applications

use crate::{
    AccessContext, AccessControlService, Action, AuditLogger, AuthService, CsrfTokenManager,
    EncryptionService, ErrorResponse, JwtManager, Permission, ResourceType, SecurityError,
    SecurityResult, User, UserRole,
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use headers::{authorization::Bearer, Authorization, HeaderMapExt};
use serde_json::json;
use std::sync::Arc;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{error, info, warn};

/// Security configuration for middleware
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Enable CSRF protection
    pub csrf_protection: bool,
    /// Enable HSTS (HTTP Strict Transport Security)
    pub hsts_enabled: bool,
    /// HSTS max age in seconds
    pub hsts_max_age: u32,
    /// Enable content security policy
    pub csp_enabled: bool,
    /// Custom CSP policy
    pub csp_policy: Option<String>,
    /// Enable X-Frame-Options
    pub frame_options_enabled: bool,
    /// Enable X-Content-Type-Options
    pub content_type_options_enabled: bool,
    /// Enable Referrer Policy
    pub referrer_policy_enabled: bool,
    /// Enable Permissions Policy
    pub permissions_policy_enabled: bool,
    /// Require HTTPS for authentication endpoints
    pub require_https_auth: bool,
    /// Log all requests for audit
    pub log_all_requests: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            csrf_protection: true,
            hsts_enabled: true,
            hsts_max_age: 31536000, // 1 year
            csp_enabled: true,
            csp_policy: None, // Use default
            frame_options_enabled: true,
            content_type_options_enabled: true,
            referrer_policy_enabled: true,
            permissions_policy_enabled: true,
            require_https_auth: true,
            log_all_requests: false,
        }
    }
}

/// Security middleware state
#[derive(Clone)]
pub struct SecurityMiddleware {
    pub config: SecurityConfig,
    pub auth_service: Arc<AuthService>,
    pub access_control: Arc<AccessControlService>,
    pub audit_logger: Arc<AuditLogger>,
    pub encryption_service: Arc<EncryptionService>,
    pub csrf_manager: Arc<CsrfTokenManager>,
}

impl SecurityMiddleware {
    /// Create new security middleware
    pub fn new(
        config: SecurityConfig,
        auth_service: Arc<AuthService>,
        access_control: Arc<AccessControlService>,
        audit_logger: Arc<AuditLogger>,
        encryption_service: Arc<EncryptionService>,
        csrf_manager: Arc<CsrfTokenManager>,
    ) -> Self {
        Self {
            config,
            auth_service,
            access_control,
            audit_logger,
            encryption_service,
            csrf_manager,
        }
    }
}

/// Authentication middleware
pub async fn auth_middleware(
    State(security): State<Arc<SecurityMiddleware>>,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let uri = request.uri().clone();
    let path = uri.path();
    let method = request.method().clone();

    // Skip authentication for public endpoints
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Check HTTPS requirement for authentication endpoints
    if security.config.require_https_auth && is_auth_endpoint(path) {
        if !is_https_request(&request) {
            warn!("HTTPS required for authentication endpoint: {}", path);
            return Ok(create_error_response(
                StatusCode::FORBIDDEN,
                "HTTPS required for authentication endpoints",
            ));
        }
    }

    // Extract and validate JWT token
    let user = match extract_and_validate_user(&security.auth_service, request.headers()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            // No authentication provided
            return Ok(create_error_response(
                StatusCode::UNAUTHORIZED,
                "Authentication required",
            ));
        }
        Err(e) => {
            warn!("Authentication error: {}", e.internal_message());
            return Ok(create_sanitized_error_response(&e));
        }
    };

    // Add user to request extensions
    request.extensions_mut().insert(user.clone());

    // Log authentication success for audit (if enabled)
    if security.config.log_all_requests {
        if let Err(e) = security
            .audit_logger
            .log_event(
                crate::audit::AuditEvent::new(
                    crate::audit::AuditEventType::PermissionGranted,
                    crate::audit::AuditSeverity::Low,
                    format!("User {} accessed {} {}", user.username, method, path),
                )
                .with_user(&user.id, &user.username)
                .with_action(&method.to_string()),
            )
            .await
        {
            error!("Failed to log audit event: {}", e);
        }
    }

    Ok(next.run(request).await)
}

/// Authorization middleware for checking permissions
pub async fn authorization_middleware(
    State(security): State<Arc<SecurityMiddleware>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let uri = request.uri().clone();
    let path = uri.path();
    let method = request.method().clone();

    // Skip authorization for public endpoints
    if is_public_endpoint(path) {
        return Ok(next.run(request).await);
    }

    // Get user from request extensions
    let user = match request.extensions().get::<User>() {
        Some(user) => user,
        None => {
            // No user in extensions - should not happen if auth middleware ran first
            warn!("No user found in request extensions for path: {}", path);
            return Ok(create_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication state error",
            ));
        }
    };

    // Determine resource type and action from path and method
    let (resource_type, action) = determine_resource_and_action(path, &method.to_string());

    // Create access context
    let context = AccessContext {
        user_id: user.id.clone(),
        user_role: user.role.clone(),
        resource_type,
        resource_id: extract_resource_id_from_path(path),
        action,
        resource_owner_id: None, // This would need to be determined from the actual resource
    };

    // Check permissions
    let permission_result = security.access_control.check_permission(&context);

    if !permission_result.allowed {
        // Log permission denial
        if let Some(required_permission) = &permission_result.required_permission {
            if let Err(e) = security
                .audit_logger
                .log_permission_denied(
                    &user.id,
                    &user.username,
                    context.resource_type.as_str(),
                    context.resource_id.as_deref(),
                    context.action.as_str(),
                    &format!("{:?}", required_permission),
                )
                .await
            {
                error!("Failed to log permission denial: {}", e);
            }
        }

        return Ok(create_error_response(
            StatusCode::FORBIDDEN,
            &permission_result
                .reason
                .unwrap_or_else(|| "Access denied".to_string()),
        ));
    }

    Ok(next.run(request).await)
}

/// Security headers middleware
pub async fn security_headers_middleware(
    State(security): State<Arc<SecurityMiddleware>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(request).await;

    // Add security headers
    let headers = response.headers_mut();

    // HSTS (HTTP Strict Transport Security)
    if security.config.hsts_enabled {
        let hsts_value = format!(
            "max-age={}; includeSubDomains",
            security.config.hsts_max_age
        );
        headers.insert(
            HeaderName::from_static("strict-transport-security"),
            HeaderValue::from_str(&hsts_value).unwrap(),
        );
    }

    // Content Security Policy
    if security.config.csp_enabled {
        let csp_policy = security
            .config
            .csp_policy
            .as_deref()
            .unwrap_or("default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: https:; font-src 'self' https:; connect-src 'self' ws: wss:; frame-ancestors 'none'");

        headers.insert(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_str(csp_policy).unwrap(),
        );
    }

    // X-Frame-Options
    if security.config.frame_options_enabled {
        headers.insert(
            HeaderName::from_static("x-frame-options"),
            HeaderValue::from_static("DENY"),
        );
    }

    // X-Content-Type-Options
    if security.config.content_type_options_enabled {
        headers.insert(
            HeaderName::from_static("x-content-type-options"),
            HeaderValue::from_static("nosniff"),
        );
    }

    // Referrer Policy
    if security.config.referrer_policy_enabled {
        headers.insert(
            HeaderName::from_static("referrer-policy"),
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        );
    }

    // Permissions Policy
    if security.config.permissions_policy_enabled {
        headers.insert(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static(
                "camera=(), microphone=(), geolocation=(), interest-cohort=()",
            ),
        );
    }

    // X-XSS-Protection (legacy, but still useful)
    headers.insert(
        HeaderName::from_static("x-xss-protection"),
        HeaderValue::from_static("1; mode=block"),
    );

    // Cache Control for sensitive endpoints
    if is_sensitive_endpoint(request.uri().path()) {
        headers.insert(
            HeaderName::from_static("cache-control"),
            HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
        );
        headers.insert(
            HeaderName::from_static("pragma"),
            HeaderValue::from_static("no-cache"),
        );
    }

    Ok(response)
}

/// CSRF protection middleware
pub async fn csrf_middleware(
    State(security): State<Arc<SecurityMiddleware>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !security.config.csrf_protection {
        return Ok(next.run(request).await);
    }

    let method = request.method();
    let path = request.uri().path();

    // Only check CSRF for state-changing operations and non-API endpoints
    if matches!(method.as_str(), "POST" | "PUT" | "DELETE" | "PATCH") && !is_api_endpoint(path) {
        // Get session ID from user context
        let session_id = match request.extensions().get::<User>() {
            Some(user) => &user.id, // Use user ID as session identifier
            None => {
                warn!("No user context for CSRF validation on {}", path);
                return Ok(create_error_response(
                    StatusCode::UNAUTHORIZED,
                    "Authentication required for CSRF protection",
                ));
            }
        };

        // Check for CSRF token in headers
        let csrf_token = request
            .headers()
            .get("X-CSRF-Token")
            .or_else(|| request.headers().get("x-csrf-token"))
            .and_then(|v| v.to_str().ok());

        match csrf_token {
            Some(token) => {
                // Validate CSRF token against session
                if !security.csrf_manager.consume_token(token, session_id).await {
                    warn!(
                        "Invalid or expired CSRF token for {} from session {}",
                        path, session_id
                    );

                    // Log security event
                    if let Err(e) = security
                        .audit_logger
                        .log_event(
                            crate::audit::AuditEvent::new(
                                crate::audit::AuditEventType::SecurityViolation,
                                crate::audit::AuditSeverity::Medium,
                                format!(
                                    "Invalid CSRF token attempt on {} from session {}",
                                    path, session_id
                                ),
                            )
                            .with_user(session_id, "unknown")
                            .with_action("csrf_validation_failed")
                            .with_resource("web_request", Some(path)),
                        )
                        .await
                    {
                        error!("Failed to log CSRF violation: {}", e);
                    }

                    return Ok(create_error_response(
                        StatusCode::FORBIDDEN,
                        "Invalid or expired CSRF token",
                    ));
                }
            }
            None => {
                warn!(
                    "Missing CSRF token for {} from session {}",
                    path, session_id
                );

                // Log security event
                if let Err(e) = security
                    .audit_logger
                    .log_event(
                        crate::audit::AuditEvent::new(
                            crate::audit::AuditEventType::SecurityViolation,
                            crate::audit::AuditSeverity::Medium,
                            format!(
                                "Missing CSRF token for {} from session {}",
                                path, session_id
                            ),
                        )
                        .with_user(session_id, "unknown")
                        .with_action("csrf_token_missing")
                        .with_resource("web_request", Some(path)),
                    )
                    .await
                {
                    error!("Failed to log CSRF violation: {}", e);
                }

                return Ok(create_error_response(
                    StatusCode::FORBIDDEN,
                    "CSRF token required",
                ));
            }
        }
    }

    Ok(next.run(request).await)
}

/// Extract and validate user from JWT token
async fn extract_and_validate_user(
    auth_service: &AuthService,
    headers: &HeaderMap,
) -> SecurityResult<Option<User>> {
    // Try to extract JWT token from Authorization header
    if let Some(auth_header) = headers.typed_get::<Authorization<Bearer>>() {
        let token = auth_header.token();

        // Validate JWT token
        let claims = auth_service.jwt_manager().validate_access_token(token)?;

        // Get user from database
        let user = auth_service.get_user_by_id(&claims.sub).await?;

        if let Some(user) = user {
            if user.is_active && !user.is_locked() {
                return Ok(Some(user));
            } else {
                return Err(SecurityError::AuthenticationFailed(
                    "User account is disabled or locked".to_string(),
                ));
            }
        } else {
            return Err(SecurityError::AuthenticationFailed(
                "User not found".to_string(),
            ));
        }
    }

    Ok(None)
}

/// Helper functions
fn is_public_endpoint(path: &str) -> bool {
    matches!(
        path,
        "/health" | "/metrics" | "/login" | "/static/" | "/" | "/favicon.ico"
    ) || path.starts_with("/static/")
}

fn is_auth_endpoint(path: &str) -> bool {
    path.starts_with("/auth/") || path.contains("login") || path.contains("logout")
}

fn is_sensitive_endpoint(path: &str) -> bool {
    path.contains("admin")
        || path.contains("config")
        || path.contains("user")
        || path.contains("token")
}

fn is_api_endpoint(path: &str) -> bool {
    path.starts_with("/api/")
}

fn is_https_request(request: &Request) -> bool {
    // Check various indicators of HTTPS
    request.uri().scheme_str() == Some("https")
        || request
            .headers()
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            == Some("https")
        || request
            .headers()
            .get("x-forwarded-ssl")
            .and_then(|v| v.to_str().ok())
            == Some("on")
}

fn determine_resource_and_action(path: &str, method: &str) -> (ResourceType, Action) {
    let action = match method {
        "GET" => Action::Read,
        "POST" => Action::Create,
        "PUT" | "PATCH" => Action::Update,
        "DELETE" => Action::Delete,
        _ => Action::Read,
    };

    let resource_type = if path.contains("agent") {
        ResourceType::Agent
    } else if path.contains("issue") {
        ResourceType::Issue
    } else if path.contains("knowledge") {
        ResourceType::Knowledge
    } else if path.contains("message") {
        ResourceType::Message
    } else if path.contains("user") {
        ResourceType::User
    } else if path.contains("admin") || path.contains("config") {
        ResourceType::System
    } else if path.contains("audit") {
        ResourceType::AuditLog
    } else {
        ResourceType::Dashboard
    };

    (resource_type, action)
}

fn extract_resource_id_from_path(path: &str) -> Option<String> {
    // Extract resource ID from path patterns like /api/users/123
    let segments: Vec<&str> = path.split('/').collect();
    for (i, segment) in segments.iter().enumerate() {
        if matches!(
            *segment,
            "agents" | "issues" | "knowledge" | "users" | "messages"
        ) {
            if let Some(id) = segments.get(i + 1) {
                if !id.is_empty() && *id != "new" && *id != "edit" {
                    return Some(id.to_string());
                }
            }
        }
    }
    None
}

/// Generate CSRF token endpoint handler
pub async fn generate_csrf_token_handler(
    State(security): State<Arc<SecurityMiddleware>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Extract user from JWT token
    match extract_and_validate_user(&security.auth_service, &headers).await {
        Ok(Some(user)) => {
            // Generate CSRF token for user session
            match security.csrf_manager.generate_token(&user.id).await {
                Ok(token) => {
                    Ok(Json(serde_json::json!({
                        "csrf_token": token,
                        "expires_in": 3600 // 1 hour in seconds
                    })))
                }
                Err(e) => {
                    error!("Failed to generate CSRF token: {}", e.internal_message());
                    let response = create_sanitized_error_response(&e);
                    // Extract status code from response
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Ok(None) => Err(StatusCode::UNAUTHORIZED),
        Err(e) => {
            error!(
                "Authentication failed for CSRF token generation: {}",
                e.internal_message()
            );
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

fn create_error_response(status: StatusCode, message: &str) -> Response {
    let body = json!({
        "error": message,
        "status": status.as_u16(),
        "timestamp": chrono::Utc::now()
    });

    (status, Json(body)).into_response()
}

/// Create sanitized error response from SecurityError
fn create_sanitized_error_response(error: &SecurityError) -> Response {
    let error_response = ErrorResponse::from_security_error(error);
    let status =
        StatusCode::from_u16(error_response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    (status, Json(error_response)).into_response()
}


/// Middleware configuration builder
pub struct SecurityMiddlewareBuilder {
    config: SecurityConfig,
}

impl SecurityMiddlewareBuilder {
    /// Create new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: SecurityConfig::default(),
        }
    }

    /// Enable/disable CSRF protection
    pub fn csrf_protection(mut self, enabled: bool) -> Self {
        self.config.csrf_protection = enabled;
        self
    }

    /// Enable/disable HSTS
    pub fn hsts(mut self, enabled: bool, max_age: Option<u32>) -> Self {
        self.config.hsts_enabled = enabled;
        if let Some(age) = max_age {
            self.config.hsts_max_age = age;
        }
        self
    }

    /// Set custom CSP policy
    pub fn csp_policy(mut self, policy: String) -> Self {
        self.config.csp_enabled = true;
        self.config.csp_policy = Some(policy);
        self
    }

    /// Enable/disable frame options
    pub fn frame_options(mut self, enabled: bool) -> Self {
        self.config.frame_options_enabled = enabled;
        self
    }

    /// Require HTTPS for authentication
    pub fn require_https_auth(mut self, required: bool) -> Self {
        self.config.require_https_auth = required;
        self
    }

    /// Enable request logging
    pub fn log_all_requests(mut self, enabled: bool) -> Self {
        self.config.log_all_requests = enabled;
        self
    }

    /// Build the configuration
    pub fn build(self) -> SecurityConfig {
        self.config
    }
}

impl Default for SecurityMiddlewareBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_and_action_determination() {
        assert_eq!(
            determine_resource_and_action("/api/agents/123", "GET"),
            (ResourceType::Agent, Action::Read)
        );
        assert_eq!(
            determine_resource_and_action("/api/issues", "POST"),
            (ResourceType::Issue, Action::Create)
        );
        assert_eq!(
            determine_resource_and_action("/api/users/456", "PUT"),
            (ResourceType::User, Action::Update)
        );
        assert_eq!(
            determine_resource_and_action("/api/knowledge/789", "DELETE"),
            (ResourceType::Knowledge, Action::Delete)
        );
    }

    #[test]
    fn test_resource_id_extraction() {
        assert_eq!(
            extract_resource_id_from_path("/api/users/123"),
            Some("123".to_string())
        );
        assert_eq!(
            extract_resource_id_from_path("/api/agents/agent_456"),
            Some("agent_456".to_string())
        );
        assert_eq!(extract_resource_id_from_path("/api/issues"), None);
        assert_eq!(extract_resource_id_from_path("/api/users/new"), None);
        assert_eq!(extract_resource_id_from_path("/dashboard"), None);
    }

    #[test]
    fn test_endpoint_classification() {
        assert!(is_public_endpoint("/health"));
        assert!(is_public_endpoint("/login"));
        assert!(is_public_endpoint("/static/css/style.css"));
        assert!(!is_public_endpoint("/api/users"));

        assert!(is_auth_endpoint("/auth/login"));
        assert!(is_auth_endpoint("/login"));
        assert!(!is_auth_endpoint("/api/users"));

        assert!(is_sensitive_endpoint("/admin/config"));
        assert!(is_sensitive_endpoint("/api/users"));
        assert!(!is_sensitive_endpoint("/dashboard"));

        assert!(is_api_endpoint("/api/users"));
        assert!(!is_api_endpoint("/dashboard"));
    }

    #[test]
    fn test_csrf_token_validation() {
        assert!(!is_valid_csrf_token(""));
        assert!(!is_valid_csrf_token("short"));
        assert!(is_valid_csrf_token(
            "a_very_long_token_that_is_at_least_32_characters_long"
        ));
    }

    #[test]
    fn test_security_config_builder() {
        let config = SecurityMiddlewareBuilder::new()
            .csrf_protection(false)
            .hsts(true, Some(86400))
            .csp_policy("default-src 'self'".to_string())
            .require_https_auth(true)
            .log_all_requests(true)
            .build();

        assert!(!config.csrf_protection);
        assert!(config.hsts_enabled);
        assert_eq!(config.hsts_max_age, 86400);
        assert_eq!(config.csp_policy, Some("default-src 'self'".to_string()));
        assert!(config.require_https_auth);
        assert!(config.log_all_requests);
    }
}
