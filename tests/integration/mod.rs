//! Integration tests for vibe-ensemble-mcp
//!
//! These tests verify the integration between different components
//! and ensure the system works correctly as a whole.

pub mod mcp_protocol;
pub mod storage_integration;
pub mod agent_coordination;
pub mod web_interface;

use crate::common::TestContext;

/// Helper function to set up integration test environment
pub async fn setup_integration_test() -> TestContext {
    TestContext::new().await.expect("Failed to setup integration test context")
}

/// Helper function for cleaning up after integration tests
pub async fn cleanup_integration_test(_ctx: TestContext) {
    // Cleanup code if needed - TestContext handles most cleanup automatically
}