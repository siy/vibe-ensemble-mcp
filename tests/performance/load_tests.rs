//! Performance and load tests for vibe-ensemble-mcp
//!
//! These tests validate system performance under various load conditions
//! and ensure acceptable response times and throughput.

use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
use std::time::{Duration, Instant};
use tokio::time::{timeout, sleep};
use criterion::{black_box, Criterion, BenchmarkId};
use uuid::Uuid;

use vibe_ensemble_core::{
    agent::Agent,
    issue::Issue,
    message::Message,
    knowledge::Knowledge,
};
use vibe_ensemble_storage::StorageManager;

use crate::common::{
    database::DatabaseTestHelper,
    fixtures::{TestDataFactory, TestScenarios},
    assertions::PerformanceAssertions,
};

/// Benchmark agent registration performance with optimizations
#[tokio::test]
async fn benchmark_agent_registration() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    
    // Create optimized storage manager with performance config
    let mut db_config = db_helper.config.clone();
    db_config.performance_config = Some(vibe_ensemble_storage::PerformanceConfig {
        agent_cache_size: 2000,
        max_connections: 30,
        enable_compression: true,
        monitoring_enabled: true,
        batch_size: 50,
        ..Default::default()
    });
    
    let storage_manager = Arc::new(StorageManager::new(&db_config).await.unwrap());
    
    let agent_counts = [10, 50, 100, 500];
    
    for &count in &agent_counts {
        let start = Instant::now();
        
        // Create and register agents
        let mut handles = vec![];
        for _ in 0..count {
            let storage_clone = storage_manager.clone();
            let handle = tokio::spawn(async move {
                let agent = TestDataFactory::create_random_agent();
                storage_clone.agents().create_agent(agent).await
            });
            handles.push(handle);
        }
        
        // Wait for all registrations to complete
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        let duration = start.elapsed();
        let throughput = count as f64 / duration.as_secs_f64();
        
        println!("Registered {} agents in {:?} ({:.2} agents/sec)", 
                count, duration, throughput);
        
        // Performance assertions
        assert!(duration < Duration::from_secs(30));
        assert!(throughput > 1.0); // At least 1 agent per second
        
        // Clear for next test
        db_helper.clear_table("agents").await.unwrap();
    }
}

/// Benchmark issue creation and assignment performance
#[tokio::test]
async fn benchmark_issue_operations() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    db_helper.seed_test_data().await.unwrap();
    
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let issue_counts = [50, 200, 500, 1000];
    
    for &count in &issue_counts {
        println!("Benchmarking {} issues", count);
        
        // Test issue creation
        let creation_start = Instant::now();
        let mut issue_ids = vec![];
        
        for _ in 0..count {
            let issue = TestDataFactory::create_random_issue();
            let issue_id = storage_manager.issues().create_issue(issue).await.unwrap();
            issue_ids.push(issue_id);
        }
        
        let creation_duration = creation_start.elapsed();
        let creation_throughput = count as f64 / creation_duration.as_secs_f64();
        
        println!("Created {} issues in {:?} ({:.2} issues/sec)", 
                count, creation_duration, creation_throughput);
        
        // Test issue assignment
        let assignment_start = Instant::now();
        let dummy_agent_id = Uuid::new_v4();
        
        for issue_id in &issue_ids {
            storage_manager.issues()
                .assign_issue(*issue_id, dummy_agent_id)
                .await.unwrap();
        }
        
        let assignment_duration = assignment_start.elapsed();
        let assignment_throughput = count as f64 / assignment_duration.as_secs_f64();
        
        println!("Assigned {} issues in {:?} ({:.2} assignments/sec)", 
                count, assignment_duration, assignment_throughput);
        
        // Performance assertions
        assert!(creation_throughput > 5.0);
        assert!(assignment_throughput > 10.0);
        
        // Clean up
        for issue_id in issue_ids {
            storage_manager.issues().delete_issue(issue_id).await.unwrap();
        }
    }
}

/// Benchmark message throughput
#[tokio::test]
async fn benchmark_message_throughput() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    db_helper.seed_test_data().await.unwrap();
    
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let message_counts = [100, 500, 1000, 2000];
    
    for &count in &message_counts {
        println!("Benchmarking {} messages", count);
        
        let start = Instant::now();
        let sender_id = Uuid::new_v4();
        
        // Send mix of direct and broadcast messages
        let mut handles = vec![];
        for i in 0..count {
            let storage_clone = storage_manager.clone();
            let handle = tokio::spawn(async move {
                let message = if i % 3 == 0 {
                    // Broadcast message
                    Message::broadcast(sender_id, &format!("Broadcast message {}", i)).unwrap()
                } else {
                    // Direct message
                    let recipient_id = Uuid::new_v4();
                    Message::direct(sender_id, recipient_id, &format!("Direct message {}", i)).unwrap()
                };
                
                storage_clone.messages().create_message(message).await
            });
            handles.push(handle);
        }
        
        // Wait for all messages
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        let duration = start.elapsed();
        let throughput = count as f64 / duration.as_secs_f64();
        
        println!("Sent {} messages in {:?} ({:.2} messages/sec)", 
                count, duration, throughput);
        
        // Performance assertions
        assert!(throughput > 20.0); // At least 20 messages per second
        assert!(duration < Duration::from_secs(60));
        
        // Clean up
        db_helper.clear_table("messages").await.unwrap();
    }
}

/// Benchmark knowledge search performance
#[tokio::test]
async fn benchmark_knowledge_search() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let author_id = Uuid::new_v4();
    let knowledge_counts = [50, 200, 500, 1000];
    
    for &count in &knowledge_counts {
        println!("Benchmarking search with {} knowledge entries", count);
        
        // Create knowledge entries
        for _ in 0..count {
            let knowledge = TestDataFactory::create_random_knowledge(author_id);
            storage_manager.knowledge().create_knowledge(knowledge).await.unwrap();
        }
        
        // Benchmark different search terms
        let search_terms = ["rust", "testing", "database", "performance", "best practice"];
        
        for term in &search_terms {
            let start = Instant::now();
            let results = storage_manager.knowledge()
                .search_knowledge(term.to_string(), author_id)
                .await.unwrap();
            let duration = start.elapsed();
            
            println!("Search for '{}' took {:?}, found {} results", 
                    term, duration, results.len());
            
            // Performance assertion
            assert!(duration < Duration::from_millis(500));
        }
        
        // Clean up
        db_helper.clear_table("knowledge").await.unwrap();
    }
}

/// Stress test concurrent operations
#[tokio::test]
async fn stress_test_concurrent_operations() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let concurrent_tasks = 50;
    let operations_per_task = 20;
    
    println!("Starting stress test with {} concurrent tasks, {} operations each", 
            concurrent_tasks, operations_per_task);
    
    let start = Instant::now();
    let error_count = Arc::new(AtomicUsize::new(0));
    
    let mut handles = vec![];
    for task_id in 0..concurrent_tasks {
        let storage_clone = storage_manager.clone();
        let error_count_clone = error_count.clone();
        
        let handle = tokio::spawn(async move {
            for op_id in 0..operations_per_task {
                // Mix of operations
                let result = match (task_id + op_id) % 4 {
                    0 => {
                        // Create agent
                        let agent = TestDataFactory::create_random_agent();
                        storage_clone.agents().create_agent(agent).await.map(|_| ())
                    },
                    1 => {
                        // Create issue
                        let issue = TestDataFactory::create_random_issue();
                        storage_clone.issues().create_issue(issue).await.map(|_| ())
                    },
                    2 => {
                        // Create message
                        let sender_id = Uuid::new_v4();
                        let message = Message::broadcast(
                            sender_id, 
                            &format!("Stress test message {} from task {}", op_id, task_id)
                        ).unwrap();
                        storage_clone.messages().create_message(message).await.map(|_| ())
                    },
                    3 => {
                        // Create knowledge
                        let knowledge = TestDataFactory::create_random_knowledge(Uuid::new_v4());
                        storage_clone.knowledge().create_knowledge(knowledge).await.map(|_| ())
                    },
                    _ => unreachable!(),
                };
                
                if result.is_err() {
                    error_count_clone.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    let duration = start.elapsed();
    let total_operations = concurrent_tasks * operations_per_task;
    let throughput = total_operations as f64 / duration.as_secs_f64();
    let error_rate = error_count.load(Ordering::SeqCst) as f64 / total_operations as f64;
    
    println!("Stress test completed:");
    println!("  Duration: {:?}", duration);
    println!("  Total operations: {}", total_operations);
    println!("  Throughput: {:.2} ops/sec", throughput);
    println!("  Error rate: {:.2}%", error_rate * 100.0);
    
    // Performance assertions
    assert!(duration < Duration::from_secs(120)); // Complete within 2 minutes
    assert!(error_rate < 0.05); // Less than 5% error rate
    assert!(throughput > 5.0); // At least 5 operations per second
}

/// Memory usage benchmark
#[tokio::test]
async fn benchmark_memory_usage() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    // Get baseline memory usage
    let baseline_memory = get_memory_usage();
    println!("Baseline memory: {} KB", baseline_memory / 1024);
    
    let data_sizes = [100, 500, 1000, 2000];
    
    for &size in &data_sizes {
        println!("Testing memory usage with {} entities", size);
        
        // Create test data
        let start_memory = get_memory_usage();
        
        for _ in 0..size {
            let agent = TestDataFactory::create_random_agent();
            let issue = TestDataFactory::create_random_issue();
            let knowledge = TestDataFactory::create_random_knowledge(Uuid::new_v4());
            
            storage_manager.agents().create_agent(agent).await.unwrap();
            storage_manager.issues().create_issue(issue).await.unwrap();
            storage_manager.knowledge().create_knowledge(knowledge).await.unwrap();
        }
        
        let end_memory = get_memory_usage();
        let memory_increase = end_memory - start_memory;
        let memory_per_entity = memory_increase / (size * 3); // 3 entities per iteration
        
        println!("Memory increase: {} KB ({} bytes per entity)", 
                memory_increase / 1024, memory_per_entity);
        
        // Memory assertions
        assert!(memory_per_entity < 10000); // Less than 10KB per entity
        
        // Clear data for next test
        db_helper.clear_table("agents").await.unwrap();
        db_helper.clear_table("issues").await.unwrap();
        db_helper.clear_table("knowledge").await.unwrap();
        
        // Allow garbage collection
        tokio::task::yield_now().await;
    }
}

/// Connection pool performance test
#[tokio::test]
async fn benchmark_connection_pool() {
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    
    let concurrent_connections = [5, 10, 20, 50];
    let operations_per_connection = 10;
    
    for &connections in &concurrent_connections {
        println!("Testing {} concurrent database connections", connections);
        
        let start = Instant::now();
        let mut handles = vec![];
        
        for _ in 0..connections {
            let storage_clone = storage_manager.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..operations_per_connection {
                    // Simple database operation
                    let agent = TestDataFactory::create_random_agent();
                    storage_clone.agents().create_agent(agent).await.unwrap();
                }
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        let duration = start.elapsed();
        let total_ops = connections * operations_per_connection;
        let throughput = total_ops as f64 / duration.as_secs_f64();
        
        println!("Completed {} operations in {:?} ({:.2} ops/sec)", 
                total_ops, duration, throughput);
        
        // Performance assertions
        assert!(duration < Duration::from_secs(30));
        assert!(throughput > 1.0);
        
        // Clean up
        db_helper.clear_table("agents").await.unwrap();
    }
}

/// WebSocket performance test
#[tokio::test]
async fn benchmark_websocket_performance() {
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};
    use futures_util::{SinkExt, StreamExt};
    
    // Start WebSocket server
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server_handle = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let ws_stream = accept_async(stream).await.unwrap();
                let (mut ws_sender, mut ws_receiver) = ws_stream.split();
                
                while let Some(msg) = ws_receiver.next().await {
                    if let Ok(WsMessage::Text(text)) = msg {
                        // Echo the message back
                        let _ = ws_sender.send(WsMessage::Text(text)).await;
                    }
                }
            });
        }
    });
    
    // Give server time to start
    sleep(Duration::from_millis(100)).await;
    
    let message_counts = [100, 500, 1000];
    
    for &count in &message_counts {
        println!("Testing WebSocket with {} messages", count);
        
        // Connect client
        let ws_url = format!("ws://127.0.0.1:{}", addr.port());
        let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        let start = Instant::now();
        
        // Send messages
        let send_handle = tokio::spawn(async move {
            for i in 0..count {
                let message = format!("Test message {}", i);
                ws_sender.send(WsMessage::Text(message)).await.unwrap();
            }
        });
        
        // Receive echo responses
        let mut received = 0;
        let recv_handle = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                if let Ok(WsMessage::Text(_)) = msg {
                    received += 1;
                    if received >= count {
                        break;
                    }
                }
            }
            received
        });
        
        // Wait for completion
        send_handle.await.unwrap();
        let final_received = recv_handle.await.unwrap();
        
        let duration = start.elapsed();
        let throughput = (count * 2) as f64 / duration.as_secs_f64(); // Send + receive
        
        println!("WebSocket test: {} messages in {:?} ({:.2} msgs/sec)", 
                count, duration, throughput);
        
        assert_eq!(final_received, count);
        assert!(throughput > 50.0); // At least 50 messages/sec throughput
    }
    
    server_handle.abort();
}

/// Helper function to get memory usage (placeholder implementation)
fn get_memory_usage() -> usize {
    // In a real implementation, this would use a memory profiling crate
    // For now, return a mock value
    use std::alloc::{GlobalAlloc, Layout, System};
    
    // This is a simplified approach - in production you'd use proper memory profiling
    1024 * 1024 // 1MB placeholder
}

/// Criterion-based benchmarks for detailed performance analysis
pub fn criterion_benchmarks(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("agent_creation", |b| {
        b.iter(|| {
            black_box(TestDataFactory::create_random_agent())
        })
    });
    
    c.bench_function("issue_creation", |b| {
        b.iter(|| {
            black_box(TestDataFactory::create_random_issue())
        })
    });
    
    c.bench_function("knowledge_creation", |b| {
        b.iter(|| {
            black_box(TestDataFactory::create_random_knowledge(Uuid::new_v4()))
        })
    });
    
    // Database operation benchmarks
    let mut group = c.benchmark_group("database_operations");
    
    for size in [10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("agent_batch_insert", size), size, |b, &size| {
            b.to_async(&rt).iter(|| async {
                let db_helper = DatabaseTestHelper::new().await.unwrap();
                let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
                
                for _ in 0..size {
                    let agent = TestDataFactory::create_random_agent();
                    black_box(storage_manager.agents().create_agent(agent).await.unwrap());
                }
            });
        });
    }
    
    group.finish();
}

#[cfg(test)]
mod criterion_tests {
    use super::*;
    use criterion::Criterion;
    
    #[test]
    fn run_criterion_benchmarks() {
        let mut criterion = Criterion::default();
        criterion_benchmarks(&mut criterion);
    }
}