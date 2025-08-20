//! Comprehensive audit logging system

use crate::{SecurityError, SecurityResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;
use std::net::IpAddr;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Audit event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    // Authentication events
    LoginSuccess,
    LoginFailure,
    Logout,
    PasswordChanged,
    TokenRefreshed,
    TokenExpired,
    AccountLocked,
    AccountUnlocked,

    // Authorization events
    PermissionGranted,
    PermissionDenied,
    RoleChanged,

    // User management
    UserCreated,
    UserUpdated,
    UserDeleted,
    UserActivated,
    UserDeactivated,

    // Agent management
    AgentRegistered,
    AgentUpdated,
    AgentDeactivated,
    AgentTokenCreated,
    AgentTokenRevoked,

    // Resource operations
    IssueCreated,
    IssueUpdated,
    IssueDeleted,
    IssueAssigned,
    KnowledgeCreated,
    KnowledgeUpdated,
    KnowledgeDeleted,
    MessageSent,
    MessageReceived,

    // System events
    SystemConfigChanged,
    DatabaseMigration,
    BackupCreated,
    BackupRestored,

    // Security events
    SuspiciousActivity,
    RateLimitExceeded,
    InvalidTokenUsed,
    UnauthorizedAccess,
    DataExport,
    DataImport,
    EncryptionKeyRotated,

    // Custom events
    Custom(String),
}

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventType::LoginSuccess => write!(f, "LoginSuccess"),
            AuditEventType::LoginFailure => write!(f, "LoginFailure"),
            AuditEventType::Logout => write!(f, "Logout"),
            AuditEventType::PasswordChanged => write!(f, "PasswordChanged"),
            AuditEventType::TokenRefreshed => write!(f, "TokenRefreshed"),
            AuditEventType::TokenExpired => write!(f, "TokenExpired"),
            AuditEventType::AccountLocked => write!(f, "AccountLocked"),
            AuditEventType::AccountUnlocked => write!(f, "AccountUnlocked"),
            AuditEventType::PermissionGranted => write!(f, "PermissionGranted"),
            AuditEventType::PermissionDenied => write!(f, "PermissionDenied"),
            AuditEventType::RoleChanged => write!(f, "RoleChanged"),
            AuditEventType::UserCreated => write!(f, "UserCreated"),
            AuditEventType::UserUpdated => write!(f, "UserUpdated"),
            AuditEventType::UserDeleted => write!(f, "UserDeleted"),
            AuditEventType::UserActivated => write!(f, "UserActivated"),
            AuditEventType::UserDeactivated => write!(f, "UserDeactivated"),
            AuditEventType::AgentRegistered => write!(f, "AgentRegistered"),
            AuditEventType::AgentUpdated => write!(f, "AgentUpdated"),
            AuditEventType::AgentDeactivated => write!(f, "AgentDeactivated"),
            AuditEventType::AgentTokenCreated => write!(f, "AgentTokenCreated"),
            AuditEventType::AgentTokenRevoked => write!(f, "AgentTokenRevoked"),
            AuditEventType::IssueCreated => write!(f, "IssueCreated"),
            AuditEventType::IssueUpdated => write!(f, "IssueUpdated"),
            AuditEventType::IssueDeleted => write!(f, "IssueDeleted"),
            AuditEventType::IssueAssigned => write!(f, "IssueAssigned"),
            AuditEventType::KnowledgeCreated => write!(f, "KnowledgeCreated"),
            AuditEventType::KnowledgeUpdated => write!(f, "KnowledgeUpdated"),
            AuditEventType::KnowledgeDeleted => write!(f, "KnowledgeDeleted"),
            AuditEventType::MessageSent => write!(f, "MessageSent"),
            AuditEventType::MessageReceived => write!(f, "MessageReceived"),
            AuditEventType::SystemConfigChanged => write!(f, "SystemConfigChanged"),
            AuditEventType::DatabaseMigration => write!(f, "DatabaseMigration"),
            AuditEventType::BackupCreated => write!(f, "BackupCreated"),
            AuditEventType::BackupRestored => write!(f, "BackupRestored"),
            AuditEventType::SuspiciousActivity => write!(f, "SuspiciousActivity"),
            AuditEventType::RateLimitExceeded => write!(f, "RateLimitExceeded"),
            AuditEventType::InvalidTokenUsed => write!(f, "InvalidTokenUsed"),
            AuditEventType::UnauthorizedAccess => write!(f, "UnauthorizedAccess"),
            AuditEventType::DataExport => write!(f, "DataExport"),
            AuditEventType::DataImport => write!(f, "DataImport"),
            AuditEventType::EncryptionKeyRotated => write!(f, "EncryptionKeyRotated"),
            AuditEventType::Custom(s) => write!(f, "Custom({})", s),
        }
    }
}

/// Audit event severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditSeverity::Low => write!(f, "LOW"),
            AuditSeverity::Medium => write!(f, "MEDIUM"),
            AuditSeverity::High => write!(f, "HIGH"),
            AuditSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Audit event record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEvent {
    pub id: String,
    pub event_type: String, // JSON serialized AuditEventType
    pub severity: String,   // JSON serialized AuditSeverity
    pub user_id: Option<String>,
    pub username: Option<String>,
    pub agent_id: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub action: Option<String>,
    pub description: String,
    pub metadata: String, // JSON serialized HashMap<String, String>
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub session_id: Option<String>,
    pub result: String, // "success" or "failure"
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(event_type: AuditEventType, severity: AuditSeverity, description: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            event_type: serde_json::to_string(&event_type).unwrap_or_default(),
            severity: serde_json::to_string(&severity).unwrap_or_default(),
            user_id: None,
            username: None,
            agent_id: None,
            resource_type: None,
            resource_id: None,
            action: None,
            description,
            metadata: serde_json::to_string(&HashMap::<String, String>::new()).unwrap(),
            ip_address: None,
            user_agent: None,
            session_id: None,
            result: "success".to_string(),
            error_message: None,
            timestamp: Utc::now(),
        }
    }

    /// Set user information
    pub fn with_user(mut self, user_id: &str, username: &str) -> Self {
        self.user_id = Some(user_id.to_string());
        self.username = Some(username.to_string());
        self
    }

    /// Set agent information
    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }

    /// Set resource information
    pub fn with_resource(mut self, resource_type: &str, resource_id: &str) -> Self {
        self.resource_type = Some(resource_type.to_string());
        self.resource_id = Some(resource_id.to_string());
        self
    }

    /// Set action
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = Some(action.to_string());
        self
    }

    /// Set session information
    pub fn with_session(
        mut self,
        session_id: &str,
        ip_address: Option<IpAddr>,
        user_agent: Option<&str>,
    ) -> Self {
        self.session_id = Some(session_id.to_string());
        self.ip_address = ip_address.map(|ip| ip.to_string());
        self.user_agent = user_agent.map(|ua| ua.to_string());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = serde_json::to_string(&metadata).unwrap_or_default();
        self
    }

    /// Mark as failure with error message
    pub fn with_failure(mut self, error_message: &str) -> Self {
        self.result = "failure".to_string();
        self.error_message = Some(error_message.to_string());
        self
    }

    /// Get parsed event type
    pub fn get_event_type(&self) -> AuditEventType {
        serde_json::from_str(&self.event_type)
            .unwrap_or(AuditEventType::Custom("unknown".to_string()))
    }

    /// Get parsed severity
    pub fn get_severity(&self) -> AuditSeverity {
        serde_json::from_str(&self.severity).unwrap_or(AuditSeverity::Low)
    }

    /// Get parsed metadata
    pub fn get_metadata(&self) -> HashMap<String, String> {
        serde_json::from_str(&self.metadata).unwrap_or_default()
    }
}

/// Audit logging service
#[derive(Debug, Clone)]
pub struct AuditLogger {
    db_pool: Pool<Sqlite>,
    /// Whether to log to console in addition to database
    log_to_console: bool,
    /// Minimum severity level to log
    min_severity: AuditSeverity,
}

impl AuditLogger {
    /// Create new audit logger
    pub fn new(db_pool: Pool<Sqlite>, log_to_console: bool, min_severity: AuditSeverity) -> Self {
        Self {
            db_pool,
            log_to_console,
            min_severity,
        }
    }

    /// Create with default settings
    pub fn with_defaults(db_pool: Pool<Sqlite>) -> Self {
        Self::new(db_pool, true, AuditSeverity::Low)
    }

    /// Log an audit event
    pub async fn log_event(&self, event: AuditEvent) -> SecurityResult<()> {
        // Check if event meets minimum severity level
        if event.get_severity() < self.min_severity {
            return Ok(());
        }

        // Log to console if enabled
        if self.log_to_console {
            self.log_to_console_output(&event);
        }

        // Store in database
        self.store_event(&event).await?;

        Ok(())
    }

    /// Log authentication success
    pub async fn log_auth_success(
        &self,
        user_id: &str,
        username: &str,
        session_id: &str,
        ip_address: Option<IpAddr>,
        user_agent: Option<&str>,
    ) -> SecurityResult<()> {
        let event = AuditEvent::new(
            AuditEventType::LoginSuccess,
            AuditSeverity::Low,
            format!("User '{}' successfully authenticated", username),
        )
        .with_user(user_id, username)
        .with_session(session_id, ip_address, user_agent);

        self.log_event(event).await
    }

    /// Log authentication failure
    pub async fn log_auth_failure(
        &self,
        username: &str,
        reason: &str,
        ip_address: Option<IpAddr>,
        user_agent: Option<&str>,
    ) -> SecurityResult<()> {
        let mut event = AuditEvent::new(
            AuditEventType::LoginFailure,
            AuditSeverity::Medium,
            format!("Authentication failed for user '{}': {}", username, reason),
        )
        .with_session("", ip_address, user_agent)
        .with_failure(reason);

        event.username = Some(username.to_string());

        self.log_event(event).await
    }

    /// Log permission denial
    pub async fn log_permission_denied(
        &self,
        user_id: &str,
        username: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        action: &str,
        required_permission: &str,
    ) -> SecurityResult<()> {
        let mut metadata = HashMap::new();
        metadata.insert(
            "required_permission".to_string(),
            required_permission.to_string(),
        );

        let event = AuditEvent::new(
            AuditEventType::PermissionDenied,
            AuditSeverity::Medium,
            format!(
                "User '{}' denied access to {} {} (action: {})",
                username,
                resource_type,
                resource_id.unwrap_or("*"),
                action
            ),
        )
        .with_user(user_id, username)
        .with_resource(resource_type, resource_id.unwrap_or(""))
        .with_action(action)
        .with_metadata(metadata)
        .with_failure(&format!("Missing permission: {}", required_permission));

        self.log_event(event).await
    }

    /// Log resource creation
    pub async fn log_resource_created(
        &self,
        user_id: &str,
        username: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> SecurityResult<()> {
        let event_type = match resource_type {
            "issue" => AuditEventType::IssueCreated,
            "knowledge" => AuditEventType::KnowledgeCreated,
            "user" => AuditEventType::UserCreated,
            "agent" => AuditEventType::AgentRegistered,
            _ => AuditEventType::Custom(format!("{}_created", resource_type)),
        };

        let event = AuditEvent::new(
            event_type,
            AuditSeverity::Low,
            format!(
                "User '{}' created {} '{}'",
                username, resource_type, resource_id
            ),
        )
        .with_user(user_id, username)
        .with_resource(resource_type, resource_id)
        .with_action("create");

        self.log_event(event).await
    }

    /// Log resource update
    pub async fn log_resource_updated(
        &self,
        user_id: &str,
        username: &str,
        resource_type: &str,
        resource_id: &str,
        changes: HashMap<String, String>,
    ) -> SecurityResult<()> {
        let event_type = match resource_type {
            "issue" => AuditEventType::IssueUpdated,
            "knowledge" => AuditEventType::KnowledgeUpdated,
            "user" => AuditEventType::UserUpdated,
            "agent" => AuditEventType::AgentUpdated,
            _ => AuditEventType::Custom(format!("{}_updated", resource_type)),
        };

        let event = AuditEvent::new(
            event_type,
            AuditSeverity::Low,
            format!(
                "User '{}' updated {} '{}'",
                username, resource_type, resource_id
            ),
        )
        .with_user(user_id, username)
        .with_resource(resource_type, resource_id)
        .with_action("update")
        .with_metadata(changes);

        self.log_event(event).await
    }

    /// Log resource deletion
    pub async fn log_resource_deleted(
        &self,
        user_id: &str,
        username: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> SecurityResult<()> {
        let event_type = match resource_type {
            "issue" => AuditEventType::IssueDeleted,
            "knowledge" => AuditEventType::KnowledgeDeleted,
            "user" => AuditEventType::UserDeleted,
            _ => AuditEventType::Custom(format!("{}_deleted", resource_type)),
        };

        let event = AuditEvent::new(
            event_type,
            AuditSeverity::Medium,
            format!(
                "User '{}' deleted {} '{}'",
                username, resource_type, resource_id
            ),
        )
        .with_user(user_id, username)
        .with_resource(resource_type, resource_id)
        .with_action("delete");

        self.log_event(event).await
    }

    /// Log suspicious activity
    pub async fn log_suspicious_activity(
        &self,
        description: &str,
        user_id: Option<&str>,
        username: Option<&str>,
        ip_address: Option<IpAddr>,
        metadata: HashMap<String, String>,
    ) -> SecurityResult<()> {
        let mut event = AuditEvent::new(
            AuditEventType::SuspiciousActivity,
            AuditSeverity::High,
            description.to_string(),
        )
        .with_metadata(metadata);

        if let (Some(user_id), Some(username)) = (user_id, username) {
            event = event.with_user(user_id, username);
        }

        if let Some(ip) = ip_address {
            event.ip_address = Some(ip.to_string());
        }

        self.log_event(event).await
    }

    /// Log rate limit exceeded
    pub async fn log_rate_limit_exceeded(
        &self,
        user_id: Option<&str>,
        ip_address: IpAddr,
        endpoint: &str,
        limit: u32,
    ) -> SecurityResult<()> {
        let mut metadata = HashMap::new();
        metadata.insert("endpoint".to_string(), endpoint.to_string());
        metadata.insert("limit".to_string(), limit.to_string());

        let mut event = AuditEvent::new(
            AuditEventType::RateLimitExceeded,
            AuditSeverity::Medium,
            format!(
                "Rate limit exceeded for endpoint '{}' from IP {}",
                endpoint, ip_address
            ),
        )
        .with_metadata(metadata);

        event.ip_address = Some(ip_address.to_string());

        if let Some(user_id) = user_id {
            event.user_id = Some(user_id.to_string());
        }

        self.log_event(event).await
    }

    /// Query audit events
    pub async fn query_events(
        &self,
        filters: AuditQueryFilters,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> SecurityResult<Vec<AuditEvent>> {
        let mut query = "SELECT * FROM audit_events WHERE 1=1".to_string();
        let mut params = Vec::new();

        // Add filters
        if let Some(event_type) = &filters.event_type {
            query.push_str(" AND event_type LIKE ?");
            params.push(format!("%{}%", event_type));
        }

        if let Some(user_id) = &filters.user_id {
            query.push_str(" AND user_id = ?");
            params.push(user_id.clone());
        }

        if let Some(severity) = &filters.min_severity {
            query.push_str(" AND severity >= ?");
            params.push(serde_json::to_string(severity).unwrap_or_default());
        }

        if let Some(from_date) = &filters.from_date {
            query.push_str(" AND timestamp >= ?");
            params.push(from_date.to_rfc3339());
        }

        if let Some(to_date) = &filters.to_date {
            query.push_str(" AND timestamp <= ?");
            params.push(to_date.to_rfc3339());
        }

        query.push_str(" ORDER BY timestamp DESC");

        if let Some(limit) = limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        if let Some(offset) = offset {
            query.push_str(&format!(" OFFSET {}", offset));
        }

        let mut query_builder = sqlx::query_as::<_, AuditEvent>(&query);
        for param in params {
            query_builder = query_builder.bind(param);
        }

        let events = query_builder.fetch_all(&self.db_pool).await?;
        Ok(events)
    }

    /// Get audit statistics
    pub async fn get_audit_statistics(&self, days: i32) -> SecurityResult<AuditStatistics> {
        let from_date = Utc::now() - chrono::Duration::days(days as i64);

        let total_events = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_events WHERE timestamp >= ?",
            from_date
        )
        .fetch_one(&self.db_pool)
        .await?;

        let failed_logins = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_events WHERE event_type LIKE '%LoginFailure%' AND timestamp >= ?",
            from_date
        )
        .fetch_one(&self.db_pool)
        .await?;

        let permission_denials = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_events WHERE event_type LIKE '%PermissionDenied%' AND timestamp >= ?",
            from_date
        )
        .fetch_one(&self.db_pool)
        .await?;

        let suspicious_activities = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM audit_events WHERE event_type LIKE '%SuspiciousActivity%' AND timestamp >= ?",
            from_date
        )
        .fetch_one(&self.db_pool)
        .await?;

        Ok(AuditStatistics {
            total_events: total_events as u64,
            failed_logins: failed_logins as u64,
            permission_denials: permission_denials as u64,
            suspicious_activities: suspicious_activities as u64,
            period_days: days,
        })
    }

    /// Store event in database
    async fn store_event(&self, event: &AuditEvent) -> SecurityResult<()> {
        sqlx::query!(
            r#"
            INSERT INTO audit_events (
                id, event_type, severity, user_id, username, agent_id, resource_type, 
                resource_id, action, description, metadata, ip_address, user_agent, 
                session_id, result, error_message, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            event.id,
            event.event_type,
            event.severity,
            event.user_id,
            event.username,
            event.agent_id,
            event.resource_type,
            event.resource_id,
            event.action,
            event.description,
            event.metadata,
            event.ip_address,
            event.user_agent,
            event.session_id,
            event.result,
            event.error_message,
            event.timestamp
        )
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    /// Log to console
    fn log_to_console_output(&self, event: &AuditEvent) {
        let severity = event.get_severity();
        let log_message = format!(
            "AUDIT [{}] {}: {} (User: {}, Resource: {}/{})",
            severity,
            event.get_event_type(),
            event.description,
            event.username.as_deref().unwrap_or("system"),
            event.resource_type.as_deref().unwrap_or("none"),
            event.resource_id.as_deref().unwrap_or("none")
        );

        match severity {
            AuditSeverity::Critical | AuditSeverity::High => error!("{}", log_message),
            AuditSeverity::Medium => warn!("{}", log_message),
            AuditSeverity::Low => info!("{}", log_message),
        }
    }
}

/// Query filters for audit events
#[derive(Debug, Clone, Default)]
pub struct AuditQueryFilters {
    pub event_type: Option<String>,
    pub user_id: Option<String>,
    pub min_severity: Option<AuditSeverity>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
}

/// Audit statistics
#[derive(Debug, Clone, Serialize)]
pub struct AuditStatistics {
    pub total_events: u64,
    pub failed_logins: u64,
    pub permission_denials: u64,
    pub suspicious_activities: u64,
    pub period_days: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Create audit_events table
        sqlx::query!(
            r#"
            CREATE TABLE audit_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                severity TEXT NOT NULL,
                user_id TEXT,
                username TEXT,
                agent_id TEXT,
                resource_type TEXT,
                resource_id TEXT,
                action TEXT,
                description TEXT NOT NULL,
                metadata TEXT NOT NULL,
                ip_address TEXT,
                user_agent TEXT,
                session_id TEXT,
                result TEXT NOT NULL,
                error_message TEXT,
                timestamp TEXT NOT NULL
            )
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_audit_event_creation() {
        let event = AuditEvent::new(
            AuditEventType::LoginSuccess,
            AuditSeverity::Low,
            "Test event".to_string(),
        )
        .with_user("user_123", "testuser")
        .with_session(
            "session_123",
            Some("127.0.0.1".parse().unwrap()),
            Some("test-agent"),
        );

        assert_eq!(event.get_event_type(), AuditEventType::LoginSuccess);
        assert_eq!(event.get_severity(), AuditSeverity::Low);
        assert_eq!(event.user_id, Some("user_123".to_string()));
        assert_eq!(event.username, Some("testuser".to_string()));
        assert_eq!(event.session_id, Some("session_123".to_string()));
        assert_eq!(event.ip_address, Some("127.0.0.1".to_string()));
        assert_eq!(event.user_agent, Some("test-agent".to_string()));
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let pool = setup_test_db().await;
        let audit_logger = AuditLogger::with_defaults(pool);

        // Test logging auth success
        audit_logger
            .log_auth_success(
                "user_123",
                "testuser",
                "session_123",
                Some("127.0.0.1".parse().unwrap()),
                Some("test-agent"),
            )
            .await
            .unwrap();

        // Test logging auth failure
        audit_logger
            .log_auth_failure(
                "testuser",
                "Invalid password",
                Some("127.0.0.1".parse().unwrap()),
                Some("test-agent"),
            )
            .await
            .unwrap();

        // Query events
        let events = audit_logger
            .query_events(AuditQueryFilters::default(), Some(10), None)
            .await
            .unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].get_event_type(), AuditEventType::LoginFailure);
        assert_eq!(events[1].get_event_type(), AuditEventType::LoginSuccess);
    }

    #[tokio::test]
    async fn test_audit_statistics() {
        let pool = setup_test_db().await;
        let audit_logger = AuditLogger::with_defaults(pool);

        // Log some events
        audit_logger
            .log_auth_success("user_123", "testuser", "session_123", None, None)
            .await
            .unwrap();

        audit_logger
            .log_auth_failure("testuser", "Invalid password", None, None)
            .await
            .unwrap();

        audit_logger
            .log_permission_denied(
                "user_123",
                "testuser",
                "issue",
                Some("issue_123"),
                "delete",
                "DeleteIssue",
            )
            .await
            .unwrap();

        // Get statistics
        let stats = audit_logger.get_audit_statistics(7).await.unwrap();
        assert_eq!(stats.total_events, 3);
        assert_eq!(stats.failed_logins, 1);
        assert_eq!(stats.permission_denials, 1);
        assert_eq!(stats.period_days, 7);
    }

    #[tokio::test]
    async fn test_audit_query_filters() {
        let pool = setup_test_db().await;
        let audit_logger = AuditLogger::with_defaults(pool);

        // Log events with different users
        audit_logger
            .log_auth_success("user_1", "user1", "session_1", None, None)
            .await
            .unwrap();

        audit_logger
            .log_auth_success("user_2", "user2", "session_2", None, None)
            .await
            .unwrap();

        // Query for specific user
        let filters = AuditQueryFilters {
            user_id: Some("user_1".to_string()),
            ..Default::default()
        };

        let events = audit_logger
            .query_events(filters, None, None)
            .await
            .unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].user_id, Some("user_1".to_string()));
    }
}
