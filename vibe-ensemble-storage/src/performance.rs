//! Performance optimization utilities and caching layer

use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use lru::LruCache;
use moka::future::Cache;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::num::NonZeroUsize;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::{RwLock as TokioRwLock, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Performance configuration settings
#[derive(Debug, Clone)]
pub struct PerformanceConfig {
    /// Maximum cache size for agents
    pub agent_cache_size: usize,
    /// Maximum cache size for issues
    pub issue_cache_size: usize,
    /// Maximum cache size for messages
    pub message_cache_size: usize,
    /// Maximum cache size for knowledge
    pub knowledge_cache_size: usize,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Connection pool size
    pub max_connections: u32,
    /// Query timeout in seconds
    pub query_timeout_seconds: u64,
    /// Enable compression for message storage
    pub enable_compression: bool,
    /// Compression level (1-9)
    pub compression_level: u32,
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,
    /// Performance monitoring enabled
    pub monitoring_enabled: bool,
    /// Batch size for bulk operations
    pub batch_size: usize,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            agent_cache_size: 1000,
            issue_cache_size: 5000,
            message_cache_size: 10000,
            knowledge_cache_size: 2000,
            cache_ttl_seconds: 3600, // 1 hour
            max_connections: 20,
            query_timeout_seconds: 30,
            enable_compression: true,
            compression_level: 6,
            max_concurrent_operations: 100,
            monitoring_enabled: true,
            batch_size: 100,
        }
    }
}

/// Cache key types for type safety
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheKey {
    Agent(Uuid),
    Issue(Uuid),
    Message(Uuid),
    Knowledge(Uuid),
    AgentsByStatus(String),
    IssuesByAgent(Uuid),
    MessagesByRecipient(Uuid),
    KnowledgeByType(String),
    DatabaseStats,
}

impl std::fmt::Display for CacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheKey::Agent(id) => write!(f, "agent:{}", id),
            CacheKey::Issue(id) => write!(f, "issue:{}", id),
            CacheKey::Message(id) => write!(f, "message:{}", id),
            CacheKey::Knowledge(id) => write!(f, "knowledge:{}", id),
            CacheKey::AgentsByStatus(status) => write!(f, "agents_by_status:{}", status),
            CacheKey::IssuesByAgent(agent_id) => write!(f, "issues_by_agent:{}", agent_id),
            CacheKey::MessagesByRecipient(recipient_id) => {
                write!(f, "messages_by_recipient:{}", recipient_id)
            }
            CacheKey::KnowledgeByType(type_name) => write!(f, "knowledge_by_type:{}", type_name),
            CacheKey::DatabaseStats => write!(f, "database_stats"),
        }
    }
}

/// Cached value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedValue<T> {
    pub data: T,
    pub cached_at: DateTime<Utc>,
    pub access_count: u64,
    pub compressed: bool,
}

impl<T> CachedValue<T>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    pub fn new(data: T, compressed: bool) -> Self {
        Self {
            data,
            cached_at: Utc::now(),
            access_count: 0,
            compressed,
        }
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        Utc::now().signed_duration_since(self.cached_at)
            > chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::zero())
    }
}

/// Performance metrics for monitoring
#[derive(Debug, Default)]
pub struct PerformanceMetrics {
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub cache_evictions: AtomicU64,
    pub query_count: AtomicU64,
    pub total_query_time: AtomicU64,
    pub compression_count: AtomicU64,
    pub decompression_count: AtomicU64,
    pub batch_operations: AtomicU64,
    pub concurrent_operations: AtomicUsize,
}

impl PerformanceMetrics {
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    pub fn average_query_time_ms(&self) -> f64 {
        let total_time = self.total_query_time.load(Ordering::Relaxed);
        let query_count = self.query_count.load(Ordering::Relaxed);
        if query_count == 0 {
            0.0
        } else {
            total_time as f64 / query_count as f64
        }
    }
}

/// Multi-level caching system
pub struct CacheManager {
    /// L1 cache - fast in-memory cache
    l1_cache: Cache<String, Vec<u8>>,
    /// L2 cache - LRU cache for frequently accessed items
    l2_cache: Arc<RwLock<LruCache<String, Vec<u8>>>>,
    /// Performance metrics
    metrics: Arc<PerformanceMetrics>,
    /// Configuration
    config: PerformanceConfig,
}

impl CacheManager {
    pub fn new(config: PerformanceConfig) -> Self {
        let l1_cache = Cache::builder()
            .max_capacity(config.agent_cache_size as u64 + config.issue_cache_size as u64)
            .time_to_live(Duration::from_secs(config.cache_ttl_seconds))
            .build();

        let l2_cache = Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(config.agent_cache_size + config.issue_cache_size).unwrap(),
        )));

        Self {
            l1_cache,
            l2_cache,
            metrics: Arc::new(PerformanceMetrics::default()),
            config,
        }
    }

    /// Get value from cache
    pub async fn get<T>(&self, key: &CacheKey) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let key_str = key.to_string();

        // Try L1 cache first
        if let Some(data) = self.l1_cache.get(&key_str).await {
            self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
            if let Ok(cached_value) = self.deserialize_cached_value::<T>(&data) {
                debug!("L1 cache hit for key: {}", key_str);
                return Some(cached_value.data);
            }
        }

        // Try L2 cache
        let l2_data = { self.l2_cache.read().peek(&key_str).cloned() };
        if let Some(data) = l2_data {
            self.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
            if let Ok(cached_value) = self.deserialize_cached_value::<T>(&data) {
                debug!("L2 cache hit for key: {}", key_str);
                // Promote to L1
                self.l1_cache.insert(key_str, data).await;
                return Some(cached_value.data);
            }
        }

        self.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
        debug!("Cache miss for key: {}", key_str);
        None
    }

    /// Store value in cache
    pub async fn set<T>(&self, key: &CacheKey, value: T)
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let key_str = key.to_string();
        let cached_value = CachedValue::new(value, self.config.enable_compression);

        if let Ok(serialized) = self.serialize_cached_value(&cached_value) {
            // Store in both caches
            self.l1_cache
                .insert(key_str.clone(), serialized.clone())
                .await;
            self.l2_cache.write().put(key_str.clone(), serialized);
            debug!("Cached value for key: {}", key_str);
        }
    }

    /// Remove value from cache
    pub async fn remove(&self, key: &CacheKey) {
        let key_str = key.to_string();
        self.l1_cache.remove(&key_str).await;
        self.l2_cache.write().pop(&key_str);
        debug!("Removed cache entry for key: {}", key_str);
    }

    /// Clear all caches
    pub async fn clear(&self) {
        self.l1_cache.run_pending_tasks().await;
        self.l1_cache.invalidate_all();
        self.l2_cache.write().clear();
        info!("Cleared all caches");
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        self.l1_cache.run_pending_tasks().await;

        CacheStats {
            l1_size: self.l1_cache.entry_count(),
            l1_weighted_size: self.l1_cache.weighted_size(),
            l2_size: self.l2_cache.read().len(),
            hit_rate: self.metrics.cache_hit_rate(),
            total_hits: self.metrics.cache_hits.load(Ordering::Relaxed),
            total_misses: self.metrics.cache_misses.load(Ordering::Relaxed),
            evictions: self.metrics.cache_evictions.load(Ordering::Relaxed),
        }
    }

    /// Serialize cached value with optional compression
    fn serialize_cached_value<T>(&self, value: &CachedValue<T>) -> Result<Vec<u8>>
    where
        T: Serialize,
    {
        let json_data = serde_json::to_vec(value)?;

        if self.config.enable_compression && json_data.len() > 1024 {
            self.metrics
                .compression_count
                .fetch_add(1, Ordering::Relaxed);
            let mut encoder =
                GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
            encoder.write_all(&json_data)?;
            let compressed = encoder.finish()?;
            debug!(
                "Compressed data from {} to {} bytes",
                json_data.len(),
                compressed.len()
            );
            Ok(compressed)
        } else {
            Ok(json_data)
        }
    }

    /// Deserialize cached value with optional decompression
    fn deserialize_cached_value<T>(&self, data: &[u8]) -> Result<CachedValue<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        // Try to deserialize as-is first
        if let Ok(cached_value) = serde_json::from_slice::<CachedValue<T>>(data) {
            return Ok(cached_value);
        }

        // Try decompression
        self.metrics
            .decompression_count
            .fetch_add(1, Ordering::Relaxed);
        let mut decoder = GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        let cached_value = serde_json::from_slice::<CachedValue<T>>(&decompressed)?;
        debug!(
            "Decompressed data from {} to {} bytes",
            data.len(),
            decompressed.len()
        );
        Ok(cached_value)
    }

    pub fn metrics(&self) -> Arc<PerformanceMetrics> {
        self.metrics.clone()
    }
}

/// Cache statistics
#[derive(Debug)]
pub struct CacheStats {
    pub l1_size: u64,
    pub l1_weighted_size: u64,
    pub l2_size: usize,
    pub hit_rate: f64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub evictions: u64,
}

/// Connection pool manager for database operations
pub struct ConnectionPoolManager {
    /// Semaphore to limit concurrent connections
    semaphore: Arc<Semaphore>,
    /// Connection pool statistics
    stats: Arc<ConnectionPoolStats>,
    /// Configuration
    #[allow(dead_code)]
    config: PerformanceConfig,
}

#[derive(Debug, Default)]
pub struct ConnectionPoolStats {
    pub active_connections: AtomicUsize,
    pub total_connections_created: AtomicU64,
    pub connection_wait_time: AtomicU64,
    pub connection_errors: AtomicU64,
}

impl ConnectionPoolManager {
    pub fn new(config: PerformanceConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_connections as usize));
        let stats = Arc::new(ConnectionPoolStats::default());

        Self {
            semaphore,
            stats,
            config,
        }
    }

    /// Acquire a connection permit
    pub async fn acquire_connection(&self) -> Result<ConnectionPermit<'_>> {
        let start_time = Instant::now();
        let permit = self.semaphore.acquire().await.map_err(|e| {
            Error::Internal(anyhow::anyhow!(
                "Failed to acquire connection permit: {}",
                e
            ))
        })?;

        let wait_time = start_time.elapsed().as_millis() as u64;
        self.stats
            .connection_wait_time
            .fetch_add(wait_time, Ordering::Relaxed);
        self.stats
            .active_connections
            .fetch_add(1, Ordering::Relaxed);

        debug!("Acquired connection permit, waited {} ms", wait_time);

        Ok(ConnectionPermit {
            _permit: permit,
            stats: self.stats.clone(),
        })
    }

    pub fn stats(&self) -> ConnectionPoolStats {
        ConnectionPoolStats {
            active_connections: AtomicUsize::new(
                self.stats.active_connections.load(Ordering::Relaxed),
            ),
            total_connections_created: AtomicU64::new(
                self.stats.total_connections_created.load(Ordering::Relaxed),
            ),
            connection_wait_time: AtomicU64::new(
                self.stats.connection_wait_time.load(Ordering::Relaxed),
            ),
            connection_errors: AtomicU64::new(self.stats.connection_errors.load(Ordering::Relaxed)),
        }
    }
}

/// Connection permit that automatically releases when dropped
pub struct ConnectionPermit<'a> {
    _permit: tokio::sync::SemaphorePermit<'a>,
    stats: Arc<ConnectionPoolStats>,
}

impl<'a> Drop for ConnectionPermit<'a> {
    fn drop(&mut self) {
        self.stats
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);
        debug!("Released connection permit");
    }
}

/// Batch operation manager for optimizing bulk operations
pub struct BatchManager<T> {
    items: Vec<T>,
    batch_size: usize,
    metrics: Arc<PerformanceMetrics>,
}

impl<T> BatchManager<T> {
    pub fn new(batch_size: usize, metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            items: Vec::with_capacity(batch_size),
            batch_size,
            metrics,
        }
    }

    /// Add item to batch
    pub fn add(&mut self, item: T) -> bool {
        self.items.push(item);
        self.items.len() >= self.batch_size
    }

    /// Get current batch and reset
    pub fn take_batch(&mut self) -> Vec<T> {
        if !self.items.is_empty() {
            self.metrics
                .batch_operations
                .fetch_add(1, Ordering::Relaxed);
        }
        std::mem::take(&mut self.items)
    }

    /// Check if batch is full
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.batch_size
    }

    /// Check if batch has items
    pub fn has_items(&self) -> bool {
        !self.items.is_empty()
    }

    /// Get current batch size
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Query optimizer for analyzing and optimizing database queries
pub struct QueryOptimizer {
    /// Query execution history
    query_history: Arc<TokioRwLock<DashMap<String, QueryStats>>>,
    /// Performance metrics
    metrics: Arc<PerformanceMetrics>,
}

#[derive(Debug, Clone)]
pub struct QueryStats {
    pub query_hash: String,
    pub execution_count: u64,
    pub total_time_ms: u64,
    pub avg_time_ms: f64,
    pub min_time_ms: u64,
    pub max_time_ms: u64,
    pub last_executed: DateTime<Utc>,
}

impl QueryOptimizer {
    pub fn new(metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            query_history: Arc::new(TokioRwLock::new(DashMap::new())),
            metrics,
        }
    }

    /// Record query execution
    pub async fn record_query_execution(&self, query: &str, execution_time_ms: u64) {
        let query_hash = self.hash_query(query);
        let history = self.query_history.read().await;

        let mut stats = history
            .get(&query_hash)
            .map(|s| s.clone())
            .unwrap_or(QueryStats {
                query_hash: query_hash.clone(),
                execution_count: 0,
                total_time_ms: 0,
                avg_time_ms: 0.0,
                min_time_ms: u64::MAX,
                max_time_ms: 0,
                last_executed: Utc::now(),
            });

        stats.execution_count += 1;
        stats.total_time_ms += execution_time_ms;
        stats.avg_time_ms = stats.total_time_ms as f64 / stats.execution_count as f64;
        stats.min_time_ms = stats.min_time_ms.min(execution_time_ms);
        stats.max_time_ms = stats.max_time_ms.max(execution_time_ms);
        stats.last_executed = Utc::now();

        history.insert(query_hash, stats);
        self.metrics.query_count.fetch_add(1, Ordering::Relaxed);
        self.metrics
            .total_query_time
            .fetch_add(execution_time_ms, Ordering::Relaxed);

        if execution_time_ms > 1000 {
            warn!("Slow query detected: {} ms - {}", execution_time_ms, query);
        }
    }

    /// Get query statistics
    pub async fn get_query_stats(&self) -> Vec<QueryStats> {
        let history = self.query_history.read().await;
        history.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get slow queries (> 500ms average)
    pub async fn get_slow_queries(&self) -> Vec<QueryStats> {
        let history = self.query_history.read().await;
        history
            .iter()
            .filter(|entry| entry.value().avg_time_ms > 500.0)
            .map(|entry| entry.value().clone())
            .collect()
    }

    fn hash_query(&self, query: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        query.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

/// Performance optimization layer combining all optimizations
pub struct PerformanceLayer {
    pub cache_manager: CacheManager,
    pub connection_pool: ConnectionPoolManager,
    pub query_optimizer: QueryOptimizer,
    pub metrics: Arc<PerformanceMetrics>,
    pub config: PerformanceConfig,
}

impl PerformanceLayer {
    pub fn new(config: PerformanceConfig) -> Self {
        let metrics = Arc::new(PerformanceMetrics::default());
        let cache_manager = CacheManager::new(config.clone());
        let connection_pool = ConnectionPoolManager::new(config.clone());
        let query_optimizer = QueryOptimizer::new(metrics.clone());

        Self {
            cache_manager,
            connection_pool,
            query_optimizer,
            metrics,
            config,
        }
    }

    /// Execute operation with performance tracking
    pub async fn execute_with_tracking<F, R>(&self, operation_name: &str, operation: F) -> Result<R>
    where
        F: std::future::Future<Output = Result<R>>,
    {
        let start_time = Instant::now();
        self.metrics
            .concurrent_operations
            .fetch_add(1, Ordering::Relaxed);

        let result = operation.await;

        let execution_time = start_time.elapsed().as_millis() as u64;
        self.metrics
            .concurrent_operations
            .fetch_sub(1, Ordering::Relaxed);

        if self.config.monitoring_enabled {
            debug!(
                "Operation '{}' completed in {} ms",
                operation_name, execution_time
            );
            if execution_time > 1000 {
                warn!(
                    "Slow operation detected: '{}' took {} ms",
                    operation_name, execution_time
                );
            }
        }

        result
    }

    /// Get comprehensive performance report
    pub async fn performance_report(&self) -> PerformanceReport {
        let cache_stats = self.cache_manager.stats().await;
        let connection_stats = self.connection_pool.stats();
        let query_stats = self.query_optimizer.get_query_stats().await;
        let slow_queries = self.query_optimizer.get_slow_queries().await;

        PerformanceReport {
            cache_stats,
            connection_stats,
            total_queries: query_stats.len(),
            slow_queries_count: slow_queries.len(),
            cache_hit_rate: self.metrics.cache_hit_rate(),
            avg_query_time_ms: self.metrics.average_query_time_ms(),
            concurrent_operations: self.metrics.concurrent_operations.load(Ordering::Relaxed),
            compression_ratio: self.calculate_compression_ratio(),
        }
    }

    fn calculate_compression_ratio(&self) -> f64 {
        let compression_count = self.metrics.compression_count.load(Ordering::Relaxed);
        let _decompression_count = self.metrics.decompression_count.load(Ordering::Relaxed);

        if compression_count == 0 {
            1.0
        } else {
            // This is a simplified calculation - in practice you'd track actual sizes
            0.6 // Assume ~40% compression on average
        }
    }
}

/// Comprehensive performance report
#[derive(Debug)]
pub struct PerformanceReport {
    pub cache_stats: CacheStats,
    pub connection_stats: ConnectionPoolStats,
    pub total_queries: usize,
    pub slow_queries_count: usize,
    pub cache_hit_rate: f64,
    pub avg_query_time_ms: f64,
    pub concurrent_operations: usize,
    pub compression_ratio: f64,
}
