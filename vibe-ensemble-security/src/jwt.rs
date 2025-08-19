//! JWT token management

use crate::{SecurityError, SecurityResult, TokenClaims, TokenType, User, UserRole};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT token manager
#[derive(Debug, Clone)]
pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    audience: String,
    access_token_duration: Duration,
    refresh_token_duration: Duration,
}

impl JwtManager {
    /// Create a new JWT manager with the provided secret
    pub fn new(
        secret: &[u8],
        issuer: String,
        audience: String,
        access_token_duration: Duration,
        refresh_token_duration: Duration,
    ) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            issuer,
            audience,
            access_token_duration,
            refresh_token_duration,
        }
    }

    /// Create JWT manager with default settings
    pub fn with_secret(secret: &str) -> Self {
        Self::new(
            secret.as_bytes(),
            "vibe-ensemble".to_string(),
            "vibe-ensemble-users".to_string(),
            Duration::hours(1), // 1 hour access tokens
            Duration::days(30), // 30 day refresh tokens
        )
    }

    /// Generate access token for user
    pub fn generate_access_token(&self, user: &User) -> SecurityResult<String> {
        let now = Utc::now();
        let claims = TokenClaims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            iat: now.timestamp(),
            exp: (now + self.access_token_duration).timestamp(),
            aud: self.audience.clone(),
            iss: self.issuer.clone(),
            token_type: TokenType::Access,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(SecurityError::JwtError)
    }

    /// Generate refresh token for user
    pub fn generate_refresh_token(&self, user: &User) -> SecurityResult<String> {
        let now = Utc::now();
        let claims = TokenClaims {
            sub: user.id.clone(),
            username: user.username.clone(),
            role: user.role.clone(),
            iat: now.timestamp(),
            exp: (now + self.refresh_token_duration).timestamp(),
            aud: self.audience.clone(),
            iss: self.issuer.clone(),
            token_type: TokenType::Refresh,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(SecurityError::JwtError)
    }

    /// Generate agent authentication token
    pub fn generate_agent_token(
        &self,
        agent_id: &str,
        permissions: Vec<String>,
    ) -> SecurityResult<String> {
        let now = Utc::now();
        let claims = AgentTokenClaims {
            sub: agent_id.to_string(),
            permissions,
            iat: now.timestamp(),
            exp: (now + Duration::days(365)).timestamp(), // Agent tokens last 1 year
            aud: "vibe-ensemble-agents".to_string(),
            iss: self.issuer.clone(),
            token_type: TokenType::AgentAuth,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(SecurityError::JwtError)
    }

    /// Validate and decode access token
    pub fn validate_access_token(&self, token: &str) -> SecurityResult<TokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[&self.audience]);
        validation.set_issuer(&[&self.issuer]);

        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(SecurityError::JwtError)?;

        // Verify token type
        if token_data.claims.token_type != TokenType::Access {
            return Err(SecurityError::InvalidTokenFormat);
        }

        // Check if token is expired
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(SecurityError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Validate and decode refresh token
    pub fn validate_refresh_token(&self, token: &str) -> SecurityResult<TokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&[&self.audience]);
        validation.set_issuer(&[&self.issuer]);

        let token_data = decode::<TokenClaims>(token, &self.decoding_key, &validation)
            .map_err(SecurityError::JwtError)?;

        // Verify token type
        if token_data.claims.token_type != TokenType::Refresh {
            return Err(SecurityError::InvalidTokenFormat);
        }

        // Check if token is expired
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(SecurityError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Validate agent token
    pub fn validate_agent_token(&self, token: &str) -> SecurityResult<AgentTokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_audience(&["vibe-ensemble-agents"]);
        validation.set_issuer(&[&self.issuer]);

        let token_data = decode::<AgentTokenClaims>(token, &self.decoding_key, &validation)
            .map_err(SecurityError::JwtError)?;

        // Verify token type
        if token_data.claims.token_type != TokenType::AgentAuth {
            return Err(SecurityError::InvalidTokenFormat);
        }

        // Check if token is expired
        let now = Utc::now().timestamp();
        if token_data.claims.exp < now {
            return Err(SecurityError::TokenExpired);
        }

        Ok(token_data.claims)
    }

    /// Extract token from Authorization header
    pub fn extract_token_from_header(auth_header: &str) -> Option<&str> {
        if auth_header.starts_with("Bearer ") {
            Some(&auth_header[7..])
        } else {
            None
        }
    }

    /// Get token expiration time
    pub fn get_access_token_duration(&self) -> Duration {
        self.access_token_duration
    }

    /// Get refresh token expiration time
    pub fn get_refresh_token_duration(&self) -> Duration {
        self.refresh_token_duration
    }
}

/// JWT claims for agent tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTokenClaims {
    pub sub: String,              // Subject (agent ID)
    pub permissions: Vec<String>, // Agent permissions
    pub iat: i64,                 // Issued at
    pub exp: i64,                 // Expires at
    pub aud: String,              // Audience
    pub iss: String,              // Issuer
    pub token_type: TokenType,    // Type of token
}

/// Token pair for authentication responses
#[derive(Debug, Serialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
}

impl TokenPair {
    pub fn new(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{User, UserRole};

    #[test]
    fn test_jwt_token_generation_and_validation() {
        let jwt_manager = JwtManager::with_secret("test_secret_key_for_jwt_testing");

        let user = User::new(
            "testuser".to_string(),
            Some("test@example.com".to_string()),
            "hashed_password".to_string(),
            UserRole::Agent,
        );

        // Test access token
        let access_token = jwt_manager.generate_access_token(&user).unwrap();
        let claims = jwt_manager.validate_access_token(&access_token).unwrap();

        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.username, user.username);
        assert_eq!(claims.role, user.role);
        assert_eq!(claims.token_type, TokenType::Access);

        // Test refresh token
        let refresh_token = jwt_manager.generate_refresh_token(&user).unwrap();
        let claims = jwt_manager.validate_refresh_token(&refresh_token).unwrap();

        assert_eq!(claims.sub, user.id);
        assert_eq!(claims.username, user.username);
        assert_eq!(claims.role, user.role);
        assert_eq!(claims.token_type, TokenType::Refresh);
    }

    #[test]
    fn test_agent_token_generation_and_validation() {
        let jwt_manager = JwtManager::with_secret("test_secret_key_for_jwt_testing");

        let agent_id = "agent_123";
        let permissions = vec!["read".to_string(), "write".to_string()];

        let agent_token = jwt_manager
            .generate_agent_token(agent_id, permissions.clone())
            .unwrap();
        let claims = jwt_manager.validate_agent_token(&agent_token).unwrap();

        assert_eq!(claims.sub, agent_id);
        assert_eq!(claims.permissions, permissions);
        assert_eq!(claims.token_type, TokenType::AgentAuth);
    }

    #[test]
    fn test_token_header_extraction() {
        let token = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
        let header = format!("Bearer {}", token);

        let extracted = JwtManager::extract_token_from_header(&header);
        assert_eq!(extracted, Some(token));

        let invalid_header = "InvalidHeader";
        let extracted = JwtManager::extract_token_from_header(invalid_header);
        assert_eq!(extracted, None);
    }

    #[test]
    fn test_invalid_token_validation() {
        let jwt_manager = JwtManager::with_secret("test_secret_key_for_jwt_testing");

        // Test invalid token
        let result = jwt_manager.validate_access_token("invalid_token");
        assert!(result.is_err());

        // Test wrong token type
        let user = User::new(
            "testuser".to_string(),
            Some("test@example.com".to_string()),
            "hashed_password".to_string(),
            UserRole::Agent,
        );

        let refresh_token = jwt_manager.generate_refresh_token(&user).unwrap();
        let result = jwt_manager.validate_access_token(&refresh_token);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::InvalidTokenFormat
        ));
    }
}
