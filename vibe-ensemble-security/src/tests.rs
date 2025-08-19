//! Integration tests for the security module

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{
        AccessContext, AccessControlService, Action, AuditEventType, AuditLogger, AuditSeverity,
        AuthService, EncryptionService, JwtManager, Permission, RateLimitService, RateLimitType,
        ResourceType, SecurityConfig, SecurityMiddleware, User, UserRole,
    };
    use chrono::{Duration, Utc};
    use sqlx::sqlite::SqlitePool;
    use std::collections::HashMap;
    use std::net::Ipv4Addr;
    use std::sync::Arc;
    use tokio;
    use uuid::Uuid;

    async fn setup_test_database() -> sqlx::Pool<sqlx::Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Create all necessary tables
        sqlx::query!(
            r#"
            CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                email TEXT,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                last_login_at TEXT,
                failed_login_attempts INTEGER NOT NULL DEFAULT 0,
                locked_until TEXT,
                created_by TEXT NOT NULL
            );

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
            );

            CREATE TABLE agent_tokens (
                id TEXT PRIMARY KEY,
                agent_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                name TEXT NOT NULL,
                permissions TEXT NOT NULL,
                is_active BOOLEAN NOT NULL DEFAULT 1,
                expires_at TEXT,
                last_used_at TEXT,
                created_at TEXT NOT NULL,
                created_by TEXT NOT NULL
            );
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_complete_authentication_flow() {
        let pool = setup_test_database().await;
        let auth_service =
            AuthService::with_defaults(pool, "test_secret_key_for_integration_testing");

        // Test user creation
        let username = "testuser";
        let email = "test@example.com";
        let password = "SecurePassword123!";

        let user = auth_service
            .create_user(username, Some(email), password, UserRole::Agent, "system")
            .await
            .unwrap();

        assert_eq!(user.username, username);
        assert_eq!(user.email, Some(email.to_string()));
        assert_eq!(user.role, UserRole::Agent);
        assert!(user.is_active);

        // Test authentication with correct password
        let token_pair = auth_service
            .authenticate(username, password, Some("127.0.0.1".to_string()))
            .await
            .unwrap();

        assert!(!token_pair.access_token.is_empty());
        assert!(!token_pair.refresh_token.is_empty());

        // Test JWT token validation
        let claims = auth_service
            .jwt_manager()
            .validate_access_token(&token_pair.access_token)
            .unwrap();

        assert_eq!(claims.username, username);
        assert_eq!(claims.role, UserRole::Agent);

        // Test token refresh
        let new_token_pair = auth_service
            .refresh_token(&token_pair.refresh_token)
            .await
            .unwrap();

        assert_ne!(new_token_pair.access_token, token_pair.access_token);

        // Test authentication with wrong password
        let result = auth_service
            .authenticate(username, "WrongPassword", None)
            .await;
        assert!(result.is_err());

        // Test password change
        let new_password = "NewSecurePassword456!";
        auth_service
            .change_password(&user.id, password, new_password)
            .await
            .unwrap();

        // Test authentication with new password
        let token_pair = auth_service
            .authenticate(username, new_password, None)
            .await
            .unwrap();
        assert!(!token_pair.access_token.is_empty());

        // Test authentication with old password should fail
        let result = auth_service.authenticate(username, password, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_role_based_access_control() {
        let access_control = AccessControlService::new();

        // Test viewer permissions
        let viewer_context = AccessContext {
            user_id: "viewer_1".to_string(),
            user_role: UserRole::Viewer,
            resource_type: ResourceType::Dashboard,
            resource_id: None,
            action: Action::Read,
            resource_owner_id: None,
        };

        let result = access_control.check_permission(&viewer_context);
        assert!(result.allowed);

        // Viewer should not be able to create issues
        let create_context = AccessContext {
            user_id: "viewer_1".to_string(),
            user_role: UserRole::Viewer,
            resource_type: ResourceType::Issue,
            resource_id: None,
            action: Action::Create,
            resource_owner_id: None,
        };

        let result = access_control.check_permission(&create_context);
        assert!(!result.allowed);

        // Test agent permissions
        let agent_context = AccessContext {
            user_id: "agent_1".to_string(),
            user_role: UserRole::Agent,
            resource_type: ResourceType::Issue,
            resource_id: None,
            action: Action::Create,
            resource_owner_id: None,
        };

        let result = access_control.check_permission(&agent_context);
        assert!(result.allowed);

        // Agent should be able to update their own issues
        let own_issue_context = AccessContext {
            user_id: "agent_1".to_string(),
            user_role: UserRole::Agent,
            resource_type: ResourceType::Issue,
            resource_id: Some("issue_123".to_string()),
            action: Action::Update,
            resource_owner_id: Some("agent_1".to_string()),
        };

        let result = access_control.check_permission(&own_issue_context);
        assert!(result.allowed);

        // Agent should not be able to update others' issues
        let other_issue_context = AccessContext {
            user_id: "agent_1".to_string(),
            user_role: UserRole::Agent,
            resource_type: ResourceType::Issue,
            resource_id: Some("issue_123".to_string()),
            action: Action::Update,
            resource_owner_id: Some("agent_2".to_string()),
        };

        let result = access_control.check_permission(&other_issue_context);
        assert!(!result.allowed);

        // Test admin permissions
        let admin_context = AccessContext {
            user_id: "admin_1".to_string(),
            user_role: UserRole::Admin,
            resource_type: ResourceType::User,
            resource_id: None,
            action: Action::Manage,
            resource_owner_id: None,
        };

        let result = access_control.check_permission(&admin_context);
        assert!(result.allowed);
    }

    #[tokio::test]
    async fn test_audit_logging() {
        let pool = setup_test_database().await;
        let audit_logger = AuditLogger::with_defaults(pool);

        // Test authentication success logging
        audit_logger
            .log_auth_success(
                "user_123",
                "testuser",
                "session_456",
                Some("127.0.0.1".parse().unwrap()),
                Some("Mozilla/5.0"),
            )
            .await
            .unwrap();

        // Test authentication failure logging
        audit_logger
            .log_auth_failure(
                "testuser",
                "Invalid password",
                Some("192.168.1.1".parse().unwrap()),
                Some("Mozilla/5.0"),
            )
            .await
            .unwrap();

        // Test permission denied logging
        audit_logger
            .log_permission_denied(
                "user_123",
                "testuser",
                "issue",
                Some("issue_456"),
                "delete",
                "DeleteIssue",
            )
            .await
            .unwrap();

        // Test resource operations
        audit_logger
            .log_resource_created("user_123", "testuser", "issue", "issue_789")
            .await
            .unwrap();

        let mut changes = HashMap::new();
        changes.insert("status".to_string(), "completed".to_string());
        audit_logger
            .log_resource_updated("user_123", "testuser", "issue", "issue_789", changes)
            .await
            .unwrap();

        audit_logger
            .log_resource_deleted("user_123", "testuser", "issue", "issue_789")
            .await
            .unwrap();

        // Test suspicious activity logging
        let mut metadata = HashMap::new();
        metadata.insert("attempts".to_string(), "5".to_string());
        audit_logger
            .log_suspicious_activity(
                "Multiple failed login attempts",
                Some("user_123"),
                Some("testuser"),
                Some("10.0.0.1".parse().unwrap()),
                metadata,
            )
            .await
            .unwrap();

        // Test rate limit logging
        audit_logger
            .log_rate_limit_exceeded(
                Some("user_123"),
                "192.168.1.100".parse().unwrap(),
                "/api/issues",
                60,
            )
            .await
            .unwrap();

        // Query events
        let events = audit_logger
            .query_events(crate::audit::AuditQueryFilters::default(), Some(20), None)
            .await
            .unwrap();

        assert_eq!(events.len(), 8);

        // Check that we have different event types
        let event_types: std::collections::HashSet<AuditEventType> =
            events.iter().map(|e| e.get_event_type()).collect();

        assert!(event_types.contains(&AuditEventType::LoginSuccess));
        assert!(event_types.contains(&AuditEventType::LoginFailure));
        assert!(event_types.contains(&AuditEventType::PermissionDenied));
        assert!(event_types.contains(&AuditEventType::IssueCreated));
        assert!(event_types.contains(&AuditEventType::IssueUpdated));
        assert!(event_types.contains(&AuditEventType::IssueDeleted));
        assert!(event_types.contains(&AuditEventType::SuspiciousActivity));
        assert!(event_types.contains(&AuditEventType::RateLimitExceeded));

        // Test audit statistics
        let stats = audit_logger.get_audit_statistics(1).await.unwrap();
        assert_eq!(stats.total_events, 8);
        assert_eq!(stats.failed_logins, 1);
        assert_eq!(stats.permission_denials, 1);
        assert_eq!(stats.suspicious_activities, 1);
    }

    #[tokio::test]
    async fn test_rate_limiting() {
        let pool = setup_test_database().await;
        let audit_logger = Arc::new(AuditLogger::with_defaults(pool));

        let rate_limiter = RateLimitService::new(
            crate::rate_limiting::RateLimitConfig {
                general_rpm: 5,
                auth_rpm: 3,
                api_rpm: 10,
                burst_capacity: 2,
                per_ip_enabled: true,
                per_user_enabled: true,
                whitelist_ips: vec![],
                blacklist_ips: vec!["10.0.0.1".parse().unwrap()],
            },
            Some(audit_logger),
        );

        let test_ip = std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let blacklist_ip = std::net::IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

        // Test normal rate limiting
        let mut allowed_count = 0;
        for i in 0..10 {
            let allowed = rate_limiter
                .check_rate_limit(
                    test_ip,
                    Some("user_123"),
                    RateLimitType::General,
                    "/dashboard",
                )
                .await
                .unwrap();

            if allowed {
                allowed_count += 1;
            }

            if i < 5 {
                assert!(allowed, "Request {} should be allowed", i);
            }
        }

        // Should have allowed at least the first few requests
        assert!(
            allowed_count >= 2,
            "Should have allowed at least burst capacity"
        );

        // Test blacklisted IP
        let blacklist_result = rate_limiter
            .check_rate_limit(blacklist_ip, None, RateLimitType::General, "/dashboard")
            .await
            .unwrap();

        assert!(!blacklist_result, "Blacklisted IP should be denied");

        // Test different endpoint types
        let auth_allowed = rate_limiter
            .check_rate_limit(
                "127.0.0.1".parse().unwrap(),
                None,
                RateLimitType::Authentication,
                "/auth/login",
            )
            .await
            .unwrap();

        assert!(auth_allowed, "First auth request should be allowed");

        let api_allowed = rate_limiter
            .check_rate_limit(
                "127.0.0.2".parse().unwrap(),
                Some("user_456"),
                RateLimitType::Api,
                "/api/users",
            )
            .await
            .unwrap();

        assert!(api_allowed, "First API request should be allowed");
    }

    #[tokio::test]
    async fn test_encryption_service() {
        let encryption_service = EncryptionService::new();

        // Test basic encryption/decryption
        let plaintext = "This is a secret message for testing";
        let encrypted = encryption_service.encrypt_message(plaintext).await.unwrap();

        assert_ne!(encrypted.content, plaintext);
        assert!(!encrypted.content.is_empty());
        assert!(!encrypted.nonce.is_empty());
        assert_eq!(encrypted.algorithm, "AES-256-GCM");

        let decrypted = encryption_service
            .decrypt_message(&encrypted)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext);

        // Test encryption with AAD
        let aad = "user:123:message";
        let encrypted_with_aad = encryption_service
            .encrypt_message_with_aad(plaintext, Some(aad))
            .await
            .unwrap();

        assert_eq!(encrypted_with_aad.aad, Some(aad.to_string()));

        let decrypted_with_aad = encryption_service
            .decrypt_message(&encrypted_with_aad)
            .await
            .unwrap();
        assert_eq!(decrypted_with_aad, plaintext);

        // Test that tampering with AAD fails decryption
        let mut tampered = encrypted_with_aad.clone();
        tampered.aad = Some("user:456:message".to_string());

        let result = encryption_service.decrypt_message(&tampered).await;
        assert!(result.is_err());

        // Test key rotation
        let original_key_id = encryption_service.get_active_key_id().await;
        let encrypted_original = encryption_service.encrypt_message("test").await.unwrap();

        let new_key_id = encryption_service.rotate_key(None).await.unwrap();
        assert_ne!(new_key_id, original_key_id);

        let encrypted_new = encryption_service.encrypt_message("test").await.unwrap();
        assert_ne!(encrypted_new.key_id, encrypted_original.key_id);

        // Both should decrypt correctly
        let decrypted_original = encryption_service
            .decrypt_message(&encrypted_original)
            .await
            .unwrap();
        let decrypted_new = encryption_service
            .decrypt_message(&encrypted_new)
            .await
            .unwrap();

        assert_eq!(decrypted_original, "test");
        assert_eq!(decrypted_new, "test");

        // Test storage encryption
        let sensitive_data = "sensitive user information";
        let context = "user_profile:123";

        let encrypted_storage = encryption_service
            .encrypt_for_storage(sensitive_data, context)
            .await
            .unwrap();

        let decrypted_storage = encryption_service
            .decrypt_from_storage(&encrypted_storage)
            .await
            .unwrap();

        assert_eq!(decrypted_storage, sensitive_data);

        // Test specialized encryption methods
        let password = "user_password_123";
        let user_id = "user_456";
        let encrypted_password = encryption_service
            .encrypt_password(password, user_id)
            .await
            .unwrap();

        let decrypted_password = encryption_service
            .decrypt_from_storage(&encrypted_password)
            .await
            .unwrap();

        assert_eq!(decrypted_password, password);

        // Test message content encryption
        let message_content = "Private message between agents";
        let sender_id = "agent_1";
        let recipient_id = "agent_2";

        let encrypted_message = encryption_service
            .encrypt_message_content(message_content, sender_id, recipient_id)
            .await
            .unwrap();

        let decrypted_message = encryption_service
            .decrypt_message(&encrypted_message)
            .await
            .unwrap();

        assert_eq!(decrypted_message, message_content);

        // Test key management
        let keys = encryption_service.list_keys().await;
        assert!(keys.len() >= 2); // Original + rotated key

        let active_key_metadata = encryption_service
            .get_key_metadata(&new_key_id)
            .await
            .unwrap();

        assert_eq!(active_key_metadata.id, new_key_id);
        assert!(active_key_metadata.is_active);
        assert!(!active_key_metadata.is_expired);

        // Test key deactivation
        encryption_service
            .deactivate_key(&original_key_id)
            .await
            .unwrap();

        let deactivated_metadata = encryption_service
            .get_key_metadata(&original_key_id)
            .await
            .unwrap();

        assert!(!deactivated_metadata.is_active);
    }

    #[tokio::test]
    async fn test_jwt_token_management() {
        let jwt_manager = JwtManager::with_secret("test_secret_for_comprehensive_testing");

        // Create test user
        let user = User {
            id: "user_123".to_string(),
            username: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            password_hash: "hashed_password".to_string(),
            role: UserRole::Coordinator,
            is_active: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_login_at: None,
            failed_login_attempts: 0,
            locked_until: None,
        };

        // Test access token generation and validation
        let access_token = jwt_manager.generate_access_token(&user).unwrap();
        let access_claims = jwt_manager.validate_access_token(&access_token).unwrap();

        assert_eq!(access_claims.sub, user.id);
        assert_eq!(access_claims.username, user.username);
        assert_eq!(access_claims.role, user.role);

        // Test refresh token generation and validation
        let refresh_token = jwt_manager.generate_refresh_token(&user).unwrap();
        let refresh_claims = jwt_manager.validate_refresh_token(&refresh_token).unwrap();

        assert_eq!(refresh_claims.sub, user.id);
        assert_eq!(refresh_claims.username, user.username);

        // Test that access token validation fails for refresh token
        let result = jwt_manager.validate_access_token(&refresh_token);
        assert!(result.is_err());

        // Test that refresh token validation fails for access token
        let result = jwt_manager.validate_refresh_token(&access_token);
        assert!(result.is_err());

        // Test agent token generation and validation
        let agent_id = "agent_456";
        let permissions = vec!["read".to_string(), "write".to_string()];

        let agent_token = jwt_manager
            .generate_agent_token(agent_id, permissions.clone())
            .unwrap();

        let agent_claims = jwt_manager.validate_agent_token(&agent_token).unwrap();

        assert_eq!(agent_claims.sub, agent_id);
        assert_eq!(agent_claims.permissions, permissions);

        // Test token header extraction
        let auth_header = format!("Bearer {}", access_token);
        let extracted_token = JwtManager::extract_token_from_header(&auth_header).unwrap();
        assert_eq!(extracted_token, access_token);

        let invalid_header = "InvalidHeader";
        let extracted_token = JwtManager::extract_token_from_header(invalid_header);
        assert!(extracted_token.is_none());

        // Test token duration methods
        let access_duration = jwt_manager.get_access_token_duration();
        let refresh_duration = jwt_manager.get_refresh_token_duration();

        assert!(access_duration < refresh_duration);
    }

    #[tokio::test]
    async fn test_password_security() {
        let pool = setup_test_database().await;
        let auth_service = AuthService::with_defaults(pool, "test_secret");

        // Test password validation
        let valid_passwords = vec!["SecurePassword123!", "AnotherGood1", "ValidPass99"];

        let invalid_passwords = vec![
            "short",          // too short
            "nouppercase123", // no uppercase
            "NOLOWERCASE123", // no lowercase
            "NoNumbers!",     // no numbers
            "password",       // too common/simple
        ];

        for password in valid_passwords {
            let result = auth_service.validate_password(password);
            assert!(result.is_ok(), "Password '{}' should be valid", password);
        }

        for password in invalid_passwords {
            let result = auth_service.validate_password(password);
            assert!(result.is_err(), "Password '{}' should be invalid", password);
        }

        // Test password hashing
        let password = "TestPassword123!";
        let hash1 = auth_service.hash_password(password).unwrap();
        let hash2 = auth_service.hash_password(password).unwrap();

        // Hashes should be different (due to salt)
        assert_ne!(hash1, hash2);

        // Both should verify correctly
        assert!(auth_service.verify_password(password, &hash1).unwrap());
        assert!(auth_service.verify_password(password, &hash2).unwrap());

        // Wrong password should not verify
        assert!(!auth_service
            .verify_password("WrongPassword", &hash1)
            .unwrap());
    }

    #[tokio::test]
    async fn test_account_lockout() {
        let pool = setup_test_database().await;
        let auth_service = AuthService::new(
            pool,
            Arc::new(JwtManager::with_secret("test_secret")),
            3,                    // Max 3 login attempts
            Duration::minutes(5), // 5 minute lockout
        );

        // Create test user
        let username = "lockout_test";
        let password = "CorrectPassword123!";

        let user = auth_service
            .create_user(username, None, password, UserRole::Agent, "system")
            .await
            .unwrap();

        // First few failed attempts should not lock account
        for i in 0..2 {
            let result = auth_service
                .authenticate(username, "WrongPassword", None)
                .await;
            assert!(result.is_err(), "Attempt {} should fail", i);
        }

        // User should still be able to login with correct password
        let result = auth_service.authenticate(username, password, None).await;
        assert!(
            result.is_ok(),
            "Should be able to login with correct password"
        );

        // Reset failed attempts by getting user again
        let user = auth_service
            .get_user_by_username(username)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.failed_login_attempts, 0);

        // Now try failed attempts until lockout
        for i in 0..3 {
            let result = auth_service
                .authenticate(username, "WrongPassword", None)
                .await;
            assert!(result.is_err(), "Attempt {} should fail", i);
        }

        // Account should now be locked
        let locked_user = auth_service
            .get_user_by_username(username)
            .await
            .unwrap()
            .unwrap();
        assert!(locked_user.is_locked());

        // Even correct password should fail when locked
        let result = auth_service.authenticate(username, password, None).await;
        assert!(
            result.is_err(),
            "Authentication should fail when account is locked"
        );
    }

    #[tokio::test]
    async fn test_security_integration() {
        // This test verifies that all security components work together
        let pool = setup_test_database().await;
        let audit_logger = Arc::new(AuditLogger::with_defaults(pool.clone()));
        let auth_service = Arc::new(AuthService::with_defaults(
            pool.clone(),
            "integration_test_secret",
        ));
        let access_control = Arc::new(AccessControlService::new());
        let encryption_service = Arc::new(EncryptionService::new());

        let security_middleware = SecurityMiddleware::new(
            SecurityConfig::default(),
            auth_service.clone(),
            access_control.clone(),
            audit_logger.clone(),
            encryption_service.clone(),
        );

        // Create test users with different roles
        let admin_user = auth_service
            .create_user("admin", None, "AdminPass123!", UserRole::Admin, "system")
            .await
            .unwrap();

        let agent_user = auth_service
            .create_user("agent", None, "AgentPass123!", UserRole::Agent, "system")
            .await
            .unwrap();

        let viewer_user = auth_service
            .create_user("viewer", None, "ViewerPass123!", UserRole::Viewer, "system")
            .await
            .unwrap();

        // Test authentication for all users
        let admin_tokens = auth_service
            .authenticate("admin", "AdminPass123!", None)
            .await
            .unwrap();

        let agent_tokens = auth_service
            .authenticate("agent", "AgentPass123!", None)
            .await
            .unwrap();

        let viewer_tokens = auth_service
            .authenticate("viewer", "ViewerPass123!", None)
            .await
            .unwrap();

        // Verify JWT tokens work
        let admin_claims = auth_service
            .jwt_manager()
            .validate_access_token(&admin_tokens.access_token)
            .unwrap();
        assert_eq!(admin_claims.role, UserRole::Admin);

        let agent_claims = auth_service
            .jwt_manager()
            .validate_access_token(&agent_tokens.access_token)
            .unwrap();
        assert_eq!(agent_claims.role, UserRole::Agent);

        // Test role-based permissions
        let admin_can_manage_users = access_control.can_access_resource(
            &admin_user,
            ResourceType::User,
            None,
            Action::Manage,
            None,
        );
        assert!(admin_can_manage_users);

        let agent_cannot_manage_users = access_control.can_access_resource(
            &agent_user,
            ResourceType::User,
            None,
            Action::Manage,
            None,
        );
        assert!(!agent_cannot_manage_users);

        let agent_can_create_issues = access_control.can_access_resource(
            &agent_user,
            ResourceType::Issue,
            None,
            Action::Create,
            None,
        );
        assert!(agent_can_create_issues);

        let viewer_cannot_create_issues = access_control.can_access_resource(
            &viewer_user,
            ResourceType::Issue,
            None,
            Action::Create,
            None,
        );
        assert!(!viewer_cannot_create_issues);

        // Test encryption for sensitive data
        let sensitive_message = "Confidential business information";
        let encrypted = encryption_service
            .encrypt_message(sensitive_message)
            .await
            .unwrap();

        let decrypted = encryption_service
            .decrypt_message(&encrypted)
            .await
            .unwrap();
        assert_eq!(decrypted, sensitive_message);

        // Test audit logging captures various events
        audit_logger
            .log_auth_success(
                &admin_user.id,
                &admin_user.username,
                "session_1",
                None,
                None,
            )
            .await
            .unwrap();

        audit_logger
            .log_permission_denied(
                &agent_user.id,
                &agent_user.username,
                "user",
                None,
                "manage",
                "ManageUsers",
            )
            .await
            .unwrap();

        // Verify audit events were logged
        let events = audit_logger
            .query_events(crate::audit::AuditQueryFilters::default(), Some(10), None)
            .await
            .unwrap();

        assert!(!events.is_empty());
        let event_types: std::collections::HashSet<AuditEventType> =
            events.iter().map(|e| e.get_event_type()).collect();

        assert!(event_types.contains(&AuditEventType::LoginSuccess));
        assert!(event_types.contains(&AuditEventType::PermissionDenied));
    }
}
