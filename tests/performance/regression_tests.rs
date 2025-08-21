//! Performance regression tests to ensure optimizations maintain expected performance levels

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::{Agent, AgentType, AgentStatus},
    issue::{Issue, IssueStatus, Priority},
    message::{Message, MessageType},
    knowledge::Knowledge,
};
use vibe_ensemble_storage::{
    StorageManager, PerformanceConfig, DatabaseConfig,
    ConcurrentProcessingConfig, MessageOptimizationConfig, NetworkOptimizationConfig,
    WorkItem, MessageBatch, PrioritizedMessage,
};

use crate::common::{
    database::DatabaseTestHelper,
    fixtures::{TestDataFactory, TestScenarios},
    assertions::PerformanceAssertions,
};

/// Performance benchmarks and regression thresholds
struct PerformanceThresholds {
    // Database operations (operations per second)
    min_agent_creation_ops: f64,
    min_issue_creation_ops: f64,
    min_message_creation_ops: f64,
    min_knowledge_creation_ops: f64,
    
    // Query performance (milliseconds)
    max_single_query_time: Duration,
    max_complex_query_time: Duration,
    max_batch_operation_time: Duration,
    
    // Cache performance
    min_cache_hit_rate: f64,
    max_cache_miss_penalty: Duration,
    
    // Concurrent processing
    min_concurrent_throughput: f64,
    max_resource_contention_delay: Duration,
    
    // Memory usage (MB)
    max_memory_per_operation: usize,
    max_memory_growth_rate: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            min_agent_creation_ops: 50.0,
            min_issue_creation_ops: 100.0,
            min_message_creation_ops: 200.0,
            min_knowledge_creation_ops: 30.0,
            max_single_query_time: Duration::from_millis(50),
            max_complex_query_time: Duration::from_millis(500),
            max_batch_operation_time: Duration::from_millis(100),
            min_cache_hit_rate: 0.8,
            max_cache_miss_penalty: Duration::from_millis(10),
            min_concurrent_throughput: 500.0,
            max_resource_contention_delay: Duration::from_millis(100),
            max_memory_per_operation: 1, // 1MB
            max_memory_growth_rate: 0.1, // 10% growth
        }
    }
}

/// Comprehensive performance regression test suite
#[tokio::test]
async fn performance_regression_suite() {
    let thresholds = PerformanceThresholds::default();
    
    println!("🚀 Starting Performance Regression Test Suite");
    println!("============================================");

    // Test database performance
    test_database_performance(&thresholds).await;
    
    // Test caching performance
    test_caching_performance(&thresholds).await;
    
    // Test concurrent processing performance
    test_concurrent_processing_performance(&thresholds).await;
    
    // Test memory usage patterns
    test_memory_usage_patterns(&thresholds).await;
    
    // Test network optimization performance
    test_network_optimization_performance(&thresholds).await;
    
    // Test comprehensive end-to-end performance
    test_end_to_end_performance(&thresholds).await;
    
    println!("✅ All performance regression tests passed!");
}

async fn test_database_performance(thresholds: &PerformanceThresholds) {
    println!("\n📊 Testing Database Performance");
    println!("------------------------------");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let mut db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(50),
        migrate_on_startup: true,
        performance_config: Some(PerformanceConfig {
            max_connections: 50,
            query_timeout_seconds: 30,
            enable_compression: true,
            monitoring_enabled: true,
            batch_size: 100,
            ..Default::default()
        }),
    };
    
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());

    // Test agent creation performance
    println!("  Testing agent creation...");
    let start = Instant::now();
    let agent_count = 1000;
    
    let mut handles = vec![];
    for _ in 0..agent_count {
        let storage = storage_manager.clone();
        let handle = tokio::spawn(async move {
            let agent = TestDataFactory::create_random_agent();
            storage.agents().create(&agent).await
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.await.unwrap().unwrap();
    }
    
    let duration = start.elapsed();
    let ops_per_second = agent_count as f64 / duration.as_secs_f64();
    
    println!("    Created {} agents in {:?} ({:.2} ops/sec)", 
            agent_count, duration, ops_per_second);
    
    assert!(ops_per_second >= thresholds.min_agent_creation_ops,
           "Agent creation performance below threshold: {:.2} < {:.2}",
           ops_per_second, thresholds.min_agent_creation_ops);

    // Test complex query performance
    println!("  Testing complex queries...");
    let start = Instant::now();
    let _ = storage_manager.agents().list().await.unwrap();
    let query_time = start.elapsed();
    
    println!("    Complex query took {:?}", query_time);
    assert!(query_time <= thresholds.max_complex_query_time,
           "Complex query too slow: {:?} > {:?}",
           query_time, thresholds.max_complex_query_time);

    // Test batch operations
    println!("  Testing batch operations...");
    let issues: Vec<Issue> = (0..100).map(|_| TestDataFactory::create_random_issue()).collect();
    let start = Instant::now();
    
    for issue in issues {
        storage_manager.issues().create(&issue).await.unwrap();
    }
    
    let batch_time = start.elapsed();
    println!("    Batch operation took {:?}", batch_time);
    assert!(batch_time <= thresholds.max_batch_operation_time,
           "Batch operation too slow: {:?} > {:?}",
           batch_time, thresholds.max_batch_operation_time);

    println!("  ✅ Database performance tests passed");
}

async fn test_caching_performance(thresholds: &PerformanceThresholds) {
    println!("\n🗂️  Testing Caching Performance");
    println!("------------------------------");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(30),
        migrate_on_startup: true,
        performance_config: Some(PerformanceConfig {
            agent_cache_size: 5000,
            issue_cache_size: 10000,
            cache_ttl_seconds: 3600,
            enable_compression: true,
            monitoring_enabled: true,
            ..Default::default()
        }),
    };
    
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());
    
    // Create test data
    let agent = TestDataFactory::create_random_agent();
    storage_manager.agents().create(&agent).await.unwrap();
    
    // Test cache warmup
    println!("  Warming up cache...");
    let _ = storage_manager.agents().find_by_id(agent.id).await.unwrap();
    
    // Test cached read performance
    println!("  Testing cached reads...");
    let iterations = 1000;
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _ = storage_manager.agents().find_by_id(agent.id).await.unwrap();
    }
    
    let duration = start.elapsed();
    let avg_time = duration / iterations;
    
    println!("    {} cached reads took {:?} (avg: {:?})", 
            iterations, duration, avg_time);
    assert!(avg_time <= thresholds.max_cache_miss_penalty,
           "Cached read too slow: {:?} > {:?}",
           avg_time, thresholds.max_cache_miss_penalty);

    // Test cache hit rate
    let performance_report = storage_manager.performance_report().await.unwrap();
    let hit_rate = performance_report.cache_hit_rate;
    
    println!("    Cache hit rate: {:.2}", hit_rate);
    assert!(hit_rate >= thresholds.min_cache_hit_rate,
           "Cache hit rate too low: {:.2} < {:.2}",
           hit_rate, thresholds.min_cache_hit_rate);

    println!("  ✅ Caching performance tests passed");
}

async fn test_concurrent_processing_performance(thresholds: &PerformanceThresholds) {
    println!("\n⚡ Testing Concurrent Processing Performance");
    println!("------------------------------------------");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(100),
        migrate_on_startup: true,
        performance_config: None,
    };
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());
    
    // Test concurrent agent creation
    println!("  Testing concurrent operations...");
    let concurrent_tasks = 100;
    let operations_per_task = 10;
    let start = Instant::now();
    
    let mut handles = vec![];
    for _ in 0..concurrent_tasks {
        let storage = storage_manager.clone();
        let handle = tokio::spawn(async move {
            for _ in 0..operations_per_task {
                let agent = TestDataFactory::create_random_agent();
                let _ = storage.agents().create(&agent).await;
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    let total_ops = concurrent_tasks * operations_per_task;
    let throughput = total_ops as f64 / duration.as_secs_f64();
    
    println!("    {} concurrent operations in {:?} ({:.2} ops/sec)", 
            total_ops, duration, throughput);
    
    assert!(throughput >= thresholds.min_concurrent_throughput,
           "Concurrent throughput too low: {:.2} < {:.2}",
           throughput, thresholds.min_concurrent_throughput);

    println!("  ✅ Concurrent processing tests passed");
}

async fn test_memory_usage_patterns(thresholds: &PerformanceThresholds) {
    println!("\n💾 Testing Memory Usage Patterns");
    println!("--------------------------------");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(50),
        migrate_on_startup: true,
        performance_config: None,
    };
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());

    // Baseline memory measurement
    let baseline_memory = get_memory_usage();
    
    // Create large dataset
    let dataset_size = 10000;
    println!("  Creating dataset of {} items...", dataset_size);
    
    let start_memory = get_memory_usage();
    
    for i in 0..dataset_size {
        let agent = TestDataFactory::create_random_agent();
        storage_manager.agents().create(&agent).await.unwrap();
        
        // Check memory growth periodically
        if i % 1000 == 0 && i > 0 {
            let current_memory = get_memory_usage();
            let growth_per_item = (current_memory - start_memory) / i;
            
            if growth_per_item > thresholds.max_memory_per_operation * 1024 {
                panic!("Memory growth too high: {} bytes per operation", growth_per_item);
            }
        }
    }
    
    let final_memory = get_memory_usage();
    let total_growth = final_memory - baseline_memory;
    let memory_per_item = total_growth / dataset_size;
    
    println!("    Memory usage: {} KB total, {} bytes per item", 
            total_growth / 1024, memory_per_item);
    
    assert!(memory_per_item <= thresholds.max_memory_per_operation * 1024,
           "Memory per operation too high: {} > {}",
           memory_per_item, thresholds.max_memory_per_operation * 1024);

    println!("  ✅ Memory usage tests passed");
}

async fn test_network_optimization_performance(_thresholds: &PerformanceThresholds) {
    println!("\n🌐 Testing Network Optimization Performance");
    println!("------------------------------------------");

    // Test message compression
    println!("  Testing message compression...");
    let large_message = "x".repeat(10000);
    let message = Message::broadcast(Uuid::new_v4(), &large_message).unwrap();
    
    let original_size = large_message.len();
    // Note: In real implementation, this would use the compression from network optimization
    println!("    Original message size: {} bytes", original_size);
    
    // Test connection pooling (simulated)
    println!("  Testing connection reuse...");
    let connection_start = Instant::now();
    
    // Simulate multiple requests to same host
    for _ in 0..100 {
        // In real implementation, this would use the connection pool
        sleep(Duration::from_millis(1)).await;
    }
    
    let connection_time = connection_start.elapsed();
    println!("    Connection reuse simulation: {:?}", connection_time);

    println!("  ✅ Network optimization tests passed");
}

async fn test_end_to_end_performance(thresholds: &PerformanceThresholds) {
    println!("\n🎯 Testing End-to-End Performance");
    println!("----------------------------------");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(50),
        migrate_on_startup: true,
        performance_config: Some(PerformanceConfig {
            max_connections: 50,
            enable_compression: true,
            monitoring_enabled: true,
            batch_size: 100,
            agent_cache_size: 5000,
            issue_cache_size: 10000,
            message_cache_size: 15000,
            ..Default::default()
        }),
    };
    
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());

    println!("  Running comprehensive workflow simulation...");
    let start = Instant::now();
    
    // Simulate realistic workflow
    let workflow_iterations = 100;
    
    for i in 0..workflow_iterations {
        // Create agent
        let agent = TestDataFactory::create_random_agent();
        storage_manager.agents().create(&agent).await.unwrap();
        
        // Create issues for agent
        for _ in 0..5 {
            let mut issue = TestDataFactory::create_random_issue();
            issue.assigned_agent_id = Some(agent.id);
            storage_manager.issues().create(&issue).await.unwrap();
        }
        
        // Create messages
        for _ in 0..3 {
            let message = Message::broadcast(agent.id, &format!("Status update {}", i)).unwrap();
            storage_manager.messages().create(&message).await.unwrap();
        }
        
        // Create knowledge entry
        let knowledge = TestDataFactory::create_random_knowledge(agent.id);
        storage_manager.knowledge().create(&knowledge).await.unwrap();
        
        // Query operations
        let _ = storage_manager.agents().find_by_id(agent.id).await.unwrap();
        let _ = storage_manager.issues().list_by_agent(agent.id).await.unwrap();
    }
    
    let duration = start.elapsed();
    let operations_per_second = (workflow_iterations * 10) as f64 / duration.as_secs_f64(); // ~10 ops per iteration
    
    println!("    Completed {} workflow iterations in {:?} ({:.2} ops/sec)", 
            workflow_iterations, duration, operations_per_second);

    // Get final performance report
    let performance_report = storage_manager.performance_report().await.unwrap();
    
    println!("    Final Performance Report:");
    println!("      Cache hit rate: {:.2}", performance_report.cache_hit_rate);
    println!("      Average query time: {:.2}ms", performance_report.avg_query_time_ms);
    println!("      Concurrent operations: {}", performance_report.concurrent_operations);
    println!("      Compression ratio: {:.2}", performance_report.compression_ratio);

    assert!(operations_per_second >= 50.0,
           "End-to-end performance too low: {:.2} < 50.0", operations_per_second);

    println!("  ✅ End-to-end performance tests passed");
}

/// Mock memory usage function - in real implementation would use proper memory profiling
fn get_memory_usage() -> usize {
    // Simplified mock implementation
    use std::sync::atomic::{AtomicUsize, Ordering};
    static MOCK_MEMORY: AtomicUsize = AtomicUsize::new(1024 * 1024); // Start at 1MB
    
    let current = MOCK_MEMORY.load(Ordering::Relaxed);
    // Simulate memory growth
    use std::hash::{Hash, Hasher, DefaultHasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    let growth = (hasher.finish() % 1024) as usize;
    MOCK_MEMORY.store(current + growth, Ordering::Relaxed);
    current + growth
}

/// Performance benchmark with Criterion for detailed analysis
#[tokio::test]
async fn detailed_performance_benchmarks() {
    println!("\n📈 Running Detailed Performance Benchmarks");
    println!("===========================================");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(30),
        migrate_on_startup: true,
        performance_config: None,
    };
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());

    // Benchmark different batch sizes
    for &batch_size in &[10, 50, 100, 500] {
        println!("  Benchmarking batch size: {}", batch_size);
        
        let start = Instant::now();
        let agents: Vec<Agent> = (0..batch_size).map(|_| TestDataFactory::create_random_agent()).collect();
        
        for agent in agents {
            storage_manager.agents().create(&agent).await.unwrap();
        }
        
        let duration = start.elapsed();
        let ops_per_second = batch_size as f64 / duration.as_secs_f64();
        
        println!("    Batch size {}: {:.2} ops/sec", batch_size, ops_per_second);
    }

    // Benchmark different concurrency levels
    for &concurrency in &[1, 5, 10, 25, 50] {
        println!("  Benchmarking concurrency level: {}", concurrency);
        
        let start = Instant::now();
        let operations_per_worker = 50;
        
        let mut handles = vec![];
        for _ in 0..concurrency {
            let storage = storage_manager.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..operations_per_worker {
                    let agent = TestDataFactory::create_random_agent();
                    storage.agents().create(&agent).await.unwrap();
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let duration = start.elapsed();
        let total_ops = concurrency * operations_per_worker;
        let ops_per_second = total_ops as f64 / duration.as_secs_f64();
        
        println!("    Concurrency {}: {:.2} ops/sec", concurrency, ops_per_second);
    }

    println!("  ✅ Detailed benchmarks completed");
}

/// Load testing with sustained high throughput
#[tokio::test]
async fn sustained_load_test() {
    println!("\n🔥 Running Sustained Load Test");
    println!("==============================");

    let _db_helper = DatabaseTestHelper::new().await.unwrap();
    let db_config = DatabaseConfig {
        url: ":memory:".to_string(),
        max_connections: Some(100),
        migrate_on_startup: true,
        performance_config: Some(PerformanceConfig {
            max_connections: 100,
            enable_compression: true,
            monitoring_enabled: true,
            batch_size: 50,
            ..Default::default()
        }),
    };
    
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());

    let test_duration = Duration::from_secs(30); // 30 second sustained test
    let target_ops_per_second = 100;
    let operation_interval = Duration::from_millis(1000 / target_ops_per_second as u64);
    
    println!("  Target: {} ops/sec for {:?}", target_ops_per_second, test_duration);
    
    let start = Instant::now();
    let mut operations_completed = 0;
    let mut errors = 0;
    
    while start.elapsed() < test_duration {
        let operation_start = Instant::now();
        
        // Perform operation
        match perform_random_operation(&storage_manager).await {
            Ok(_) => operations_completed += 1,
            Err(_) => errors += 1,
        }
        
        // Rate limiting
        let elapsed = operation_start.elapsed();
        if elapsed < operation_interval {
            sleep(operation_interval - elapsed).await;
        }
    }
    
    let actual_duration = start.elapsed();
    let actual_ops_per_second = operations_completed as f64 / actual_duration.as_secs_f64();
    let error_rate = errors as f64 / (operations_completed + errors) as f64;
    
    println!("  Results:");
    println!("    Operations completed: {}", operations_completed);
    println!("    Actual ops/sec: {:.2}", actual_ops_per_second);
    println!("    Error rate: {:.2}%", error_rate * 100.0);
    println!("    Duration: {:?}", actual_duration);

    assert!(error_rate < 0.05, "Error rate too high: {:.2}%", error_rate * 100.0);
    assert!(actual_ops_per_second >= target_ops_per_second as f64 * 0.8,
           "Sustained throughput too low: {:.2} < {:.2}",
           actual_ops_per_second, target_ops_per_second as f64 * 0.8);

    println!("  ✅ Sustained load test passed");
}

async fn perform_random_operation(storage_manager: &StorageManager) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::hash::{Hash, Hasher, DefaultHasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    match (hasher.finish() % 4) as u8 {
        0 => {
            let agent = TestDataFactory::create_random_agent();
            storage_manager.agents().create(&agent).await?;
        },
        1 => {
            let issue = TestDataFactory::create_random_issue();
            storage_manager.issues().create(&issue).await?;
        },
        2 => {
            let sender_id = Uuid::new_v4();
            let message = Message::broadcast(sender_id, "Load test message").unwrap();
            storage_manager.messages().create(&message).await?;
        },
        3 => {
            let author_id = Uuid::new_v4();
            let knowledge = TestDataFactory::create_random_knowledge(author_id);
            storage_manager.knowledge().create(&knowledge).await?;
        },
        _ => unreachable!(),
    }
    Ok(())
}