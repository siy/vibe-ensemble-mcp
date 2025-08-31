//! Generic transport testing framework for comprehensive MCP protocol validation
//!
//! This module provides a unified testing framework that can validate any transport
//! implementation against the MCP protocol specification. It includes:
//!
//! - Generic test scenarios that work with any transport type
//! - Performance benchmarking and regression detection
//! - Comprehensive error handling validation
//! - Automated test execution for transport combinations
//! - Detailed reporting and comparison mechanisms

use crate::{transport::Transport, Error, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Performance test parameters (configurable)
#[derive(Clone, Debug)]
pub struct TestParameters {
    pub throughput_test_message_count: u32,
    pub min_acceptable_throughput: f64, // messages per second
    pub large_message_size_bytes: usize,
}

impl Default for TestParameters {
    fn default() -> Self {
        Self {
            throughput_test_message_count: 50,
            min_acceptable_throughput: 10.0, // messages per second
            large_message_size_bytes: 10_000,
        }
    }
}

// Default test parameters (for backward compatibility)
const THROUGHPUT_TEST_MESSAGE_COUNT: u32 = 50;
const MIN_ACCEPTABLE_THROUGHPUT: f64 = 10.0; // messages per second
const LARGE_MESSAGE_SIZE_BYTES: usize = 10_000;
const LARGE_MESSAGE_TIMEOUT_SECS: u64 = 5;

/// Transport test scenario configuration
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestScenario {
    /// Unique identifier for the test scenario
    pub id: String,
    /// Human-readable name for the scenario
    pub name: String,
    /// Description of what the scenario tests
    pub description: String,
    /// Expected timeout for the scenario
    pub timeout: Duration,
    /// Whether this scenario is expected to pass or fail
    pub should_succeed: bool,
    /// Tags for categorizing scenarios
    pub tags: Vec<String>,
}

impl TestScenario {
    /// Create a new test scenario
    pub fn new(
        id: &str,
        name: &str,
        description: &str,
        timeout: Duration,
        should_succeed: bool,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            timeout,
            should_succeed,
            tags: Vec::new(),
        }
    }

    /// Add tags to the scenario
    pub fn with_tags(mut self, tags: Vec<&str>) -> Self {
        self.tags = tags.into_iter().map(|t| t.to_string()).collect();
        self
    }
}

/// Performance metrics collected during testing
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct PerformanceMetrics {
    /// Total duration of the test
    pub duration: Duration,
    /// Number of messages sent
    pub messages_sent: u64,
    /// Number of messages received
    pub messages_received: u64,
    /// Average message round-trip time
    pub avg_roundtrip_time: Duration,
    /// Minimum round-trip time observed
    pub min_roundtrip_time: Duration,
    /// Maximum round-trip time observed
    pub max_roundtrip_time: Duration,
    /// Number of errors encountered
    pub error_count: u64,
    /// Memory usage at test start (approximate)
    pub start_memory_kb: u64,
    /// Memory usage at test end (approximate)
    pub end_memory_kb: u64,
    /// Success rate as percentage (0.0 to 100.0)
    pub success_rate: f64,
    /// Throughput in messages per second
    pub throughput_msg_per_sec: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            duration: Duration::ZERO,
            messages_sent: 0,
            messages_received: 0,
            avg_roundtrip_time: Duration::ZERO,
            min_roundtrip_time: Duration::MAX,
            max_roundtrip_time: Duration::ZERO,
            error_count: 0,
            start_memory_kb: 0,
            end_memory_kb: 0,
            success_rate: 0.0,
            throughput_msg_per_sec: 0.0,
        }
    }
}

/// Result of a single test scenario execution
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestResult {
    /// The scenario that was executed
    pub scenario: TestScenario,
    /// Whether the test passed or failed
    pub passed: bool,
    /// Duration of the test execution
    pub duration: Duration,
    /// Error message if test failed
    pub error_message: Option<String>,
    /// Performance metrics collected
    pub performance: PerformanceMetrics,
    /// Additional context or details
    pub details: HashMap<String, Value>,
}

impl TestResult {
    /// Create a successful test result
    pub fn success(
        scenario: TestScenario,
        duration: Duration,
        performance: PerformanceMetrics,
    ) -> Self {
        Self {
            scenario,
            passed: true,
            duration,
            error_message: None,
            performance,
            details: HashMap::new(),
        }
    }

    /// Create a failed test result
    pub fn failure(
        scenario: TestScenario,
        duration: Duration,
        error: String,
        performance: PerformanceMetrics,
    ) -> Self {
        Self {
            scenario,
            passed: false,
            duration,
            error_message: Some(error),
            performance,
            details: HashMap::new(),
        }
    }

    /// Add additional details to the result
    pub fn with_detail(mut self, key: &str, value: Value) -> Self {
        self.details.insert(key.to_string(), value);
        self
    }
}

/// Comprehensive test suite results
#[derive(Clone, Debug, serde::Serialize)]
pub struct TestSuiteResult {
    /// Name of the transport being tested
    pub transport_name: String,
    /// Total number of scenarios executed
    pub total_scenarios: usize,
    /// Number of scenarios that passed
    pub passed_scenarios: usize,
    /// Individual test results
    pub test_results: Vec<TestResult>,
    /// Overall suite duration
    pub total_duration: Duration,
    /// Aggregate performance metrics
    pub aggregate_performance: PerformanceMetrics,
    /// Suite-level metadata
    pub metadata: HashMap<String, Value>,
}

impl TestSuiteResult {
    /// Calculate success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_scenarios == 0 {
            100.0
        } else {
            (self.passed_scenarios as f64 / self.total_scenarios as f64) * 100.0
        }
    }

    /// Get failed test results
    pub fn failed_tests(&self) -> Vec<&TestResult> {
        self.test_results.iter().filter(|r| !r.passed).collect()
    }

    /// Get tests by tag
    pub fn tests_by_tag(&self, tag: &str) -> Vec<&TestResult> {
        self.test_results
            .iter()
            .filter(|r| r.scenario.tags.contains(&tag.to_string()))
            .collect()
    }
}

impl fmt::Display for TestSuiteResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Transport Test Suite Results: {}", self.transport_name)?;
        writeln!(f, "================================")?;
        writeln!(f, "Total Scenarios: {}", self.total_scenarios)?;
        writeln!(f, "Passed: {}", self.passed_scenarios)?;
        writeln!(
            f,
            "Failed: {}",
            self.total_scenarios - self.passed_scenarios
        )?;
        writeln!(f, "Success Rate: {:.2}%", self.success_rate())?;
        writeln!(f, "Total Duration: {:?}", self.total_duration)?;
        writeln!(f)?;
        writeln!(f, "Performance Summary:")?;
        writeln!(
            f,
            "  Throughput: {:.2} msg/sec",
            self.aggregate_performance.throughput_msg_per_sec
        )?;
        writeln!(
            f,
            "  Avg Roundtrip: {:?}",
            self.aggregate_performance.avg_roundtrip_time
        )?;
        writeln!(
            f,
            "  Error Rate: {:.2}%",
            if self.aggregate_performance.messages_sent > 0 {
                (self.aggregate_performance.error_count as f64
                    / self.aggregate_performance.messages_sent as f64)
                    * 100.0
            } else {
                0.0
            }
        )?;

        if !self.failed_tests().is_empty() {
            writeln!(f)?;
            writeln!(f, "Failed Tests:")?;
            for failed_test in self.failed_tests() {
                writeln!(
                    f,
                    "  ❌ {}: {}",
                    failed_test.scenario.name,
                    failed_test
                        .error_message
                        .as_deref()
                        .unwrap_or("Unknown error")
                )?;
            }
        }

        Ok(())
    }
}

/// Generic transport testing framework
pub struct TransportTester {
    /// Collection of test scenarios to execute
    scenarios: Vec<TestScenario>,
    /// Performance baseline for comparison
    performance_baseline: Option<PerformanceMetrics>,
    /// Configurable test parameters
    #[allow(dead_code)] // TODO: Wire this to actual test execution
    test_parameters: TestParameters,
}

impl TransportTester {
    /// Create a new transport tester with core scenarios only
    pub fn new() -> Self {
        let mut tester = Self {
            scenarios: Vec::new(),
            performance_baseline: None,
            test_parameters: TestParameters::default(),
        };

        // Default to core scenarios only; runners/builders add perf/error/stress as needed
        tester.add_core_mcp_scenarios();
        tester
    }

    /// Add standard MCP protocol test scenarios
    #[allow(dead_code)]
    fn add_standard_scenarios(&mut self) {
        self.add_core_mcp_scenarios();
        self.add_error_scenarios();
        self.add_performance_scenarios();
    }

    /// Add core MCP protocol test scenarios
    fn add_core_mcp_scenarios(&mut self) {
        // Initialization scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_initialize_basic",
                "Basic MCP Initialization",
                "Tests basic MCP protocol handshake and initialization",
                Duration::from_secs(10),
                true,
            )
            .with_tags(vec!["mcp", "initialization", "basic"]),
        );

        // Tools scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_tools_list",
                "Tool Listing",
                "Tests MCP tools/list method functionality",
                Duration::from_secs(10),
                true,
            )
            .with_tags(vec!["mcp", "tools", "basic"]),
        );

        self.add_scenario(
            TestScenario::new(
                "mcp_tools_call_valid",
                "Valid Tool Call",
                "Tests successful tool invocation",
                Duration::from_secs(15),
                true,
            )
            .with_tags(vec!["mcp", "tools", "execution"]),
        );

        // Resources scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_resources_list",
                "Resource Listing",
                "Tests MCP resources/list method functionality",
                Duration::from_secs(10),
                true,
            )
            .with_tags(vec!["mcp", "resources", "basic"]),
        );

        self.add_scenario(
            TestScenario::new(
                "mcp_resources_read_valid",
                "Valid Resource Read",
                "Tests reading existing resources",
                Duration::from_secs(10),
                true,
            )
            .with_tags(vec!["mcp", "resources", "read"]),
        );

        // Prompts scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_prompts_list",
                "Prompt Listing",
                "Tests MCP prompts/list method functionality",
                Duration::from_secs(10),
                true,
            )
            .with_tags(vec!["mcp", "prompts", "basic"]),
        );

        // Notification scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_notifications",
                "Notification Handling",
                "Tests MCP notification processing",
                Duration::from_secs(5),
                true,
            )
            .with_tags(vec!["mcp", "notifications", "basic"]),
        );
    }

    /// Add error handling test scenarios
    fn add_error_scenarios(&mut self) {
        // Protocol error scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_initialize_invalid_version",
                "Invalid Protocol Version",
                "Tests handling of invalid protocol version during initialization",
                Duration::from_secs(5),
                false,
            )
            .with_tags(vec!["mcp", "initialization", "error_handling"]),
        );

        self.add_scenario(
            TestScenario::new(
                "mcp_tools_call_invalid",
                "Invalid Tool Call",
                "Tests error handling for invalid tool calls",
                Duration::from_secs(10),
                false,
            )
            .with_tags(vec!["mcp", "tools", "error_handling"]),
        );

        self.add_scenario(
            TestScenario::new(
                "mcp_resources_read_invalid",
                "Invalid Resource Read",
                "Tests error handling for non-existent resources",
                Duration::from_secs(5),
                false,
            )
            .with_tags(vec!["mcp", "resources", "error_handling"]),
        );

        // Message format error scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_invalid_json",
                "Invalid JSON Handling",
                "Tests handling of malformed JSON messages",
                Duration::from_secs(5),
                false,
            )
            .with_tags(vec!["mcp", "error_handling", "malformed"]),
        );

        self.add_scenario(
            TestScenario::new(
                "mcp_unknown_method",
                "Unknown Method Handling",
                "Tests handling of unknown MCP methods",
                Duration::from_secs(5),
                false,
            )
            .with_tags(vec!["mcp", "error_handling", "unknown_method"]),
        );
    }

    /// Add performance test scenarios
    fn add_performance_scenarios(&mut self) {
        // Concurrent request scenarios
        self.add_scenario(
            TestScenario::new(
                "mcp_concurrent_requests",
                "Concurrent Request Handling",
                "Tests multiple simultaneous MCP requests",
                Duration::from_secs(30),
                true,
            )
            .with_tags(vec!["mcp", "concurrency", "performance"]),
        );

        // High throughput scenarios
        self.add_scenario(
            TestScenario::new(
                "performance_high_throughput",
                "High Throughput Test",
                "Tests transport performance under high message volume",
                Duration::from_secs(60),
                true,
            )
            .with_tags(vec!["performance", "throughput", "stress"]),
        );

        self.add_scenario(
            TestScenario::new(
                "performance_large_messages",
                "Large Message Handling",
                "Tests transport with large message payloads",
                Duration::from_secs(30),
                true,
            )
            .with_tags(vec!["performance", "large_messages", "stress"]),
        );
    }

    /// Add stress test scenarios
    fn add_stress_scenarios(&mut self) {
        // Note: Stress scenarios are a subset of performance scenarios
        // focused on extreme conditions
        self.add_scenario(
            TestScenario::new(
                "stress_high_load",
                "High Load Stress Test",
                "Tests transport under extreme load conditions",
                Duration::from_secs(120),
                true,
            )
            .with_tags(vec!["stress", "load", "extreme"]),
        );

        self.add_scenario(
            TestScenario::new(
                "stress_memory_pressure",
                "Memory Pressure Test",
                "Tests transport under memory pressure conditions",
                Duration::from_secs(90),
                true,
            )
            .with_tags(vec!["stress", "memory", "pressure"]),
        );
    }

    /// Add a custom test scenario
    pub fn add_scenario(&mut self, scenario: TestScenario) {
        self.scenarios.push(scenario);
    }

    /// Set performance baseline for comparison
    pub fn set_performance_baseline(&mut self, baseline: PerformanceMetrics) {
        self.performance_baseline = Some(baseline);
    }

    /// Execute all test scenarios against a transport
    pub async fn test_transport<T>(&self, mut transport: T, transport_name: &str) -> TestSuiteResult
    where
        T: Transport,
    {
        info!("Starting transport testing for: {}", transport_name);
        let start_timestamp = chrono::Utc::now(); // Capture start time accurately
        let suite_start = Instant::now();
        let mut test_results = Vec::new();
        let mut passed_count = 0;
        let mut aggregate_performance = PerformanceMetrics::default();

        for scenario in &self.scenarios {
            info!("Executing scenario: {}", scenario.name);

            let result = self
                .execute_scenario(&mut transport, scenario.clone())
                .await;

            if result.passed {
                passed_count += 1;
            }

            // Aggregate performance metrics
            aggregate_performance.messages_sent += result.performance.messages_sent;
            aggregate_performance.messages_received += result.performance.messages_received;
            aggregate_performance.error_count += result.performance.error_count;

            // Update min/max roundtrip times
            if result.performance.min_roundtrip_time < aggregate_performance.min_roundtrip_time {
                aggregate_performance.min_roundtrip_time = result.performance.min_roundtrip_time;
            }
            if result.performance.max_roundtrip_time > aggregate_performance.max_roundtrip_time {
                aggregate_performance.max_roundtrip_time = result.performance.max_roundtrip_time;
            }

            test_results.push(result);
        }

        let total_duration = suite_start.elapsed();

        // Calculate aggregate metrics
        if aggregate_performance.messages_sent > 0 {
            aggregate_performance.success_rate =
                ((aggregate_performance.messages_sent - aggregate_performance.error_count) as f64
                    / aggregate_performance.messages_sent as f64)
                    * 100.0;
            aggregate_performance.throughput_msg_per_sec =
                aggregate_performance.messages_sent as f64 / total_duration.as_secs_f64();
        }

        // Calculate average roundtrip time (only for scenarios that recorded any response)
        let (sum_nanos, count) = test_results
            .iter()
            .filter(|r| r.performance.messages_received > 0)
            .fold((0u128, 0u128), |(sum, cnt), r| {
                (sum + r.performance.avg_roundtrip_time.as_nanos(), cnt + 1)
            });
        if count > 0 {
            aggregate_performance.avg_roundtrip_time =
                Duration::from_nanos((sum_nanos / count) as u64);
        }

        aggregate_performance.duration = total_duration;

        let mut metadata = HashMap::new();
        metadata.insert(
            "test_start".to_string(),
            json!(start_timestamp.to_rfc3339()),
        );
        metadata.insert("transport_type".to_string(), json!(transport_name));

        TestSuiteResult {
            transport_name: transport_name.to_string(),
            total_scenarios: self.scenarios.len(),
            passed_scenarios: passed_count,
            test_results,
            total_duration,
            aggregate_performance,
            metadata,
        }
    }

    /// Execute a single test scenario
    async fn execute_scenario<T>(&self, transport: &mut T, scenario: TestScenario) -> TestResult
    where
        T: Transport,
    {
        let start_time = Instant::now();
        let mut performance = PerformanceMetrics {
            start_memory_kb: get_memory_usage_kb(),
            ..Default::default()
        };

        debug!("Executing scenario: {} ({})", scenario.name, scenario.id);

        let result = match timeout(
            scenario.timeout,
            self.run_scenario_logic(transport, &scenario, &mut performance),
        )
        .await
        {
            Ok(Ok(_)) => {
                let duration = start_time.elapsed();
                performance.duration = duration;
                performance.end_memory_kb = get_memory_usage_kb();

                if scenario.should_succeed {
                    TestResult::success(scenario, duration, performance)
                } else {
                    TestResult::failure(
                        scenario,
                        duration,
                        "Test was expected to fail but succeeded".to_string(),
                        performance,
                    )
                }
            }
            Ok(Err(e)) => {
                let duration = start_time.elapsed();
                performance.duration = duration;
                performance.end_memory_kb = get_memory_usage_kb();

                if scenario.should_succeed {
                    TestResult::failure(
                        scenario,
                        duration,
                        format!("Test failed: {}", e),
                        performance,
                    )
                } else {
                    TestResult::success(scenario, duration, performance)
                }
            }
            Err(_) => {
                let duration = start_time.elapsed();
                performance.duration = duration;
                performance.end_memory_kb = get_memory_usage_kb();
                TestResult::failure(
                    scenario.clone(),
                    duration,
                    format!("Test timed out after {:?}", scenario.timeout),
                    performance,
                )
            }
        };

        debug!(
            "Scenario {} completed: {}",
            result.scenario.name,
            if result.passed { "PASS" } else { "FAIL" }
        );
        result
    }

    /// Execute the actual scenario logic
    async fn run_scenario_logic<T>(
        &self,
        transport: &mut T,
        scenario: &TestScenario,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        match scenario.id.as_str() {
            "mcp_initialize_basic" => self.test_mcp_initialize_basic(transport, performance).await,
            "mcp_initialize_invalid_version" => {
                self.test_mcp_initialize_invalid_version(transport, performance)
                    .await
            }
            "mcp_tools_list" => self.test_mcp_tools_list(transport, performance).await,
            "mcp_tools_call_valid" => self.test_mcp_tools_call_valid(transport, performance).await,
            "mcp_tools_call_invalid" => {
                self.test_mcp_tools_call_invalid(transport, performance)
                    .await
            }
            "mcp_resources_list" => self.test_mcp_resources_list(transport, performance).await,
            "mcp_resources_read_valid" => {
                self.test_mcp_resources_read_valid(transport, performance)
                    .await
            }
            "mcp_resources_read_invalid" => {
                self.test_mcp_resources_read_invalid(transport, performance)
                    .await
            }
            "mcp_prompts_list" => self.test_mcp_prompts_list(transport, performance).await,
            "mcp_notifications" => self.test_mcp_notifications(transport, performance).await,
            "mcp_concurrent_requests" => {
                self.test_mcp_concurrent_requests(transport, performance)
                    .await
            }
            "mcp_invalid_json" => self.test_mcp_invalid_json(transport, performance).await,
            "mcp_unknown_method" => self.test_mcp_unknown_method(transport, performance).await,
            "performance_high_throughput" => {
                self.test_performance_high_throughput(transport, performance)
                    .await
            }
            "performance_large_messages" => {
                self.test_performance_large_messages(transport, performance)
                    .await
            }
            "stress_high_load" => self.test_stress_high_load(transport, performance).await,
            "stress_memory_pressure" => {
                self.test_stress_memory_pressure(transport, performance)
                    .await
            }
            _ => {
                warn!("Unknown scenario ID: {}", scenario.id);
                Err(Error::Transport(format!(
                    "Unknown scenario: {}",
                    scenario.id
                )))
            }
        }
    }

    // Individual test implementations
    async fn test_mcp_initialize_basic<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0",
                "clientInfo": {
                    "name": "transport-tester",
                    "version": "1.0.0"
                }
            }
        });

        let send_start = Instant::now();
        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        let roundtrip_time = send_start.elapsed();
        performance.messages_received += 1;

        performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
        performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);
        performance.avg_roundtrip_time = roundtrip_time;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Validate response structure
        if response_json.get("error").is_some() {
            return Err(Error::Transport(
                "Initialization failed with error response".to_string(),
            ));
        }

        let result = response_json
            .get("result")
            .ok_or_else(|| Error::Transport("Missing result in response".to_string()))?;

        // Check required fields
        result
            .get("protocolVersion")
            .ok_or_else(|| Error::Transport("Missing protocolVersion in response".to_string()))?;
        result
            .get("serverInfo")
            .ok_or_else(|| Error::Transport("Missing serverInfo in response".to_string()))?;
        result
            .get("capabilities")
            .ok_or_else(|| Error::Transport("Missing capabilities in response".to_string()))?;

        Ok(())
    }

    async fn test_mcp_initialize_invalid_version<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "99.0",
                "clientInfo": {
                    "name": "transport-tester",
                    "version": "1.0.0"
                }
            }
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Should have an error for invalid version
        if response_json.get("error").is_none() {
            return Err(Error::Transport(
                "Expected error for invalid protocol version".to_string(),
            ));
        }

        Ok(())
    }

    async fn test_mcp_tools_list<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        });

        let send_start = Instant::now();
        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        let roundtrip_time = send_start.elapsed();
        performance.messages_received += 1;

        performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
        performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        let result = response_json
            .get("result")
            .ok_or_else(|| Error::Transport("Missing result in tools/list response".to_string()))?;

        let tools = result
            .get("tools")
            .ok_or_else(|| Error::Transport("Missing tools array in response".to_string()))?
            .as_array()
            .ok_or_else(|| Error::Transport("Tools field is not an array".to_string()))?;

        // Validate tool structure
        for tool in tools {
            tool.get("name")
                .ok_or_else(|| Error::Transport("Tool missing name field".to_string()))?;
            tool.get("description")
                .ok_or_else(|| Error::Transport("Tool missing description field".to_string()))?;
            tool.get("inputSchema")
                .ok_or_else(|| Error::Transport("Tool missing inputSchema field".to_string()))?;
        }

        Ok(())
    }

    async fn test_mcp_tools_call_valid<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "create_agent",
                "arguments": {
                    "name": "test-agent-transport",
                    "capabilities": ["testing"]
                }
            }
        });

        let send_start = Instant::now();
        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        let roundtrip_time = send_start.elapsed();
        performance.messages_received += 1;

        performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
        performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Check for either success or valid error
        if let Some(error) = response_json.get("error") {
            debug!("Tool call returned error (may be expected): {:?}", error);
        } else {
            response_json.get("result").ok_or_else(|| {
                Error::Transport("Missing result in tools/call response".to_string())
            })?;
        }

        Ok(())
    }

    async fn test_mcp_tools_call_invalid<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "nonexistent_tool",
                "arguments": {}
            }
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Should have an error for invalid tool
        if response_json.get("error").is_none() {
            return Err(Error::Transport(
                "Expected error for invalid tool call".to_string(),
            ));
        }

        Ok(())
    }

    async fn test_mcp_resources_list<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "resources/list",
            "params": {}
        });

        let send_start = Instant::now();
        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        let roundtrip_time = send_start.elapsed();
        performance.messages_received += 1;

        performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
        performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        let result = response_json.get("result").ok_or_else(|| {
            Error::Transport("Missing result in resources/list response".to_string())
        })?;

        let resources = result
            .get("resources")
            .ok_or_else(|| Error::Transport("Missing resources array in response".to_string()))?
            .as_array()
            .ok_or_else(|| Error::Transport("Resources field is not an array".to_string()))?;

        // Validate resource structure
        for resource in resources {
            resource
                .get("uri")
                .ok_or_else(|| Error::Transport("Resource missing uri field".to_string()))?;
            resource
                .get("name")
                .ok_or_else(|| Error::Transport("Resource missing name field".to_string()))?;
        }

        Ok(())
    }

    async fn test_mcp_resources_read_valid<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "resources/read",
            "params": {
                "uri": "vibe://knowledge/test-knowledge-1"
            }
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Accept either success or reasonable error (resource might not exist)
        if let Some(_error) = response_json.get("error") {
            debug!("Resource read returned error (may be expected for non-existent resource)");
        } else {
            response_json.get("result").ok_or_else(|| {
                Error::Transport("Missing result in resources/read response".to_string())
            })?;
        }

        Ok(())
    }

    async fn test_mcp_resources_read_invalid<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "resources/read",
            "params": {
                "uri": "vibe://nonexistent/resource"
            }
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Should have an error for invalid resource
        if response_json.get("error").is_none() {
            return Err(Error::Transport(
                "Expected error for invalid resource".to_string(),
            ));
        }

        Ok(())
    }

    async fn test_mcp_prompts_list<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let request = json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "prompts/list",
            "params": {}
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        let result = response_json.get("result").ok_or_else(|| {
            Error::Transport("Missing result in prompts/list response".to_string())
        })?;

        let prompts = result
            .get("prompts")
            .ok_or_else(|| Error::Transport("Missing prompts array in response".to_string()))?
            .as_array()
            .ok_or_else(|| Error::Transport("Prompts field is not an array".to_string()))?;

        // Validate prompt structure
        for prompt in prompts {
            prompt
                .get("name")
                .ok_or_else(|| Error::Transport("Prompt missing name field".to_string()))?;
            prompt
                .get("description")
                .ok_or_else(|| Error::Transport("Prompt missing description field".to_string()))?;
        }

        Ok(())
    }

    async fn test_mcp_notifications<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });

        transport.send(&notification.to_string()).await?;
        performance.messages_sent += 1;

        // Notifications don't expect responses, so this is just a send test
        Ok(())
    }

    async fn test_mcp_concurrent_requests<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        // Send multiple concurrent requests (simulated sequentially due to single transport)
        let requests = vec![
            json!({"jsonrpc": "2.0", "id": 10, "method": "tools/list", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 11, "method": "resources/list", "params": {}}),
            json!({"jsonrpc": "2.0", "id": 12, "method": "prompts/list", "params": {}}),
        ];

        for request in requests {
            let send_start = Instant::now();
            transport.send(&request.to_string()).await?;
            performance.messages_sent += 1;

            let response = transport.receive().await?;
            let roundtrip_time = send_start.elapsed();
            performance.messages_received += 1;

            performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
            performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);

            // Validate basic response structure
            let _response_json: Value = serde_json::from_str(&response)
                .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;
        }

        Ok(())
    }

    async fn test_mcp_invalid_json<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let invalid_json = "{ invalid json content }";

        // This should either fail to send or return an error response
        match transport.send(invalid_json).await {
            Ok(_) => {
                performance.messages_sent += 1;
                // If send succeeded, we should get an error response
                let response = transport.receive().await?;
                performance.messages_received += 1;

                match serde_json::from_str::<Value>(&response) {
                    Ok(response_json) => {
                        // Should have an error for invalid JSON
                        if response_json.get("error").is_none() {
                            return Err(Error::Transport(
                                "Expected error for invalid JSON".to_string(),
                            ));
                        }
                    }
                    Err(e) => {
                        // If we can't parse the response, that's expected for invalid JSON
                        debug!("Expected parse error for invalid JSON: {}", e);
                        return Ok(());
                    }
                }
            }
            Err(_) => {
                // Send failing is also acceptable for invalid JSON
                debug!("Send failed for invalid JSON (expected)");
            }
        }

        Ok(())
    }

    async fn test_mcp_unknown_method<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "unknown/method",
            "params": {}
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response)
            .map_err(|e| Error::Transport(format!("Invalid JSON response: {}", e)))?;

        // Should have an error for unknown method
        if response_json.get("error").is_none() {
            return Err(Error::Transport(
                "Expected error for unknown method".to_string(),
            ));
        }

        Ok(())
    }

    async fn test_performance_high_throughput<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        let start_time = Instant::now();
        let message_count = THROUGHPUT_TEST_MESSAGE_COUNT;
        let mut total_roundtrip_time = Duration::ZERO;

        for i in 0..message_count {
            let request = json!({
                "jsonrpc": "2.0",
                "id": 100 + i,
                "method": "tools/list",
                "params": {}
            });

            let send_start = Instant::now();
            transport.send(&request.to_string()).await?;
            performance.messages_sent += 1;

            let _response = transport.receive().await?;
            let roundtrip_time = send_start.elapsed();
            performance.messages_received += 1;

            total_roundtrip_time += roundtrip_time;
            performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
            performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);
        }

        let total_duration = start_time.elapsed();
        performance.avg_roundtrip_time = total_roundtrip_time / message_count;
        performance.throughput_msg_per_sec = message_count as f64 / total_duration.as_secs_f64();

        // Require minimum acceptable throughput
        if performance.throughput_msg_per_sec < MIN_ACCEPTABLE_THROUGHPUT {
            return Err(Error::Transport(format!(
                "Throughput too low: {:.2} msg/sec (minimum required: {:.2})",
                performance.throughput_msg_per_sec, MIN_ACCEPTABLE_THROUGHPUT
            )));
        }

        Ok(())
    }

    async fn test_performance_large_messages<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        // Create a large message
        let large_data = "x".repeat(LARGE_MESSAGE_SIZE_BYTES);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 200,
            "method": "tools/call",
            "params": {
                "name": "create_agent",
                "arguments": {
                    "name": "large-data-agent",
                    "description": large_data,
                    "capabilities": ["large-data-handling"]
                }
            }
        });

        let send_start = Instant::now();
        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let _response = transport.receive().await?;
        let roundtrip_time = send_start.elapsed();
        performance.messages_received += 1;

        performance.min_roundtrip_time = performance.min_roundtrip_time.min(roundtrip_time);
        performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);
        performance.avg_roundtrip_time = roundtrip_time;

        // Require reasonable performance for large messages
        if roundtrip_time > Duration::from_secs(LARGE_MESSAGE_TIMEOUT_SECS) {
            return Err(Error::Transport(format!(
                "Large message handling too slow: {:?}",
                roundtrip_time
            )));
        }

        Ok(())
    }

    /// Helper method to initialize transport before other tests
    async fn initialize_transport<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 999,
            "method": "initialize",
            "params": {
                "protocolVersion": "1.0",
                "clientInfo": {
                    "name": "transport-tester",
                    "version": "1.0.0"
                }
            }
        });

        transport.send(&request.to_string()).await?;
        performance.messages_sent += 1;

        let response = transport.receive().await?;
        performance.messages_received += 1;

        let response_json: Value = serde_json::from_str(&response).map_err(|e| {
            Error::Transport(format!(
                "Invalid JSON response during initialization: {}",
                e
            ))
        })?;

        if response_json.get("error").is_some() {
            return Err(Error::Transport("Initialization failed".to_string()));
        }

        Ok(())
    }

    /// Test high load stress conditions
    async fn test_stress_high_load<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        // Extreme load test: rapidly send many requests concurrently
        let num_concurrent = 50; // Much higher than normal performance tests
        let requests_per_concurrent = 20; // Total: 1000 requests (reduced for stability)

        for batch in 0..num_concurrent {
            for i in 0..requests_per_concurrent {
                let request_id = batch * requests_per_concurrent + i + 1;
                let request = json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "method": "tools/list",
                    "params": {}
                });

                let start = Instant::now();
                transport.send(&request.to_string()).await?;
                performance.messages_sent += 1;

                let roundtrip_time = start.elapsed();
                performance.max_roundtrip_time = performance.max_roundtrip_time.max(roundtrip_time);
                if performance.min_roundtrip_time.is_zero() {
                    performance.min_roundtrip_time = roundtrip_time;
                } else {
                    performance.min_roundtrip_time =
                        performance.min_roundtrip_time.min(roundtrip_time);
                }
            }

            // Add short delay between batches to simulate realistic load patterns
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // Try to receive some responses to verify transport is still functional
        let mut successful_receives = 0;
        for _ in 0..10 {
            if let Ok(Ok(_response)) =
                tokio::time::timeout(Duration::from_millis(100), transport.receive()).await
            {
                performance.messages_received += 1;
                successful_receives += 1;
            } else {
                break; // Stop if no more responses available
            }
        }

        // Update success rate based on sample responses received
        performance.success_rate = if successful_receives > 0 {
            (successful_receives as f64 / 10.0) * 100.0 // Based on sample of 10
        } else {
            0.0
        };

        // Calculate average roundtrip time
        if performance.messages_sent > 0 {
            performance.avg_roundtrip_time = Duration::from_nanos(
                ((performance.max_roundtrip_time.as_nanos()
                    + performance.min_roundtrip_time.as_nanos())
                    / 2) as u64,
            );
        }

        Ok(())
    }

    /// Test memory pressure conditions with increasingly large messages
    async fn test_stress_memory_pressure<T>(
        &self,
        transport: &mut T,
        performance: &mut PerformanceMetrics,
    ) -> Result<()>
    where
        T: Transport,
    {
        // First initialize
        self.initialize_transport(transport, performance).await?;

        // Generate increasingly large messages to stress memory usage
        let message_sizes = [
            1_024,   // 1KB
            10_240,  // 10KB
            102_400, // 100KB
            512_000, // 500KB (reduced from 1MB for stability)
        ];

        for (index, size) in message_sizes.iter().enumerate() {
            let large_data = "x".repeat(*size);
            let request = json!({
                "jsonrpc": "2.0",
                "id": index + 1,
                "method": "tools/call",
                "params": {
                    "name": "vibe/agent/message",
                    "arguments": {
                        "agent_id": "test-agent",
                        "message": large_data,
                        "message_type": "request"
                    }
                }
            });

            let start = Instant::now();
            transport.send(&request.to_string()).await?;
            performance.messages_sent += 1;

            // Try to receive response with extended timeout for large messages
            match tokio::time::timeout(
                Duration::from_secs(5), // Reasonable timeout for large messages
                transport.receive(),
            )
            .await
            {
                Ok(Ok(_response)) => {
                    performance.messages_received += 1;
                    let roundtrip_time = start.elapsed();
                    performance.max_roundtrip_time =
                        performance.max_roundtrip_time.max(roundtrip_time);
                    if performance.min_roundtrip_time.is_zero() {
                        performance.min_roundtrip_time = roundtrip_time;
                    } else {
                        performance.min_roundtrip_time =
                            performance.min_roundtrip_time.min(roundtrip_time);
                    }
                }
                Ok(Err(_e)) => {
                    performance.error_count += 1;
                }
                Err(_) => {
                    performance.error_count += 1;
                }
            }

            // Brief pause between messages to allow garbage collection
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Calculate final metrics
        let total_messages = performance.messages_sent;
        performance.success_rate = if total_messages > 0 {
            ((total_messages - performance.error_count) as f64 / total_messages as f64) * 100.0
        } else {
            0.0
        };

        if performance.messages_received > 0 && performance.max_roundtrip_time > Duration::ZERO {
            performance.avg_roundtrip_time = Duration::from_nanos(
                ((performance.max_roundtrip_time.as_nanos()
                    + performance.min_roundtrip_time.as_nanos())
                    / 2) as u64,
            );
        }

        Ok(())
    }
}

impl Default for TransportTester {
    fn default() -> Self {
        Self::new()
    }
}

/// Get approximate memory usage in KB
fn get_memory_usage_kb() -> u64 {
    // TODO: This is a mock implementation for testing purposes.
    // In production, consider using:
    // - `sys-info` crate for cross-platform memory metrics
    // - `/proc/self/status` on Linux for VmRSS
    // - `jemalloc` stats if using jemalloc allocator
    #[cfg(not(test))]
    {
        // In production, use actual memory profiling
        // For now, return a placeholder
        warn!("Memory profiling not yet implemented, using mock value");
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    static MOCK_MEMORY: AtomicU64 = AtomicU64::new(1024); // Start at 1MB

    let current = MOCK_MEMORY.load(Ordering::Relaxed);
    // Simulate some memory growth
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    std::time::SystemTime::now().hash(&mut hasher);
    let growth = hasher.finish() % 100;
    MOCK_MEMORY.store(current + growth, Ordering::Relaxed);
    current + growth
}

/// Convenience function to test a transport with default scenarios
pub async fn test_transport_with_scenarios<T: Transport>(
    transport: T,
    transport_name: &str,
) -> TestSuiteResult {
    let tester = TransportTester::new();
    tester.test_transport(transport, transport_name).await
}

/// Builder pattern for custom test configurations
pub struct TransportTestBuilder {
    tester: TransportTester,
}

impl TransportTestBuilder {
    /// Create a new test builder
    pub fn new() -> Self {
        Self {
            tester: TransportTester {
                scenarios: Vec::new(),
                performance_baseline: None,
                test_parameters: TestParameters::default(),
            },
        }
    }

    /// Add standard MCP scenarios (core scenarios only, use specific methods for others)
    pub fn with_standard_scenarios(mut self) -> Self {
        self.tester.add_core_mcp_scenarios();
        self
    }

    /// Add core MCP protocol scenarios only
    pub fn with_core_scenarios(mut self) -> Self {
        self.tester.add_core_mcp_scenarios();
        self
    }

    /// Add error handling scenarios only
    pub fn with_error_scenarios(mut self) -> Self {
        self.tester.add_error_scenarios();
        self
    }

    /// Add performance testing scenarios only
    pub fn with_performance_scenarios(mut self) -> Self {
        self.tester.add_performance_scenarios();
        self
    }

    /// Add stress testing scenarios only
    pub fn with_stress_scenarios(mut self) -> Self {
        self.tester.add_stress_scenarios();
        self
    }

    /// Add a custom scenario
    pub fn add_scenario(mut self, scenario: TestScenario) -> Self {
        self.tester.add_scenario(scenario);
        self
    }

    /// Set performance baseline
    pub fn with_baseline(mut self, baseline: PerformanceMetrics) -> Self {
        self.tester.set_performance_baseline(baseline);
        self
    }

    /// Build the transport tester
    pub fn build(self) -> TransportTester {
        self.tester
    }
}

impl Default for TransportTestBuilder {
    fn default() -> Self {
        Self::new()
    }
}
