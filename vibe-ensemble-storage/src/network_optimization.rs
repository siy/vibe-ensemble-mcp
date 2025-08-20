//! Network communication optimizations

use anyhow::Result;
use dashmap::DashMap;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use futures_util::SinkExt;
use parking_lot::RwLock;
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio_tungstenite::{tungstenite::Message as WsMessage, WebSocketStream};
use tracing::{debug, error, info};
use uuid::Uuid;

use crate::performance::PerformanceMetrics;

/// Network optimization configuration
#[derive(Debug, Clone)]
pub struct NetworkOptimizationConfig {
    /// Enable connection pooling for HTTP clients
    pub enable_connection_pooling: bool,
    /// Maximum connections per host
    pub max_connections_per_host: usize,
    /// Connection timeout (seconds)
    pub connection_timeout_seconds: u64,
    /// Read/write timeout (seconds)
    pub io_timeout_seconds: u64,
    /// Enable compression for large payloads
    pub enable_compression: bool,
    /// Minimum payload size for compression (bytes)
    pub compression_threshold: usize,
    /// Compression level (1-9)
    pub compression_level: u32,
    /// Enable keep-alive connections
    pub enable_keep_alive: bool,
    /// Keep-alive timeout (seconds)
    pub keep_alive_timeout_seconds: u64,
    /// Maximum concurrent connections
    pub max_concurrent_connections: usize,
    /// Enable request pipelining
    pub enable_pipelining: bool,
    /// Enable WebSocket compression
    pub enable_websocket_compression: bool,
    /// WebSocket ping interval (seconds)
    pub websocket_ping_interval_seconds: u64,
    /// Buffer size for network operations
    pub network_buffer_size: usize,
}

impl Default for NetworkOptimizationConfig {
    fn default() -> Self {
        Self {
            enable_connection_pooling: true,
            max_connections_per_host: 50,
            connection_timeout_seconds: 10,
            io_timeout_seconds: 30,
            enable_compression: true,
            compression_threshold: 1024,
            compression_level: 6,
            enable_keep_alive: true,
            keep_alive_timeout_seconds: 60,
            max_concurrent_connections: 1000,
            enable_pipelining: false,
            enable_websocket_compression: true,
            websocket_ping_interval_seconds: 30,
            network_buffer_size: 8192,
        }
    }
}

/// Connection pool for managing reusable connections
pub struct ConnectionPool {
    /// Active connections by host
    connections: Arc<DashMap<String, Vec<PooledConnection>>>,
    /// Connection statistics
    stats: Arc<ConnectionPoolStats>,
    /// Configuration
    config: NetworkOptimizationConfig,
    /// Semaphore for connection limits
    connection_semaphore: Arc<Semaphore>,
}

/// Pooled connection wrapper
#[derive(Debug, Clone)]
pub struct PooledConnection {
    pub id: Uuid,
    pub host: String,
    pub created_at: Instant,
    pub last_used: Instant,
    pub use_count: usize,
    pub is_healthy: bool,
}

impl PooledConnection {
    pub fn new(host: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            host,
            created_at: Instant::now(),
            last_used: Instant::now(),
            use_count: 0,
            is_healthy: true,
        }
    }

    pub fn mark_used(&mut self) {
        self.last_used = Instant::now();
        self.use_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    pub fn idle_time(&self) -> Duration {
        self.last_used.elapsed()
    }

    pub fn should_retire(&self, max_age: Duration, max_idle: Duration) -> bool {
        self.age() > max_age || self.idle_time() > max_idle || !self.is_healthy
    }
}

#[derive(Debug, Default)]
pub struct ConnectionPoolStats {
    pub active_connections: AtomicUsize,
    pub total_connections_created: AtomicU64,
    pub connections_reused: AtomicU64,
    pub connections_retired: AtomicU64,
    pub connection_errors: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
}

impl ConnectionPool {
    pub fn new(config: NetworkOptimizationConfig) -> Self {
        let connection_semaphore = Arc::new(Semaphore::new(config.max_concurrent_connections));

        Self {
            connections: Arc::new(DashMap::new()),
            stats: Arc::new(ConnectionPoolStats::default()),
            config,
            connection_semaphore,
        }
    }

    /// Get or create a connection for the specified host
    pub async fn get_connection(&self, host: &str) -> Result<ConnectionPermit<'_>> {
        let permit = self
            .connection_semaphore
            .acquire()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to acquire connection permit: {}", e))?;

        // Try to reuse existing connection
        if let Some(mut connections) = self.connections.get_mut(host) {
            if let Some(mut conn) = connections.pop() {
                if !conn.should_retire(
                    Duration::from_secs(self.config.keep_alive_timeout_seconds * 2),
                    Duration::from_secs(self.config.keep_alive_timeout_seconds),
                ) {
                    conn.mark_used();
                    self.stats
                        .connections_reused
                        .fetch_add(1, Ordering::Relaxed);
                    debug!("Reusing connection {} for host {}", conn.id, host);

                    return Ok(ConnectionPermit {
                        connection: conn,
                        pool: self.connections.clone(),
                        stats: self.stats.clone(),
                        _permit: permit,
                    });
                } else {
                    self.stats
                        .connections_retired
                        .fetch_add(1, Ordering::Relaxed);
                    debug!("Retiring stale connection {} for host {}", conn.id, host);
                }
            }
        }

        // Create new connection
        let connection = PooledConnection::new(host.to_string());
        self.stats
            .total_connections_created
            .fetch_add(1, Ordering::Relaxed);
        self.stats
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
        debug!("Created new connection {} for host {}", connection.id, host);

        Ok(ConnectionPermit {
            connection,
            pool: self.connections.clone(),
            stats: self.stats.clone(),
            _permit: permit,
        })
    }

    /// Clean up expired connections
    pub fn cleanup_expired_connections(&self) {
        let max_age = Duration::from_secs(self.config.keep_alive_timeout_seconds * 2);
        let max_idle = Duration::from_secs(self.config.keep_alive_timeout_seconds);

        for mut entry in self.connections.iter_mut() {
            let host = entry.key().clone();
            let connections = entry.value_mut();

            let original_len = connections.len();
            connections.retain(|conn| !conn.should_retire(max_age, max_idle));
            let removed = original_len - connections.len();

            if removed > 0 {
                self.stats
                    .connections_retired
                    .fetch_add(removed as u64, Ordering::Relaxed);
                self.stats
                    .active_connections
                    .fetch_sub(removed, Ordering::Relaxed);
                debug!(
                    "Cleaned up {} expired connections for host {}",
                    removed, host
                );
            }
        }
    }

    pub fn stats(&self) -> &ConnectionPoolStats {
        &self.stats
    }
}

/// Connection permit that manages connection lifecycle
pub struct ConnectionPermit<'a> {
    pub connection: PooledConnection,
    pool: Arc<DashMap<String, Vec<PooledConnection>>>,
    stats: Arc<ConnectionPoolStats>,
    _permit: tokio::sync::SemaphorePermit<'a>,
}

impl<'a> ConnectionPermit<'a> {
    /// Return connection to pool for reuse
    pub fn return_to_pool(mut self) {
        if self.connection.is_healthy {
            let host = self.connection.host.clone();
            self.connection.last_used = Instant::now();

            self.pool
                .entry(host)
                .or_insert_with(Vec::new)
                .push(self.connection.clone());
            debug!("Returned connection to pool");
        } else {
            self.stats
                .active_connections
                .fetch_sub(1, Ordering::Relaxed);
            debug!("Connection unhealthy, not returning to pool");
        }
    }

    /// Mark connection as unhealthy
    pub fn mark_unhealthy(&mut self) {
        self.connection.is_healthy = false;
    }

    /// Record bytes sent/received
    pub fn record_traffic(&self, bytes_sent: u64, bytes_received: u64) {
        self.stats
            .bytes_sent
            .fetch_add(bytes_sent, Ordering::Relaxed);
        self.stats
            .bytes_received
            .fetch_add(bytes_received, Ordering::Relaxed);
    }
}

impl<'a> Drop for ConnectionPermit<'a> {
    fn drop(&mut self) {
        if self.connection.is_healthy {
            // Connection will be returned to pool
        } else {
            self.stats
                .active_connections
                .fetch_sub(1, Ordering::Relaxed);
        }
    }
}

/// HTTP request/response compression
pub struct HttpCompression {
    config: NetworkOptimizationConfig,
}

impl HttpCompression {
    pub fn new(config: NetworkOptimizationConfig) -> Self {
        Self { config }
    }

    /// Compress request body if it exceeds threshold
    pub fn compress_request(&self, body: &[u8]) -> Result<(Vec<u8>, bool)> {
        if !self.config.enable_compression || body.len() < self.config.compression_threshold {
            return Ok((body.to_vec(), false));
        }

        let mut encoder =
            GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(body)?;
        let compressed = encoder.finish()?;

        if compressed.len() < body.len() {
            debug!(
                "Compressed request from {} to {} bytes",
                body.len(),
                compressed.len()
            );
            Ok((compressed, true))
        } else {
            Ok((body.to_vec(), false))
        }
    }

    /// Decompress response body
    pub fn decompress_response(&self, body: &[u8]) -> Result<Vec<u8>> {
        let mut decoder = GzDecoder::new(body);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;

        debug!(
            "Decompressed response from {} to {} bytes",
            body.len(),
            decompressed.len()
        );
        Ok(decompressed)
    }
}

/// WebSocket optimization manager
pub struct WebSocketOptimizer {
    config: NetworkOptimizationConfig,
    connection_stats: Arc<DashMap<Uuid, WebSocketStats>>,
    metrics: Arc<PerformanceMetrics>,
}

#[derive(Debug)]
pub struct WebSocketStats {
    pub connection_id: Uuid,
    pub connected_at: Instant,
    pub messages_sent: AtomicU64,
    pub messages_received: AtomicU64,
    pub bytes_sent: AtomicU64,
    pub bytes_received: AtomicU64,
    pub last_activity: RwLock<Instant>,
}

impl WebSocketStats {
    pub fn new(connection_id: Uuid) -> Self {
        Self {
            connection_id,
            connected_at: Instant::now(),
            messages_sent: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            last_activity: RwLock::new(Instant::now()),
        }
    }

    pub fn record_sent_message(&self, size: usize) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(size as u64, Ordering::Relaxed);
        *self.last_activity.write() = Instant::now();
    }

    pub fn record_received_message(&self, size: usize) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(size as u64, Ordering::Relaxed);
        *self.last_activity.write() = Instant::now();
    }

    pub fn connection_age(&self) -> Duration {
        self.connected_at.elapsed()
    }

    pub fn idle_time(&self) -> Duration {
        self.last_activity.read().elapsed()
    }
}

impl WebSocketOptimizer {
    pub fn new(config: NetworkOptimizationConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        Self {
            config,
            connection_stats: Arc::new(DashMap::new()),
            metrics,
        }
    }

    /// Register a new WebSocket connection
    pub fn register_connection(&self, connection_id: Uuid) {
        let stats = WebSocketStats::new(connection_id);
        self.connection_stats.insert(connection_id, stats);
        debug!("Registered WebSocket connection {}", connection_id);
    }

    /// Unregister a WebSocket connection
    pub fn unregister_connection(&self, connection_id: &Uuid) {
        if let Some((_, stats)) = self.connection_stats.remove(connection_id) {
            debug!(
                "Unregistered WebSocket connection {} (messages: {}/{}, bytes: {}/{})",
                connection_id,
                stats.messages_sent.load(Ordering::Relaxed),
                stats.messages_received.load(Ordering::Relaxed),
                stats.bytes_sent.load(Ordering::Relaxed),
                stats.bytes_received.load(Ordering::Relaxed)
            );
        }
    }

    /// Optimize WebSocket message
    pub fn optimize_message(&self, message: WsMessage) -> Result<WsMessage> {
        match message {
            WsMessage::Text(text)
                if self.config.enable_websocket_compression
                    && text.len() > self.config.compression_threshold =>
            {
                let compressed = self.compress_text(&text)?;
                Ok(WsMessage::Binary(compressed))
            }
            WsMessage::Binary(data)
                if self.config.enable_websocket_compression
                    && data.len() > self.config.compression_threshold =>
            {
                let compressed = self.compress_binary(&data)?;
                Ok(WsMessage::Binary(compressed))
            }
            _ => Ok(message),
        }
    }

    /// Start ping/pong heartbeat for a WebSocket connection
    pub async fn start_heartbeat(
        &self,
        connection_id: Uuid,
        mut sender: futures_util::stream::SplitSink<WebSocketStream<TcpStream>, WsMessage>,
    ) -> Result<JoinHandle<()>> {
        let interval_duration = Duration::from_secs(self.config.websocket_ping_interval_seconds);
        let stats = self
            .connection_stats
            .get(&connection_id)
            .map(|s| WebSocketStats {
                connection_id: s.connection_id,
                connected_at: s.connected_at,
                messages_sent: AtomicU64::new(s.messages_sent.load(Ordering::Relaxed)),
                messages_received: AtomicU64::new(s.messages_received.load(Ordering::Relaxed)),
                bytes_sent: AtomicU64::new(s.bytes_sent.load(Ordering::Relaxed)),
                bytes_received: AtomicU64::new(s.bytes_received.load(Ordering::Relaxed)),
                last_activity: RwLock::new(*s.last_activity.read()),
            });

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval_duration);

            loop {
                interval.tick().await;

                match sender.send(WsMessage::Ping(vec![])).await {
                    Ok(()) => {
                        if let Some(ref stats) = stats {
                            stats.record_sent_message(0);
                        }
                        debug!("Sent ping to connection {}", connection_id);
                    }
                    Err(e) => {
                        error!("Failed to send ping to connection {}: {}", connection_id, e);
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Record message statistics
    pub fn record_sent_message(&self, connection_id: &Uuid, message: &WsMessage) {
        if let Some(stats) = self.connection_stats.get(connection_id) {
            let size = self.message_size(message);
            stats.record_sent_message(size);
        }
    }

    pub fn record_received_message(&self, connection_id: &Uuid, message: &WsMessage) {
        if let Some(stats) = self.connection_stats.get(connection_id) {
            let size = self.message_size(message);
            stats.record_received_message(size);
        }
    }

    /// Get connection statistics
    pub fn get_connection_stats(&self, connection_id: &Uuid) -> Option<WebSocketStats> {
        self.connection_stats
            .get(connection_id)
            .map(|s| WebSocketStats {
                connection_id: s.connection_id,
                connected_at: s.connected_at,
                messages_sent: AtomicU64::new(s.messages_sent.load(Ordering::Relaxed)),
                messages_received: AtomicU64::new(s.messages_received.load(Ordering::Relaxed)),
                bytes_sent: AtomicU64::new(s.bytes_sent.load(Ordering::Relaxed)),
                bytes_received: AtomicU64::new(s.bytes_received.load(Ordering::Relaxed)),
                last_activity: RwLock::new(*s.last_activity.read()),
            })
    }

    /// Get all connection statistics
    pub fn get_all_connection_stats(&self) -> Vec<WebSocketStats> {
        self.connection_stats
            .iter()
            .map(|entry| {
                let s = entry.value();
                WebSocketStats {
                    connection_id: s.connection_id,
                    connected_at: s.connected_at,
                    messages_sent: AtomicU64::new(s.messages_sent.load(Ordering::Relaxed)),
                    messages_received: AtomicU64::new(s.messages_received.load(Ordering::Relaxed)),
                    bytes_sent: AtomicU64::new(s.bytes_sent.load(Ordering::Relaxed)),
                    bytes_received: AtomicU64::new(s.bytes_received.load(Ordering::Relaxed)),
                    last_activity: RwLock::new(*s.last_activity.read()),
                }
            })
            .collect()
    }

    /// Clean up idle connections
    pub fn cleanup_idle_connections(&self, max_idle_duration: Duration) -> Vec<Uuid> {
        let mut idle_connections = Vec::new();

        for entry in self.connection_stats.iter() {
            let connection_id = *entry.key();
            let stats = entry.value();

            if stats.idle_time() > max_idle_duration {
                idle_connections.push(connection_id);
            }
        }

        // Remove idle connections from tracking
        for connection_id in &idle_connections {
            self.connection_stats.remove(connection_id);
        }

        if !idle_connections.is_empty() {
            info!(
                "Cleaned up {} idle WebSocket connections",
                idle_connections.len()
            );
        }

        idle_connections
    }

    fn compress_text(&self, text: &str) -> Result<Vec<u8>> {
        let mut encoder =
            GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(text.as_bytes())?;
        Ok(encoder.finish()?)
    }

    fn compress_binary(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut encoder =
            GzEncoder::new(Vec::new(), Compression::new(self.config.compression_level));
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }

    fn message_size(&self, message: &WsMessage) -> usize {
        match message {
            WsMessage::Text(text) => text.len(),
            WsMessage::Binary(data) => data.len(),
            WsMessage::Ping(data) => data.len(),
            WsMessage::Pong(data) => data.len(),
            WsMessage::Close(_) => 0,
            WsMessage::Frame(_) => 0,
        }
    }
}

/// Network optimization manager
pub struct NetworkOptimizationManager {
    connection_pool: Arc<ConnectionPool>,
    http_compression: HttpCompression,
    websocket_optimizer: Arc<WebSocketOptimizer>,
    config: NetworkOptimizationConfig,
    metrics: Arc<PerformanceMetrics>,
    cleanup_handle: Option<tokio::task::JoinHandle<()>>,
}

impl NetworkOptimizationManager {
    pub fn new(config: NetworkOptimizationConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        let connection_pool = Arc::new(ConnectionPool::new(config.clone()));
        let http_compression = HttpCompression::new(config.clone());
        let websocket_optimizer =
            Arc::new(WebSocketOptimizer::new(config.clone(), metrics.clone()));

        Self {
            connection_pool,
            http_compression,
            websocket_optimizer,
            config,
            metrics,
            cleanup_handle: None,
        }
    }

    /// Start background cleanup processes
    pub fn start_cleanup_processes(&mut self) {
        let pool = self.connection_pool.clone();
        let ws_optimizer = self.websocket_optimizer.clone();
        let cleanup_interval = Duration::from_secs(60); // Cleanup every minute
        let max_idle = Duration::from_secs(self.config.keep_alive_timeout_seconds * 2);

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);

            loop {
                interval.tick().await;

                // Cleanup expired HTTP connections
                pool.cleanup_expired_connections();

                // Cleanup idle WebSocket connections
                let _idle_connections = ws_optimizer.cleanup_idle_connections(max_idle);
            }
        });

        self.cleanup_handle = Some(handle);
        info!("Started network optimization cleanup processes");
    }

    /// Get HTTP connection from pool
    pub async fn get_http_connection(&self, host: &str) -> Result<ConnectionPermit<'_>> {
        self.connection_pool.get_connection(host).await
    }

    /// Compress HTTP request
    pub fn compress_http_request(&self, body: &[u8]) -> Result<(Vec<u8>, bool)> {
        self.http_compression.compress_request(body)
    }

    /// Decompress HTTP response
    pub fn decompress_http_response(&self, body: &[u8]) -> Result<Vec<u8>> {
        self.http_compression.decompress_response(body)
    }

    /// Get WebSocket optimizer
    pub fn websocket_optimizer(&self) -> Arc<WebSocketOptimizer> {
        self.websocket_optimizer.clone()
    }

    /// Get network statistics
    pub fn stats(&self) -> NetworkOptimizationStats {
        let connection_stats = self.connection_pool.stats();
        let websocket_connections = self.websocket_optimizer.get_all_connection_stats();

        let total_ws_messages_sent: u64 = websocket_connections
            .iter()
            .map(|s| s.messages_sent.load(Ordering::Relaxed))
            .sum();
        let total_ws_messages_received: u64 = websocket_connections
            .iter()
            .map(|s| s.messages_received.load(Ordering::Relaxed))
            .sum();
        let total_ws_bytes_sent: u64 = websocket_connections
            .iter()
            .map(|s| s.bytes_sent.load(Ordering::Relaxed))
            .sum();
        let total_ws_bytes_received: u64 = websocket_connections
            .iter()
            .map(|s| s.bytes_received.load(Ordering::Relaxed))
            .sum();

        NetworkOptimizationStats {
            http_connections_active: connection_stats.active_connections.load(Ordering::Relaxed),
            http_connections_created: connection_stats
                .total_connections_created
                .load(Ordering::Relaxed),
            http_connections_reused: connection_stats.connections_reused.load(Ordering::Relaxed),
            http_bytes_sent: connection_stats.bytes_sent.load(Ordering::Relaxed),
            http_bytes_received: connection_stats.bytes_received.load(Ordering::Relaxed),
            websocket_connections_active: websocket_connections.len(),
            websocket_messages_sent: total_ws_messages_sent,
            websocket_messages_received: total_ws_messages_received,
            websocket_bytes_sent: total_ws_bytes_sent,
            websocket_bytes_received: total_ws_bytes_received,
        }
    }
}

/// Network optimization statistics
#[derive(Debug, Clone)]
pub struct NetworkOptimizationStats {
    pub http_connections_active: usize,
    pub http_connections_created: u64,
    pub http_connections_reused: u64,
    pub http_bytes_sent: u64,
    pub http_bytes_received: u64,
    pub websocket_connections_active: usize,
    pub websocket_messages_sent: u64,
    pub websocket_messages_received: u64,
    pub websocket_bytes_sent: u64,
    pub websocket_bytes_received: u64,
}
