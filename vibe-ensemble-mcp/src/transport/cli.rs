//! Command-line interface for automated transport testing
//!
//! This module provides a CLI for running transport tests from the command line,
//! enabling easy integration with CI/CD pipelines and manual testing.

use crate::transport::automated_runner::{
    run_automated_transport_tests_with_config, AutomatedTestConfig, OutputFormat,
    PerformanceThresholds, TransportType,
};
use crate::Result;

use clap::{Parser, Subcommand, ValueEnum};
use std::time::Duration;
use tracing::info;

/// Command-line interface for transport testing
#[derive(Parser)]
#[command(name = "transport-tester")]
#[command(about = "Comprehensive automated transport testing framework")]
#[command(version = env!("CARGO_PKG_VERSION"))]
pub struct TransportTestCli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Run comprehensive transport tests
    Test {
        /// Transport types to test
        #[arg(long, value_enum, value_delimiter = ',')]
        transports: Option<Vec<CliTransportType>>,

        /// Maximum test suite duration in seconds
        #[arg(long, default_value = "300")]
        max_duration: u64,

        /// Include performance tests
        #[arg(long, default_value = "true")]
        include_performance: bool,

        /// Include stress tests
        #[arg(long, default_value = "true")]
        include_stress: bool,

        /// Include error handling tests
        #[arg(long, default_value = "true")]
        include_errors: bool,

        /// Minimum success rate required (0-100)
        #[arg(long, default_value = "80.0")]
        min_success_rate: f64,

        /// Output format
        #[arg(long, value_enum, default_value = "console")]
        output_format: CliOutputFormat,

        /// Save detailed results to directory
        #[arg(long)]
        save_results: Option<String>,

        /// Performance regression thresholds
        #[arg(long, default_value = "20.0")]
        max_throughput_decrease: f64,

        #[arg(long, default_value = "50.0")]
        max_latency_increase: f64,

        #[arg(long, default_value = "5.0")]
        max_error_rate_increase: f64,
    },

    /// Benchmark specific transport types
    Benchmark {
        /// Transport types to benchmark
        #[arg(long, value_enum, value_delimiter = ',')]
        transports: Option<Vec<CliTransportType>>,

        /// Duration for each benchmark in seconds
        #[arg(long, default_value = "60")]
        duration: u64,

        /// Number of concurrent operations
        #[arg(long, default_value = "10")]
        concurrency: usize,

        /// Output format
        #[arg(long, value_enum, default_value = "console")]
        output_format: CliOutputFormat,
    },

    /// List available transport types
    ListTransports,

    /// Generate sample configuration file
    GenerateConfig {
        /// Output file path
        #[arg(long, default_value = "transport-test-config.json")]
        output: String,
    },

    /// Run tests from configuration file
    RunConfig {
        /// Configuration file path
        #[arg(long)]
        config: String,
    },
}

/// Transport types available via CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum CliTransportType {
    InMemory,
    Stdio,
    WebSocket,
    Sse,
}

impl From<CliTransportType> for TransportType {
    fn from(cli_type: CliTransportType) -> Self {
        match cli_type {
            CliTransportType::InMemory => TransportType::InMemory,
            CliTransportType::Stdio => TransportType::Stdio,
            CliTransportType::WebSocket => TransportType::WebSocket,
            CliTransportType::Sse => TransportType::Sse,
        }
    }
}

/// Output formats available via CLI
#[derive(ValueEnum, Clone, Debug)]
pub enum CliOutputFormat {
    Console,
    Json,
    Both,
    JunitXml,
}

impl From<CliOutputFormat> for OutputFormat {
    fn from(cli_format: CliOutputFormat) -> Self {
        match cli_format {
            CliOutputFormat::Console => OutputFormat::Console,
            CliOutputFormat::Json => OutputFormat::Json,
            CliOutputFormat::Both => OutputFormat::Both,
            CliOutputFormat::JunitXml => OutputFormat::JunitXml,
        }
    }
}

/// Execute the CLI command
pub async fn execute_cli_command(cli: TransportTestCli) -> Result<()> {
    match cli.command {
        Commands::Test {
            transports,
            max_duration,
            include_performance,
            include_stress,
            include_errors,
            min_success_rate,
            output_format,
            save_results,
            max_throughput_decrease,
            max_latency_increase,
            max_error_rate_increase,
        } => {
            let transport_types = transports
                .unwrap_or_else(|| vec![CliTransportType::InMemory])
                .into_iter()
                .map(TransportType::from)
                .collect();

            let config = AutomatedTestConfig {
                transport_types,
                max_suite_duration: Duration::from_secs(max_duration),
                include_performance_tests: include_performance,
                include_stress_tests: include_stress,
                include_error_tests: include_errors,
                min_success_rate,
                performance_thresholds: PerformanceThresholds {
                    max_throughput_decrease,
                    max_latency_increase,
                    min_success_rate, // Use same value for consistency
                    max_error_rate_increase,
                },
                output_format: output_format.clone().into(),
                save_detailed_results: save_results.is_some(),
                results_directory: save_results,
                concurrency: None, // Use default concurrency for regular tests
            };

            let results = run_automated_transport_tests_with_config(config).await?;

            // Exit with appropriate code for CI integration
            if !results.overall_success {
                std::process::exit(1);
            }
        }

        Commands::Benchmark {
            transports,
            duration,
            concurrency,
            output_format,
        } => {
            let transport_types = transports
                .unwrap_or_else(|| vec![CliTransportType::InMemory])
                .into_iter()
                .map(TransportType::from)
                .collect();

            info!("Running benchmarks for transports: {:?}", transport_types);
            info!("Benchmark duration: {}s", duration);
            info!("Concurrency level: {}", concurrency);

            // Create benchmark-focused configuration
            let config = AutomatedTestConfig {
                transport_types,
                max_suite_duration: Duration::from_secs(duration + 60), // Add buffer
                include_performance_tests: true,
                include_stress_tests: true,
                include_error_tests: false,
                min_success_rate: 0.0, // Don't fail on success rate for benchmarks
                performance_thresholds: PerformanceThresholds::default(),
                output_format: output_format.clone().into(),
                save_detailed_results: false,
                results_directory: None,
                concurrency: Some(concurrency), // Use CLI-specified concurrency
            };

            let results = run_automated_transport_tests_with_config(config).await?;

            // Display benchmark results summary only for console-friendly formats
            if matches!(
                output_format,
                CliOutputFormat::Console | CliOutputFormat::Both
            ) {
                println!("\n🚀 Benchmark Results Summary:");
                println!("═══════════════════════════════");
                println!(
                    "Overall Success: {}",
                    if results.overall_success {
                        "✅"
                    } else {
                        "❌"
                    }
                );
                println!("Total Tests: {}", results.summary.total_scenarios);
                println!("Tests Passed: {}", results.summary.total_passed);

                if !results.summary.best_performing_transport.is_empty() {
                    println!(
                        "🏆 Best Performance: {}",
                        results.summary.best_performing_transport
                    );
                }

                if results.summary.avg_throughput_msg_per_sec > 0.0 {
                    println!(
                        "📊 Average Throughput: {:.2} msg/sec",
                        results.summary.avg_throughput_msg_per_sec
                    );
                    println!(
                        "⏱️  Average Latency: {:.2}ms",
                        results.summary.avg_latency_ms
                    );
                }
            }
        }

        Commands::ListTransports => {
            use crate::transport::automated_runner::TransportType;

            println!("Available transport types:");
            println!(
                "  {:12} - {}",
                TransportType::InMemory.id(),
                TransportType::InMemory.name()
            );
            println!(
                "  {:12} - {}",
                TransportType::Stdio.id(),
                TransportType::Stdio.name()
            );
            println!(
                "  {:12} - {}",
                TransportType::WebSocket.id(),
                TransportType::WebSocket.name()
            );
            println!(
                "  {:12} - {}",
                TransportType::Sse.id(),
                TransportType::Sse.name()
            );
        }

        Commands::GenerateConfig { output } => {
            let sample_config = AutomatedTestConfig::default();
            let json_config = serde_json::to_string_pretty(&sample_config).map_err(|e| {
                crate::Error::Transport(format!("Failed to serialize config: {}", e))
            })?;

            tokio::fs::write(&output, json_config).await.map_err(|e| {
                crate::Error::Transport(format!("Failed to write config file: {}", e))
            })?;

            println!("Sample configuration written to: {}", output);
        }

        Commands::RunConfig { config } => {
            let config_content = tokio::fs::read_to_string(&config).await.map_err(|e| {
                crate::Error::Transport(format!("Failed to read config file: {}", e))
            })?;

            let test_config: AutomatedTestConfig =
                serde_json::from_str(&config_content).map_err(|e| {
                    crate::Error::Transport(format!("Failed to parse config file: {}", e))
                })?;

            let results = run_automated_transport_tests_with_config(test_config).await?;

            // Exit with appropriate code for CI integration
            if !results.overall_success {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Entry point for CLI execution
pub async fn run_transport_test_cli() -> Result<()> {
    let cli = TransportTestCli::parse();
    execute_cli_command(cli).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_transport_type_conversion() {
        let cli_type = CliTransportType::InMemory;
        let transport_type: TransportType = cli_type.into();
        assert_eq!(transport_type, TransportType::InMemory);
    }

    #[test]
    fn test_cli_output_format_conversion() {
        let cli_format = CliOutputFormat::Json;
        let output_format: OutputFormat = cli_format.into();
        assert_eq!(output_format, OutputFormat::Json);
    }

    #[tokio::test]
    async fn test_config_generation() {
        let config = AutomatedTestConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();

        // Should be valid JSON that can be parsed back
        let _parsed: AutomatedTestConfig = serde_json::from_str(&json).unwrap();
    }
}
