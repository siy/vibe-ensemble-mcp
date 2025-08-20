//! Security-related data models

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// User authentication information
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub password_hash: String,
    pub role: UserRole,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_login_at: Option<DateTime<Utc>>,
    pub failed_login_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
}

impl User {
    /// Create a new user
    pub fn new(
        username: String,
        email: Option<String>,
        password_hash: String,
        role: UserRole,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            username,
            email,
            password_hash,
            role,
            is_active: true,
            created_at: now,
            updated_at: now,
            last_login_at: None,
            failed_login_attempts: 0,
            locked_until: None,
        }
    }

    /// Check if user account is locked
    pub fn is_locked(&self) -> bool {
        if let Some(locked_until) = self.locked_until {
            Utc::now() < locked_until
        } else {
            false
        }
    }

    /// Check if user has permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.role.has_permission(permission)
    }
}

/// User roles with hierarchical permissions
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq, Eq, PartialOrd, Ord)]
#[sqlx(type_name = "user_role", rename_all = "snake_case")]
pub enum UserRole {
    /// Read-only access
    Viewer,
    /// Can execute tasks and modify own resources
    Agent,
    /// Can manage agents and issues
    Coordinator,
    /// Full administrative access
    Admin,
}

impl UserRole {
    /// Check if role has specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        match (self, permission) {
            // Viewer permissions
            (_, Permission::ViewDashboard) => true,
            (_, Permission::ViewAgents) => true,
            (_, Permission::ViewIssues) => true,
            (_, Permission::ViewKnowledge) => true,

            // Agent permissions
            (
                UserRole::Agent | UserRole::Coordinator | UserRole::Admin,
                Permission::CreateIssue,
            ) => true,
            (
                UserRole::Agent | UserRole::Coordinator | UserRole::Admin,
                Permission::UpdateOwnIssue,
            ) => true,
            (
                UserRole::Agent | UserRole::Coordinator | UserRole::Admin,
                Permission::SendMessage,
            ) => true,
            (
                UserRole::Agent | UserRole::Coordinator | UserRole::Admin,
                Permission::CreateKnowledge,
            ) => true,
            (
                UserRole::Agent | UserRole::Coordinator | UserRole::Admin,
                Permission::UpdateOwnKnowledge,
            ) => true,

            // Coordinator permissions
            (UserRole::Coordinator | UserRole::Admin, Permission::ManageAgents) => true,
            (UserRole::Coordinator | UserRole::Admin, Permission::AssignIssues) => true,
            (UserRole::Coordinator | UserRole::Admin, Permission::UpdateAnyIssue) => true,
            (UserRole::Coordinator | UserRole::Admin, Permission::DeleteIssue) => true,
            (UserRole::Coordinator | UserRole::Admin, Permission::ManageKnowledge) => true,
            (UserRole::Coordinator | UserRole::Admin, Permission::ViewAuditLogs) => true,

            // Admin permissions
            (UserRole::Admin, Permission::ManageUsers) => true,
            (UserRole::Admin, Permission::ManageRoles) => true,
            (UserRole::Admin, Permission::SystemConfiguration) => true,
            (UserRole::Admin, Permission::ViewSecurityLogs) => true,
            (UserRole::Admin, Permission::ManageEncryption) => true,

            _ => false,
        }
    }

    /// Get all permissions for this role
    pub fn permissions(&self) -> Vec<Permission> {
        Permission::all()
            .into_iter()
            .filter(|p| self.has_permission(p))
            .collect()
    }
}

/// System permissions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // Basic viewing permissions
    ViewDashboard,
    ViewAgents,
    ViewIssues,
    ViewKnowledge,

    // Issue management
    CreateIssue,
    UpdateOwnIssue,
    UpdateAnyIssue,
    DeleteIssue,
    AssignIssues,

    // Agent management
    ManageAgents,

    // Knowledge management
    CreateKnowledge,
    UpdateOwnKnowledge,
    ManageKnowledge,

    // Communication
    SendMessage,

    // User and role management
    ManageUsers,
    ManageRoles,

    // System administration
    SystemConfiguration,
    ViewAuditLogs,
    ViewSecurityLogs,
    ManageEncryption,
}

impl Permission {
    /// Get all available permissions
    pub fn all() -> Vec<Self> {
        vec![
            Self::ViewDashboard,
            Self::ViewAgents,
            Self::ViewIssues,
            Self::ViewKnowledge,
            Self::CreateIssue,
            Self::UpdateOwnIssue,
            Self::UpdateAnyIssue,
            Self::DeleteIssue,
            Self::AssignIssues,
            Self::ManageAgents,
            Self::CreateKnowledge,
            Self::UpdateOwnKnowledge,
            Self::ManageKnowledge,
            Self::SendMessage,
            Self::ManageUsers,
            Self::ManageRoles,
            Self::SystemConfiguration,
            Self::ViewAuditLogs,
            Self::ViewSecurityLogs,
            Self::ManageEncryption,
        ]
    }
}

/// JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String,           // Subject (user ID)
    pub username: String,      // Username
    pub role: UserRole,        // User role
    pub iat: i64,              // Issued at
    pub exp: i64,              // Expires at
    pub aud: String,           // Audience
    pub iss: String,           // Issuer
    pub token_type: TokenType, // Type of token
}

/// Type of JWT token
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TokenType {
    Access,
    Refresh,
    AgentAuth,
}

/// Agent authentication token
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentToken {
    pub id: String,
    pub agent_id: String,
    pub token_hash: String,
    pub name: String,
    pub permissions: String, // JSON-encoded permissions
    pub is_active: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

/// Login request
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub remember_me: Option<bool>,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: UserInfo,
}

/// User information for responses
#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: Option<String>,
    pub role: UserRole,
    pub permissions: Vec<Permission>,
}

/// Token refresh request
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

/// Password change request
#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// User creation request
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
    pub role: UserRole,
}

/// User update request
#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub role: Option<UserRole>,
    pub is_active: Option<bool>,
}

/// Agent token creation request
#[derive(Debug, Deserialize)]
pub struct CreateAgentTokenRequest {
    pub agent_id: String,
    pub name: String,
    pub permissions: Vec<Permission>,
    pub expires_at: Option<DateTime<Utc>>,
}
