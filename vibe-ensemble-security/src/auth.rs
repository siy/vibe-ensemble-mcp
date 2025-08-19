//! Authentication services and password management

use crate::{JwtManager, SecurityError, SecurityResult, TokenPair, User, UserRole};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Duration, Utc};
use sqlx::{Pool, Row, Sqlite};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Password requirements
pub struct PasswordRequirements {
    pub min_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_numbers: bool,
    pub require_symbols: bool,
}

impl Default for PasswordRequirements {
    fn default() -> Self {
        Self {
            min_length: 8,
            require_uppercase: true,
            require_lowercase: true,
            require_numbers: true,
            require_symbols: false,
        }
    }
}

/// Authentication service
#[derive(Debug, Clone)]
pub struct AuthService {
    db_pool: Pool<Sqlite>,
    jwt_manager: Arc<JwtManager>,
    password_requirements: PasswordRequirements,
    max_login_attempts: i32,
    lockout_duration: Duration,
}

impl AuthService {
    /// Create new authentication service
    pub fn new(
        db_pool: Pool<Sqlite>,
        jwt_manager: Arc<JwtManager>,
        max_login_attempts: i32,
        lockout_duration: Duration,
    ) -> Self {
        Self {
            db_pool,
            jwt_manager,
            password_requirements: PasswordRequirements::default(),
            max_login_attempts,
            lockout_duration,
        }
    }

    /// Create with default settings
    pub fn with_defaults(db_pool: Pool<Sqlite>, jwt_secret: &str) -> Self {
        let jwt_manager = Arc::new(JwtManager::with_secret(jwt_secret));
        Self::new(db_pool, jwt_manager, 5, Duration::minutes(15))
    }

    /// Hash password using bcrypt
    pub fn hash_password(&self, password: &str) -> SecurityResult<String> {
        // Validate password requirements
        self.validate_password(password)?;

        hash(password, DEFAULT_COST).map_err(SecurityError::PasswordHashError)
    }

    /// Verify password against hash
    pub fn verify_password(&self, password: &str, hash: &str) -> SecurityResult<bool> {
        verify(password, hash).map_err(SecurityError::PasswordHashError)
    }

    /// Validate password against requirements
    pub fn validate_password(&self, password: &str) -> SecurityResult<()> {
        let requirements = &self.password_requirements;

        if password.len() < requirements.min_length {
            return Err(SecurityError::AuthenticationFailed(format!(
                "Password must be at least {} characters long",
                requirements.min_length
            )));
        }

        if requirements.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
            return Err(SecurityError::AuthenticationFailed(
                "Password must contain at least one uppercase letter".to_string(),
            ));
        }

        if requirements.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
            return Err(SecurityError::AuthenticationFailed(
                "Password must contain at least one lowercase letter".to_string(),
            ));
        }

        if requirements.require_numbers && !password.chars().any(|c| c.is_numeric()) {
            return Err(SecurityError::AuthenticationFailed(
                "Password must contain at least one number".to_string(),
            ));
        }

        if requirements.require_symbols && !password.chars().any(|c| c.is_ascii_punctuation()) {
            return Err(SecurityError::AuthenticationFailed(
                "Password must contain at least one symbol".to_string(),
            ));
        }

        Ok(())
    }

    /// Create a new user
    pub async fn create_user(
        &self,
        username: &str,
        email: Option<&str>,
        password: &str,
        role: UserRole,
        created_by: &str,
    ) -> SecurityResult<User> {
        // Check if username already exists
        if self.get_user_by_username(username).await?.is_some() {
            return Err(SecurityError::AuthenticationFailed(
                "Username already exists".to_string(),
            ));
        }

        // Hash the password
        let password_hash = self.hash_password(password)?;

        // Create user
        let user = User::new(
            username.to_string(),
            email.map(|e| e.to_string()),
            password_hash,
            role,
        );

        // Insert into database
        sqlx::query!(
            r#"
            INSERT INTO users (id, username, email, password_hash, role, is_active, created_at, updated_at, failed_login_attempts, created_by)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            user.id,
            user.username,
            user.email,
            user.password_hash,
            user.role as UserRole,
            user.is_active,
            user.created_at,
            user.updated_at,
            user.failed_login_attempts,
            created_by
        ).execute(&self.db_pool).await?;

        info!("Created user: {} with role: {:?}", username, role);
        Ok(user)
    }

    /// Authenticate user with username and password
    pub async fn authenticate(
        &self,
        username: &str,
        password: &str,
        ip_address: Option<String>,
    ) -> SecurityResult<TokenPair> {
        // Get user by username
        let mut user = self.get_user_by_username(username).await?.ok_or_else(|| {
            SecurityError::AuthenticationFailed("Invalid credentials".to_string())
        })?;

        // Check if user is active
        if !user.is_active {
            warn!("Authentication attempt for inactive user: {}", username);
            return Err(SecurityError::AuthenticationFailed(
                "Account is disabled".to_string(),
            ));
        }

        // Check if user is locked
        if user.is_locked() {
            warn!("Authentication attempt for locked user: {}", username);
            return Err(SecurityError::AuthenticationFailed(
                "Account is temporarily locked".to_string(),
            ));
        }

        // Verify password
        if !self.verify_password(password, &user.password_hash)? {
            // Increment failed login attempts
            user.failed_login_attempts += 1;

            // Lock account if too many failed attempts
            if user.failed_login_attempts >= self.max_login_attempts {
                user.locked_until = Some(Utc::now() + self.lockout_duration);
                warn!(
                    "User {} locked due to too many failed login attempts",
                    username
                );
            }

            // Update user in database
            self.update_login_attempts(&user).await?;

            return Err(SecurityError::AuthenticationFailed(
                "Invalid credentials".to_string(),
            ));
        }

        // Reset failed login attempts on successful authentication
        user.failed_login_attempts = 0;
        user.locked_until = None;
        user.last_login_at = Some(Utc::now());

        // Update user in database
        self.update_successful_login(&user).await?;

        // Generate tokens
        let access_token = self.jwt_manager.generate_access_token(&user)?;
        let refresh_token = self.jwt_manager.generate_refresh_token(&user)?;
        let expires_in = self.jwt_manager.get_access_token_duration().num_seconds();

        info!(
            "Successful authentication for user: {} from IP: {:?}",
            username, ip_address
        );

        Ok(TokenPair::new(access_token, refresh_token, expires_in))
    }

    /// Refresh access token using refresh token
    pub async fn refresh_token(&self, refresh_token: &str) -> SecurityResult<TokenPair> {
        // Validate refresh token
        let claims = self.jwt_manager.validate_refresh_token(refresh_token)?;

        // Get current user info
        let user = self
            .get_user_by_id(&claims.sub)
            .await?
            .ok_or_else(|| SecurityError::AuthenticationFailed("User not found".to_string()))?;

        // Check if user is still active
        if !user.is_active {
            return Err(SecurityError::AuthenticationFailed(
                "Account is disabled".to_string(),
            ));
        }

        // Generate new tokens
        let access_token = self.jwt_manager.generate_access_token(&user)?;
        let new_refresh_token = self.jwt_manager.generate_refresh_token(&user)?;
        let expires_in = self.jwt_manager.get_access_token_duration().num_seconds();

        Ok(TokenPair::new(access_token, new_refresh_token, expires_in))
    }

    /// Change user password
    pub async fn change_password(
        &self,
        user_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> SecurityResult<()> {
        // Get user
        let user = self
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| SecurityError::AuthenticationFailed("User not found".to_string()))?;

        // Verify current password
        if !self.verify_password(current_password, &user.password_hash)? {
            return Err(SecurityError::AuthenticationFailed(
                "Current password is incorrect".to_string(),
            ));
        }

        // Hash new password
        let new_password_hash = self.hash_password(new_password)?;

        // Update password in database
        sqlx::query!(
            "UPDATE users SET password_hash = ?, updated_at = ? WHERE id = ?",
            new_password_hash,
            Utc::now(),
            user_id
        )
        .execute(&self.db_pool)
        .await?;

        info!("Password changed for user: {}", user.username);
        Ok(())
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, user_id: &str) -> SecurityResult<Option<User>> {
        let row = sqlx::query!("SELECT * FROM users WHERE id = ?", user_id)
            .fetch_optional(&self.db_pool)
            .await?;

        if let Some(row) = row {
            let role: UserRole = serde_json::from_str(&row.role).map_err(|e| {
                SecurityError::Internal(anyhow::anyhow!("Failed to parse user role: {}", e))
            })?;

            Ok(Some(User {
                id: row.id,
                username: row.username,
                email: row.email,
                password_hash: row.password_hash,
                role,
                is_active: row.is_active,
                created_at: row.created_at,
                updated_at: row.updated_at,
                last_login_at: row.last_login_at,
                failed_login_attempts: row.failed_login_attempts,
                locked_until: row.locked_until,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> SecurityResult<Option<User>> {
        let row = sqlx::query!("SELECT * FROM users WHERE username = ?", username)
            .fetch_optional(&self.db_pool)
            .await?;

        if let Some(row) = row {
            let role: UserRole = serde_json::from_str(&row.role).map_err(|e| {
                SecurityError::Internal(anyhow::anyhow!("Failed to parse user role: {}", e))
            })?;

            Ok(Some(User {
                id: row.id,
                username: row.username,
                email: row.email,
                password_hash: row.password_hash,
                role,
                is_active: row.is_active,
                created_at: row.created_at,
                updated_at: row.updated_at,
                last_login_at: row.last_login_at,
                failed_login_attempts: row.failed_login_attempts,
                locked_until: row.locked_until,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update login attempts after failed authentication
    async fn update_login_attempts(&self, user: &User) -> SecurityResult<()> {
        sqlx::query!(
            "UPDATE users SET failed_login_attempts = ?, locked_until = ?, updated_at = ? WHERE id = ?",
            user.failed_login_attempts,
            user.locked_until,
            Utc::now(),
            user.id
        ).execute(&self.db_pool).await?;

        Ok(())
    }

    /// Update user after successful login
    async fn update_successful_login(&self, user: &User) -> SecurityResult<()> {
        sqlx::query!(
            "UPDATE users SET failed_login_attempts = ?, locked_until = ?, last_login_at = ?, updated_at = ? WHERE id = ?",
            user.failed_login_attempts,
            user.locked_until,
            user.last_login_at,
            Utc::now(),
            user.id
        ).execute(&self.db_pool).await?;

        Ok(())
    }

    /// List all users (admin only)
    pub async fn list_users(&self) -> SecurityResult<Vec<User>> {
        let rows = sqlx::query!("SELECT * FROM users ORDER BY created_at DESC")
            .fetch_all(&self.db_pool)
            .await?;

        let mut users = Vec::new();
        for row in rows {
            let role: UserRole = serde_json::from_str(&row.role).map_err(|e| {
                SecurityError::Internal(anyhow::anyhow!("Failed to parse user role: {}", e))
            })?;

            users.push(User {
                id: row.id,
                username: row.username,
                email: row.email,
                password_hash: row.password_hash,
                role,
                is_active: row.is_active,
                created_at: row.created_at,
                updated_at: row.updated_at,
                last_login_at: row.last_login_at,
                failed_login_attempts: row.failed_login_attempts,
                locked_until: row.locked_until,
            });
        }

        Ok(users)
    }

    /// Update user (admin only)
    pub async fn update_user(
        &self,
        user_id: &str,
        email: Option<String>,
        role: Option<UserRole>,
        is_active: Option<bool>,
    ) -> SecurityResult<User> {
        let mut user = self
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| SecurityError::AuthenticationFailed("User not found".to_string()))?;

        if let Some(email) = email {
            user.email = Some(email);
        }
        if let Some(role) = role {
            user.role = role;
        }
        if let Some(is_active) = is_active {
            user.is_active = is_active;
        }
        user.updated_at = Utc::now();

        sqlx::query!(
            "UPDATE users SET email = ?, role = ?, is_active = ?, updated_at = ? WHERE id = ?",
            user.email,
            serde_json::to_string(&user.role).unwrap(),
            user.is_active,
            user.updated_at,
            user.id
        )
        .execute(&self.db_pool)
        .await?;

        info!("Updated user: {}", user.username);
        Ok(user)
    }

    /// Delete user (admin only)
    pub async fn delete_user(&self, user_id: &str) -> SecurityResult<()> {
        let user = self
            .get_user_by_id(user_id)
            .await?
            .ok_or_else(|| SecurityError::AuthenticationFailed("User not found".to_string()))?;

        sqlx::query!("DELETE FROM users WHERE id = ?", user_id)
            .execute(&self.db_pool)
            .await?;

        info!("Deleted user: {}", user.username);
        Ok(())
    }

    /// Get JWT manager reference
    pub fn jwt_manager(&self) -> &JwtManager {
        &self.jwt_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePool;

    async fn setup_test_db() -> Pool<Sqlite> {
        let pool = SqlitePool::connect(":memory:").await.unwrap();

        // Create users table
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
            )
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn test_password_hashing_and_verification() {
        let pool = setup_test_db().await;
        let auth_service = AuthService::with_defaults(pool, "test_secret");

        let password = "TestPassword123!";
        let hash = auth_service.hash_password(password).unwrap();

        assert!(auth_service.verify_password(password, &hash).unwrap());
        assert!(!auth_service
            .verify_password("WrongPassword", &hash)
            .unwrap());
    }

    #[tokio::test]
    async fn test_password_validation() {
        let pool = setup_test_db().await;
        let auth_service = AuthService::with_defaults(pool, "test_secret");

        // Valid password
        assert!(auth_service.validate_password("TestPassword123").is_ok());

        // Too short
        assert!(auth_service.validate_password("Test1").is_err());

        // Missing uppercase
        assert!(auth_service.validate_password("testpassword123").is_err());

        // Missing lowercase
        assert!(auth_service.validate_password("TESTPASSWORD123").is_err());

        // Missing numbers
        assert!(auth_service.validate_password("TestPassword").is_err());
    }

    #[tokio::test]
    async fn test_user_creation_and_authentication() {
        let pool = setup_test_db().await;
        let auth_service = AuthService::with_defaults(pool, "test_secret");

        let username = "testuser";
        let email = "test@example.com";
        let password = "TestPassword123";
        let created_by = "admin";

        // Create user
        let user = auth_service
            .create_user(username, Some(email), password, UserRole::Agent, created_by)
            .await
            .unwrap();
        assert_eq!(user.username, username);
        assert_eq!(user.email, Some(email.to_string()));
        assert_eq!(user.role, UserRole::Agent);

        // Test authentication
        let token_pair = auth_service
            .authenticate(username, password, None)
            .await
            .unwrap();
        assert!(!token_pair.access_token.is_empty());
        assert!(!token_pair.refresh_token.is_empty());
        assert_eq!(token_pair.token_type, "Bearer");

        // Test invalid password
        let result = auth_service
            .authenticate(username, "wrong_password", None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_token_refresh() {
        let pool = setup_test_db().await;
        let auth_service = AuthService::with_defaults(pool, "test_secret");

        let username = "testuser";
        let password = "TestPassword123";
        let created_by = "admin";

        // Create user and authenticate
        auth_service
            .create_user(username, None, password, UserRole::Agent, created_by)
            .await
            .unwrap();
        let token_pair = auth_service
            .authenticate(username, password, None)
            .await
            .unwrap();

        // Refresh token
        let new_token_pair = auth_service
            .refresh_token(&token_pair.refresh_token)
            .await
            .unwrap();
        assert!(!new_token_pair.access_token.is_empty());
        assert!(!new_token_pair.refresh_token.is_empty());
        assert_ne!(new_token_pair.access_token, token_pair.access_token);
    }
}
