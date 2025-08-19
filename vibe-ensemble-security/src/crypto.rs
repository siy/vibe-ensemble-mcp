//! Cryptographic utilities for secure communications

use crate::{SecurityError, SecurityResult};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Encrypted message envelope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedMessage {
    /// Base64-encoded encrypted content
    pub content: String,
    /// Base64-encoded nonce/IV
    pub nonce: String,
    /// Encryption algorithm used
    pub algorithm: String,
    /// Key ID used for encryption
    pub key_id: String,
    /// Additional authenticated data (AAD) if any
    pub aad: Option<String>,
    /// Timestamp when encrypted
    pub encrypted_at: chrono::DateTime<chrono::Utc>,
}

/// Encryption key metadata
#[derive(Debug, Clone)]
pub struct EncryptionKey {
    pub id: String,
    pub key: [u8; 32], // AES-256 key
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
}

impl EncryptionKey {
    /// Generate a new encryption key
    pub fn generate(id: String) -> Self {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        Self {
            id,
            key,
            created_at: chrono::Utc::now(),
            expires_at: None,
            is_active: true,
        }
    }

    /// Create key from existing bytes
    pub fn from_bytes(id: String, key_bytes: [u8; 32]) -> Self {
        Self {
            id,
            key: key_bytes,
            created_at: chrono::Utc::now(),
            expires_at: None,
            is_active: true,
        }
    }

    /// Check if key is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            chrono::Utc::now() > expires_at
        } else {
            false
        }
    }

    /// Check if key is valid (active and not expired)
    pub fn is_valid(&self) -> bool {
        self.is_active && !self.is_expired()
    }
}

/// Encryption service for secure message handling
#[derive(Debug, Clone)]
pub struct EncryptionService {
    /// Current encryption keys (key_id -> key)
    keys: Arc<RwLock<HashMap<String, EncryptionKey>>>,
    /// Current active key ID for new encryptions
    active_key_id: Arc<RwLock<String>>,
}

impl EncryptionService {
    /// Create new encryption service
    pub fn new() -> Self {
        let initial_key = EncryptionKey::generate("key_001".to_string());
        let initial_key_id = initial_key.id.clone();

        let mut keys = HashMap::new();
        keys.insert(initial_key_id.clone(), initial_key);

        Self {
            keys: Arc::new(RwLock::new(keys)),
            active_key_id: Arc::new(RwLock::new(initial_key_id)),
        }
    }

    /// Create with specific master key
    pub fn with_master_key(master_key: &[u8; 32]) -> Self {
        let initial_key = EncryptionKey::from_bytes("master".to_string(), *master_key);
        let initial_key_id = initial_key.id.clone();

        let mut keys = HashMap::new();
        keys.insert(initial_key_id.clone(), initial_key);

        Self {
            keys: Arc::new(RwLock::new(keys)),
            active_key_id: Arc::new(RwLock::new(initial_key_id)),
        }
    }

    /// Encrypt a message
    pub async fn encrypt_message(&self, plaintext: &str) -> SecurityResult<EncryptedMessage> {
        self.encrypt_message_with_aad(plaintext, None).await
    }

    /// Encrypt a message with additional authenticated data
    pub async fn encrypt_message_with_aad(
        &self,
        plaintext: &str,
        aad: Option<&str>,
    ) -> SecurityResult<EncryptedMessage> {
        let active_key_id = {
            let key_id = self.active_key_id.read().await;
            key_id.clone()
        };

        let encryption_key = {
            let keys = self.keys.read().await;
            keys.get(&active_key_id)
                .ok_or_else(|| {
                    SecurityError::EncryptionError("Active encryption key not found".to_string())
                })?
                .clone()
        };

        if !encryption_key.is_valid() {
            return Err(SecurityError::EncryptionError(
                "Active encryption key is invalid".to_string(),
            ));
        }

        // Create cipher
        let key = Key::<Aes256Gcm>::from_slice(&encryption_key.key);
        let cipher = Aes256Gcm::new(key);

        // Generate random nonce
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        // Encrypt the message
        let ciphertext = if let Some(aad_data) = aad {
            cipher.encrypt(
                &nonce,
                aes_gcm::aead::Payload {
                    msg: plaintext.as_bytes(),
                    aad: aad_data.as_bytes(),
                },
            )
        } else {
            cipher.encrypt(&nonce, plaintext.as_bytes())
        }
        .map_err(|e| SecurityError::EncryptionError(format!("Encryption failed: {}", e)))?;

        // Create encrypted message envelope
        Ok(EncryptedMessage {
            content: STANDARD.encode(&ciphertext),
            nonce: STANDARD.encode(&nonce),
            algorithm: "AES-256-GCM".to_string(),
            key_id: active_key_id,
            aad: aad.map(|s| s.to_string()),
            encrypted_at: chrono::Utc::now(),
        })
    }

    /// Decrypt a message
    pub async fn decrypt_message(
        &self,
        encrypted_msg: &EncryptedMessage,
    ) -> SecurityResult<String> {
        let encryption_key = {
            let keys = self.keys.read().await;
            keys.get(&encrypted_msg.key_id)
                .ok_or_else(|| {
                    SecurityError::EncryptionError(format!(
                        "Encryption key '{}' not found",
                        encrypted_msg.key_id
                    ))
                })?
                .clone()
        };

        // Verify algorithm
        if encrypted_msg.algorithm != "AES-256-GCM" {
            return Err(SecurityError::EncryptionError(format!(
                "Unsupported algorithm: {}",
                encrypted_msg.algorithm
            )));
        }

        // Decode base64 content and nonce
        let ciphertext = STANDARD.decode(&encrypted_msg.content).map_err(|e| {
            SecurityError::EncryptionError(format!("Failed to decode content: {}", e))
        })?;
        let nonce_bytes = STANDARD.decode(&encrypted_msg.nonce).map_err(|e| {
            SecurityError::EncryptionError(format!("Failed to decode nonce: {}", e))
        })?;

        // Create nonce
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create cipher
        let key = Key::<Aes256Gcm>::from_slice(&encryption_key.key);
        let cipher = Aes256Gcm::new(key);

        // Decrypt the message
        let plaintext = if let Some(aad_data) = &encrypted_msg.aad {
            cipher.decrypt(
                nonce,
                aes_gcm::aead::Payload {
                    msg: &ciphertext,
                    aad: aad_data.as_bytes(),
                },
            )
        } else {
            cipher.decrypt(nonce, ciphertext.as_ref())
        }
        .map_err(|e| SecurityError::EncryptionError(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext).map_err(|e| {
            SecurityError::EncryptionError(format!("Invalid UTF-8 in decrypted text: {}", e))
        })
    }

    /// Add a new encryption key
    pub async fn add_key(&self, key: EncryptionKey) -> SecurityResult<()> {
        let mut keys = self.keys.write().await;
        keys.insert(key.id.clone(), key);
        Ok(())
    }

    /// Rotate to a new encryption key
    pub async fn rotate_key(&self, new_key_id: Option<String>) -> SecurityResult<String> {
        let key_id =
            new_key_id.unwrap_or_else(|| format!("key_{}", chrono::Utc::now().timestamp()));

        // Generate new key
        let new_key = EncryptionKey::generate(key_id.clone());

        // Add to key store
        {
            let mut keys = self.keys.write().await;
            keys.insert(key_id.clone(), new_key);
        }

        // Update active key ID
        {
            let mut active_key_id = self.active_key_id.write().await;
            *active_key_id = key_id.clone();
        }

        tracing::info!("Rotated encryption key to: {}", key_id);
        Ok(key_id)
    }

    /// Deactivate an encryption key (for key retirement)
    pub async fn deactivate_key(&self, key_id: &str) -> SecurityResult<()> {
        let mut keys = self.keys.write().await;
        if let Some(key) = keys.get_mut(key_id) {
            key.is_active = false;
            tracing::info!("Deactivated encryption key: {}", key_id);
            Ok(())
        } else {
            Err(SecurityError::EncryptionError(format!(
                "Key '{}' not found",
                key_id
            )))
        }
    }

    /// Get current active key ID
    pub async fn get_active_key_id(&self) -> String {
        let active_key_id = self.active_key_id.read().await;
        active_key_id.clone()
    }

    /// List all available keys
    pub async fn list_keys(&self) -> Vec<String> {
        let keys = self.keys.read().await;
        keys.keys().cloned().collect()
    }

    /// Clean up expired keys
    pub async fn cleanup_expired_keys(&self) -> SecurityResult<Vec<String>> {
        let mut keys = self.keys.write().await;
        let expired_keys: Vec<String> = keys
            .iter()
            .filter(|(_, key)| key.is_expired())
            .map(|(id, _)| id.clone())
            .collect();

        for key_id in &expired_keys {
            keys.remove(key_id);
            tracing::info!("Removed expired encryption key: {}", key_id);
        }

        Ok(expired_keys)
    }

    /// Encrypt sensitive data for storage
    pub async fn encrypt_for_storage(&self, data: &str, context: &str) -> SecurityResult<String> {
        let encrypted = self.encrypt_message_with_aad(data, Some(context)).await?;
        serde_json::to_string(&encrypted).map_err(|e| {
            SecurityError::EncryptionError(format!("Failed to serialize encrypted data: {}", e))
        })
    }

    /// Decrypt sensitive data from storage
    pub async fn decrypt_from_storage(&self, encrypted_data: &str) -> SecurityResult<String> {
        let encrypted_msg: EncryptedMessage =
            serde_json::from_str(encrypted_data).map_err(|e| {
                SecurityError::EncryptionError(format!(
                    "Failed to deserialize encrypted data: {}",
                    e
                ))
            })?;

        self.decrypt_message(&encrypted_msg).await
    }

    /// Get key metadata (without exposing the actual key)
    pub async fn get_key_metadata(&self, key_id: &str) -> Option<KeyMetadata> {
        let keys = self.keys.read().await;
        keys.get(key_id).map(|key| KeyMetadata {
            id: key.id.clone(),
            created_at: key.created_at,
            expires_at: key.expires_at,
            is_active: key.is_active,
            is_expired: key.is_expired(),
        })
    }
}

impl Default for EncryptionService {
    fn default() -> Self {
        Self::new()
    }
}

/// Key metadata for API responses (doesn't expose the actual key)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub is_active: bool,
    pub is_expired: bool,
}

/// CSRF token for cross-site request forgery protection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsrfToken {
    /// The actual token value
    pub token: String,
    /// When the token was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the token expires
    pub expires_at: chrono::DateTime<chrono::Utc>,
    /// Session ID this token is bound to
    pub session_id: String,
}

impl CsrfToken {
    /// Create a new CSRF token
    pub fn new(session_id: String, duration: chrono::Duration) -> SecurityResult<Self> {
        let mut token_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut token_bytes);
        let token = STANDARD.encode(&token_bytes);

        let created_at = chrono::Utc::now();
        let expires_at = created_at + duration;

        Ok(Self {
            token,
            created_at,
            expires_at,
            session_id,
        })
    }

    /// Check if token is expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now() > self.expires_at
    }

    /// Validate token matches session and is not expired
    pub fn is_valid(&self, session_id: &str) -> bool {
        !self.is_expired() && self.session_id == session_id
    }
}

/// CSRF token manager for web security
#[derive(Debug, Clone)]
pub struct CsrfTokenManager {
    /// Active tokens (token -> csrf_token)
    tokens: Arc<RwLock<HashMap<String, CsrfToken>>>,
    /// Token expiration duration
    token_duration: chrono::Duration,
}

impl CsrfTokenManager {
    /// Create new CSRF token manager
    pub fn new(token_duration: chrono::Duration) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            token_duration,
        }
    }

    /// Create with default settings (1 hour expiration)
    pub fn default() -> Self {
        Self::new(chrono::Duration::hours(1))
    }

    /// Generate a new CSRF token for a session
    pub async fn generate_token(&self, session_id: &str) -> SecurityResult<String> {
        let csrf_token = CsrfToken::new(session_id.to_string(), self.token_duration)?;
        let token_value = csrf_token.token.clone();

        {
            let mut tokens = self.tokens.write().await;
            tokens.insert(token_value.clone(), csrf_token);
        }

        Ok(token_value)
    }

    /// Validate a CSRF token
    pub async fn validate_token(&self, token: &str, session_id: &str) -> bool {
        let tokens = self.tokens.read().await;

        if let Some(csrf_token) = tokens.get(token) {
            csrf_token.is_valid(session_id)
        } else {
            false
        }
    }

    /// Consume a CSRF token (single use)
    pub async fn consume_token(&self, token: &str, session_id: &str) -> bool {
        let mut tokens = self.tokens.write().await;

        if let Some(csrf_token) = tokens.get(token) {
            if csrf_token.is_valid(session_id) {
                tokens.remove(token);
                return true;
            }
        }

        false
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired_tokens(&self) -> usize {
        let mut tokens = self.tokens.write().await;
        let before_count = tokens.len();

        tokens.retain(|_, csrf_token| !csrf_token.is_expired());

        let removed = before_count - tokens.len();
        if removed > 0 {
            tracing::debug!("Cleaned up {} expired CSRF tokens", removed);
        }

        removed
    }

    /// Invalidate all tokens for a session
    pub async fn invalidate_session_tokens(&self, session_id: &str) -> usize {
        let mut tokens = self.tokens.write().await;
        let before_count = tokens.len();

        tokens.retain(|_, csrf_token| csrf_token.session_id != session_id);

        let removed = before_count - tokens.len();
        if removed > 0 {
            tracing::debug!(
                "Invalidated {} CSRF tokens for session {}",
                removed,
                session_id
            );
        }

        removed
    }

    /// Get token statistics
    pub async fn get_stats(&self) -> (usize, usize) {
        let tokens = self.tokens.read().await;
        let total = tokens.len();
        let expired = tokens.values().filter(|t| t.is_expired()).count();
        (total, expired)
    }
}

/// Generate a secure random token
pub fn generate_secure_token(length: usize) -> String {
    let mut token_bytes = vec![0u8; length];
    OsRng.fill_bytes(&mut token_bytes);
    STANDARD.encode(&token_bytes)
}

/// Helper functions for common encryption patterns
impl EncryptionService {
    /// Encrypt password for secure storage
    pub async fn encrypt_password(&self, password: &str, user_id: &str) -> SecurityResult<String> {
        let context = format!("password:{}", user_id);
        self.encrypt_for_storage(password, &context).await
    }

    /// Encrypt token for secure storage
    pub async fn encrypt_token(&self, token: &str, token_type: &str) -> SecurityResult<String> {
        let context = format!("token:{}", token_type);
        self.encrypt_for_storage(token, &context).await
    }

    /// Encrypt message content for secure transmission
    pub async fn encrypt_message_content(
        &self,
        content: &str,
        sender_id: &str,
        recipient_id: &str,
    ) -> SecurityResult<EncryptedMessage> {
        let aad = format!("message:{}:{}", sender_id, recipient_id);
        self.encrypt_message_with_aad(content, Some(&aad)).await
    }

    /// Encrypt knowledge content for secure storage
    pub async fn encrypt_knowledge_content(
        &self,
        content: &str,
        knowledge_id: &str,
        creator_id: &str,
    ) -> SecurityResult<String> {
        let context = format!("knowledge:{}:{}", knowledge_id, creator_id);
        self.encrypt_for_storage(content, &context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_encryption_decryption() {
        let encryption_service = EncryptionService::new();
        let plaintext = "This is a secret message";

        // Encrypt
        let encrypted = encryption_service.encrypt_message(plaintext).await.unwrap();
        assert_eq!(encrypted.algorithm, "AES-256-GCM");
        assert!(!encrypted.content.is_empty());
        assert!(!encrypted.nonce.is_empty());

        // Decrypt
        let decrypted = encryption_service
            .decrypt_message(&encrypted)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_encryption_with_aad() {
        let encryption_service = EncryptionService::new();
        let plaintext = "Secret message with authentication";
        let aad = "user:123:message";

        // Encrypt with AAD
        let encrypted = encryption_service
            .encrypt_message_with_aad(plaintext, Some(aad))
            .await
            .unwrap();
        assert_eq!(encrypted.aad, Some(aad.to_string()));

        // Decrypt should work
        let decrypted = encryption_service
            .decrypt_message(&encrypted)
            .await
            .unwrap();
        assert_eq!(decrypted, plaintext);

        // Modify AAD and decryption should fail
        let mut tampered_encrypted = encrypted.clone();
        tampered_encrypted.aad = Some("user:456:message".to_string());

        let result = encryption_service
            .decrypt_message(&tampered_encrypted)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let encryption_service = EncryptionService::new();
        let original_key_id = encryption_service.get_active_key_id().await;

        // Encrypt with original key
        let plaintext = "Test message";
        let encrypted1 = encryption_service.encrypt_message(plaintext).await.unwrap();
        assert_eq!(encrypted1.key_id, original_key_id);

        // Rotate key
        let new_key_id = encryption_service
            .rotate_key(Some("new_key".to_string()))
            .await
            .unwrap();
        assert_eq!(new_key_id, "new_key");
        assert_ne!(new_key_id, original_key_id);

        // Encrypt with new key
        let encrypted2 = encryption_service.encrypt_message(plaintext).await.unwrap();
        assert_eq!(encrypted2.key_id, new_key_id);

        // Both encrypted messages should decrypt correctly
        let decrypted1 = encryption_service
            .decrypt_message(&encrypted1)
            .await
            .unwrap();
        let decrypted2 = encryption_service
            .decrypt_message(&encrypted2)
            .await
            .unwrap();
        assert_eq!(decrypted1, plaintext);
        assert_eq!(decrypted2, plaintext);
    }

    #[tokio::test]
    async fn test_storage_encryption_decryption() {
        let encryption_service = EncryptionService::new();
        let data = "Sensitive data for storage";
        let context = "user_profile:123";

        // Encrypt for storage
        let encrypted_data = encryption_service
            .encrypt_for_storage(data, context)
            .await
            .unwrap();
        assert!(!encrypted_data.is_empty());
        assert!(encrypted_data.contains("content"));

        // Decrypt from storage
        let decrypted_data = encryption_service
            .decrypt_from_storage(&encrypted_data)
            .await
            .unwrap();
        assert_eq!(decrypted_data, data);
    }

    #[tokio::test]
    async fn test_key_management() {
        let encryption_service = EncryptionService::new();

        // List initial keys
        let initial_keys = encryption_service.list_keys().await;
        assert_eq!(initial_keys.len(), 1);

        // Add a new key
        let new_key = EncryptionKey::generate("test_key".to_string());
        encryption_service.add_key(new_key).await.unwrap();

        // List keys should now have 2
        let keys = encryption_service.list_keys().await;
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"test_key".to_string()));

        // Deactivate key
        encryption_service.deactivate_key("test_key").await.unwrap();

        // Get key metadata
        let metadata = encryption_service
            .get_key_metadata("test_key")
            .await
            .unwrap();
        assert_eq!(metadata.id, "test_key");
        assert!(!metadata.is_active);
    }

    #[tokio::test]
    async fn test_specialized_encryption_methods() {
        let encryption_service = EncryptionService::new();

        // Test password encryption
        let password = "super_secret_password";
        let user_id = "user_123";
        let encrypted_password = encryption_service
            .encrypt_password(password, user_id)
            .await
            .unwrap();
        let decrypted_password = encryption_service
            .decrypt_from_storage(&encrypted_password)
            .await
            .unwrap();
        assert_eq!(decrypted_password, password);

        // Test token encryption
        let token = "jwt_token_abc123";
        let token_type = "access_token";
        let encrypted_token = encryption_service
            .encrypt_token(token, token_type)
            .await
            .unwrap();
        let decrypted_token = encryption_service
            .decrypt_from_storage(&encrypted_token)
            .await
            .unwrap();
        assert_eq!(decrypted_token, token);

        // Test message content encryption
        let content = "Private message content";
        let sender_id = "sender_123";
        let recipient_id = "recipient_456";
        let encrypted_message = encryption_service
            .encrypt_message_content(content, sender_id, recipient_id)
            .await
            .unwrap();
        let decrypted_content = encryption_service
            .decrypt_message(&encrypted_message)
            .await
            .unwrap();
        assert_eq!(decrypted_content, content);
    }

    #[tokio::test]
    async fn test_csrf_token_functionality() {
        use crate::CsrfToken;

        let session_id = "test_session_123".to_string();
        let duration = chrono::Duration::hours(1);

        // Create CSRF token
        let csrf_token = CsrfToken::new(session_id.clone(), duration).unwrap();
        assert!(!csrf_token.token.is_empty());
        assert!(csrf_token.token.len() >= 32);
        assert_eq!(csrf_token.session_id, session_id);
        assert!(!csrf_token.is_expired());

        // Valid session should pass
        assert!(csrf_token.is_valid(&session_id));

        // Invalid session should fail
        assert!(!csrf_token.is_valid("wrong_session"));

        // Test expiration
        let short_duration = chrono::Duration::milliseconds(1);
        let short_token = CsrfToken::new(session_id.clone(), short_duration).unwrap();

        // Wait for expiration
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(short_token.is_expired());
        assert!(!short_token.is_valid(&session_id));
    }

    #[tokio::test]
    async fn test_csrf_token_manager_comprehensive() {
        let manager = CsrfTokenManager::new(chrono::Duration::hours(1));
        let session1 = "session_1";
        let session2 = "session_2";

        // Generate tokens for different sessions
        let token1 = manager.generate_token(session1).await.unwrap();
        let token2 = manager.generate_token(session2).await.unwrap();

        // Both tokens should validate for their respective sessions
        assert!(manager.validate_token(&token1, session1).await);
        assert!(manager.validate_token(&token2, session2).await);

        // Cross-session validation should fail
        assert!(!manager.validate_token(&token1, session2).await);
        assert!(!manager.validate_token(&token2, session1).await);

        // Consume token1
        assert!(manager.consume_token(&token1, session1).await);
        assert!(!manager.validate_token(&token1, session1).await); // Should be gone

        // token2 should still be valid
        assert!(manager.validate_token(&token2, session2).await);

        // Invalidate all tokens for session2
        let invalidated = manager.invalidate_session_tokens(session2).await;
        assert_eq!(invalidated, 1);
        assert!(!manager.validate_token(&token2, session2).await);
    }

    #[tokio::test]
    async fn test_secure_token_generation() {
        let token1 = crate::generate_secure_token(32);
        let token2 = crate::generate_secure_token(32);

        // Tokens should be different
        assert_ne!(token1, token2);

        // Should be base64 encoded
        assert!(base64::decode(&token1).is_ok());
        assert!(base64::decode(&token2).is_ok());

        // Test different lengths
        let short_token = crate::generate_secure_token(16);
        let long_token = crate::generate_secure_token(64);

        assert_ne!(short_token.len(), long_token.len());
    }

    #[tokio::test]
    async fn test_invalid_decryption() {
        let encryption_service = EncryptionService::new();

        // Test decryption with invalid key ID
        let invalid_encrypted = EncryptedMessage {
            content: "invalid_content".to_string(),
            nonce: "invalid_nonce".to_string(),
            algorithm: "AES-256-GCM".to_string(),
            key_id: "non_existent_key".to_string(),
            aad: None,
            encrypted_at: chrono::Utc::now(),
        };

        let result = encryption_service.decrypt_message(&invalid_encrypted).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            SecurityError::EncryptionError(_)
        ));
    }
}
