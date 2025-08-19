//! Role-based access control (RBAC) system

use crate::{Permission, SecurityError, SecurityResult, User, UserRole};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Access control service for managing permissions
#[derive(Debug, Clone)]
pub struct AccessControlService {
    /// Custom role definitions beyond the default hierarchy
    custom_roles: Vec<CustomRole>,
}

/// Custom role definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomRole {
    pub name: String,
    pub description: String,
    pub permissions: HashSet<Permission>,
    pub inherits_from: Option<UserRole>,
}

/// Resource access context
#[derive(Debug, Clone)]
pub struct AccessContext {
    pub user_id: String,
    pub user_role: UserRole,
    pub resource_type: ResourceType,
    pub resource_id: Option<String>,
    pub action: Action,
    pub resource_owner_id: Option<String>,
}

/// Type of resource being accessed
#[derive(Debug, Clone, PartialEq)]
pub enum ResourceType {
    Agent,
    Issue,
    Knowledge,
    Message,
    User,
    System,
    Dashboard,
    AuditLog,
}

/// Action being performed on a resource
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Create,
    Read,
    Update,
    Delete,
    List,
    Execute,
    Manage,
}

/// Permission check result
#[derive(Debug, Clone)]
pub struct PermissionResult {
    pub allowed: bool,
    pub reason: Option<String>,
    pub required_permission: Option<Permission>,
}

impl PermissionResult {
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
            required_permission: None,
        }
    }

    pub fn denied(reason: &str, required_permission: Permission) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.to_string()),
            required_permission: Some(required_permission),
        }
    }
}

impl AccessControlService {
    /// Create new access control service
    pub fn new() -> Self {
        Self {
            custom_roles: Vec::new(),
        }
    }

    /// Check if user has permission to perform action on resource
    pub fn check_permission(&self, context: &AccessContext) -> PermissionResult {
        let required_permission = self.get_required_permission(context);

        match required_permission {
            Some(permission) => {
                if self.has_permission(context, &permission) {
                    PermissionResult::allowed()
                } else {
                    PermissionResult::denied(
                        &format!(
                            "User lacks permission {:?} for {} {} on {}",
                            permission,
                            context.action.as_str(),
                            context.resource_type.as_str(),
                            context.resource_id.as_deref().unwrap_or("all")
                        ),
                        permission,
                    )
                }
            }
            None => PermissionResult::allowed(), // No permission required
        }
    }

    /// Check if user has specific permission
    pub fn has_permission(&self, context: &AccessContext, permission: &Permission) -> bool {
        // Check built-in role permissions
        if context.user_role.has_permission(permission) {
            return true;
        }

        // Check custom roles (if implemented in the future)
        for custom_role in &self.custom_roles {
            if custom_role.permissions.contains(permission) {
                return true;
            }
        }

        // Check resource ownership for certain permissions
        self.check_resource_ownership(context, permission)
    }

    /// Check resource ownership for ownership-based permissions
    fn check_resource_ownership(&self, context: &AccessContext, permission: &Permission) -> bool {
        match permission {
            Permission::UpdateOwnIssue | Permission::UpdateOwnKnowledge => {
                if let Some(resource_owner_id) = &context.resource_owner_id {
                    context.user_id == *resource_owner_id
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get required permission for a given context
    fn get_required_permission(&self, context: &AccessContext) -> Option<Permission> {
        match (&context.resource_type, &context.action) {
            // Dashboard access
            (ResourceType::Dashboard, Action::Read) => Some(Permission::ViewDashboard),

            // Agent management
            (ResourceType::Agent, Action::Read | Action::List) => Some(Permission::ViewAgents),
            (
                ResourceType::Agent,
                Action::Create | Action::Update | Action::Delete | Action::Manage,
            ) => Some(Permission::ManageAgents),

            // Issue management
            (ResourceType::Issue, Action::Read | Action::List) => Some(Permission::ViewIssues),
            (ResourceType::Issue, Action::Create) => Some(Permission::CreateIssue),
            (ResourceType::Issue, Action::Update) => {
                // Check if it's the user's own issue first
                if let Some(resource_owner_id) = &context.resource_owner_id {
                    if context.user_id == *resource_owner_id {
                        Some(Permission::UpdateOwnIssue)
                    } else {
                        Some(Permission::UpdateAnyIssue)
                    }
                } else {
                    Some(Permission::UpdateAnyIssue)
                }
            }
            (ResourceType::Issue, Action::Delete) => Some(Permission::DeleteIssue),
            (ResourceType::Issue, Action::Manage) => Some(Permission::AssignIssues),

            // Knowledge management
            (ResourceType::Knowledge, Action::Read | Action::List) => {
                Some(Permission::ViewKnowledge)
            }
            (ResourceType::Knowledge, Action::Create) => Some(Permission::CreateKnowledge),
            (ResourceType::Knowledge, Action::Update) => {
                // Check if it's the user's own knowledge first
                if let Some(resource_owner_id) = &context.resource_owner_id {
                    if context.user_id == *resource_owner_id {
                        Some(Permission::UpdateOwnKnowledge)
                    } else {
                        Some(Permission::ManageKnowledge)
                    }
                } else {
                    Some(Permission::ManageKnowledge)
                }
            }
            (ResourceType::Knowledge, Action::Delete | Action::Manage) => {
                Some(Permission::ManageKnowledge)
            }

            // Message handling
            (ResourceType::Message, Action::Create) => Some(Permission::SendMessage),

            // User management
            (
                ResourceType::User,
                Action::Create | Action::Update | Action::Delete | Action::Manage,
            ) => Some(Permission::ManageUsers),

            // System administration
            (ResourceType::System, Action::Manage) => Some(Permission::SystemConfiguration),
            (ResourceType::AuditLog, Action::Read) => Some(Permission::ViewAuditLogs),

            // Default: no permission required
            _ => None,
        }
    }

    /// Require permission or return error
    pub fn require_permission(&self, context: &AccessContext) -> SecurityResult<()> {
        let result = self.check_permission(context);
        if result.allowed {
            Ok(())
        } else {
            Err(SecurityError::InsufficientPermissions {
                required: result
                    .required_permission
                    .map(|p| format!("{:?}", p))
                    .unwrap_or_else(|| "Unknown".to_string()),
                current: format!("{:?}", context.user_role),
            })
        }
    }

    /// Check if user can access resource
    pub fn can_access_resource(
        &self,
        user: &User,
        resource_type: ResourceType,
        resource_id: Option<&str>,
        action: Action,
        resource_owner_id: Option<&str>,
    ) -> bool {
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type,
            resource_id: resource_id.map(|s| s.to_string()),
            action,
            resource_owner_id: resource_owner_id.map(|s| s.to_string()),
        };

        self.check_permission(&context).allowed
    }

    /// Get effective permissions for user
    pub fn get_effective_permissions(&self, user_role: &UserRole) -> Vec<Permission> {
        user_role.permissions()
    }

    /// Add custom role (for future extensibility)
    pub fn add_custom_role(&mut self, role: CustomRole) {
        self.custom_roles.push(role);
    }

    /// Get all custom roles
    pub fn get_custom_roles(&self) -> &[CustomRole] {
        &self.custom_roles
    }
}

impl Default for AccessControlService {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceType {
    fn as_str(&self) -> &'static str {
        match self {
            ResourceType::Agent => "agent",
            ResourceType::Issue => "issue",
            ResourceType::Knowledge => "knowledge",
            ResourceType::Message => "message",
            ResourceType::User => "user",
            ResourceType::System => "system",
            ResourceType::Dashboard => "dashboard",
            ResourceType::AuditLog => "audit_log",
        }
    }
}

impl Action {
    fn as_str(&self) -> &'static str {
        match self {
            Action::Create => "create",
            Action::Read => "read",
            Action::Update => "update",
            Action::Delete => "delete",
            Action::List => "list",
            Action::Execute => "execute",
            Action::Manage => "manage",
        }
    }
}

/// Helper macro to check permissions
#[macro_export]
macro_rules! require_permission {
    ($access_control:expr, $user:expr, $resource_type:expr, $action:expr) => {{
        let context = $crate::AccessContext {
            user_id: $user.id.clone(),
            user_role: $user.role.clone(),
            resource_type: $resource_type,
            resource_id: None,
            action: $action,
            resource_owner_id: None,
        };
        $access_control.require_permission(&context)?;
    }};

    ($access_control:expr, $user:expr, $resource_type:expr, $resource_id:expr, $action:expr, $owner_id:expr) => {{
        let context = $crate::AccessContext {
            user_id: $user.id.clone(),
            user_role: $user.role.clone(),
            resource_type: $resource_type,
            resource_id: Some($resource_id.to_string()),
            action: $action,
            resource_owner_id: Some($owner_id.to_string()),
        };
        $access_control.require_permission(&context)?;
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::User;
    use chrono::Utc;

    fn create_test_user(role: UserRole) -> User {
        User {
            id: "test_user_id".to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            password_hash: "hashed_password".to_string(),
            role,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
            failed_login_attempts: 0,
            locked_until: None,
        }
    }

    #[test]
    fn test_viewer_permissions() {
        let access_control = AccessControlService::new();
        let user = create_test_user(UserRole::Viewer);

        // Viewers can view resources
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Dashboard,
            resource_id: None,
            action: Action::Read,
            resource_owner_id: None,
        };
        assert!(access_control.check_permission(&context).allowed);

        // Viewers cannot create issues
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Issue,
            resource_id: None,
            action: Action::Create,
            resource_owner_id: None,
        };
        assert!(!access_control.check_permission(&context).allowed);
    }

    #[test]
    fn test_agent_permissions() {
        let access_control = AccessControlService::new();
        let user = create_test_user(UserRole::Agent);

        // Agents can create issues
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Issue,
            resource_id: None,
            action: Action::Create,
            resource_owner_id: None,
        };
        assert!(access_control.check_permission(&context).allowed);

        // Agents cannot manage other agents
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Agent,
            resource_id: None,
            action: Action::Manage,
            resource_owner_id: None,
        };
        assert!(!access_control.check_permission(&context).allowed);
    }

    #[test]
    fn test_ownership_based_permissions() {
        let access_control = AccessControlService::new();
        let user = create_test_user(UserRole::Agent);

        // Agent can update their own issue
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Issue,
            resource_id: Some("issue_123".to_string()),
            action: Action::Update,
            resource_owner_id: Some(user.id.clone()),
        };
        assert!(access_control.check_permission(&context).allowed);

        // Agent cannot update someone else's issue
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::Issue,
            resource_id: Some("issue_123".to_string()),
            action: Action::Update,
            resource_owner_id: Some("other_user_id".to_string()),
        };
        assert!(!access_control.check_permission(&context).allowed);
    }

    #[test]
    fn test_admin_permissions() {
        let access_control = AccessControlService::new();
        let user = create_test_user(UserRole::Admin);

        // Admin can manage users
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::User,
            resource_id: None,
            action: Action::Manage,
            resource_owner_id: None,
        };
        assert!(access_control.check_permission(&context).allowed);

        // Admin can manage system
        let context = AccessContext {
            user_id: user.id.clone(),
            user_role: user.role.clone(),
            resource_type: ResourceType::System,
            resource_id: None,
            action: Action::Manage,
            resource_owner_id: None,
        };
        assert!(access_control.check_permission(&context).allowed);
    }

    #[test]
    fn test_can_access_resource_helper() {
        let access_control = AccessControlService::new();
        let user = create_test_user(UserRole::Coordinator);

        // Coordinator can manage agents
        assert!(access_control.can_access_resource(
            &user,
            ResourceType::Agent,
            None,
            Action::Manage,
            None
        ));

        // Coordinator can update any issue
        assert!(access_control.can_access_resource(
            &user,
            ResourceType::Issue,
            Some("issue_123"),
            Action::Update,
            Some("other_user_id")
        ));
    }

    #[test]
    fn test_effective_permissions() {
        let access_control = AccessControlService::new();

        let viewer_permissions = access_control.get_effective_permissions(&UserRole::Viewer);
        assert!(viewer_permissions.contains(&Permission::ViewDashboard));
        assert!(!viewer_permissions.contains(&Permission::CreateIssue));

        let admin_permissions = access_control.get_effective_permissions(&UserRole::Admin);
        assert!(admin_permissions.contains(&Permission::ViewDashboard));
        assert!(admin_permissions.contains(&Permission::CreateIssue));
        assert!(admin_permissions.contains(&Permission::ManageUsers));
        assert!(admin_permissions.contains(&Permission::SystemConfiguration));
    }
}
