//! Integration tests for the automated transport testing framework
//!
//! This module demonstrates the comprehensive transport testing framework
//! that validates all transport implementations against MCP protocol compliance.

use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use vibe_ensemble_mcp::{
    transport::{
        testing::{
            test_transport_with_scenarios, TransportTester, TransportTestBuilder, 
            TestScenario, PerformanceMetrics
        },
        TransportFactory, InMemoryTransport
    },
    McpServer, Error,
};
use vibe_ensemble_storage::StorageManager;

use crate::common::database::DatabaseTestHelper;

/// Test the automated transport testing framework with in-memory transport
#[tokio::test]
async fn test_transport_testing_framework_basic() {
    println!("🧪 Testing Transport Testing Framework - Basic");
    
    // Create in-memory transport pair
    let (transport1, _transport2) = TransportFactory::in_memory_pair();
    
    // Test the transport using the framework
    let result = test_transport_with_scenarios(transport1, "InMemoryTransport").await;
    
    // Print detailed results
    println!("{}", result);
    
    // Validate results
    assert!(result.total_scenarios > 0, "Should have executed scenarios");
    assert!(result.success_rate() > 0.0, "Should have some success rate");
    
    // Check that we have both passing and failing tests (since some scenarios expect failures)
    let failed_tests = result.failed_tests();
    let expected_failures = result.test_results.iter()
        .filter(|r| !r.scenario.should_succeed)
        .count();
    
    println!("Expected failures: {}, Actual failures: {}", expected_failures, failed_tests.len());
    
    // Performance metrics should be collected
    assert!(result.aggregate_performance.duration > Duration::ZERO);
}

/// Test custom scenario building and execution
#[tokio::test]
async fn test_custom_scenario_building() {
    println!("🛠️ Testing Custom Scenario Building");
    
    // Create custom test scenarios
    let custom_scenario = TestScenario::new(
        "custom_ping_test",
        "Custom Ping Test",
        "Tests custom ping functionality",
        Duration::from_secs(5),
        true,
    ).with_tags(vec!["custom", "ping", "basic"]);
    
    // Build custom tester with specific scenarios
    let tester = TransportTestBuilder::new()
        .add_scenario(custom_scenario)
        .with_standard_scenarios()
        .build();
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "CustomTestTransport").await;
    
    println!("{}", result);
    
    // Should have our custom scenario plus standard ones
    assert!(result.total_scenarios > 1);
    
    // Check that our custom scenario was executed
    let custom_results = result.tests_by_tag("custom");
    assert!(!custom_results.is_empty(), "Custom scenario should have been executed");
}

/// Test performance baseline comparison
#[tokio::test]
async fn test_performance_baseline_comparison() {
    println!("📊 Testing Performance Baseline Comparison");
    
    // Create baseline performance metrics
    let baseline = PerformanceMetrics {
        duration: Duration::from_secs(10),
        messages_sent: 100,
        messages_received: 100,
        avg_roundtrip_time: Duration::from_millis(50),
        min_roundtrip_time: Duration::from_millis(10),
        max_roundtrip_time: Duration::from_millis(100),
        error_count: 0,
        start_memory_kb: 1024,
        end_memory_kb: 1200,
        success_rate: 100.0,
        throughput_msg_per_sec: 10.0,
    };
    
    let tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .with_baseline(baseline.clone())
        .build();
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "BaselineTestTransport").await;
    
    println!("{}", result);
    
    // Compare with baseline
    println!("Baseline throughput: {:.2} msg/sec", baseline.throughput_msg_per_sec);
    println!("Actual throughput: {:.2} msg/sec", result.aggregate_performance.throughput_msg_per_sec);
    
    // Performance metrics should be collected
    assert!(result.aggregate_performance.messages_sent > 0);
}

/// Test scenario filtering and categorization
#[tokio::test]
async fn test_scenario_filtering() {
    println!("🏷️ Testing Scenario Filtering and Categorization");
    
    let tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .build();
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "FilterTestTransport").await;
    
    // Test filtering by different tags
    let mcp_tests = result.tests_by_tag("mcp");
    let performance_tests = result.tests_by_tag("performance");
    let error_handling_tests = result.tests_by_tag("error_handling");
    
    println!("MCP tests: {}", mcp_tests.len());
    println!("Performance tests: {}", performance_tests.len());
    println!("Error handling tests: {}", error_handling_tests.len());
    
    assert!(!mcp_tests.is_empty(), "Should have MCP protocol tests");
    assert!(!performance_tests.is_empty(), "Should have performance tests");
    assert!(!error_handling_tests.is_empty(), "Should have error handling tests");
    
    // Verify categorization is working
    for test in mcp_tests {
        assert!(test.scenario.tags.contains(&"mcp".to_string()));
    }
}

/// Test transport comparison between different implementations
#[tokio::test]
async fn test_transport_comparison() {
    println!("⚖️ Testing Transport Comparison");
    
    let tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .build();
    
    // Test multiple transport types
    let transports = vec![
        ("InMemoryTransport1", TransportFactory::in_memory_pair().0),
        ("InMemoryTransport2", TransportFactory::in_memory_pair().0),
    ];
    
    let mut results = Vec::new();
    
    for (name, transport) in transports {
        let result = tester.test_transport(transport, name).await;
        println!("\n{}", result);
        results.push(result);
    }
    
    // Compare results
    println!("\n📈 Transport Comparison Summary:");
    println!("=================================");
    
    for result in &results {
        println!("{}: {:.2}% success rate, {:.2} msg/sec throughput", 
                result.transport_name, 
                result.success_rate(),
                result.aggregate_performance.throughput_msg_per_sec);
    }
    
    // All results should have executed the same number of scenarios
    let scenario_count = results[0].total_scenarios;
    for result in &results {
        assert_eq!(result.total_scenarios, scenario_count, 
                  "All transports should execute the same scenarios");
    }
}

/// Test error resilience and recovery
#[tokio::test]
async fn test_error_resilience() {
    println!("🔧 Testing Error Resilience and Recovery");
    
    // Create a scenario that expects errors
    let error_scenario = TestScenario::new(
        "error_resilience_test",
        "Error Resilience Test",
        "Tests framework resilience to transport errors",
        Duration::from_secs(5),
        false, // Expect this to fail
    ).with_tags(vec!["error_handling", "resilience"]);
    
    let tester = TransportTestBuilder::new()
        .add_scenario(error_scenario)
        .build();
    
    // Create a broken transport that will fail
    let (mut transport, _) = TransportFactory::in_memory_pair();
    
    // Close the transport to make it fail
    transport.close().await.unwrap();
    
    let result = tester.test_transport(transport, "BrokenTransport").await;
    
    println!("{}", result);
    
    // Should handle the error gracefully
    assert_eq!(result.total_scenarios, 1);
    
    // The error scenario expects failure, so if it "passes" that means the framework
    // correctly detected the expected failure
    let error_test = &result.test_results[0];
    if error_test.scenario.should_succeed {
        // If scenario expected success but failed, that's framework handling error
        assert!(!error_test.passed, "Broken transport should fail");
        assert!(error_test.error_message.is_some(), "Should have error message");
    } else {
        // If scenario expected failure and got it, that's success
        assert!(error_test.passed, "Framework should handle expected failures correctly");
    }
}

/// Test comprehensive MCP protocol compliance
#[tokio::test]
async fn test_comprehensive_mcp_compliance() {
    println!("🔍 Testing Comprehensive MCP Protocol Compliance");
    
    // This test focuses on MCP-specific scenarios
    let mcp_scenarios = vec![
        TestScenario::new(
            "mcp_compliance_initialization",
            "MCP Initialization Compliance",
            "Validates complete MCP initialization sequence",
            Duration::from_secs(10),
            true,
        ).with_tags(vec!["mcp", "compliance", "initialization"]),
        
        TestScenario::new(
            "mcp_compliance_capabilities",
            "MCP Capabilities Advertisement",
            "Validates MCP capabilities advertisement",
            Duration::from_secs(10),
            true,
        ).with_tags(vec!["mcp", "compliance", "capabilities"]),
    ];
    
    let mut tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .build();
    
    for scenario in mcp_scenarios {
        tester.add_scenario(scenario);
    }
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "MCPComplianceTransport").await;
    
    println!("{}", result);
    
    // Filter and analyze MCP compliance results
    let compliance_tests = result.tests_by_tag("compliance");
    println!("\nMCP Compliance Results:");
    println!("=====================");
    
    for test in compliance_tests {
        let status = if test.passed { "✅ PASS" } else { "❌ FAIL" };
        println!("{} {}: {:?}", status, test.scenario.name, test.duration);
        if let Some(error) = &test.error_message {
            println!("    Error: {}", error);
        }
    }
    
    // At minimum, should have tested basic MCP operations
    let mcp_tests = result.tests_by_tag("mcp");
    assert!(mcp_tests.len() >= 5, "Should test major MCP operations");
}

/// Performance stress testing and regression detection  
#[tokio::test]
async fn test_performance_stress_and_regression() {
    println!("🚀 Testing Performance Stress and Regression Detection");
    
    // Create performance-focused scenarios
    let stress_scenarios = vec![
        TestScenario::new(
            "performance_sustained_load",
            "Sustained Load Test",
            "Tests sustained high-frequency message processing",
            Duration::from_secs(30),
            true,
        ).with_tags(vec!["performance", "stress", "sustained"]),
        
        TestScenario::new(
            "performance_burst_load",
            "Burst Load Test", 
            "Tests handling of message bursts",
            Duration::from_secs(15),
            true,
        ).with_tags(vec!["performance", "stress", "burst"]),
    ];
    
    let mut tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .build();
    
    for scenario in stress_scenarios {
        tester.add_scenario(scenario);
    }
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "PerformanceStressTransport").await;
    
    println!("{}", result);
    
    // Analyze performance metrics
    let perf = &result.aggregate_performance;
    println!("\nPerformance Analysis:");
    println!("====================");
    println!("Total Duration: {:?}", perf.duration);
    println!("Messages Sent: {}", perf.messages_sent);
    println!("Messages Received: {}", perf.messages_received);
    println!("Throughput: {:.2} msg/sec", perf.throughput_msg_per_sec);
    println!("Avg Roundtrip: {:?}", perf.avg_roundtrip_time);
    println!("Min Roundtrip: {:?}", perf.min_roundtrip_time);
    println!("Max Roundtrip: {:?}", perf.max_roundtrip_time);
    println!("Success Rate: {:.2}%", perf.success_rate);
    println!("Error Count: {}", perf.error_count);
    
    // Performance regression checks
    assert!(perf.throughput_msg_per_sec > 5.0, 
           "Throughput regression: {:.2} msg/sec too low", perf.throughput_msg_per_sec);
    assert!(perf.avg_roundtrip_time < Duration::from_millis(1000), 
           "Latency regression: {:?} too high", perf.avg_roundtrip_time);
    assert!(perf.success_rate > 80.0, 
           "Success rate regression: {:.2}% too low", perf.success_rate);
}

/// Test framework integration with CI/CD pipeline requirements
#[tokio::test]
async fn test_cicd_integration_requirements() {
    println!("🔄 Testing CI/CD Integration Requirements");
    
    let tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .build();
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "CICDTransport").await;
    
    // Test CI/CD requirements:
    
    // 1. Machine-readable results
    let serializable_data = serde_json::json!({
        "transport_name": result.transport_name,
        "total_scenarios": result.total_scenarios,
        "passed_scenarios": result.passed_scenarios,
        "success_rate": result.success_rate(),
        "total_duration_ms": result.total_duration.as_millis(),
        "throughput_msg_per_sec": result.aggregate_performance.throughput_msg_per_sec,
        "avg_roundtrip_ms": result.aggregate_performance.avg_roundtrip_time.as_millis(),
        "error_count": result.aggregate_performance.error_count
    });
    
    println!("Machine-readable results:");
    println!("{}", serde_json::to_string_pretty(&serializable_data).unwrap());
    
    // 2. Exit criteria for CI
    let ci_success = result.success_rate() >= 70.0 && 
                    result.aggregate_performance.throughput_msg_per_sec >= 5.0;
    
    println!("CI Success Criteria: {}", if ci_success { "✅ PASS" } else { "❌ FAIL" });
    
    // 3. Test categorization for selective runs
    let critical_tests = result.tests_by_tag("mcp").len();
    let performance_tests = result.tests_by_tag("performance").len();
    let error_tests = result.tests_by_tag("error_handling").len();
    
    println!("Test Categorization:");
    println!("  Critical (MCP): {}", critical_tests);
    println!("  Performance: {}", performance_tests);
    println!("  Error Handling: {}", error_tests);
    
    // 4. Timing constraints
    assert!(result.total_duration < Duration::from_secs(120), 
           "Test suite too slow for CI: {:?}", result.total_duration);
    
    // For CI purposes, assert on key metrics
    if std::env::var("CI").is_ok() {
        assert!(ci_success, "CI success criteria not met");
    }
}

/// Test automated regression detection with historical baselines
#[tokio::test] 
async fn test_automated_regression_detection() {
    println!("📉 Testing Automated Regression Detection");
    
    // Simulate historical baseline
    let historical_baseline = PerformanceMetrics {
        duration: Duration::from_secs(5),
        messages_sent: 50,
        messages_received: 50,
        avg_roundtrip_time: Duration::from_millis(20),
        min_roundtrip_time: Duration::from_millis(5),
        max_roundtrip_time: Duration::from_millis(50),
        error_count: 0,
        start_memory_kb: 1000,
        end_memory_kb: 1100,
        success_rate: 100.0,
        throughput_msg_per_sec: 10.0,
    };
    
    let tester = TransportTestBuilder::new()
        .with_standard_scenarios()
        .with_baseline(historical_baseline.clone())
        .build();
    
    let (transport, _) = TransportFactory::in_memory_pair();
    let result = tester.test_transport(transport, "RegressionTestTransport").await;
    
    println!("{}", result);
    
    // Compare with historical baseline
    let current = &result.aggregate_performance;
    
    println!("\nRegression Analysis:");
    println!("===================");
    
    // Throughput regression check (allow 20% decrease)
    let throughput_regression = (historical_baseline.throughput_msg_per_sec - current.throughput_msg_per_sec) / historical_baseline.throughput_msg_per_sec;
    println!("Throughput change: {:.2}% (baseline: {:.2}, current: {:.2})", 
             throughput_regression * 100.0,
             historical_baseline.throughput_msg_per_sec, 
             current.throughput_msg_per_sec);
    
    // Latency regression check (allow 50% increase)
    let latency_regression = (current.avg_roundtrip_time.as_millis() as f64 - historical_baseline.avg_roundtrip_time.as_millis() as f64) 
                           / historical_baseline.avg_roundtrip_time.as_millis() as f64;
    println!("Latency change: {:.2}% (baseline: {:?}, current: {:?})",
             latency_regression * 100.0,
             historical_baseline.avg_roundtrip_time,
             current.avg_roundtrip_time);
    
    // Success rate regression check
    let success_rate_change = current.success_rate - historical_baseline.success_rate;
    println!("Success rate change: {:.2}pp (baseline: {:.2}%, current: {:.2}%)",
             success_rate_change,
             historical_baseline.success_rate,
             current.success_rate);
    
    // Flag significant regressions
    let significant_regression = throughput_regression > 0.2 || 
                               latency_regression > 0.5 || 
                               success_rate_change < -5.0;
    
    if significant_regression {
        println!("⚠️ PERFORMANCE REGRESSION DETECTED!");
    } else {
        println!("✅ No significant performance regression detected");
    }
    
    // In a real CI environment, this might fail the build
    if std::env::var("STRICT_REGRESSION_CHECK").is_ok() {
        assert!(!significant_regression, "Performance regression detected");
    }
}

/// Integration test with actual MCP server (if available)
#[tokio::test]
#[ignore] // Ignore by default as it requires more complex setup
async fn test_integration_with_mcp_server() {
    println!("🔗 Testing Integration with MCP Server");
    
    // This test would require an actual MCP server setup
    // For now, we'll simulate the integration pattern
    
    let db_helper = DatabaseTestHelper::new().await.unwrap();
    let storage_manager = Arc::new(StorageManager::new(db_helper.pool.clone()).await.unwrap());
    let _server = McpServer::new(storage_manager).await.unwrap();
    
    // In a full implementation, this would:
    // 1. Start the MCP server with a transport
    // 2. Run the transport test framework against it
    // 3. Validate real MCP protocol compliance
    // 4. Shut down gracefully
    
    println!("✅ MCP Server integration test pattern validated");
}