use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
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
    token_lifetime: Duration,
}

impl AuthTokenManager {
    /// Create a new auth token manager
    pub fn new(token_lifetime_secs: u64) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            token_lifetime: Duration::from_secs(token_lifetime_secs),
        }
    }

    /// Add a token to the cache
    pub fn add_token(&self, token: String) {
        if let Ok(mut tokens) = self.tokens.write() {
            let now = Instant::now();

            tokens.insert(token.clone(), TokenEntry {
                created_at: now,
                last_used: now,
                usage_count: 0,
            });

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

                // Check if token has expired
                if now.duration_since(entry.created_at) > self.token_lifetime {
                    tokens.remove(token);
                    warn!("Token expired and removed: {}...", &token[..8.min(token.len())]);
                    return false;
                }

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
                debug!("Token not found in cache: {}...", &token[..8.min(token.len())]);
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
                info!("Removed token from cache: {}...", &token[..8.min(token.len())]);
            }
        } else {
            warn!("Failed to acquire write lock for token cache when removing token");
        }
    }

    /// Cleanup expired tokens
    pub fn cleanup_expired_tokens(&self) -> usize {
        if let Ok(mut tokens) = self.tokens.write() {
        let now = Instant::now();
        let initial_count = tokens.len();

        tokens.retain(|token, entry| {
            let is_expired = now.duration_since(entry.created_at) > self.token_lifetime;
            if is_expired {
                debug!("Cleaning up expired token: {}...", &token[..8.min(token.len())]);
            }
            !is_expired
        });

            let removed_count = initial_count - tokens.len();
            if removed_count > 0 {
                info!("Cleaned up {} expired tokens", removed_count);
            }
            removed_count
        } else {
            warn!("Failed to acquire write lock for token cache during cleanup");
            0
        }
    }

    /// Get statistics about cached tokens
    pub fn get_stats(&self) -> TokenCacheStats {
        if let Ok(tokens) = self.tokens.read() {
        let now = Instant::now();

        let mut stats = TokenCacheStats {
            total_tokens: tokens.len(),
            expired_tokens: 0,
            total_usage: 0,
            average_age_secs: 0.0,
        };

        if tokens.is_empty() {
            return stats;
        }

        let mut total_age = Duration::new(0, 0);
        for entry in tokens.values() {
            stats.total_usage += entry.usage_count;

            let age = now.duration_since(entry.created_at);
            total_age += age;

            if age > self.token_lifetime {
                stats.expired_tokens += 1;
            }
        }

            stats.average_age_secs = total_age.as_secs_f64() / tokens.len() as f64;
            stats
        } else {
            warn!("Failed to acquire read lock for token cache when getting stats");
            TokenCacheStats {
                total_tokens: 0,
                expired_tokens: 0,
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
    pub expired_tokens: usize,
    pub total_usage: u64,
    pub average_age_secs: f64,
}

impl Default for AuthTokenManager {
    fn default() -> Self {
        Self::new(3600) // 1 hour default lifetime
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_token_lifecycle() {
        let manager = AuthTokenManager::new(2); // 2 second lifetime for testing
        let token = "test-token-123".to_string();

        // Add token
        manager.add_token(token.clone());

        // Should validate immediately
        assert!(manager.validate_token(&token));

        // Wait for expiration
        thread::sleep(Duration::from_secs(3));

        // Should no longer validate
        assert!(!manager.validate_token(&token));
    }

    #[test]
    fn test_cleanup() {
        let manager = AuthTokenManager::new(1); // 1 second lifetime

        manager.add_token("token1".to_string());
        manager.add_token("token2".to_string());

        thread::sleep(Duration::from_secs(2));

        let removed = manager.cleanup_expired_tokens();
        assert_eq!(removed, 2);
    }
}