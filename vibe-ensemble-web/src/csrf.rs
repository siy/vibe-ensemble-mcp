//! CSRF protection middleware and utilities

use axum::{
    extract::{FromRequestParts, State},
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    RequestPartsExt,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tokio::sync::RwLock;

/// CSRF token store
#[derive(Debug, Clone)]
pub struct CsrfStore {
    tokens: Arc<RwLock<HashMap<String, SystemTime>>>,
}

impl CsrfStore {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Generate a new CSRF token
    pub async fn generate_token(&self) -> String {
        let token: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let mut tokens = self.tokens.write().await;
        tokens.insert(token.clone(), SystemTime::now());
        token
    }

    /// Validate a CSRF token
    pub async fn validate_token(&self, token: &str) -> bool {
        let mut tokens = self.tokens.write().await;

        // Clean up expired tokens (older than 1 hour)
        let now = SystemTime::now();
        tokens.retain(|_, created| {
            now.duration_since(*created)
                .map(|d| d < Duration::from_secs(3600))
                .unwrap_or(false)
        });

        // Check if token exists and remove it (single use)
        tokens.remove(token).is_some()
    }
}

impl Default for CsrfStore {
    fn default() -> Self {
        Self::new()
    }
}

/// CSRF token extractor
#[derive(Debug)]
pub struct CsrfToken(pub String);

#[axum::async_trait]
impl<S> FromRequestParts<S> for CsrfToken
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let cookies = parts
            .extract::<CookieJar>()
            .await
            .map_err(|_| StatusCode::BAD_REQUEST.into_response())?;

        if let Some(cookie) = cookies.get("csrf_token") {
            Ok(CsrfToken(cookie.value().to_string()))
        } else {
            Err(StatusCode::FORBIDDEN.into_response())
        }
    }
}

/// Form CSRF token validation
#[derive(Debug, Deserialize)]
pub struct CsrfFormToken {
    pub csrf_token: String,
}

/// Validate CSRF token from form submission
pub async fn validate_csrf_form(
    State(csrf_store): State<Arc<CsrfStore>>,
    form_token: CsrfFormToken,
    cookie_token: Option<CsrfToken>,
) -> Result<(), Response> {
    let cookie_token = cookie_token.ok_or_else(|| StatusCode::FORBIDDEN.into_response())?;

    // Verify tokens match
    if form_token.csrf_token != cookie_token.0 {
        return Err(StatusCode::FORBIDDEN.into_response());
    }

    // Validate token in store
    if !csrf_store.validate_token(&form_token.csrf_token).await {
        return Err(StatusCode::FORBIDDEN.into_response());
    }

    Ok(())
}

/// Generate CSRF token for forms
pub async fn generate_csrf_token(csrf_store: &CsrfStore) -> (String, Cookie<'static>) {
    let token = csrf_store.generate_token().await;

    let cookie_builder = Cookie::build(("csrf_token", token.clone()))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(time::Duration::hours(1));

    // Conditionally set secure flag behind a feature
    #[cfg(feature = "secure_cookies")]
    let cookie_builder = cookie_builder.secure(true);

    (token, cookie_builder.build())
}

/// HTML helper to include CSRF token in forms
pub fn csrf_token_input(token: &str) -> String {
    format!(
        r#"<input type="hidden" name="csrf_token" value="{}" />"#,
        token
    )
}
