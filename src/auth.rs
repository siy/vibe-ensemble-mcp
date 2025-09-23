use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Simple token cache entry
#[derive(Debug, Clone)]
struct TokenEntry {
    created_at: Instant,
    last_used: Instant,
    usage_count: u64,
}

/// Simple auth token manager with caching
#[derive(Debug)]
pub struct AuthTokenManager {
    tokens: Arc<RwLock<HashMap<String, TokenEntry>>>,
}

impl AuthTokenManager {
    /// Create a new auth token manager
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a token to the cache
    pub fn add_token(&self, token: String) {
        if let Ok(mut tokens) = self.tokens.write() {
            let now = Instant::now();

            tokens.insert(
                token.clone(),
                TokenEntry {
                    created_at: now,
                    last_used: now,
                    usage_count: 0,
                },
            );

            info!("Added token to cache: {}...", &token[..8.min(token.len())]);
            debug!("Total cached tokens: {}", tokens.len());
        } else {
            warn!("Failed to acquire write lock for token cache when adding token");
        }
    }

    /// Validate a token and update its usage
    pub fn validate_token(&self, token: &str) -> bool {
        if let Ok(mut tokens) = self.tokens.write() {
            if let Some(entry) = tokens.get_mut(token) {
                let now = Instant::now();

                // Update usage statistics
                entry.last_used = now;
                entry.usage_count += 1;

                debug!(
                    "Token validated: {}... (usage: {}, age: {:.1}s)",
                    &token[..8.min(token.len())],
                    entry.usage_count,
                    now.duration_since(entry.created_at).as_secs_f64()
                );

                true
            } else {
                debug!(
                    "Token not found in cache: {}...",
                    &token[..8.min(token.len())]
                );
                false
            }
        } else {
            warn!("Failed to acquire write lock for token cache during validation");
            false
        }
    }

    /// Remove a token from the cache
    pub fn remove_token(&self, token: &str) {
        if let Ok(mut tokens) = self.tokens.write() {
            if tokens.remove(token).is_some() {
                info!(
                    "Removed token from cache: {}...",
                    &token[..8.min(token.len())]
                );
            }
        } else {
            warn!("Failed to acquire write lock for token cache when removing token");
        }
    }

    /// Get statistics about cached tokens
    pub fn get_stats(&self) -> TokenCacheStats {
        if let Ok(tokens) = self.tokens.read() {
            let now = Instant::now();

            let mut stats = TokenCacheStats {
                total_tokens: tokens.len(),
                total_usage: 0,
                average_age_secs: 0.0,
            };

            if tokens.is_empty() {
                return stats;
            }

            let mut total_age_secs = 0.0;
            for entry in tokens.values() {
                stats.total_usage += entry.usage_count;
                total_age_secs += now.duration_since(entry.created_at).as_secs_f64();
            }

            stats.average_age_secs = total_age_secs / tokens.len() as f64;
            stats
        } else {
            warn!("Failed to acquire read lock for token cache when getting stats");
            TokenCacheStats {
                total_tokens: 0,
                total_usage: 0,
                average_age_secs: 0.0,
            }
        }
    }
}

/// Statistics about the token cache
#[derive(Debug, Clone)]
pub struct TokenCacheStats {
    pub total_tokens: usize,
    pub total_usage: u64,
    pub average_age_secs: f64,
}

impl Default for AuthTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_lifecycle() {
        let manager = AuthTokenManager::new();
        let token = "test-token-123".to_string();

        // Add token
        manager.add_token(token.clone());

        // Should validate immediately
        assert!(manager.validate_token(&token));

        // Should continue to validate (no expiration)
        assert!(manager.validate_token(&token));
    }

    #[test]
    fn test_token_management() {
        let manager = AuthTokenManager::new();

        manager.add_token("token1".to_string());
        manager.add_token("token2".to_string());

        // Both tokens should be valid
        assert!(manager.validate_token("token1"));
        assert!(manager.validate_token("token2"));

        // Remove one token
        manager.remove_token("token1");
        assert!(!manager.validate_token("token1"));
        assert!(manager.validate_token("token2"));
    }
}
