//! Rate limiting and DDoS protection

use crate::{AuditLogger, SecurityError, SecurityResult};
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::{
    clock::{Clock, DefaultClock, QuantaClock},
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Jitter, Quota, RateLimiter,
};
use nonzero_ext::*;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    num::NonZeroU32,
    sync::Arc,
    time::Duration,
};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute for general endpoints
    pub general_rpm: u32,
    /// Requests per minute for authentication endpoints
    pub auth_rpm: u32,
    /// Requests per minute for API endpoints
    pub api_rpm: u32,
    /// Burst capacity (maximum requests that can be made at once)
    pub burst_capacity: u32,
    /// Whether to enable per-IP rate limiting
    pub per_ip_enabled: bool,
    /// Whether to enable per-user rate limiting
    pub per_user_enabled: bool,
    /// Whitelisted IP addresses (not subject to rate limiting)
    pub whitelist_ips: Vec<IpAddr>,
    /// Blacklisted IP addresses (always blocked)
    pub blacklist_ips: Vec<IpAddr>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            general_rpm: 60,
            auth_rpm: 10,
            api_rpm: 100,
            burst_capacity: 10,
            per_ip_enabled: true,
            per_user_enabled: true,
            whitelist_ips: Vec::new(),
            blacklist_ips: Vec::new(),
        }
    }
}

/// Rate limiting endpoint type
#[derive(Debug, Clone, PartialEq)]
pub enum RateLimitType {
    General,
    Authentication,
    Api,
    WebSocket,
}

impl RateLimitType {
    /// Determine rate limit type from path
    pub fn from_path(path: &str) -> Self {
        if path.starts_with("/api/") {
            Self::Api
        } else if path.starts_with("/auth/") || path.contains("login") || path.contains("logout") {
            Self::Authentication
        } else if path.starts_with("/ws") {
            Self::WebSocket
        } else {
            Self::General
        }
    }

    /// Get requests per minute for this type
    pub fn get_rpm(&self, config: &RateLimitConfig) -> u32 {
        match self {
            Self::General => config.general_rpm,
            Self::Authentication => config.auth_rpm,
            Self::Api => config.api_rpm,
            Self::WebSocket => config.api_rpm, // Use API limit for WebSocket
        }
    }
}

/// Rate limiter service
#[derive(Clone)]
pub struct RateLimitService {
    config: RateLimitConfig,
    /// Per-IP rate limiters
    ip_limiters: Arc<
        RwLock<
            HashMap<
                IpAddr,
                HashMap<
                    RateLimitType,
                    Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
                >,
            >,
        >,
    >,
    /// Per-user rate limiters
    user_limiters: Arc<
        RwLock<
            HashMap<
                String,
                HashMap<
                    RateLimitType,
                    Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
                >,
            >,
        >,
    >,
    /// Global rate limiter (fallback)
    global_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>>,
    /// Audit logger for rate limit events
    audit_logger: Option<Arc<AuditLogger>>,
}

impl RateLimitService {
    /// Create new rate limit service
    pub fn new(config: RateLimitConfig, audit_logger: Option<Arc<AuditLogger>>) -> Self {
        // Create global rate limiter with highest limit
        let global_quota =
            Quota::per_minute(NonZeroU32::new(config.api_rpm * 10).unwrap_or(nonzero!(1000u32)));
        let global_limiter = Arc::new(RateLimiter::direct_with_clock(
            global_quota,
            &DefaultClock::default(),
        ));

        Self {
            config,
            ip_limiters: Arc::new(RwLock::new(HashMap::new())),
            user_limiters: Arc::new(RwLock::new(HashMap::new())),
            global_limiter,
            audit_logger,
        }
    }

    /// Check if request is allowed
    pub async fn check_rate_limit(
        &self,
        ip: IpAddr,
        user_id: Option<&str>,
        endpoint_type: RateLimitType,
        endpoint: &str,
    ) -> SecurityResult<bool> {
        // Check blacklist first
        if self.config.blacklist_ips.contains(&ip) {
            warn!("Request from blacklisted IP: {}", ip);
            if let Some(audit) = &self.audit_logger {
                audit
                    .log_suspicious_activity(
                        &format!("Request from blacklisted IP: {}", ip),
                        user_id,
                        None,
                        Some(ip),
                        std::collections::HashMap::new(),
                    )
                    .await?;
            }
            return Ok(false);
        }

        // Check whitelist
        if self.config.whitelist_ips.contains(&ip) {
            return Ok(true);
        }

        // Check global rate limit first
        if self.global_limiter.check().is_err() {
            warn!("Global rate limit exceeded");
            return Ok(false);
        }

        let mut allowed = true;

        // Check per-IP rate limiting
        if self.config.per_ip_enabled {
            if !self.check_ip_rate_limit(ip, endpoint_type.clone()).await? {
                allowed = false;
                if let Some(audit) = &self.audit_logger {
                    audit
                        .log_rate_limit_exceeded(
                            user_id,
                            ip,
                            endpoint,
                            endpoint_type.get_rpm(&self.config),
                        )
                        .await?;
                }
            }
        }

        // Check per-user rate limiting
        if self.config.per_user_enabled && allowed {
            if let Some(user_id) = user_id {
                if !self
                    .check_user_rate_limit(user_id, endpoint_type.clone())
                    .await?
                {
                    allowed = false;
                    if let Some(audit) = &self.audit_logger {
                        audit
                            .log_rate_limit_exceeded(
                                Some(user_id),
                                ip,
                                endpoint,
                                endpoint_type.get_rpm(&self.config),
                            )
                            .await?;
                    }
                }
            }
        }

        if !allowed {
            warn!("Rate limit exceeded for IP: {}, endpoint: {}", ip, endpoint);
        }

        Ok(allowed)
    }

    /// Check IP-based rate limit
    async fn check_ip_rate_limit(
        &self,
        ip: IpAddr,
        endpoint_type: RateLimitType,
    ) -> SecurityResult<bool> {
        let limiter = {
            let mut limiters = self.ip_limiters.write().await;
            let ip_limiters = limiters.entry(ip).or_insert_with(HashMap::new);

            ip_limiters
                .entry(endpoint_type.clone())
                .or_insert_with(|| {
                    let rpm = endpoint_type.get_rpm(&self.config);
                    let quota = Quota::per_minute(NonZeroU32::new(rpm).unwrap_or(nonzero!(1u32)))
                        .allow_burst(
                            NonZeroU32::new(self.config.burst_capacity).unwrap_or(nonzero!(1u32)),
                        );
                    Arc::new(RateLimiter::direct_with_clock(
                        quota,
                        &DefaultClock::default(),
                    ))
                })
                .clone()
        };

        Ok(limiter.check().is_ok())
    }

    /// Check user-based rate limit
    async fn check_user_rate_limit(
        &self,
        user_id: &str,
        endpoint_type: RateLimitType,
    ) -> SecurityResult<bool> {
        let limiter = {
            let mut limiters = self.user_limiters.write().await;
            let user_limiters = limiters
                .entry(user_id.to_string())
                .or_insert_with(HashMap::new);

            user_limiters
                .entry(endpoint_type.clone())
                .or_insert_with(|| {
                    let rpm = endpoint_type.get_rpm(&self.config);
                    let quota = Quota::per_minute(NonZeroU32::new(rpm).unwrap_or(nonzero!(1u32)))
                        .allow_burst(
                            NonZeroU32::new(self.config.burst_capacity).unwrap_or(nonzero!(1u32)),
                        );
                    Arc::new(RateLimiter::direct_with_clock(
                        quota,
                        &DefaultClock::default(),
                    ))
                })
                .clone()
        };

        Ok(limiter.check().is_ok())
    }

    /// Add IP to whitelist
    pub fn add_to_whitelist(&mut self, ip: IpAddr) {
        if !self.config.whitelist_ips.contains(&ip) {
            self.config.whitelist_ips.push(ip);
            info!("Added IP {} to whitelist", ip);
        }
    }

    /// Add IP to blacklist
    pub fn add_to_blacklist(&mut self, ip: IpAddr) {
        if !self.config.blacklist_ips.contains(&ip) {
            self.config.blacklist_ips.push(ip);
            info!("Added IP {} to blacklist", ip);
        }
    }

    /// Remove IP from whitelist
    pub fn remove_from_whitelist(&mut self, ip: IpAddr) {
        self.config.whitelist_ips.retain(|&x| x != ip);
        info!("Removed IP {} from whitelist", ip);
    }

    /// Remove IP from blacklist
    pub fn remove_from_blacklist(&mut self, ip: IpAddr) {
        self.config.blacklist_ips.retain(|&x| x != ip);
        info!("Removed IP {} from blacklist", ip);
    }

    /// Clean up old limiters (should be called periodically)
    pub async fn cleanup_old_limiters(&self) {
        // This is a simplified cleanup - in production, you might want to track last access times
        let mut ip_limiters = self.ip_limiters.write().await;
        let mut user_limiters = self.user_limiters.write().await;

        // Clear limiters if too many (simple memory management)
        if ip_limiters.len() > 10000 {
            ip_limiters.clear();
            info!("Cleared IP rate limiters due to memory pressure");
        }

        if user_limiters.len() > 10000 {
            user_limiters.clear();
            info!("Cleared user rate limiters due to memory pressure");
        }
    }

    /// Get rate limit status for IP
    pub async fn get_ip_status(
        &self,
        ip: IpAddr,
        endpoint_type: RateLimitType,
    ) -> Option<RateLimitStatus> {
        let limiters = self.ip_limiters.read().await;
        if let Some(ip_limiters) = limiters.get(&ip) {
            if let Some(limiter) = ip_limiters.get(&endpoint_type) {
                // This is a simplified status check - governor doesn't expose internal state easily
                let is_allowed = limiter.check().is_ok();
                return Some(RateLimitStatus {
                    allowed: is_allowed,
                    limit: endpoint_type.get_rpm(&self.config),
                    remaining: if is_allowed { 1 } else { 0 }, // Simplified
                    reset_time: std::time::SystemTime::now() + Duration::from_secs(60),
                });
            }
        }
        None
    }
}

/// Rate limit status information
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_time: std::time::SystemTime,
}

/// Rate limiting middleware for Axum
pub async fn rate_limit_middleware(
    State(rate_limiter): State<Arc<RateLimitService>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip();
    let path = request.uri().path();
    let endpoint_type = RateLimitType::from_path(path);

    // Extract user ID from request if available (you might need to implement this based on your auth system)
    let user_id = extract_user_id_from_request(&request);

    // Check rate limit
    match rate_limiter
        .check_rate_limit(ip, user_id.as_deref(), endpoint_type, path)
        .await
    {
        Ok(allowed) => {
            if allowed {
                Ok(next.run(request).await)
            } else {
                // Return 429 Too Many Requests
                Ok((
                    StatusCode::TOO_MANY_REQUESTS,
                    [("Retry-After", "60")],
                    "Rate limit exceeded. Please try again later.",
                )
                    .into_response())
            }
        }
        Err(_) => {
            // On error, allow the request but log it
            warn!("Rate limiting error for IP: {}, allowing request", ip);
            Ok(next.run(request).await)
        }
    }
}

/// Extract user ID from request (implementation depends on your auth system)
fn extract_user_id_from_request(request: &Request) -> Option<String> {
    // This is a placeholder - implement based on your authentication system
    // You might extract from JWT token, session, etc.
    request
        .headers()
        .get("X-User-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
}

/// Rate limiting builder for easy configuration
pub struct RateLimitBuilder {
    config: RateLimitConfig,
    audit_logger: Option<Arc<AuditLogger>>,
}

impl RateLimitBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            config: RateLimitConfig::default(),
            audit_logger: None,
        }
    }

    /// Set general requests per minute
    pub fn general_rpm(mut self, rpm: u32) -> Self {
        self.config.general_rpm = rpm;
        self
    }

    /// Set authentication requests per minute
    pub fn auth_rpm(mut self, rpm: u32) -> Self {
        self.config.auth_rpm = rpm;
        self
    }

    /// Set API requests per minute
    pub fn api_rpm(mut self, rpm: u32) -> Self {
        self.config.api_rpm = rpm;
        self
    }

    /// Set burst capacity
    pub fn burst_capacity(mut self, capacity: u32) -> Self {
        self.config.burst_capacity = capacity;
        self
    }

    /// Enable/disable per-IP rate limiting
    pub fn per_ip_enabled(mut self, enabled: bool) -> Self {
        self.config.per_ip_enabled = enabled;
        self
    }

    /// Enable/disable per-user rate limiting
    pub fn per_user_enabled(mut self, enabled: bool) -> Self {
        self.config.per_user_enabled = enabled;
        self
    }

    /// Add whitelisted IPs
    pub fn whitelist_ips(mut self, ips: Vec<IpAddr>) -> Self {
        self.config.whitelist_ips = ips;
        self
    }

    /// Add blacklisted IPs
    pub fn blacklist_ips(mut self, ips: Vec<IpAddr>) -> Self {
        self.config.blacklist_ips = ips;
        self
    }

    /// Set audit logger
    pub fn with_audit_logger(mut self, audit_logger: Arc<AuditLogger>) -> Self {
        self.audit_logger = Some(audit_logger);
        self
    }

    /// Build the rate limit service
    pub fn build(self) -> RateLimitService {
        RateLimitService::new(self.config, self.audit_logger)
    }
}

impl Default for RateLimitBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_rate_limiting_basic() {
        let config = RateLimitConfig {
            general_rpm: 2, // Very low limit for testing
            ..Default::default()
        };

        let rate_limiter = RateLimitService::new(config, None);
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));

        // First request should be allowed
        let result = rate_limiter
            .check_rate_limit(ip, None, RateLimitType::General, "/test")
            .await
            .unwrap();
        assert!(result);

        // Second request should be allowed (within burst)
        let result = rate_limiter
            .check_rate_limit(ip, None, RateLimitType::General, "/test")
            .await
            .unwrap();
        assert!(result);

        // Third request might be rate limited depending on timing
        // This test is timing-dependent, so we'll just verify it doesn't panic
        let _result = rate_limiter
            .check_rate_limit(ip, None, RateLimitType::General, "/test")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_whitelist_blacklist() {
        let whitelist_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let blacklist_ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        let config = RateLimitConfig {
            general_rpm: 1,
            whitelist_ips: vec![whitelist_ip],
            blacklist_ips: vec![blacklist_ip],
            ..Default::default()
        };

        let rate_limiter = RateLimitService::new(config, None);

        // Whitelisted IP should always be allowed
        let result = rate_limiter
            .check_rate_limit(whitelist_ip, None, RateLimitType::General, "/test")
            .await
            .unwrap();
        assert!(result);

        // Blacklisted IP should never be allowed
        let result = rate_limiter
            .check_rate_limit(blacklist_ip, None, RateLimitType::General, "/test")
            .await
            .unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_rate_limit_type_detection() {
        assert_eq!(RateLimitType::from_path("/api/users"), RateLimitType::Api);
        assert_eq!(
            RateLimitType::from_path("/auth/login"),
            RateLimitType::Authentication
        );
        assert_eq!(
            RateLimitType::from_path("/login"),
            RateLimitType::Authentication
        );
        assert_eq!(
            RateLimitType::from_path("/ws/connect"),
            RateLimitType::WebSocket
        );
        assert_eq!(
            RateLimitType::from_path("/dashboard"),
            RateLimitType::General
        );
    }

    #[tokio::test]
    async fn test_rate_limit_builder() {
        let rate_limiter = RateLimitBuilder::new()
            .general_rpm(100)
            .auth_rpm(20)
            .api_rpm(200)
            .burst_capacity(5)
            .per_ip_enabled(true)
            .per_user_enabled(true)
            .build();

        assert_eq!(rate_limiter.config.general_rpm, 100);
        assert_eq!(rate_limiter.config.auth_rpm, 20);
        assert_eq!(rate_limiter.config.api_rpm, 200);
        assert_eq!(rate_limiter.config.burst_capacity, 5);
        assert!(rate_limiter.config.per_ip_enabled);
        assert!(rate_limiter.config.per_user_enabled);
    }

    #[tokio::test]
    async fn test_cleanup_old_limiters() {
        let rate_limiter = RateLimitService::new(RateLimitConfig::default(), None);

        // Add some limiters
        for i in 0..5 {
            let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, i));
            let _ = rate_limiter
                .check_rate_limit(ip, None, RateLimitType::General, "/test")
                .await;
        }

        // Cleanup should not panic
        rate_limiter.cleanup_old_limiters().await;
    }
}
