//! Message batching and compression optimizations

use anyhow::Result;
use base64::Engine;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use vibe_ensemble_core::message::{Message, MessageType};

use crate::performance::PerformanceMetrics;

/// Configuration for message batching and compression
#[derive(Debug, Clone)]
pub struct MessageOptimizationConfig {
    /// Maximum batch size for message operations
    pub max_batch_size: usize,
    /// Maximum time to wait before flushing incomplete batch (ms)
    pub batch_timeout_ms: u64,
    /// Enable message compression
    pub enable_compression: bool,
    /// Compression level (1-9)
    pub compression_level: u32,
    /// Minimum message size for compression (bytes)
    pub compression_threshold: usize,
    /// Maximum concurrent batch operations
    pub max_concurrent_batches: usize,
    /// Buffer size for message queue
    pub message_buffer_size: usize,
    /// Priority queue for high-priority messages
    pub enable_priority_queue: bool,
}

impl Default for MessageOptimizationConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 100,
            batch_timeout_ms: 1000, // 1 second
            enable_compression: true,
            compression_level: 6,
            compression_threshold: 1024, // 1KB
            max_concurrent_batches: 10,
            message_buffer_size: 1000,
            enable_priority_queue: true,
        }
    }
}

/// Message batch with compression support
#[derive(Debug, Clone)]
pub struct MessageBatch {
    pub messages: Vec<Message>,
    pub created_at: Instant,
    pub total_size: usize,
    pub compressed: bool,
    pub batch_id: Uuid,
}

impl Default for MessageBatch {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBatch {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            created_at: Instant::now(),
            total_size: 0,
            compressed: false,
            batch_id: Uuid::new_v4(),
        }
    }

    pub fn add_message(&mut self, message: Message) {
        self.total_size += message.content.len();
        self.messages.push(message);
    }

    pub fn is_full(&self, max_size: usize) -> bool {
        self.messages.len() >= max_size
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.created_at.elapsed() >= timeout
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

/// Priority levels for message batching
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MessagePriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl From<&MessageType> for MessagePriority {
    fn from(message_type: &MessageType) -> Self {
        match message_type {
            MessageType::Direct => MessagePriority::Normal,
            MessageType::Broadcast => MessagePriority::Low,
            MessageType::StatusUpdate => MessagePriority::High,
            MessageType::IssueNotification => MessagePriority::High,
            MessageType::KnowledgeShare => MessagePriority::Normal,
        }
    }
}

/// Prioritized message wrapper
#[derive(Debug, Clone)]
pub struct PrioritizedMessage {
    pub message: Message,
    pub priority: MessagePriority,
    pub timestamp: Instant,
}

impl PrioritizedMessage {
    pub fn new(message: Message) -> Self {
        let priority = MessagePriority::from(&message.message_type);
        Self {
            message,
            priority,
            timestamp: Instant::now(),
        }
    }

    pub fn with_priority(message: Message, priority: MessagePriority) -> Self {
        Self {
            message,
            priority,
            timestamp: Instant::now(),
        }
    }
}

impl PartialEq for PrioritizedMessage {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PrioritizedMessage {}

impl PartialOrd for PrioritizedMessage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedMessage {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first, then newer messages first
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.timestamp.cmp(&self.timestamp))
    }
}

/// Compression utilities for messages
pub struct MessageCompressor {
    compression_level: u32,
    threshold: usize,
}

impl MessageCompressor {
    pub fn new(compression_level: u32, threshold: usize) -> Self {
        Self {
            compression_level,
            threshold,
        }
    }

    /// Compress message content if it exceeds threshold
    pub fn compress_message(&self, message: &mut Message) -> Result<bool> {
        if message.content.len() < self.threshold {
            return Ok(false);
        }

        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.compression_level));
        encoder.write_all(message.content.as_bytes())?;
        let compressed = encoder.finish()?;

        if compressed.len() < message.content.len() {
            message.content = base64::engine::general_purpose::STANDARD.encode(&compressed);
            // Add compression metadata
            message.metadata.is_compressed = true;
            message.metadata.compression_type = Some("gzip".to_string());
            debug!(
                "Compressed message from {} to {} bytes",
                message.content.len(),
                compressed.len()
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Decompress message content
    pub fn decompress_message(&self, message: &mut Message) -> Result<bool> {
        if !message.metadata.is_compressed {
            return Ok(false);
        }

        let compressed = base64::engine::general_purpose::STANDARD.decode(&message.content)?;
        let mut decoder = GzDecoder::new(&compressed[..]);
        let mut decompressed = String::new();
        decoder.read_to_string(&mut decompressed)?;

        message.content = decompressed;
        message.metadata.is_compressed = false;
        message.metadata.compression_type = None;
        debug!(
            "Decompressed message from {} to {} bytes",
            compressed.len(),
            message.content.len()
        );
        Ok(true)
    }

    /// Compress batch of messages
    pub fn compress_batch(&self, batch: &mut MessageBatch) -> Result<()> {
        let mut compressed_count = 0;
        for message in &mut batch.messages {
            if self.compress_message(message)? {
                compressed_count += 1;
            }
        }

        if compressed_count > 0 {
            batch.compressed = true;
            debug!(
                "Compressed {} messages in batch {}",
                compressed_count, batch.batch_id
            );
        }

        Ok(())
    }
}

/// Message queue with priority support and batching
pub struct MessageQueue {
    /// High-priority message queue
    high_priority: Arc<Mutex<VecDeque<PrioritizedMessage>>>,
    /// Normal priority message queue  
    normal_priority: Arc<Mutex<VecDeque<PrioritizedMessage>>>,
    /// Low priority message queue
    low_priority: Arc<Mutex<VecDeque<PrioritizedMessage>>>,
    /// Current batches being processed
    active_batches: Arc<RwLock<HashMap<Uuid, MessageBatch>>>,
    /// Configuration
    config: MessageOptimizationConfig,
    /// Performance metrics
    metrics: Arc<PerformanceMetrics>,
    /// Message compressor
    compressor: MessageCompressor,
    /// Semaphore for batch concurrency control
    batch_semaphore: Arc<Semaphore>,
}

impl MessageQueue {
    pub fn new(config: MessageOptimizationConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        let batch_semaphore = Arc::new(Semaphore::new(config.max_concurrent_batches));
        let compressor =
            MessageCompressor::new(config.compression_level, config.compression_threshold);

        Self {
            high_priority: Arc::new(Mutex::new(VecDeque::with_capacity(
                config.message_buffer_size / 4,
            ))),
            normal_priority: Arc::new(Mutex::new(VecDeque::with_capacity(
                config.message_buffer_size / 2,
            ))),
            low_priority: Arc::new(Mutex::new(VecDeque::with_capacity(
                config.message_buffer_size / 4,
            ))),
            active_batches: Arc::new(RwLock::new(HashMap::new())),
            config,
            metrics,
            compressor,
            batch_semaphore,
        }
    }

    /// Add message to appropriate priority queue
    pub fn enqueue_message(&self, message: Message) -> Result<()> {
        let prioritized = PrioritizedMessage::new(message);
        let priority = prioritized.priority;

        match priority {
            MessagePriority::Critical | MessagePriority::High => {
                let mut queue = self.high_priority.lock();
                if queue.len() >= queue.capacity() {
                    warn!("High priority message queue full, dropping oldest message");
                    queue.pop_front();
                }
                queue.push_back(prioritized);
            }
            MessagePriority::Normal => {
                let mut queue = self.normal_priority.lock();
                if queue.len() >= queue.capacity() {
                    warn!("Normal priority message queue full, dropping oldest message");
                    queue.pop_front();
                }
                queue.push_back(prioritized);
            }
            MessagePriority::Low => {
                let mut queue = self.low_priority.lock();
                if queue.len() >= queue.capacity() {
                    // For low priority, just drop the message
                    debug!("Low priority message queue full, dropping message");
                    return Ok(());
                }
                queue.push_back(prioritized);
            }
        }

        debug!("Enqueued message with priority {:?}", priority);
        Ok(())
    }

    /// Dequeue messages for batch processing
    pub fn dequeue_batch(&self) -> Option<MessageBatch> {
        let mut batch = MessageBatch::new();
        let max_batch_size = self.config.max_batch_size;

        // Process high priority messages first
        {
            let mut queue = self.high_priority.lock();
            while batch.len() < max_batch_size && !queue.is_empty() {
                if let Some(prioritized) = queue.pop_front() {
                    batch.add_message(prioritized.message);
                }
            }
        }

        // Then normal priority messages
        if batch.len() < max_batch_size {
            let mut queue = self.normal_priority.lock();
            while batch.len() < max_batch_size && !queue.is_empty() {
                if let Some(prioritized) = queue.pop_front() {
                    batch.add_message(prioritized.message);
                }
            }
        }

        // Finally low priority messages
        if batch.len() < max_batch_size {
            let mut queue = self.low_priority.lock();
            while batch.len() < max_batch_size && !queue.is_empty() {
                if let Some(prioritized) = queue.pop_front() {
                    batch.add_message(prioritized.message);
                }
            }
        }

        if batch.is_empty() {
            None
        } else {
            debug!("Dequeued batch with {} messages", batch.len());
            Some(batch)
        }
    }

    /// Get queue statistics
    pub fn stats(&self) -> MessageQueueStats {
        let high_count = self.high_priority.lock().len();
        let normal_count = self.normal_priority.lock().len();
        let low_count = self.low_priority.lock().len();
        let active_batches = self.active_batches.read().len();

        MessageQueueStats {
            high_priority_count: high_count,
            normal_priority_count: normal_count,
            low_priority_count: low_count,
            total_queued: high_count + normal_count + low_count,
            active_batches,
            queue_capacity: self.config.message_buffer_size,
        }
    }

    /// Start batch processing in background
    pub async fn start_batch_processor<F>(&self, mut batch_handler: F) -> Result<()>
    where
        F: FnMut(MessageBatch) -> Result<()> + Send + 'static,
    {
        let mut interval = interval(Duration::from_millis(self.config.batch_timeout_ms / 2));

        loop {
            interval.tick().await;

            // Check if we can acquire a permit for batch processing
            if let Ok(permit) = self.batch_semaphore.try_acquire() {
                if let Some(mut batch) = self.dequeue_batch() {
                    // Compress batch if enabled
                    if self.config.enable_compression {
                        if let Err(e) = self.compressor.compress_batch(&mut batch) {
                            error!("Failed to compress batch: {}", e);
                        }
                    }

                    // Track active batch
                    {
                        let mut active_batches = self.active_batches.write();
                        active_batches.insert(batch.batch_id, batch.clone());
                    }

                    // Process batch
                    match batch_handler(batch.clone()) {
                        Ok(()) => {
                            self.metrics
                                .batch_operations
                                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            debug!("Successfully processed batch {}", batch.batch_id);
                        }
                        Err(e) => {
                            error!("Failed to process batch {}: {}", batch.batch_id, e);
                        }
                    }

                    // Remove from active batches
                    {
                        let mut active_batches = self.active_batches.write();
                        active_batches.remove(&batch.batch_id);
                    }

                    drop(permit);
                }
            } else {
                debug!("All batch processing permits in use, waiting...");
            }
        }
    }
}

/// Statistics for message queue
#[derive(Debug, Clone)]
pub struct MessageQueueStats {
    pub high_priority_count: usize,
    pub normal_priority_count: usize,
    pub low_priority_count: usize,
    pub total_queued: usize,
    pub active_batches: usize,
    pub queue_capacity: usize,
}

/// Message optimization manager combining all optimizations
pub struct MessageOptimizationManager {
    queue: Arc<MessageQueue>,
    config: MessageOptimizationConfig,
    metrics: Arc<PerformanceMetrics>,
    _background_handles: Vec<tokio::task::JoinHandle<()>>,
}

impl MessageOptimizationManager {
    pub fn new(config: MessageOptimizationConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        let queue = Arc::new(MessageQueue::new(config.clone(), metrics.clone()));

        Self {
            queue,
            config,
            metrics,
            _background_handles: Vec::new(),
        }
    }

    /// Submit message for optimized processing
    pub async fn submit_message(&self, message: Message) -> Result<()> {
        self.queue.enqueue_message(message)?;
        Ok(())
    }

    /// Start background optimization processes
    pub async fn start_optimization_processes<F>(&mut self, batch_handler: F) -> Result<()>
    where
        F: FnMut(MessageBatch) -> Result<()> + Send + 'static + Clone,
    {
        let queue = self.queue.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = queue.start_batch_processor(batch_handler).await {
                error!("Batch processor error: {}", e);
            }
        });

        self._background_handles.push(handle);
        info!("Started message optimization background processes");
        Ok(())
    }

    /// Get comprehensive statistics
    pub fn stats(&self) -> MessageOptimizationStats {
        let queue_stats = self.queue.stats();

        MessageOptimizationStats {
            queue_stats,
            batch_operations: self
                .metrics
                .batch_operations
                .load(std::sync::atomic::Ordering::Relaxed),
            compression_count: self
                .metrics
                .compression_count
                .load(std::sync::atomic::Ordering::Relaxed),
            decompression_count: self
                .metrics
                .decompression_count
                .load(std::sync::atomic::Ordering::Relaxed),
            config: self.config.clone(),
        }
    }
}

/// Comprehensive statistics for message optimization
#[derive(Debug, Clone)]
pub struct MessageOptimizationStats {
    pub queue_stats: MessageQueueStats,
    pub batch_operations: u64,
    pub compression_count: u64,
    pub decompression_count: u64,
    pub config: MessageOptimizationConfig,
}
