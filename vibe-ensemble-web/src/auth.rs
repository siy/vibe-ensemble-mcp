//! Authentication and session management
//!
//! Provides basic authentication and session management for the web interface.
//! For now, implements a simple in-memory session store suitable for development.

use crate::{Error, Result};
use axum::{
    extract::{Request, State},
    http::{header::COOKIE, HeaderMap, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};
use uuid::Uuid;

/// Session data stored in memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub is_admin: bool,
}

impl Session {
    /// Create a new session
    pub fn new(user_id: String, username: String, is_admin: bool) -> Self {
        let session_id = generate_session_id();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(24); // 24-hour sessions

        Self {
            id: session_id,
            user_id,
            username,
            created_at: now,
            expires_at,
            is_admin,
        }
    }

    /// Check if session is valid (not expired)
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.expires_at
    }
}

/// In-memory session store
#[derive(Debug, Clone, Default)]
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl SessionStore {
    /// Create a new session store
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store a session
    pub fn store(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire session lock: {}", e))
        })?;
        sessions.insert(session.id.clone(), session);
        Ok(())
    }

    /// Retrieve a session by ID
    pub fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let sessions = self.sessions.read().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire session lock: {}", e))
        })?;
        Ok(sessions.get(session_id).cloned())
    }

    /// Remove a session
    pub fn remove(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire session lock: {}", e))
        })?;
        sessions.remove(session_id);
        Ok(())
    }

    /// Clean up expired sessions
    pub fn cleanup_expired(&self) -> Result<usize> {
        let mut sessions = self.sessions.write().map_err(|e| {
            Error::Internal(anyhow::anyhow!("Failed to acquire session lock: {}", e))
        })?;
        let now = Utc::now();
        let initial_count = sessions.len();
        sessions.retain(|_, session| session.expires_at > now);
        Ok(initial_count - sessions.len())
    }
}

/// Login form data
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub username: String,
    pub password: String,
}

/// User credentials for simple authentication
/// In a real system, this would be stored in a database with hashed passwords
#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub username: String,
    pub password_hash: String, // In reality, this would be properly hashed
    pub is_admin: bool,
}

/// Simple authentication service
#[derive(Debug, Clone)]
pub struct AuthService {
    pub session_store: SessionStore,
    // In a real system, this would be a proper user store/database
    pub users: Arc<RwLock<HashMap<String, UserCredentials>>>,
}

impl AuthService {
    /// Create a new auth service with default admin user
    pub fn new() -> Self {
        let mut users = HashMap::new();

        // Add a default admin user (in real systems, this would be properly secured)
        users.insert(
            "admin".to_string(),
            UserCredentials {
                username: "admin".to_string(),
                password_hash: "admin".to_string(), // NEVER do this in production!
                is_admin: true,
            },
        );

        Self {
            session_store: SessionStore::new(),
            users: Arc::new(RwLock::new(users)),
        }
    }

    /// Authenticate user and create session
    pub fn authenticate(&self, username: &str, password: &str) -> Result<Option<Session>> {
        let users = self
            .users
            .read()
            .map_err(|e| Error::Internal(anyhow::anyhow!("Failed to acquire user lock: {}", e)))?;

        if let Some(user) = users.get(username) {
            // In a real system, you would properly hash and compare passwords
            if user.password_hash == password {
                let session = Session::new(
                    Uuid::new_v4().to_string(),
                    username.to_string(),
                    user.is_admin,
                );
                self.session_store.store(session.clone())?;
                return Ok(Some(session));
            }
        }

        Ok(None)
    }

    /// Get session from cookie
    pub fn get_session_from_cookie(&self, cookie_header: &str) -> Result<Option<Session>> {
        if let Some(session_id) = extract_session_id_from_cookie(cookie_header) {
            if let Some(session) = self.session_store.get(&session_id)? {
                if session.is_valid() {
                    return Ok(Some(session));
                } else {
                    // Remove expired session
                    self.session_store.remove(&session_id)?;
                }
            }
        }
        Ok(None)
    }

    /// Logout user (remove session)
    pub fn logout(&self, session_id: &str) -> Result<()> {
        self.session_store.remove(session_id)
    }
}

impl Default for AuthService {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a random session ID
fn generate_session_id() -> String {
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let char_type = rng.gen_range(0..3);
            match char_type {
                0 => rng.gen_range(b'a'..=b'z') as char,
                1 => rng.gen_range(b'A'..=b'Z') as char,
                _ => rng.gen_range(b'0'..=b'9') as char,
            }
        })
        .collect()
}

/// Extract session ID from cookie header
fn extract_session_id_from_cookie(cookie_header: &str) -> Option<String> {
    cookie::Cookie::split_parse(cookie_header)
        .filter_map(Result::ok)
        .find(|c| c.name() == "session_id")
        .map(|c| c.value().to_string())
}

/// Middleware to require authentication
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response> {
    // Skip auth for login/logout pages
    let path = request.uri().path();
    if path == "/login" || path == "/logout" || path.starts_with("/static/") {
        return Ok(next.run(request).await);
    }

    // Check for session cookie
    if let Some(cookie_header) = headers.get(COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if let Ok(Some(session)) = auth_service.get_session_from_cookie(cookie_str) {
                // Add session to request extensions
                request.extensions_mut().insert(session);
                return Ok(next.run(request).await);
            }
        }
    }

    // No valid session found, redirect to login
    Ok(Redirect::to("/login").into_response())
}

/// Login page handler
pub async fn login_page() -> Html<String> {
    let html = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login - Vibe Ensemble</title>
    <style>
        body { 
            font-family: Arial, sans-serif; 
            display: flex; 
            justify-content: center; 
            align-items: center; 
            height: 100vh; 
            margin: 0; 
            background-color: #f5f5f5; 
        }
        .login-form { 
            background: white; 
            padding: 2rem; 
            border-radius: 8px; 
            box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1); 
            width: 300px; 
        }
        .login-form h1 { text-align: center; margin-bottom: 2rem; }
        .form-group { margin-bottom: 1rem; }
        .form-group label { display: block; margin-bottom: 0.5rem; }
        .form-group input { 
            width: 100%; 
            padding: 0.75rem; 
            border: 1px solid #ddd; 
            border-radius: 4px; 
            box-sizing: border-box; 
        }
        .btn { 
            width: 100%; 
            padding: 0.75rem; 
            background-color: #007bff; 
            color: white; 
            border: none; 
            border-radius: 4px; 
            cursor: pointer; 
        }
        .btn:hover { background-color: #0056b3; }
        .error { color: red; text-align: center; margin-top: 1rem; }
        .demo-info { 
            background-color: #d4edda; 
            border: 1px solid #c3e6cb; 
            padding: 1rem; 
            border-radius: 4px; 
            margin-bottom: 1rem; 
            font-size: 0.9rem; 
        }
    </style>
</head>
<body>
    <div class="login-form">
        <h1>Vibe Ensemble</h1>
        <div class="demo-info">
            <strong>Demo Login:</strong><br>
            Username: admin<br>
            Password: admin
        </div>
        <form method="post" action="/login">
            <div class="form-group">
                <label for="username">Username:</label>
                <input type="text" id="username" name="username" required>
            </div>
            <div class="form-group">
                <label for="password">Password:</label>
                <input type="password" id="password" name="password" required>
            </div>
            <button type="submit" class="btn">Login</button>
        </form>
    </div>
</body>
</html>
    "#;

    Html(html.to_string())
}

/// Login form handler
pub async fn login_handler(
    State(auth_service): State<Arc<AuthService>>,
    Form(form): Form<LoginForm>,
) -> impl IntoResponse {
    match auth_service.authenticate(&form.username, &form.password) {
        Ok(Some(session)) => {
            // SameSite=Lax prevents CSRF on cross-site POSTs; Secure recommended when served over HTTPS
            let cookie = format!(
                "session_id={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400{}",
                session.id,
                if cfg!(feature = "secure_cookies") {
                    "; Secure"
                } else {
                    ""
                }
            );
            let mut response = Redirect::to("/dashboard").into_response();
            response
                .headers_mut()
                .insert(axum::http::header::SET_COOKIE, cookie.parse().unwrap());
            response
        }
        Ok(None) => {
            // Authentication failed
            let html = r#"
<!DOCTYPE html>
<html>
<head>
    <title>Login Failed</title>
    <meta http-equiv="refresh" content="3;url=/login">
</head>
<body>
    <h1>Login Failed</h1>
    <p>Invalid username or password. Redirecting...</p>
    <a href="/login">Back to Login</a>
</body>
</html>
            "#;
            Html(html.to_string()).into_response()
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Authentication error").into_response(),
    }
}

/// Logout handler
pub async fn logout_handler(
    State(auth_service): State<Arc<AuthService>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Remove session if exists
    if let Some(cookie_header) = headers.get(COOKIE) {
        if let Ok(cookie_str) = cookie_header.to_str() {
            if let Some(session_id) = extract_session_id_from_cookie(cookie_str) {
                let _ = auth_service.logout(&session_id);
            }
        }
    }

    // Clear the cookie and redirect to login
    let mut response = Redirect::to("/login").into_response();
    response.headers_mut().insert(
        axum::http::header::SET_COOKIE,
        "session_id=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .unwrap(),
    );
    response
}

/// Extract session from request extensions
pub fn get_session_from_request(request: &Request) -> Option<&Session> {
    request.extensions().get::<Session>()
}
