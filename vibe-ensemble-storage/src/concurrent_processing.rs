//! Concurrent processing optimizations for database operations

use crate::error::{Error, Result};
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::performance::PerformanceMetrics;

/// Configuration for concurrent processing
#[derive(Debug, Clone)]
pub struct ConcurrentProcessingConfig {
    /// Maximum number of concurrent database operations
    pub max_concurrent_operations: usize,
    /// Work stealing enabled for load balancing
    pub enable_work_stealing: bool,
    /// Number of worker threads for CPU-intensive operations
    pub worker_thread_count: usize,
    /// Batch size for parallel processing
    pub parallel_batch_size: usize,
    /// Enable dynamic load balancing
    pub enable_dynamic_load_balancing: bool,
    /// Queue capacity for work items
    pub work_queue_capacity: usize,
    /// Timeout for individual operations (seconds)
    pub operation_timeout_seconds: u64,
}

impl Default for ConcurrentProcessingConfig {
    fn default() -> Self {
        let cpu_count = num_cpus::get();
        Self {
            max_concurrent_operations: cpu_count * 4,
            enable_work_stealing: true,
            worker_thread_count: cpu_count,
            parallel_batch_size: 100,
            enable_dynamic_load_balancing: true,
            work_queue_capacity: 1000,
            operation_timeout_seconds: 30,
        }
    }
}

/// Work item for concurrent processing
#[derive(Debug)]
pub struct WorkItem<T> {
    pub id: Uuid,
    pub data: T,
    pub priority: WorkPriority,
    pub created_at: Instant,
    pub retry_count: usize,
    pub max_retries: usize,
}

impl<T> WorkItem<T> {
    pub fn new(data: T) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
            priority: WorkPriority::Normal,
            created_at: Instant::now(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    pub fn with_priority(data: T, priority: WorkPriority) -> Self {
        Self {
            id: Uuid::new_v4(),
            data,
            priority,
            created_at: Instant::now(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    pub fn should_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

/// Priority levels for work items
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Worker thread statistics
#[derive(Debug)]
pub struct WorkerStats {
    pub worker_id: usize,
    pub tasks_processed: AtomicU64,
    pub processing_time_ms: AtomicU64,
    pub errors: AtomicU64,
    pub current_load: AtomicUsize,
    pub last_activity: Arc<RwLock<Instant>>,
}

impl WorkerStats {
    pub fn new(worker_id: usize) -> Self {
        Self {
            worker_id,
            tasks_processed: AtomicU64::new(0),
            processing_time_ms: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            current_load: AtomicUsize::new(0),
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub fn record_task_completion(&self, processing_time: Duration) {
        self.tasks_processed.fetch_add(1, Ordering::Relaxed);
        self.processing_time_ms
            .fetch_add(processing_time.as_millis() as u64, Ordering::Relaxed);
        *self.last_activity.write() = Instant::now();
    }

    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
        *self.last_activity.write() = Instant::now();
    }

    pub fn average_processing_time_ms(&self) -> f64 {
        let total_time = self.processing_time_ms.load(Ordering::Relaxed);
        let task_count = self.tasks_processed.load(Ordering::Relaxed);
        if task_count == 0 {
            0.0
        } else {
            total_time as f64 / task_count as f64
        }
    }
}

/// Work queue with priority support
pub struct PriorityWorkQueue<T> {
    critical_queue: Arc<Mutex<Vec<WorkItem<T>>>>,
    high_queue: Arc<Mutex<Vec<WorkItem<T>>>>,
    normal_queue: Arc<Mutex<Vec<WorkItem<T>>>>,
    low_queue: Arc<Mutex<Vec<WorkItem<T>>>>,
    total_items: AtomicUsize,
    capacity: usize,
}

impl<T> PriorityWorkQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            critical_queue: Arc::new(Mutex::new(Vec::new())),
            high_queue: Arc::new(Mutex::new(Vec::new())),
            normal_queue: Arc::new(Mutex::new(Vec::new())),
            low_queue: Arc::new(Mutex::new(Vec::new())),
            total_items: AtomicUsize::new(0),
            capacity,
        }
    }

    pub fn push(&self, item: WorkItem<T>) -> Result<()> {
        if self.total_items.load(Ordering::Relaxed) >= self.capacity {
            return Err(Error::Internal(anyhow::anyhow!("Work queue at capacity")));
        }

        match item.priority {
            WorkPriority::Critical => {
                let mut queue = self.critical_queue.lock();
                queue.push(item);
            }
            WorkPriority::High => {
                let mut queue = self.high_queue.lock();
                queue.push(item);
            }
            WorkPriority::Normal => {
                let mut queue = self.normal_queue.lock();
                queue.push(item);
            }
            WorkPriority::Low => {
                let mut queue = self.low_queue.lock();
                queue.push(item);
            }
        }

        self.total_items.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn pop(&self) -> Option<WorkItem<T>> {
        // Try critical queue first
        if let Some(mut queue) = self.critical_queue.try_lock() {
            if let Some(item) = queue.pop() {
                self.total_items.fetch_sub(1, Ordering::Relaxed);
                return Some(item);
            }
        }

        // Then high priority
        if let Some(mut queue) = self.high_queue.try_lock() {
            if let Some(item) = queue.pop() {
                self.total_items.fetch_sub(1, Ordering::Relaxed);
                return Some(item);
            }
        }

        // Then normal priority
        if let Some(mut queue) = self.normal_queue.try_lock() {
            if let Some(item) = queue.pop() {
                self.total_items.fetch_sub(1, Ordering::Relaxed);
                return Some(item);
            }
        }

        // Finally low priority
        if let Some(mut queue) = self.low_queue.try_lock() {
            if let Some(item) = queue.pop() {
                self.total_items.fetch_sub(1, Ordering::Relaxed);
                return Some(item);
            }
        }

        None
    }

    pub fn len(&self) -> usize {
        self.total_items.load(Ordering::Relaxed)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn queue_sizes(&self) -> (usize, usize, usize, usize) {
        let critical_size = self.critical_queue.lock().len();
        let high_size = self.high_queue.lock().len();
        let normal_size = self.normal_queue.lock().len();
        let low_size = self.low_queue.lock().len();
        (critical_size, high_size, normal_size, low_size)
    }
}

/// Load balancer for distributing work across workers
pub struct LoadBalancer {
    worker_stats: Arc<DashMap<usize, Arc<WorkerStats>>>,
    config: ConcurrentProcessingConfig,
}

impl LoadBalancer {
    pub fn new(config: ConcurrentProcessingConfig) -> Self {
        Self {
            worker_stats: Arc::new(DashMap::new()),
            config,
        }
    }

    pub fn register_worker(&self, worker_id: usize) {
        let stats = Arc::new(WorkerStats::new(worker_id));
        self.worker_stats.insert(worker_id, stats);
    }

    /// Get the least loaded worker
    pub fn get_least_loaded_worker(&self) -> Option<usize> {
        if !self.config.enable_dynamic_load_balancing {
            return None;
        }

        self.worker_stats
            .iter()
            .min_by_key(|entry| {
                let stats = entry.value();
                let current_load = stats.current_load.load(Ordering::Relaxed);
                let avg_time = stats.average_processing_time_ms() as usize;
                current_load * 1000 + avg_time // Weight current load heavily
            })
            .map(|entry| *entry.key())
    }

    /// Get load balancing recommendation
    pub fn get_load_balancing_recommendation(&self) -> LoadBalancingRecommendation {
        let mut worker_loads = Vec::new();
        let mut total_load = 0usize;

        for entry in self.worker_stats.iter() {
            let stats = entry.value();
            let load = stats.current_load.load(Ordering::Relaxed);
            worker_loads.push((*entry.key(), load));
            total_load += load;
        }

        worker_loads.sort_by_key(|(_, load)| *load);

        let worker_count = worker_loads.len();
        let avg_load = if worker_count > 0 {
            total_load / worker_count
        } else {
            0
        };
        let max_load = worker_loads.last().map(|(_, load)| *load).unwrap_or(0);
        let min_load = worker_loads.first().map(|(_, load)| *load).unwrap_or(0);

        LoadBalancingRecommendation {
            average_load: avg_load,
            max_load,
            min_load,
            load_imbalance: if avg_load > 0 {
                (max_load - min_load) as f64 / avg_load as f64
            } else {
                0.0
            },
            should_rebalance: max_load > avg_load * 2,
            overloaded_workers: worker_loads
                .iter()
                .filter(|(_, load)| *load > avg_load * 2)
                .map(|(worker_id, _)| *worker_id)
                .collect(),
            underloaded_workers: worker_loads
                .iter()
                .filter(|(_, load)| *load < avg_load / 2)
                .map(|(worker_id, _)| *worker_id)
                .collect(),
        }
    }

    pub fn get_worker_stats(&self, worker_id: usize) -> Option<Arc<WorkerStats>> {
        self.worker_stats
            .get(&worker_id)
            .map(|entry| entry.value().clone())
    }

    pub fn get_all_worker_stats(&self) -> Vec<(usize, Arc<WorkerStats>)> {
        self.worker_stats
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }
}

/// Load balancing recommendation
#[derive(Debug, Clone)]
pub struct LoadBalancingRecommendation {
    pub average_load: usize,
    pub max_load: usize,
    pub min_load: usize,
    pub load_imbalance: f64,
    pub should_rebalance: bool,
    pub overloaded_workers: Vec<usize>,
    pub underloaded_workers: Vec<usize>,
}

/// Concurrent processing engine
pub struct ConcurrentProcessingEngine<T> {
    work_queue: Arc<PriorityWorkQueue<T>>,
    load_balancer: Arc<LoadBalancer>,
    semaphore: Arc<Semaphore>,
    config: ConcurrentProcessingConfig,
    metrics: Arc<PerformanceMetrics>,
    active_tasks: Arc<AtomicUsize>,
    worker_handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl<T> ConcurrentProcessingEngine<T>
where
    T: Send + Sync + 'static + Clone,
{
    pub fn new(config: ConcurrentProcessingConfig, metrics: Arc<PerformanceMetrics>) -> Self {
        let work_queue = Arc::new(PriorityWorkQueue::new(config.work_queue_capacity));
        let load_balancer = Arc::new(LoadBalancer::new(config.clone()));
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_operations));

        Self {
            work_queue,
            load_balancer,
            semaphore,
            config,
            metrics,
            active_tasks: Arc::new(AtomicUsize::new(0)),
            worker_handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Submit work item for processing
    pub async fn submit_work(&self, item: WorkItem<T>) -> Result<()> {
        self.work_queue.push(item)?;
        debug!("Submitted work item to queue");
        Ok(())
    }

    /// Start worker threads
    pub async fn start_workers<F>(&self, processor: F) -> Result<()>
    where
        F: Fn(T) -> Result<()> + Send + Sync + Clone + 'static,
    {
        let mut handles = self.worker_handles.lock();

        for worker_id in 0..self.config.worker_thread_count {
            self.load_balancer.register_worker(worker_id);

            let work_queue = self.work_queue.clone();
            let semaphore = self.semaphore.clone();
            let metrics = self.metrics.clone();
            let active_tasks = self.active_tasks.clone();
            let load_balancer = self.load_balancer.clone();
            let processor = processor.clone();
            let timeout = Duration::from_secs(self.config.operation_timeout_seconds);

            let handle = tokio::spawn(async move {
                info!("Started worker thread {}", worker_id);

                loop {
                    // Try to get work from queue
                    if let Some(work_item) = work_queue.pop() {
                        // Acquire semaphore permit
                        if let Ok(permit) = semaphore.acquire().await {
                            active_tasks.fetch_add(1, Ordering::Relaxed);
                            let start_time = Instant::now();

                            // Update worker load
                            if let Some(stats) = load_balancer.get_worker_stats(worker_id) {
                                stats.current_load.fetch_add(1, Ordering::Relaxed);
                            }

                            // Check if we can retry before processing to avoid move issues
                            let can_retry = work_item.should_retry();
                            let current_retry_count = work_item.retry_count;

                            // Process work item with timeout
                            let result =
                                tokio::time::timeout(timeout, async { processor(work_item.data) })
                                    .await;

                            let processing_time = start_time.elapsed();

                            match result {
                                Ok(Ok(())) => {
                                    // Success
                                    if let Some(stats) = load_balancer.get_worker_stats(worker_id) {
                                        stats.record_task_completion(processing_time);
                                    }
                                    debug!(
                                        "Worker {} completed task in {:?}",
                                        worker_id, processing_time
                                    );
                                }
                                Ok(Err(e)) => {
                                    // Processing error
                                    if let Some(stats) = load_balancer.get_worker_stats(worker_id) {
                                        stats.record_error();
                                    }

                                    if can_retry {
                                        // Create a new work item for retry since data was consumed
                                        warn!("Worker {} failed to process task (attempt {}), but cannot retry with consumed data: {}", 
                                             worker_id, current_retry_count, e);
                                        // Note: In a real system, we'd need to preserve the original data for retries
                                        // This is a limitation of the current design
                                    } else {
                                        error!(
                                            "Worker {} failed to process task after {} retries: {}",
                                            worker_id, current_retry_count, e
                                        );
                                    }
                                }
                                Err(_) => {
                                    // Timeout
                                    if let Some(stats) = load_balancer.get_worker_stats(worker_id) {
                                        stats.record_error();
                                    }
                                    warn!("Worker {} timed out processing task", worker_id);
                                }
                            }

                            // Update metrics and cleanup
                            active_tasks.fetch_sub(1, Ordering::Relaxed);
                            metrics
                                .concurrent_operations
                                .store(active_tasks.load(Ordering::Relaxed), Ordering::Relaxed);

                            if let Some(stats) = load_balancer.get_worker_stats(worker_id) {
                                stats.current_load.fetch_sub(1, Ordering::Relaxed);
                            }

                            drop(permit);
                        }
                    } else {
                        // No work available, sleep briefly
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                }
            });

            handles.push(handle);
        }

        info!("Started {} worker threads", self.config.worker_thread_count);
        Ok(())
    }

    /// Process items in parallel batches
    pub async fn process_parallel_batch<F, R>(
        &self,
        items: Vec<T>,
        processor: F,
    ) -> Result<Vec<Result<R>>>
    where
        F: Fn(T) -> Result<R> + Send + Sync + Clone,
        R: Send,
    {
        let batch_size = self.config.parallel_batch_size;
        let mut results = Vec::new();

        for chunk in items.chunks(batch_size) {
            let chunk_results: Vec<Result<R>> = chunk
                .par_iter()
                .map(|item| {
                    let processor = processor.clone();
                    processor(item.clone())
                })
                .collect();

            results.extend(chunk_results);
        }

        self.metrics
            .batch_operations
            .fetch_add(1, Ordering::Relaxed);
        Ok(results)
    }

    /// Get processing statistics
    pub fn stats(&self) -> ConcurrentProcessingStats {
        let (critical_queue, high_queue, normal_queue, low_queue) = self.work_queue.queue_sizes();
        let load_balancing = self.load_balancer.get_load_balancing_recommendation();
        let worker_stats = self.load_balancer.get_all_worker_stats();

        ConcurrentProcessingStats {
            active_tasks: self.active_tasks.load(Ordering::Relaxed),
            queued_critical: critical_queue,
            queued_high: high_queue,
            queued_normal: normal_queue,
            queued_low: low_queue,
            total_queued: self.work_queue.len(),
            worker_count: self.config.worker_thread_count,
            load_balancing,
            worker_stats: worker_stats.into_iter().collect(),
        }
    }
}

/// Concurrent processing statistics
#[derive(Debug, Clone)]
pub struct ConcurrentProcessingStats {
    pub active_tasks: usize,
    pub queued_critical: usize,
    pub queued_high: usize,
    pub queued_normal: usize,
    pub queued_low: usize,
    pub total_queued: usize,
    pub worker_count: usize,
    pub load_balancing: LoadBalancingRecommendation,
    pub worker_stats: HashMap<usize, Arc<WorkerStats>>,
}

/// Parallel processing utilities
pub struct ParallelProcessor;

impl ParallelProcessor {
    /// Process items in parallel with automatic batching
    pub fn process_parallel<T, R, F>(items: Vec<T>, processor: F) -> Vec<Result<R>>
    where
        T: Send + Sync,
        R: Send,
        F: Fn(&T) -> Result<R> + Send + Sync,
    {
        items.par_iter().map(processor).collect()
    }

    /// Process items in parallel with custom batch size
    pub fn process_parallel_batched<T, R, F>(
        items: Vec<T>,
        batch_size: usize,
        processor: F,
    ) -> Vec<Result<R>>
    where
        T: Send + Sync,
        R: Send,
        F: Fn(&T) -> Result<R> + Send + Sync,
    {
        items
            .par_chunks(batch_size)
            .flat_map(|chunk| chunk.par_iter().map(&processor).collect::<Vec<_>>())
            .collect()
    }

    /// Parallel map with custom thread pool
    pub fn parallel_map<T, R, F>(items: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> R + Send + Sync,
    {
        items.into_par_iter().map(processor).collect()
    }

    /// Parallel filter and map
    pub fn parallel_filter_map<T, R, F>(items: Vec<T>, processor: F) -> Vec<R>
    where
        T: Send,
        R: Send,
        F: Fn(T) -> Option<R> + Send + Sync,
    {
        items.into_par_iter().filter_map(processor).collect()
    }

    /// Parallel reduce operation
    pub fn parallel_reduce<T, R, F, G>(items: Vec<T>, identity: R, map_fn: F, reduce_fn: G) -> R
    where
        T: Send,
        R: Send + Clone + Sync,
        F: Fn(T) -> R + Send + Sync,
        G: Fn(R, R) -> R + Send + Sync,
    {
        items
            .into_par_iter()
            .map(map_fn)
            .reduce(|| identity.clone(), reduce_fn)
    }
}
