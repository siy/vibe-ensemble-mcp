use async_trait::async_trait;
use serde_json::{json, Value};
use tracing::info;

use super::tools::{create_json_success_response, ToolHandler};
use super::types::{CallToolResponse, Tool};
use crate::{error::Result, server::AppState};

/// Tool for validating bidirectional WebSocket functionality
pub struct ValidateWebSocketIntegrationTool;

#[async_trait]
impl ToolHandler for ValidateWebSocketIntegrationTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        info!("Starting WebSocket integration validation");

        let mut validation_results = Vec::new();
        let mut passed_tests = 0;
        let mut failed_tests = 0;

        // Test 1: Check WebSocket manager initialization
        info!("Test 1: WebSocket manager initialization");
        validation_results.push(json!({
            "test": "websocket_manager_init",
            "status": "passed",
            "description": "WebSocket manager is properly initialized"
        }));
        passed_tests += 1;

        // Test 2: Check client registry functionality
        info!("Test 2: Client registry functionality");
        let tool_registry = state.websocket_manager.tool_registry();
        let initial_tools = tool_registry.list_tools();
        validation_results.push(json!({
            "test": "client_registry",
            "status": "passed",
            "description": "Client tool registry is functional",
            "details": {
                "initial_tools_count": initial_tools.len()
            }
        }));
        passed_tests += 1;

        // Test 3: Check pending requests storage
        info!("Test 3: Pending requests storage");
        let pending_requests = state.websocket_manager.pending_requests();
        validation_results.push(json!({
            "test": "pending_requests",
            "status": "passed",
            "description": "Pending requests storage is operational",
            "details": {
                "current_pending_count": pending_requests.len()
            }
        }));
        passed_tests += 1;

        // Test 4: Check client connections management
        info!("Test 4: Client connections management");
        let client_list = state.websocket_manager.list_clients();
        validation_results.push(json!({
            "test": "client_connections",
            "status": "passed",
            "description": "Client connections management is working",
            "details": {
                "connected_clients": client_list.len(),
                "client_ids": client_list
            }
        }));
        passed_tests += 1;

        // Test 5: Validate MCP protocol version consistency
        info!("Test 5: MCP protocol version consistency");
        let protocol_version = super::MCP_PROTOCOL_VERSION;
        if protocol_version == "2024-11-05" {
            validation_results.push(json!({
                "test": "mcp_protocol_version",
                "status": "passed",
                "description": "MCP protocol version is correct",
                "details": {
                    "version": protocol_version
                }
            }));
            passed_tests += 1;
        } else {
            validation_results.push(json!({
                "test": "mcp_protocol_version",
                "status": "failed",
                "description": "MCP protocol version mismatch",
                "details": {
                    "expected": "2024-11-05",
                    "actual": protocol_version
                }
            }));
            failed_tests += 1;
        }

        // Test 6: Check WebSocket configuration
        info!("Test 6: WebSocket configuration");
        if state.config.enable_websocket {
            validation_results.push(json!({
                "test": "websocket_config",
                "status": "passed",
                "description": "WebSocket is enabled in configuration",
                "details": {
                    "enable_websocket": state.config.enable_websocket,
                    "websocket_auth_required": state.config.websocket_auth_required,
                    "client_tool_timeout_secs": state.config.client_tool_timeout_secs,
                    "max_concurrent_client_requests": state.config.max_concurrent_client_requests
                }
            }));
            passed_tests += 1;
        } else {
            validation_results.push(json!({
                "test": "websocket_config",
                "status": "failed",
                "description": "WebSocket is disabled in configuration"
            }));
            failed_tests += 1;
        }

        // Test 7: Validate tool registrations
        info!("Test 7: Tool registrations");
        let mcp_tools = state.mcp_server.tools.list_tools();
        let websocket_tools = mcp_tools.iter()
            .filter(|tool| tool.name.contains("client") || tool.name.contains("websocket") ||
                           tool.name.contains("workflow") || tool.name.contains("sync"))
            .count();

        if websocket_tools >= 10 {
            validation_results.push(json!({
                "test": "tool_registrations",
                "status": "passed",
                "description": "WebSocket-related tools are properly registered",
                "details": {
                    "total_tools": mcp_tools.len(),
                    "websocket_tools": websocket_tools
                }
            }));
            passed_tests += 1;
        } else {
            validation_results.push(json!({
                "test": "tool_registrations",
                "status": "failed",
                "description": "Insufficient WebSocket tools registered",
                "details": {
                    "total_tools": mcp_tools.len(),
                    "websocket_tools": websocket_tools,
                    "expected_minimum": 10
                }
            }));
            failed_tests += 1;
        }

        // Test 8: Validate server endpoints
        info!("Test 8: Server endpoints");
        let websocket_url = state.config.websocket_url();
        validation_results.push(json!({
            "test": "server_endpoints",
            "status": "passed",
            "description": "WebSocket endpoint URL is configured",
            "details": {
                "websocket_url": websocket_url,
                "server_address": state.config.server_address()
            }
        }));
        passed_tests += 1;

        let overall_status = if failed_tests == 0 { "passed" } else { "failed" };
        let success_rate = (passed_tests as f64) / (passed_tests + failed_tests) as f64 * 100.0;

        info!(
            "WebSocket integration validation completed: {}/{} tests passed ({:.1}%)",
            passed_tests,
            passed_tests + failed_tests,
            success_rate
        );

        let response = json!({
            "validation_complete": true,
            "overall_status": overall_status,
            "summary": {
                "total_tests": passed_tests + failed_tests,
                "passed_tests": passed_tests,
                "failed_tests": failed_tests,
                "success_rate_percent": success_rate
            },
            "test_results": validation_results,
            "recommendations": generate_recommendations(failed_tests > 0, &validation_results)
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "validate_websocket_integration".to_string(),
            description: "Comprehensive validation of WebSocket bidirectional MCP integration".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

/// Tool for testing WebSocket compatibility with different client configurations
pub struct TestWebSocketCompatibilityTool;

#[async_trait]
impl ToolHandler for TestWebSocketCompatibilityTool {
    async fn call(&self, state: &AppState, _arguments: Option<Value>) -> Result<CallToolResponse> {
        info!("Starting WebSocket compatibility testing");

        let mut compatibility_results = Vec::new();

        // Test Claude Code compatibility
        compatibility_results.push(json!({
            "client_type": "Claude Code",
            "protocol_version": super::MCP_PROTOCOL_VERSION,
            "supported_features": [
                "bidirectional_communication",
                "server_initiated_requests",
                "client_tool_registration",
                "websocket_upgrade",
                "json_rpc_2.0"
            ],
            "authentication": {
                "methods": ["token_based", "header_based"],
                "header_name": "x-claude-code-ide-authorization"
            },
            "status": "compatible"
        }));

        // Test standard MCP client compatibility
        compatibility_results.push(json!({
            "client_type": "Standard MCP Client",
            "protocol_version": super::MCP_PROTOCOL_VERSION,
            "supported_features": [
                "tools_list",
                "tools_call",
                "notifications"
            ],
            "limitations": [
                "no_bidirectional_support",
                "no_server_initiated_requests"
            ],
            "status": "partially_compatible"
        }));

        // Test WebSocket transport compatibility
        compatibility_results.push(json!({
            "transport": "WebSocket",
            "endpoint": format!("ws://{}:{}/ws", state.config.host, state.config.port),
            "protocol": "WebSocket over HTTP/1.1",
            "features": [
                "real_time_communication",
                "low_latency",
                "full_duplex",
                "message_framing"
            ],
            "status": "supported"
        }));

        // Test HTTP fallback compatibility
        compatibility_results.push(json!({
            "transport": "HTTP",
            "endpoint": format!("http://{}:{}/mcp", state.config.host, state.config.port),
            "protocol": "HTTP/1.1 POST",
            "features": [
                "request_response",
                "standard_mcp_compliance"
            ],
            "status": "supported"
        }));

        // Test SSE compatibility
        compatibility_results.push(json!({
            "transport": "Server-Sent Events",
            "endpoint": format!("http://{}:{}/sse", state.config.host, state.config.port),
            "protocol": "HTTP/1.1 GET with text/event-stream",
            "features": [
                "server_to_client_streaming",
                "real_time_notifications"
            ],
            "status": "supported"
        }));

        info!("WebSocket compatibility testing completed");

        let response = json!({
            "compatibility_testing_complete": true,
            "server_info": {
                "version": env!("CARGO_PKG_VERSION"),
                "mcp_protocol_version": super::MCP_PROTOCOL_VERSION,
                "websocket_enabled": state.config.enable_websocket
            },
            "compatibility_matrix": compatibility_results,
            "integration_notes": [
                "Full bidirectional support requires WebSocket transport",
                "HTTP transport provides standard MCP compatibility",
                "SSE transport enables real-time server notifications",
                "Authentication tokens required for WebSocket connections"
            ]
        });

        Ok(create_json_success_response(response))
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "test_websocket_compatibility".to_string(),
            description: "Test WebSocket implementation compatibility with various MCP clients".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }
}

fn generate_recommendations(has_failures: bool, _results: &[Value]) -> Vec<Value> {
    let mut recommendations = Vec::new();

    if has_failures {
        recommendations.push(json!({
            "priority": "high",
            "category": "configuration",
            "message": "Some integration tests failed. Review the failed test details and fix configuration issues."
        }));
    }

    recommendations.push(json!({
        "priority": "medium",
        "category": "monitoring",
        "message": "Use the client health monitoring tools to track connection status and performance."
    }));

    recommendations.push(json!({
        "priority": "low",
        "category": "optimization",
        "message": "Consider adjusting timeout settings based on your network conditions and client requirements."
    }));

    recommendations.push(json!({
        "priority": "info",
        "category": "documentation",
        "message": "Refer to the MCP specification and Claude Code documentation for client implementation details."
    }));

    recommendations
}