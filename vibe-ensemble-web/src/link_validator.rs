//! Link Validation & Navigation Integrity System
//!
//! Provides comprehensive link validation, health monitoring, and navigation integrity
//! checking for the Vibe Ensemble web dashboard. Ensures all navigation links,
//! API endpoints, and WebSocket connections are functional and accessible.

use crate::Result;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};
use tokio::time::interval;
use vibe_ensemble_storage::StorageManager;

/// Link validation status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LinkStatus {
    /// Link is healthy and accessible
    Healthy,
    /// Link has warning but is functional
    Warning,
    /// Link is broken or inaccessible
    Broken,
    /// Link validation is pending
    Pending,
    /// Link was not tested yet
    Unknown,
}

/// Types of links that can be validated
#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub enum LinkType {
    /// Navigation links in templates
    Navigation,
    /// API endpoints
    Api,
    /// WebSocket connections
    WebSocket,
    /// Static assets (CSS, JS, images)
    Asset,
    /// External links
    External,
    /// Template references
    Template,
}

/// Validation result for a specific link
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkValidationResult {
    pub url: String,
    pub link_type: LinkType,
    pub status: LinkStatus,
    pub response_time: Option<Duration>,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
    pub last_checked: DateTime<Utc>,
    pub check_count: u32,
    pub success_rate: f64,
}

impl LinkValidationResult {
    pub fn new(url: String, link_type: LinkType) -> Self {
        Self {
            url,
            link_type,
            status: LinkStatus::Unknown,
            response_time: None,
            status_code: None,
            error_message: None,
            last_checked: Utc::now(),
            check_count: 0,
            success_rate: 0.0,
        }
    }

    pub fn update_result(
        &mut self,
        status: LinkStatus,
        response_time: Option<Duration>,
        status_code: Option<u16>,
        error: Option<String>,
    ) {
        let _was_success = matches!(self.status, LinkStatus::Healthy);
        let is_success = matches!(status, LinkStatus::Healthy);

        self.status = status;
        self.response_time = response_time;
        self.status_code = status_code;
        self.error_message = error;
        self.last_checked = Utc::now();
        self.check_count += 1;

        // Update success rate
        if self.check_count == 1 {
            self.success_rate = if is_success { 1.0 } else { 0.0 };
        } else {
            let previous_successes =
                (self.success_rate * (self.check_count - 1) as f64).round() as u32;
            let current_successes = previous_successes + if is_success { 1 } else { 0 };
            self.success_rate = current_successes as f64 / self.check_count as f64;
        }
    }
}

/// Navigation analytics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NavigationAnalytics {
    pub path: String,
    pub visit_count: u64,
    pub error_count: u64,
    pub last_visit: DateTime<Utc>,
    pub average_response_time: Option<Duration>,
    pub user_agents: HashSet<String>,
}

/// Link validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Timeout for HTTP requests
    pub request_timeout: Duration,
    /// Interval between automatic validations
    pub validation_interval: Duration,
    /// Maximum concurrent validation requests
    pub max_concurrent_validations: usize,
    /// Enable deep validation (follow redirects, check content)
    pub deep_validation: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            request_timeout: Duration::from_secs(10),
            validation_interval: Duration::from_secs(300), // 5 minutes
            max_concurrent_validations: 10,
            deep_validation: false,
        }
    }
}

/// Core link validator system
#[derive(Clone)]
pub struct LinkValidator {
    /// Validation results storage
    results: Arc<RwLock<HashMap<String, LinkValidationResult>>>,
    /// Navigation analytics
    analytics: Arc<RwLock<HashMap<String, NavigationAnalytics>>>,
    /// Known application routes
    app_routes: Arc<RwLock<HashSet<String>>>,
    /// Configuration
    config: ValidationConfig,
    /// HTTP client
    client: reqwest::Client,
    /// Storage manager for persistence
    #[allow(dead_code)]
    storage: Option<Arc<StorageManager>>,
}

impl LinkValidator {
    /// Create a new link validator
    pub fn new(config: ValidationConfig, storage: Option<Arc<StorageManager>>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .user_agent("vibe-ensemble-link-validator/0.1.0")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
            analytics: Arc::new(RwLock::new(HashMap::new())),
            app_routes: Arc::new(RwLock::new(HashSet::new())),
            config,
            client,
            storage,
        }
    }

    /// Register application routes for validation
    pub fn register_routes(&self, routes: Vec<String>) {
        let mut app_routes = self
            .app_routes
            .write()
            .expect("Failed to acquire write lock");
        for route in routes {
            app_routes.insert(route);
        }
    }

    /// Discover links from templates and handlers
    pub async fn discover_links(&self) -> Result<Vec<(String, LinkType)>> {
        let mut links = Vec::new();

        // Add known application routes
        {
            let app_routes = self.app_routes.read().expect("Failed to acquire read lock");
            for route in app_routes.iter() {
                if route.starts_with("/api/") {
                    links.push((route.clone(), LinkType::Api));
                } else if route == "/ws" {
                    links.push((route.clone(), LinkType::WebSocket));
                } else {
                    links.push((route.clone(), LinkType::Navigation));
                }
            }
        }

        // Parse templates for navigation links
        links.extend(self.parse_template_links().await?);

        Ok(links)
    }

    /// Parse templates to extract navigation links
    async fn parse_template_links(&self) -> Result<Vec<(String, LinkType)>> {
        let mut links = Vec::new();

        // Template-based navigation links found in base.html
        let template_links = vec![
            ("/dashboard", LinkType::Navigation),
            ("/messages", LinkType::Navigation),
            ("/link-health", LinkType::Navigation),
            ("/agents", LinkType::Navigation),
            ("/issues", LinkType::Navigation),
            ("/knowledge", LinkType::Navigation),
            ("/admin", LinkType::Navigation),
            ("/api/health", LinkType::Api),
            ("/logout", LinkType::Navigation),
        ];

        for (url, link_type) in template_links {
            links.push((url.to_string(), link_type));
        }

        Ok(links)
    }

    /// Validate a single link
    pub async fn validate_link(&self, url: &str, base_url: &str) -> Result<LinkValidationResult> {
        let full_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("{}{}", base_url.trim_end_matches('/'), url)
        };

        let link_type = self.determine_link_type(url);
        let mut result = LinkValidationResult::new(url.to_string(), link_type.clone());

        let start_time = Instant::now();

        match link_type {
            LinkType::Api => match self.client.get(&full_url).send().await {
                Ok(response) => {
                    let response_time = start_time.elapsed();
                    let status_code = response.status().as_u16();

                    let status = if response.status().is_success() {
                        LinkStatus::Healthy
                    } else if response.status().is_client_error()
                        || response.status().is_server_error()
                    {
                        LinkStatus::Broken
                    } else {
                        LinkStatus::Warning
                    };

                    result.update_result(status, Some(response_time), Some(status_code), None);
                }
                Err(e) => {
                    let response_time = start_time.elapsed();
                    result.update_result(
                        LinkStatus::Broken,
                        Some(response_time),
                        None,
                        Some(e.to_string()),
                    );
                }
            },
            LinkType::Navigation => {
                // For navigation links, we'll check if they're handled by our router
                let status = if self.is_route_registered(url) {
                    LinkStatus::Healthy
                } else {
                    LinkStatus::Broken
                };
                let error_msg = if matches!(status, LinkStatus::Broken) {
                    Some("Route not registered in application router".to_string())
                } else {
                    None
                };
                result.update_result(status, None, None, error_msg);
            }
            LinkType::WebSocket => {
                // WebSocket validation would require more complex testing
                result.update_result(
                    LinkStatus::Warning,
                    None,
                    None,
                    Some("WebSocket validation not fully implemented".to_string()),
                );
            }
            _ => {
                result.update_result(
                    LinkStatus::Warning,
                    None,
                    None,
                    Some("Link type validation not implemented".to_string()),
                );
            }
        }

        Ok(result)
    }

    /// Check if a route is registered in the application
    pub fn is_route_registered(&self, route: &str) -> bool {
        let app_routes = self.app_routes.read().expect("Failed to acquire read lock");
        app_routes.contains(route)
    }

    /// Determine link type based on URL pattern
    pub fn determine_link_type(&self, url: &str) -> LinkType {
        if url.starts_with("/api/") {
            LinkType::Api
        } else if url == "/ws" {
            LinkType::WebSocket
        } else if url.starts_with("http://") || url.starts_with("https://") {
            LinkType::External
        } else {
            LinkType::Navigation
        }
    }

    /// Validate all discovered links
    pub async fn validate_all_links(&self, base_url: &str) -> Result<Vec<LinkValidationResult>> {
        let links = self.discover_links().await?;
        let mut results = Vec::new();

        for (url, _) in links {
            let result = self.validate_link(&url, base_url).await?;

            // Store result in cache
            {
                let mut cache = self.results.write().expect("Failed to acquire write lock");
                cache.insert(url.clone(), result.clone());
            }

            results.push(result);
        }

        Ok(results)
    }

    /// Get validation results for all links
    pub fn get_all_results(&self) -> Vec<LinkValidationResult> {
        let results = self.results.read().expect("Failed to acquire read lock");
        results.values().cloned().collect()
    }

    /// Get validation result for a specific link
    pub fn get_result(&self, url: &str) -> Option<LinkValidationResult> {
        let results = self.results.read().expect("Failed to acquire read lock");
        results.get(url).cloned()
    }

    /// Get health summary statistics
    pub fn get_health_summary(&self) -> HealthSummary {
        let results = self.results.read().expect("Failed to acquire read lock");
        let mut summary = HealthSummary::default();

        for result in results.values() {
            summary.total_links += 1;
            match result.status {
                LinkStatus::Healthy => summary.healthy_links += 1,
                LinkStatus::Warning => summary.warning_links += 1,
                LinkStatus::Broken => summary.broken_links += 1,
                LinkStatus::Pending => summary.pending_links += 1,
                LinkStatus::Unknown => summary.unknown_links += 1,
            }

            if let Some(response_time) = result.response_time {
                summary.total_response_time += response_time;
                summary.response_time_count += 1;
            }
        }

        if summary.response_time_count > 0 {
            summary.average_response_time =
                Some(summary.total_response_time / summary.response_time_count as u32);
        }

        summary.health_score = if summary.total_links > 0 {
            (summary.healthy_links as f64 / summary.total_links as f64) * 100.0
        } else {
            100.0
        };

        summary
    }

    /// Record navigation analytics
    pub fn record_navigation(
        &self,
        path: &str,
        user_agent: Option<&str>,
        response_time: Option<Duration>,
    ) {
        let mut analytics = self
            .analytics
            .write()
            .expect("Failed to acquire write lock");

        let entry = analytics
            .entry(path.to_string())
            .or_insert_with(|| NavigationAnalytics {
                path: path.to_string(),
                visit_count: 0,
                error_count: 0,
                last_visit: Utc::now(),
                average_response_time: None,
                user_agents: HashSet::new(),
            });

        entry.visit_count += 1;
        entry.last_visit = Utc::now();

        if let Some(ua) = user_agent {
            entry.user_agents.insert(ua.to_string());
        }

        if let Some(rt) = response_time {
            entry.average_response_time = Some(
                entry
                    .average_response_time
                    .map(|avg| Duration::from_nanos((avg.as_nanos() + rt.as_nanos()) as u64 / 2))
                    .unwrap_or(rt),
            );
        }
    }

    /// Get navigation analytics
    pub fn get_analytics(&self) -> Vec<NavigationAnalytics> {
        let analytics = self.analytics.read().expect("Failed to acquire read lock");
        analytics.values().cloned().collect()
    }

    /// Start background validation service
    pub async fn start_background_validation(&self, base_url: String) -> Result<()> {
        let validator = self.clone();
        let interval_duration = self.config.validation_interval;

        tokio::spawn(async move {
            let mut interval = interval(interval_duration);

            loop {
                interval.tick().await;

                if let Err(e) = validator.validate_all_links(&base_url).await {
                    tracing::error!("Background link validation failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Generate repair suggestions for broken links
    pub fn generate_repair_suggestions(&self, broken_url: &str) -> Vec<LinkRepairSuggestion> {
        let mut suggestions = Vec::new();

        // Get all known routes for comparison
        let app_routes = self.app_routes.read().expect("Failed to acquire read lock");

        // 1. Check for similar URLs (typo correction)
        if let Some(similar_url) = self.find_similar_url(broken_url, &app_routes) {
            suggestions.push(LinkRepairSuggestion {
                broken_url: broken_url.to_string(),
                suggested_url: similar_url.clone(),
                confidence: self.calculate_similarity_confidence(broken_url, &similar_url),
                reason: "Similar URL found - possible typo correction".to_string(),
                repair_type: RepairType::UrlCorrection,
            });
        }

        // 2. Check for missing route handlers
        if self.is_navigation_link(broken_url) && !app_routes.contains(broken_url) {
            suggestions.push(LinkRepairSuggestion {
                broken_url: broken_url.to_string(),
                suggested_url: broken_url.to_string(),
                confidence: 0.9,
                reason: "Route handler missing - needs implementation".to_string(),
                repair_type: RepairType::MissingHandler,
            });
        }

        // 3. Suggest alternative endpoints
        if let Some(alternative) = self.suggest_alternative_endpoint(broken_url) {
            suggestions.push(LinkRepairSuggestion {
                broken_url: broken_url.to_string(),
                suggested_url: alternative.clone(),
                confidence: 0.7,
                reason: "Alternative endpoint available".to_string(),
                repair_type: RepairType::AlternativeEndpoint,
            });
        }

        // 4. Suggest redirect rules for moved content
        if let Some(redirect_target) = self.suggest_redirect_target(broken_url) {
            suggestions.push(LinkRepairSuggestion {
                broken_url: broken_url.to_string(),
                suggested_url: redirect_target.clone(),
                confidence: 0.6,
                reason: "Content may have moved - redirect suggested".to_string(),
                repair_type: RepairType::RedirectRule,
            });
        }

        suggestions
    }

    /// Find similar URLs using string distance algorithms
    fn find_similar_url(&self, target: &str, available_urls: &HashSet<String>) -> Option<String> {
        let mut best_match = None;
        let mut best_distance = f64::INFINITY;

        for url in available_urls {
            let distance = self.calculate_string_distance(target, url);
            if distance < best_distance && distance < 0.3 {
                // 30% difference threshold
                best_distance = distance;
                best_match = Some(url.clone());
            }
        }

        best_match
    }

    /// Calculate similarity confidence based on string distance
    fn calculate_similarity_confidence(&self, url1: &str, url2: &str) -> f64 {
        let distance = self.calculate_string_distance(url1, url2);
        (1.0 - distance).clamp(0.0, 1.0)
    }

    /// Simple Levenshtein distance calculation
    pub fn calculate_string_distance(&self, s1: &str, s2: &str) -> f64 {
        let c1: Vec<char> = s1.chars().collect();
        let c2: Vec<char> = s2.chars().collect();
        let len1 = c1.len();
        let len2 = c2.len();

        if len1 == 0 {
            return len2 as f64 / len2.max(1) as f64;
        }
        if len2 == 0 {
            return len1 as f64 / len1.max(1) as f64;
        }

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
            row[0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if c1[i - 1] == c2[j - 1] { 0 } else { 1 };
                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2] as f64 / len1.max(len2) as f64
    }

    /// Check if URL is a navigation link
    fn is_navigation_link(&self, url: &str) -> bool {
        !url.starts_with("/api/") && !url.starts_with("http://") && !url.starts_with("https://")
    }

    /// Suggest alternative endpoint based on patterns
    fn suggest_alternative_endpoint(&self, broken_url: &str) -> Option<String> {
        // Common alternative patterns
        let alternatives = vec![
            ("/agents", "/api/agents"),
            ("/issues", "/api/issues"),
            ("/knowledge", "/api/knowledge"),
            ("/admin", "/dashboard"),
            ("/settings", "/admin"),
            ("/config", "/admin"),
        ];

        for (pattern, alternative) in alternatives {
            if broken_url == pattern {
                return Some(alternative.to_string());
            }
        }

        None
    }

    /// Suggest redirect target for moved content
    fn suggest_redirect_target(&self, broken_url: &str) -> Option<String> {
        // Common redirect patterns
        let redirects = vec![
            ("/home", "/dashboard"),
            ("/main", "/dashboard"),
            ("/index", "/dashboard"),
            ("/login", "/auth/login"),
            ("/logout", "/auth/logout"),
            ("/status", "/api/health"),
            ("/health", "/api/health"),
        ];

        for (old_url, new_url) in redirects {
            if broken_url == old_url {
                return Some(new_url.to_string());
            }
        }

        None
    }

    /// Apply automatic repairs for safe issues
    pub async fn apply_auto_repairs(&self, config: &AutoRepairConfig) -> Result<Vec<String>> {
        if !config.enabled {
            return Ok(vec!["Auto-repair is disabled".to_string()]);
        }

        let mut applied_repairs = Vec::new();
        let results = self.get_all_results();

        for result in results {
            if matches!(result.status, LinkStatus::Broken) {
                let suggestions = self.generate_repair_suggestions(&result.url);

                for suggestion in suggestions {
                    if suggestion.confidence >= config.confidence_threshold {
                        match suggestion.repair_type {
                            RepairType::UrlCorrection if config.auto_fix_safe_issues => {
                                // Only log the suggestion - actual URL correction would require template updates
                                applied_repairs.push(format!(
                                    "Suggested URL correction: {} -> {}",
                                    suggestion.broken_url, suggestion.suggested_url
                                ));
                            }
                            RepairType::RedirectRule if config.create_redirects => {
                                // Log redirect rule suggestion
                                applied_repairs.push(format!(
                                    "Redirect rule suggested: {} -> {}",
                                    suggestion.broken_url, suggestion.suggested_url
                                ));
                            }
                            RepairType::MissingHandler => {
                                applied_repairs.push(format!(
                                    "Handler needed for route: {}",
                                    suggestion.broken_url
                                ));
                            }
                            _ => {
                                if config.suggest_alternatives {
                                    applied_repairs.push(format!(
                                        "Alternative suggested for {}: {}",
                                        suggestion.broken_url, suggestion.suggested_url
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(applied_repairs)
    }
}

/// Health summary statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HealthSummary {
    pub total_links: usize,
    pub healthy_links: usize,
    pub warning_links: usize,
    pub broken_links: usize,
    pub pending_links: usize,
    pub unknown_links: usize,
    pub health_score: f64, // 0-100%
    pub average_response_time: Option<Duration>,
    #[serde(skip)]
    pub total_response_time: Duration,
    #[serde(skip)]
    pub response_time_count: usize,
}

/// Link repair suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRepairSuggestion {
    pub broken_url: String,
    pub suggested_url: String,
    pub confidence: f64, // 0.0 to 1.0
    pub reason: String,
    pub repair_type: RepairType,
}

/// Type of repair suggested
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RepairType {
    /// URL pattern correction (e.g., fix typos)
    UrlCorrection,
    /// Route mapping update needed
    RouteMapping,
    /// Redirect rule needed
    RedirectRule,
    /// Alternative endpoint available
    AlternativeEndpoint,
    /// Handler implementation missing
    MissingHandler,
    /// Template update required
    TemplateUpdate,
}

/// Auto-repair configuration
#[derive(Debug, Clone)]
pub struct AutoRepairConfig {
    pub enabled: bool,
    pub confidence_threshold: f64,
    pub auto_fix_safe_issues: bool,
    pub create_redirects: bool,
    pub suggest_alternatives: bool,
}

impl Default for AutoRepairConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            confidence_threshold: 0.8,
            auto_fix_safe_issues: false,
            create_redirects: false,
            suggest_alternatives: true,
        }
    }
}

/// Query parameters for link validation API
#[derive(Debug, Deserialize)]
pub struct ValidationQuery {
    pub link_type: Option<String>,
    pub status: Option<String>,
    pub min_success_rate: Option<f64>,
}

/// Create router for link validation API endpoints
pub fn create_router() -> Router<Arc<StorageManager>> {
    Router::new()
        .route("/api/links/validate", get(validate_links_handler))
        .route("/api/links/status", get(link_status_handler))
        .route("/api/links/health", get(health_summary_handler))
        .route("/api/links/analytics", get(analytics_handler))
        .route(
            "/api/links/:url/validate",
            get(validate_single_link_handler),
        )
        .route(
            "/api/links/:url/repair-suggestions",
            get(repair_suggestions_handler),
        )
        .route("/api/links/auto-repair", get(auto_repair_handler))
}

/// API handler to trigger link validation
pub async fn validate_links_handler(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register known routes
    validator.register_routes(get_known_routes());

    let base_url = "http://127.0.0.1:8081"; // TODO: Make this configurable
    let results = validator.validate_all_links(base_url).await?;

    Ok(Json(serde_json::json!({
        "status": "completed",
        "results": results,
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// API handler to get link status
pub async fn link_status_handler(
    State(storage): State<Arc<StorageManager>>,
    Query(query): Query<ValidationQuery>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));
    let results = validator.get_all_results();

    // Apply filters
    let filtered_results: Vec<_> = results
        .into_iter()
        .filter(|result| {
            if let Some(link_type) = &query.link_type {
                format!("{:?}", result.link_type).to_lowercase() == link_type.to_lowercase()
            } else {
                true
            }
        })
        .filter(|result| {
            if let Some(status) = &query.status {
                format!("{:?}", result.status).to_lowercase() == status.to_lowercase()
            } else {
                true
            }
        })
        .filter(|result| {
            if let Some(min_rate) = query.min_success_rate {
                result.success_rate >= min_rate
            } else {
                true
            }
        })
        .collect();

    Ok(Json(serde_json::json!({
        "links": filtered_results,
        "total": filtered_results.len(),
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// API handler to get health summary
pub async fn health_summary_handler(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));
    let summary = validator.get_health_summary();

    Ok(Json(summary))
}

/// API handler to get navigation analytics
pub async fn analytics_handler(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));
    let analytics = validator.get_analytics();

    Ok(Json(serde_json::json!({
        "analytics": analytics,
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// API handler to validate a single link
pub async fn validate_single_link_handler(
    State(storage): State<Arc<StorageManager>>,
    Path(url): Path<String>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));
    let base_url = "http://127.0.0.1:8081"; // TODO: Make this configurable

    let result = validator.validate_link(&url, base_url).await?;

    Ok(Json(serde_json::json!({
        "result": result,
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// Get list of known application routes
/// API handler to get repair suggestions for a link
pub async fn repair_suggestions_handler(
    State(storage): State<Arc<StorageManager>>,
    Path(url): Path<String>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register known routes
    validator.register_routes(get_known_routes());

    let suggestions = validator.generate_repair_suggestions(&url);

    Ok(Json(serde_json::json!({
        "url": url,
        "suggestions": suggestions,
        "timestamp": Utc::now().to_rfc3339()
    })))
}

/// API handler to run auto-repair
pub async fn auto_repair_handler(
    State(storage): State<Arc<StorageManager>>,
) -> Result<impl IntoResponse> {
    let validator = LinkValidator::new(ValidationConfig::default(), Some(storage));

    // Register known routes
    validator.register_routes(get_known_routes());

    let repair_config = AutoRepairConfig::default();
    let applied_repairs = validator.apply_auto_repairs(&repair_config).await?;

    Ok(Json(serde_json::json!({
        "status": "completed",
        "repairs_applied": applied_repairs,
        "config": {
            "enabled": repair_config.enabled,
            "confidence_threshold": repair_config.confidence_threshold,
            "auto_fix_safe_issues": repair_config.auto_fix_safe_issues,
            "create_redirects": repair_config.create_redirects,
            "suggest_alternatives": repair_config.suggest_alternatives
        },
        "timestamp": Utc::now().to_rfc3339()
    })))
}

fn get_known_routes() -> Vec<String> {
    vec![
        "/".to_string(),
        "/dashboard".to_string(),
        "/link-health".to_string(),
        "/api/health".to_string(),
        "/api/stats".to_string(),
        "/api/agents".to_string(),
        "/api/agents/:id".to_string(),
        "/api/issues".to_string(),
        "/api/issues/:id".to_string(),
        "/api/links/validate".to_string(),
        "/api/links/status".to_string(),
        "/api/links/health".to_string(),
        "/api/links/analytics".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_link_validator_creation() {
        let validator = LinkValidator::new(ValidationConfig::default(), None);
        assert!(validator.get_all_results().is_empty());
    }

    #[tokio::test]
    async fn test_route_registration() {
        let validator = LinkValidator::new(ValidationConfig::default(), None);
        let routes = vec!["/test".to_string(), "/api/test".to_string()];
        validator.register_routes(routes);

        assert!(validator.is_route_registered("/test"));
        assert!(validator.is_route_registered("/api/test"));
        assert!(!validator.is_route_registered("/nonexistent"));
    }

    #[tokio::test]
    async fn test_link_type_determination() {
        let validator = LinkValidator::new(ValidationConfig::default(), None);

        assert_eq!(validator.determine_link_type("/api/test"), LinkType::Api);
        assert_eq!(validator.determine_link_type("/ws"), LinkType::WebSocket);
        assert_eq!(
            validator.determine_link_type("https://example.com"),
            LinkType::External
        );
        assert_eq!(
            validator.determine_link_type("/dashboard"),
            LinkType::Navigation
        );
    }

    #[test]
    fn test_validation_result_updates() {
        let mut result = LinkValidationResult::new("/test".to_string(), LinkType::Navigation);

        result.update_result(LinkStatus::Healthy, None, Some(200), None);
        assert_eq!(result.status, LinkStatus::Healthy);
        assert_eq!(result.success_rate, 1.0);
        assert_eq!(result.check_count, 1);

        result.update_result(LinkStatus::Broken, None, Some(404), None);
        assert_eq!(result.status, LinkStatus::Broken);
        assert_eq!(result.success_rate, 0.5);
        assert_eq!(result.check_count, 2);
    }
}
