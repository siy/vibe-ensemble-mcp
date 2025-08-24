//! Middleware for request logging and performance monitoring

use axum::{
    extract::{MatchedPath, Request},
    middleware::Next,
    response::Response,
};
use std::time::Instant;
use tracing::{info_span, Instrument};

/// Threshold for slow request warnings in milliseconds
const SLOW_REQUEST_THRESHOLD_MS: u128 = 1000;

/// Middleware for logging requests with timing information
pub async fn logging_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let matched_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| req.uri().path());

    // Create a span for this request
    let span = info_span!(
        "request",
        method = %method,
        uri = %uri,
        matched_path = matched_path,
    );

    async move {
        let response = next.run(req).await;
        let elapsed = start.elapsed();
        let status = response.status();

        // Log request completion with performance metrics
        tracing::info!(
            status = %status,
            elapsed_ms = elapsed.as_millis(),
            "Request completed"
        );

        // Log slow requests as warnings
        if elapsed.as_millis() > SLOW_REQUEST_THRESHOLD_MS {
            tracing::warn!(
                status = %status,
                elapsed_ms = elapsed.as_millis(),
                "Slow request detected"
            );
        }

        response
    }
    .instrument(span)
    .await
}

/// Middleware for basic security headers
pub async fn security_headers_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    // Add basic security headers
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );

    // Add CSP header for basic protection
    headers.insert(
        "Content-Security-Policy",
        "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline';"
            .parse()
            .unwrap(),
    );

    response
}
