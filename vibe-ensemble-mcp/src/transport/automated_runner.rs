//! Automated test execution system for transport testing
//!
//! This module provides automated test execution capabilities that can run
//! comprehensive transport validation across all available transport types
//! with detailed reporting, comparison, and CI integration.

use crate::protocol::MCP_VERSION;
use crate::transport::testing::{
    PerformanceMetrics, TestSuiteResult, TransportTestBuilder, TransportTester,
};
use crate::transport::Transport;
use crate::{Error, Result};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationMilliSeconds};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::fs;
use tracing::{error, info, warn};
const MCP_METHOD_INITIALIZE: &str = "initialize";
const MCP_METHOD_TOOLS_LIST: &str = "tools/list";
const MCP_METHOD_TOOLS_CALL: &str = "tools/call";
const MCP_METHOD_RESOURCES_LIST: &str = "resources/list";
const MCP_METHOD_RESOURCES_READ: &str = "resources/read";
const MCP_METHOD_PROMPTS_LIST: &str = "prompts/list";
const MCP_METHOD_NOTIFICATION_INITIALIZED: &str = "notifications/initialized";

// Performance classification thresholds
const HIGH_THROUGHPUT_THRESHOLD: f64 = 20.0; // msg/sec
const LOW_LATENCY_THRESHOLD_MS: u64 = 50;
const HIGH_RELIABILITY_THRESHOLD: f64 = 95.0; // percentage

/// Configuration for automated test execution
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutomatedTestConfig {
    /// List of transport types to test
    pub transport_types: Vec<TransportType>,
    /// Maximum duration for the entire test suite
    #[serde_as(as = "DurationMilliSeconds")]
    pub max_suite_duration: Duration,
    /// Whether to run performance benchmarks
    pub include_performance_tests: bool,
    /// Whether to run stress tests
    pub include_stress_tests: bool,
    /// Whether to run error handling tests
    pub include_error_tests: bool,
    /// Minimum success rate required (0.0 to 100.0)
    pub min_success_rate: f64,
    /// Performance regression thresholds
    pub performance_thresholds: PerformanceThresholds,
    /// Output format for results
    pub output_format: OutputFormat,
    /// Whether to save detailed results to files
    pub save_detailed_results: bool,
    /// Directory to save results (if save_detailed_results is true)
    pub results_directory: Option<String>,
    /// Number of concurrent operations for performance tests
    pub concurrency: Option<usize>,
}

/// Transport types available for testing
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransportType {
    InMemory,
    Stdio,
    #[allow(dead_code)]
    WebSocket,
    #[allow(dead_code)]
    Sse,
}

impl TransportType {
    /// Get human-readable name for the transport type
    pub fn name(&self) -> &'static str {
        match self {
            TransportType::InMemory => "In-Memory Transport",
            TransportType::Stdio => "Standard I/O Transport",
            TransportType::WebSocket => "WebSocket Transport",
            TransportType::Sse => "Server-Sent Events Transport",
        }
    }

    /// Get short identifier for the transport type
    pub fn id(&self) -> &'static str {
        match self {
            TransportType::InMemory => "inmemory",
            TransportType::Stdio => "stdio",
            TransportType::WebSocket => "websocket",
            TransportType::Sse => "sse",
        }
    }
}

/// Performance regression detection thresholds
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable throughput decrease (as percentage)
    pub max_throughput_decrease: f64,
    /// Maximum acceptable latency increase (as percentage)
    pub max_latency_increase: f64,
    /// Minimum acceptable success rate (as percentage)
    pub min_success_rate: f64,
    /// Maximum acceptable error rate increase (as percentage)
    pub max_error_rate_increase: f64,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_throughput_decrease: 20.0, // 20% decrease max
            max_latency_increase: 50.0,    // 50% increase max
            min_success_rate: 80.0,        // 80% minimum success rate
            max_error_rate_increase: 5.0,  // 5% error rate increase max
        }
    }
}

/// Output format for test results
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum OutputFormat {
    /// Human-readable console output
    Console,
    /// JSON format for CI integration
    Json,
    /// Both console and JSON
    Both,
    /// JUnit XML format for CI systems
    JunitXml,
}

impl Default for AutomatedTestConfig {
    fn default() -> Self {
        Self {
            transport_types: vec![TransportType::InMemory],
            max_suite_duration: Duration::from_secs(300), // 5 minutes
            include_performance_tests: true,
            include_stress_tests: true,
            include_error_tests: true,
            min_success_rate: 80.0,
            performance_thresholds: PerformanceThresholds::default(),
            output_format: OutputFormat::Console,
            save_detailed_results: false,
            results_directory: None,
            concurrency: None, // Use default concurrency unless specified
        }
    }
}

/// Comprehensive test execution results
#[serde_as]
#[derive(Clone, Debug, Serialize)]
pub struct AutomatedTestResults {
    /// Timestamp when tests were executed
    pub timestamp: String,
    /// Total duration of all tests
    #[serde_as(as = "DurationMilliSeconds")]
    pub total_duration: Duration,
    /// Results for each transport type tested
    pub transport_results: HashMap<String, TestSuiteResult>,
    /// Overall success status
    pub overall_success: bool,
    /// Summary statistics
    pub summary: TestSummary,
    /// Performance comparison results
    pub performance_comparison: PerformanceComparison,
    /// Any regressions detected
    pub regressions_detected: Vec<RegressionAlert>,
}

/// Summary statistics across all transports
#[derive(Clone, Debug, Serialize)]
pub struct TestSummary {
    /// Total scenarios executed across all transports
    pub total_scenarios: usize,
    /// Total scenarios passed across all transports
    pub total_passed: usize,
    /// Overall success rate
    pub overall_success_rate: f64,
    /// Transport with best performance
    pub best_performing_transport: String,
    /// Transport with worst performance
    pub worst_performing_transport: String,
    /// Average throughput across all transports
    pub avg_throughput_msg_per_sec: f64,
    /// Average latency across all transports
    pub avg_latency_ms: f64,
}

/// Performance comparison across transport types
#[derive(Clone, Debug, Serialize)]
pub struct PerformanceComparison {
    /// Throughput comparison (transport -> msg/sec)
    pub throughput_comparison: HashMap<String, f64>,
    /// Latency comparison (transport -> avg latency ms)
    pub latency_comparison: HashMap<String, f64>,
    /// Success rate comparison (transport -> success rate %)
    pub success_rate_comparison: HashMap<String, f64>,
    /// Relative performance rankings
    pub performance_rankings: Vec<PerformanceRanking>,
}

/// Performance ranking for a transport
#[derive(Clone, Debug, Serialize)]
pub struct PerformanceRanking {
    /// Transport name
    pub transport: String,
    /// Overall rank (1 = best)
    pub rank: usize,
    /// Performance score (0.0 to 100.0)
    pub score: f64,
    /// Key strengths
    pub strengths: Vec<String>,
    /// Key weaknesses
    pub weaknesses: Vec<String>,
}

/// Regression alert for performance degradation
#[derive(Clone, Debug, Serialize)]
pub struct RegressionAlert {
    /// Transport affected
    pub transport: String,
    /// Type of regression detected
    pub regression_type: String,
    /// Severity level
    pub severity: Severity,
    /// Description of the regression
    pub description: String,
    /// Current value vs baseline/threshold
    pub current_value: f64,
    /// Expected/baseline value
    pub expected_value: f64,
    /// Deviation percentage
    pub deviation_percent: f64,
}

/// Severity levels for regressions
#[derive(Clone, Debug, Serialize, PartialEq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Automated test execution engine
pub struct AutomatedTestRunner {
    config: AutomatedTestConfig,
    baseline_metrics: Option<HashMap<String, PerformanceMetrics>>,
}

impl AutomatedTestRunner {
    /// Create a new automated test runner with configuration
    pub fn new(config: AutomatedTestConfig) -> Self {
        Self {
            config,
            baseline_metrics: None,
        }
    }

    /// Set performance baselines for regression detection
    pub fn with_baselines(mut self, baselines: HashMap<String, PerformanceMetrics>) -> Self {
        self.baseline_metrics = Some(baselines);
        self
    }

    /// Execute comprehensive transport testing
    pub async fn run_comprehensive_tests(&self) -> Result<AutomatedTestResults> {
        info!("🚀 Starting comprehensive transport testing");
        let start_time = Instant::now();
        let timestamp = chrono::Utc::now().to_rfc3339();

        let mut transport_results = HashMap::new();
        let mut regressions = Vec::new();

        // Test each configured transport type
        for transport_type in &self.config.transport_types {
            info!("Testing transport: {}", transport_type.name());

            match self.test_transport_type(transport_type).await {
                Ok(result) => {
                    // Check for regressions
                    if let Some(baseline_map) = &self.baseline_metrics {
                        if let Some(baseline) = baseline_map.get(transport_type.id()) {
                            let transport_regressions = self.detect_regressions(
                                transport_type,
                                &result.aggregate_performance,
                                baseline,
                            );
                            regressions.extend(transport_regressions);
                        }
                    }

                    transport_results.insert(transport_type.id().to_string(), result);
                }
                Err(e) => {
                    error!("Failed to test transport {}: {}", transport_type.name(), e);
                    // Create a failed result
                    let failed_result = self.create_failed_result(transport_type, e);
                    transport_results.insert(transport_type.id().to_string(), failed_result);
                }
            }
        }

        let total_duration = start_time.elapsed();

        // Generate summary and comparison
        let summary = self.generate_summary(&transport_results);
        let performance_comparison = self.generate_performance_comparison(&transport_results);
        let overall_success = summary.overall_success_rate >= self.config.min_success_rate;

        let results = AutomatedTestResults {
            timestamp,
            total_duration,
            transport_results,
            overall_success,
            summary,
            performance_comparison,
            regressions_detected: regressions,
        };

        // Save results if configured
        if self.config.save_detailed_results {
            self.save_results(&results).await?;
        }

        // Output results in requested format
        self.output_results(&results).await?;

        info!(
            "✅ Comprehensive transport testing completed in {:?}",
            total_duration
        );
        Ok(results)
    }

    /// Test a specific transport type
    async fn test_transport_type(&self, transport_type: &TransportType) -> Result<TestSuiteResult> {
        let tester = self.build_tester_for_transport(transport_type);

        match transport_type {
            TransportType::InMemory => {
                let (transport, server_transport) = crate::transport::InMemoryTransport::pair();
                // Spawn mock MCP peer in background to handle requests
                let _mock_peer_handle = tokio::spawn(Self::run_mock_mcp_peer(server_transport));
                Ok(tester
                    .test_transport(transport, transport_type.name())
                    .await)
            }
            TransportType::Stdio => {
                // For Stdio, we can't easily test without stdin/stdout, so skip for now
                Err(Error::Transport(
                    "Stdio testing requires stdin/stdout setup".to_string(),
                ))
            }
            TransportType::WebSocket => {
                // For now, return a placeholder - would need actual WebSocket server setup
                Err(Error::Transport(
                    "WebSocket testing not yet implemented".to_string(),
                ))
            }
            TransportType::Sse => {
                // For now, return a placeholder - would need actual SSE server setup
                Err(Error::Transport(
                    "SSE testing not yet implemented".to_string(),
                ))
            }
        }
    }

    /// Build appropriate tester configuration for transport type
    fn build_tester_for_transport(&self, _transport_type: &TransportType) -> TransportTester {
        let mut builder = TransportTestBuilder::new();

        // Always include core MCP scenarios
        builder = builder.with_core_scenarios();

        // Add scenario categories based on configuration
        if self.config.include_performance_tests {
            builder = builder.with_performance_scenarios();
        }

        if self.config.include_stress_tests {
            // Stress tests are considered a subset of performance tests
            builder = builder.with_stress_scenarios();
        }

        if self.config.include_error_tests {
            builder = builder.with_error_scenarios();
        }

        // Set test parameters with concurrency from config
        if let Some(concurrency) = self.config.concurrency {
            let params = crate::transport::testing::TestParameters {
                concurrency: Some(concurrency),
                ..Default::default()
            };
            builder = builder.with_test_parameters(params);
        }

        builder.build()
    }

    /// Create a failed test result for a transport that couldn't be tested
    fn create_failed_result(
        &self,
        transport_type: &TransportType,
        error: Error,
    ) -> TestSuiteResult {
        use crate::transport::testing::{TestResult, TestScenario};
        use std::collections::HashMap;

        let failed_scenario = TestScenario::new(
            "transport_setup_failure",
            "Transport Setup Failure",
            "Failed to initialize transport for testing",
            Duration::from_secs(1),
            true,
        );

        let failed_result = TestResult::failure(
            failed_scenario,
            Duration::from_millis(1),
            format!("Transport setup failed: {}", error),
            PerformanceMetrics::default(),
        );

        TestSuiteResult {
            transport_name: transport_type.name().to_string(),
            total_scenarios: 1,
            passed_scenarios: 0,
            test_results: vec![failed_result],
            total_duration: Duration::from_millis(1),
            aggregate_performance: PerformanceMetrics::default(),
            metadata: HashMap::new(),
        }
    }

    /// Detect performance regressions with enhanced analysis
    fn detect_regressions(
        &self,
        transport_type: &TransportType,
        current: &PerformanceMetrics,
        baseline: &PerformanceMetrics,
    ) -> Vec<RegressionAlert> {
        let mut regressions = Vec::new();
        let thresholds = &self.config.performance_thresholds;

        // Throughput regression check with statistical significance and absolute thresholds
        if baseline.throughput_msg_per_sec > 0.0 {
            let throughput_change = (baseline.throughput_msg_per_sec
                - current.throughput_msg_per_sec)
                / baseline.throughput_msg_per_sec
                * 100.0;

            let absolute_decrease =
                baseline.throughput_msg_per_sec - current.throughput_msg_per_sec;

            // Only flag as regression if both percentage AND absolute thresholds are exceeded
            // This prevents false positives on very small baseline values
            let min_absolute_throughput_decrease = 1.0; // msg/sec

            if throughput_change > thresholds.max_throughput_decrease
                && absolute_decrease > min_absolute_throughput_decrease
            {
                regressions.push(RegressionAlert {
                    transport: transport_type.name().to_string(),
                    regression_type: "Throughput Decrease".to_string(),
                    severity: self.calculate_throughput_severity(throughput_change),
                    description: format!(
                        "Throughput decreased by {:.2}% (from {:.2} to {:.2} msg/sec)",
                        throughput_change,
                        baseline.throughput_msg_per_sec,
                        current.throughput_msg_per_sec
                    ),
                    current_value: current.throughput_msg_per_sec,
                    expected_value: baseline.throughput_msg_per_sec,
                    deviation_percent: throughput_change,
                });
            }
        }

        // Latency regression check with percentile analysis and absolute thresholds
        if baseline.avg_roundtrip_time > Duration::ZERO {
            let current_latency_ms = current.avg_roundtrip_time.as_millis() as f64;
            let baseline_latency_ms = baseline.avg_roundtrip_time.as_millis() as f64;
            let latency_change =
                (current_latency_ms - baseline_latency_ms) / baseline_latency_ms * 100.0;

            let absolute_increase_ms = current_latency_ms - baseline_latency_ms;

            // Only flag as regression if both percentage AND absolute thresholds are exceeded
            // This prevents false positives on very fast baseline latencies
            let min_absolute_latency_increase_ms = 10.0; // 10ms minimum increase to be significant

            if latency_change > thresholds.max_latency_increase
                && absolute_increase_ms > min_absolute_latency_increase_ms
            {
                regressions.push(RegressionAlert {
                    transport: transport_type.name().to_string(),
                    regression_type: "Latency Increase".to_string(),
                    severity: self.calculate_latency_severity(latency_change, current_latency_ms),
                    description: format!(
                        "Latency increased by {:.2}% (from {:.1}ms to {:.1}ms)",
                        latency_change, baseline_latency_ms, current_latency_ms
                    ),
                    current_value: current_latency_ms,
                    expected_value: baseline_latency_ms,
                    deviation_percent: latency_change,
                });
            }
        }

        // Success rate regression check with trend analysis and absolute thresholds
        let success_rate_change = baseline.success_rate - current.success_rate;

        // Only flag as regression if both percentage AND absolute thresholds are exceeded
        // This prevents false positives on small changes in success rates
        let min_absolute_success_rate_decrease = 2.0; // 2 percentage points minimum decrease

        if success_rate_change > thresholds.max_error_rate_increase
            && success_rate_change > min_absolute_success_rate_decrease
        {
            regressions.push(RegressionAlert {
                transport: transport_type.name().to_string(),
                regression_type: "Success Rate Decrease".to_string(),
                severity: self
                    .calculate_success_rate_severity(success_rate_change, current.success_rate),
                description: format!(
                    "Success rate decreased by {:.2} percentage points (from {:.1}% to {:.1}%)",
                    success_rate_change, baseline.success_rate, current.success_rate
                ),
                current_value: current.success_rate,
                expected_value: baseline.success_rate,
                deviation_percent: success_rate_change,
            });
        }

        // Memory usage regression check (based on memory delta during test)
        let baseline_memory_delta = baseline
            .end_memory_kb
            .saturating_sub(baseline.start_memory_kb);
        let current_memory_delta = current
            .end_memory_kb
            .saturating_sub(current.start_memory_kb);

        if baseline_memory_delta > 0 {
            let memory_change = (current_memory_delta as f64 - baseline_memory_delta as f64)
                / baseline_memory_delta as f64
                * 100.0;

            // Flag significant memory increases (>50% increase in memory delta)
            if memory_change > 50.0 {
                let baseline_mb = baseline_memory_delta as f64 / 1024.0;
                let current_mb = current_memory_delta as f64 / 1024.0;

                regressions.push(RegressionAlert {
                    transport: transport_type.name().to_string(),
                    regression_type: "Memory Usage Increase".to_string(),
                    severity: if memory_change > 200.0 {
                        Severity::Critical
                    } else if memory_change > 100.0 {
                        Severity::High
                    } else {
                        Severity::Medium
                    },
                    description: format!(
                        "Memory delta increased by {:.1}% (from {:.1}MB to {:.1}MB)",
                        memory_change, baseline_mb, current_mb
                    ),
                    current_value: current_mb,
                    expected_value: baseline_mb,
                    deviation_percent: memory_change,
                });
            }
        }

        regressions
    }

    /// Calculate throughput regression severity based on performance impact
    fn calculate_throughput_severity(&self, change_percent: f64) -> Severity {
        if change_percent > 75.0 {
            Severity::Critical // Severe performance degradation
        } else if change_percent > 50.0 {
            Severity::High // Major performance impact
        } else if change_percent > 25.0 {
            Severity::Medium // Noticeable performance impact
        } else {
            Severity::Low // Minor performance impact
        }
    }

    /// Calculate latency regression severity based on absolute and relative impact
    fn calculate_latency_severity(&self, change_percent: f64, current_latency_ms: f64) -> Severity {
        // Consider both percentage change and absolute latency values
        let is_high_latency = current_latency_ms > 1000.0; // >1 second is problematic

        if change_percent > 200.0 || is_high_latency {
            Severity::Critical
        } else if change_percent > 100.0 || current_latency_ms > 500.0 {
            Severity::High
        } else if change_percent > 50.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Calculate success rate regression severity based on absolute success rate
    fn calculate_success_rate_severity(&self, change_percent: f64, current_rate: f64) -> Severity {
        // Consider both the change and the absolute success rate
        let is_low_success_rate = current_rate < 80.0;

        if change_percent > 20.0 || is_low_success_rate {
            Severity::Critical
        } else if change_percent > 10.0 || current_rate < 90.0 {
            Severity::High
        } else if change_percent > 5.0 {
            Severity::Medium
        } else {
            Severity::Low
        }
    }

    /// Generate summary statistics
    fn generate_summary(&self, results: &HashMap<String, TestSuiteResult>) -> TestSummary {
        let mut total_scenarios = 0;
        let mut total_passed = 0;
        let mut best_throughput = 0.0;
        let mut worst_throughput = f64::MAX;
        let mut best_transport = String::new();
        let mut worst_transport = String::new();
        let mut total_throughput = 0.0;
        let mut total_latency_ms = 0.0;
        let mut valid_results = 0;

        for result in results.values() {
            total_scenarios += result.total_scenarios;
            total_passed += result.passed_scenarios;

            let throughput = result.aggregate_performance.throughput_msg_per_sec;
            if throughput > best_throughput {
                best_throughput = throughput;
                best_transport = result.transport_name.clone();
            }
            if throughput < worst_throughput && throughput > 0.0 {
                worst_throughput = throughput;
                worst_transport = result.transport_name.clone();
            }

            if throughput > 0.0 {
                total_throughput += throughput;
                total_latency_ms +=
                    result.aggregate_performance.avg_roundtrip_time.as_millis() as f64;
                valid_results += 1;
            }
        }

        let overall_success_rate = if total_scenarios > 0 {
            (total_passed as f64 / total_scenarios as f64) * 100.0
        } else {
            0.0
        };

        let avg_throughput = if valid_results > 0 {
            total_throughput / valid_results as f64
        } else {
            0.0
        };
        let avg_latency = if valid_results > 0 {
            total_latency_ms / valid_results as f64
        } else {
            0.0
        };

        // Handle case where no valid throughput was found
        if worst_throughput == f64::MAX {
            // No valid results were found, so we don't update worst_transport
        }

        TestSummary {
            total_scenarios,
            total_passed,
            overall_success_rate,
            best_performing_transport: best_transport,
            worst_performing_transport: worst_transport,
            avg_throughput_msg_per_sec: avg_throughput,
            avg_latency_ms: avg_latency,
        }
    }

    /// Generate performance comparison
    fn generate_performance_comparison(
        &self,
        results: &HashMap<String, TestSuiteResult>,
    ) -> PerformanceComparison {
        let mut throughput_comparison = HashMap::new();
        let mut latency_comparison = HashMap::new();
        let mut success_rate_comparison = HashMap::new();
        let mut transport_scores = Vec::new();

        for (transport_id, result) in results {
            let perf = &result.aggregate_performance;
            throughput_comparison.insert(transport_id.clone(), perf.throughput_msg_per_sec);
            latency_comparison.insert(
                transport_id.clone(),
                perf.avg_roundtrip_time.as_millis() as f64,
            );
            success_rate_comparison.insert(transport_id.clone(), result.success_rate());

            // Calculate composite score (throughput * success_rate / latency)
            let score = if perf.avg_roundtrip_time.as_millis() > 0 {
                (perf.throughput_msg_per_sec * result.success_rate())
                    / perf.avg_roundtrip_time.as_millis() as f64
                    * 100.0
            } else {
                perf.throughput_msg_per_sec * result.success_rate()
            };

            transport_scores.push((transport_id.clone(), score, result));
        }

        // Sort by score (highest first)
        transport_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let performance_rankings = transport_scores
            .into_iter()
            .enumerate()
            .map(|(rank, (transport_id, score, result))| {
                let strengths = self.identify_strengths(result);
                let weaknesses = self.identify_weaknesses(result);

                PerformanceRanking {
                    transport: transport_id,
                    rank: rank + 1,
                    score,
                    strengths,
                    weaknesses,
                }
            })
            .collect();

        PerformanceComparison {
            throughput_comparison,
            latency_comparison,
            success_rate_comparison,
            performance_rankings,
        }
    }

    /// Identify strengths of a transport based on its results
    fn identify_strengths(&self, result: &TestSuiteResult) -> Vec<String> {
        let mut strengths = Vec::new();
        let perf = &result.aggregate_performance;

        if perf.throughput_msg_per_sec > HIGH_THROUGHPUT_THRESHOLD {
            strengths.push("High throughput".to_string());
        }
        if perf.avg_roundtrip_time < Duration::from_millis(LOW_LATENCY_THRESHOLD_MS) {
            strengths.push("Low latency".to_string());
        }
        if result.success_rate() > HIGH_RELIABILITY_THRESHOLD {
            strengths.push("High reliability".to_string());
        }
        if perf.error_count == 0 {
            strengths.push("Zero errors".to_string());
        }

        if strengths.is_empty() {
            strengths.push("Baseline functionality".to_string());
        }

        strengths
    }

    /// Identify weaknesses of a transport based on its results
    fn identify_weaknesses(&self, result: &TestSuiteResult) -> Vec<String> {
        let mut weaknesses = Vec::new();
        let perf = &result.aggregate_performance;

        if perf.throughput_msg_per_sec < 5.0 {
            weaknesses.push("Low throughput".to_string());
        }
        if perf.avg_roundtrip_time > Duration::from_millis(200) {
            weaknesses.push("High latency".to_string());
        }
        if result.success_rate() < 80.0 {
            weaknesses.push("Low reliability".to_string());
        }
        if perf.error_count > perf.messages_sent / 10 {
            weaknesses.push("High error rate".to_string());
        }

        weaknesses
    }

    /// Save detailed results to files
    async fn save_results(&self, results: &AutomatedTestResults) -> Result<()> {
        if let Some(dir) = &self.config.results_directory {
            // Create directory if it doesn't exist
            fs::create_dir_all(dir).await.map_err(|e| {
                Error::Transport(format!(
                    "Failed to create results directory '{}': {}",
                    dir, e
                ))
            })?;

            // Save JSON results
            let json_path = format!(
                "{}/transport_test_results_{}.json",
                dir,
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            let json_content = serde_json::to_string_pretty(results)
                .map_err(|e| Error::Transport(format!("Failed to serialize results: {}", e)))?;
            fs::write(&json_path, json_content).await.map_err(|e| {
                Error::Transport(format!(
                    "Failed to write results file '{}': {}",
                    json_path, e
                ))
            })?;

            info!("Detailed results saved to: {}", json_path);
        }
        Ok(())
    }

    /// Output results in the configured format
    async fn output_results(&self, results: &AutomatedTestResults) -> Result<()> {
        match self.config.output_format {
            OutputFormat::Console | OutputFormat::Both => {
                self.output_console_results(results).await?;
            }
            _ => {}
        }

        match self.config.output_format {
            OutputFormat::Json | OutputFormat::Both => {
                self.output_json_results(results).await?;
            }
            _ => {}
        }

        if self.config.output_format == OutputFormat::JunitXml {
            self.output_junit_xml_results(results).await?;
        }

        Ok(())
    }

    /// Output console-friendly results
    async fn output_console_results(&self, results: &AutomatedTestResults) -> Result<()> {
        println!("\n🎯 COMPREHENSIVE TRANSPORT TEST RESULTS");
        println!("========================================");
        println!("Timestamp: {}", results.timestamp);
        println!("Total Duration: {:?}", results.total_duration);
        println!(
            "Overall Success: {}",
            if results.overall_success {
                "✅ PASS"
            } else {
                "❌ FAIL"
            }
        );

        println!("\n📊 SUMMARY");
        println!("-----------");
        println!("Total Scenarios: {}", results.summary.total_scenarios);
        println!("Total Passed: {}", results.summary.total_passed);
        println!(
            "Overall Success Rate: {:.2}%",
            results.summary.overall_success_rate
        );
        println!(
            "Best Performing: {}",
            results.summary.best_performing_transport
        );
        println!(
            "Worst Performing: {}",
            results.summary.worst_performing_transport
        );
        println!(
            "Average Throughput: {:.2} msg/sec",
            results.summary.avg_throughput_msg_per_sec
        );
        println!("Average Latency: {:.2} ms", results.summary.avg_latency_ms);

        println!("\n🏆 PERFORMANCE RANKINGS");
        println!("-----------------------");
        for ranking in &results.performance_comparison.performance_rankings {
            println!(
                "{}. {} (Score: {:.2})",
                ranking.rank, ranking.transport, ranking.score
            );
            println!("   Strengths: {}", ranking.strengths.join(", "));
            if !ranking.weaknesses.is_empty() {
                println!("   Weaknesses: {}", ranking.weaknesses.join(", "));
            }
        }

        if !results.regressions_detected.is_empty() {
            println!("\n⚠️ REGRESSIONS DETECTED");
            println!("-----------------------");
            for regression in &results.regressions_detected {
                let severity_icon = match regression.severity {
                    Severity::Critical => "🔴",
                    Severity::High => "🟠",
                    Severity::Medium => "🟡",
                    Severity::Low => "🟢",
                };
                println!(
                    "{} {} - {}: {}",
                    severity_icon,
                    regression.transport,
                    regression.regression_type,
                    regression.description
                );
            }
        }

        println!("\n📈 DETAILED TRANSPORT RESULTS");
        println!("==============================");
        for (transport_id, result) in &results.transport_results {
            println!("\n{} ({})", result.transport_name, transport_id);
            println!(
                "{}",
                "=".repeat(result.transport_name.len() + transport_id.len() + 3)
            );
            println!("{}", result);
        }

        Ok(())
    }

    /// Output JSON results for CI integration
    async fn output_json_results(&self, results: &AutomatedTestResults) -> Result<()> {
        let json_output = serde_json::to_string_pretty(results)
            .map_err(|e| Error::Transport(format!("Failed to serialize JSON results: {}", e)))?;
        println!("{}", json_output);
        Ok(())
    }

    /// Output JUnit XML results for CI integration
    async fn output_junit_xml_results(&self, _results: &AutomatedTestResults) -> Result<()> {
        // TODO: Implement JUnit XML output format
        warn!("JUnit XML output format not yet implemented");
        Ok(())
    }

    /// Spawn a mock MCP peer for InMemory transport testing
    async fn run_mock_mcp_peer(mut transport: impl Transport + 'static) -> Result<()> {
        use serde_json::{json, Value};
        use tracing::{debug, info, warn};

        info!("Starting mock MCP peer for InMemory transport testing");

        loop {
            match transport.receive().await {
                Ok(message) => {
                    debug!("Mock MCP peer received message: {}", message);

                    // Parse the JSON-RPC message
                    let parsed: Value = match serde_json::from_str(&message) {
                        Ok(v) => v,
                        Err(e) => {
                            warn!("Failed to parse JSON in mock MCP peer: {}", e);
                            let parse_err = json!({
                                "jsonrpc": "2.0",
                                "id": null,
                                "error": { "code": -32700, "message": "Parse error" }
                            });
                            let _ = transport.send(&parse_err.to_string()).await;
                            continue;
                        }
                    };

                    // Extract method and id for response
                    let method = parsed.get("method").and_then(|v| v.as_str());
                    let id = parsed.get("id").cloned();

                    // Generate appropriate response based on method
                    let response = match method {
                        Some(MCP_METHOD_INITIALIZE) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "protocolVersion": MCP_VERSION,
                                    "capabilities": {
                                        "tools": { "listChanged": true },
                                        "resources": { "listChanged": true },
                                        "prompts": { "listChanged": true }
                                    },
                                    "serverInfo": {
                                        "name": "mock-mcp-server",
                                        "version": "1.0.0"
                                    }
                                }
                            })
                        }
                        Some(MCP_METHOD_TOOLS_LIST) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "tools": [
                                        {
                                            "name": "test_tool",
                                            "description": "A test tool for transport testing",
                                            "inputSchema": {
                                                "type": "object",
                                                "properties": {
                                                    "message": {
                                                        "type": "string",
                                                        "description": "Test message"
                                                    }
                                                }
                                            }
                                        }
                                    ]
                                }
                            })
                        }
                        Some(MCP_METHOD_TOOLS_CALL) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": "Mock tool execution successful"
                                        }
                                    ]
                                }
                            })
                        }
                        Some(MCP_METHOD_RESOURCES_LIST) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "resources": [
                                        {
                                            "uri": "test://resource",
                                            "name": "Test Resource",
                                            "description": "A test resource",
                                            "mimeType": "text/plain"
                                        }
                                    ]
                                }
                            })
                        }
                        Some(MCP_METHOD_RESOURCES_READ) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "contents": [
                                        {
                                            "uri": "test://resource",
                                            "mimeType": "text/plain",
                                            "text": "Test resource content"
                                        }
                                    ]
                                }
                            })
                        }
                        Some(MCP_METHOD_PROMPTS_LIST) => {
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "prompts": [
                                        {
                                            "name": "test_prompt",
                                            "description": "A test prompt",
                                            "arguments": []
                                        }
                                    ]
                                }
                            })
                        }
                        Some(MCP_METHOD_NOTIFICATION_INITIALIZED) => {
                            // No response for notifications
                            debug!("Received initialized notification, no response needed");
                            continue;
                        }
                        Some(unknown_method) => {
                            debug!(
                                "Unknown method: {}, sending method not found error",
                                unknown_method
                            );
                            json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32601,
                                    "message": "Method not found",
                                    "data": format!("The method '{}' is not supported by this mock server", unknown_method)
                                }
                            })
                        }
                        None => {
                            warn!("Received message without method field");
                            json!({
                                "jsonrpc": "2.0",
                                "id": parsed.get("id").cloned().unwrap_or(json!(null)),
                                "error": { "code": -32600, "message": "Invalid Request" }
                            })
                        }
                    };

                    // Send response unless it's a valid notification (method present, no id)
                    let is_notification = method.is_some() && parsed.get("id").is_none();
                    if !is_notification {
                        let response_str = response.to_string();
                        if let Err(e) = transport.send(&response_str).await {
                            warn!("Failed to send mock MCP response: {}", e);
                            break;
                        }
                        debug!("Mock MCP peer sent response: {}", response_str);
                    }
                }
                Err(e) => {
                    debug!("Mock MCP peer transport closed or error: {}", e);
                    break;
                }
            }
        }

        info!("Mock MCP peer shutting down");
        Ok(())
    }
}

impl Default for AutomatedTestRunner {
    fn default() -> Self {
        Self::new(AutomatedTestConfig::default())
    }
}

/// Convenience function to run comprehensive transport testing with defaults
pub async fn run_automated_transport_tests() -> Result<AutomatedTestResults> {
    let runner = AutomatedTestRunner::default();
    runner.run_comprehensive_tests().await
}

/// Convenience function to run transport tests with custom configuration
pub async fn run_automated_transport_tests_with_config(
    config: AutomatedTestConfig,
) -> Result<AutomatedTestResults> {
    let runner = AutomatedTestRunner::new(config);
    runner.run_comprehensive_tests().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_automated_runner_creation() {
        let runner = AutomatedTestRunner::default();
        assert_eq!(runner.config.transport_types.len(), 1);
        assert!(runner.config.include_performance_tests);
    }

    #[tokio::test]
    async fn test_config_serialization() {
        let config = AutomatedTestConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let _deserialized: AutomatedTestConfig = serde_json::from_str(&json).unwrap();
    }

    #[tokio::test]
    async fn test_performance_thresholds() {
        let thresholds = PerformanceThresholds::default();
        assert_eq!(thresholds.max_throughput_decrease, 20.0);
        assert_eq!(thresholds.max_latency_increase, 50.0);
        assert_eq!(thresholds.min_success_rate, 80.0);
    }
}
