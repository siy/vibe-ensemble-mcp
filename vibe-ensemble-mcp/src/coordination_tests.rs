//! Tests for new coordination MCP tools and functionality
//!
//! This module tests the new coordination tools added in Issues #52-54:
//! - Intelligent Work Orchestration (Issue #52)
//! - Knowledge-Driven Coordination (Issue #53)
//! - Integration testing for coordination workflows

#![allow(deprecated)] // Allow deprecated method constants for backward compatibility

#[cfg(test)]
mod tests {
    use crate::protocol::*;
    use crate::server::McpServer;
    use serde_json::json;
    use uuid::Uuid;

    fn create_test_server() -> McpServer {
        McpServer::new()
    }

    fn create_test_request(method: &str, params: serde_json::Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: json!(1),
            method: method.to_string(),
            params: Some(params),
        }
    }

    // Issue #52: Intelligent Work Orchestration Tests

    #[tokio::test]
    async fn test_schedule_coordinate_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "coordinatorAgentId": Uuid::new_v4().to_string(),
            "workSequences": [
                {"task": "implement_feature", "estimated_duration": "2h"},
                {"task": "write_tests", "estimated_duration": "1h"}
            ],
            "involvedAgents": [Uuid::new_v4().to_string()],
            "projectScopes": ["core", "api"],
            "resourceRequirements": {"cpu": 2, "memory": "4gb"}
        });

        let request = create_test_request("vibe/schedule/coordinate", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_conflict_predict_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "analyzerAgentId": Uuid::new_v4().to_string(),
            "plannedActions": [
                {"action": "modify_file", "target": "src/main.rs"},
                {"action": "update_config", "target": "config.toml"}
            ],
            "activeWorkflows": [
                {"workflow_id": "build_process", "status": "running"}
            ],
            "resourceMap": {"files": ["src/main.rs", "src/lib.rs"]}
        });

        let request = create_test_request("vibe/conflict/predict", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("analysisId").is_some());
            assert!(result_value.get("predictedConflicts").is_some());
            assert!(result_value.get("confidence").is_some());
        }
    }

    #[tokio::test]
    async fn test_resource_reserve_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "reservingAgentId": Uuid::new_v4().to_string(),
            "resourcePaths": ["src/main.rs", "config/app.toml"],
            "reservationType": "EXCLUSIVE",
            "reservationDuration": "2h",
            "exclusiveAccess": true,
            "allowedOperations": ["read", "write"],
            "justification": "Critical file modifications for feature implementation"
        });

        let request = create_test_request("vibe/resource/reserve", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("reservationId").is_some());
            assert!(result_value.get("accessToken").is_some());
            assert!(result_value.get("expirationTime").is_some());
        }
    }

    #[tokio::test]
    async fn test_merge_coordinate_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "coordinatorAgentId": Uuid::new_v4().to_string(),
            "mergeScenario": "FEATURE_INTEGRATION",
            "sourceBranches": ["feature/auth", "feature/payments"],
            "targetBranch": "main",
            "involvedAgents": [Uuid::new_v4().to_string(), Uuid::new_v4().to_string()],
            "complexityAnalysis": {"conflict_risk": "medium", "test_coverage": 0.85},
            "conflictResolutionStrategy": "MANUAL"
        });

        let request = create_test_request("vibe/merge/coordinate", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("mergeCoordinationId").is_some());
            assert!(result_value.get("mergeStrategy").is_some());
            assert!(result_value.get("sequencePlan").is_some());
        }
    }

    // Issue #53: Knowledge-Driven Coordination Tests

    #[tokio::test]
    async fn test_knowledge_query_coordination_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "queryingAgentId": Uuid::new_v4().to_string(),
            "coordinationContext": "multi_agent_code_review",
            "query": "best practices for conflict resolution",
            "searchScope": ["patterns", "practices", "guidelines"]
        });

        let request = create_test_request("vibe/knowledge/query/coordination", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("queryId").is_some());
            assert!(result_value.get("relevantPatterns").is_some());
            assert!(result_value.get("confidenceScore").is_some());
        }
    }

    #[tokio::test]
    async fn test_pattern_suggest_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "requestingAgentId": Uuid::new_v4().to_string(),
            "coordinationScenario": "parallel_development_coordination",
            "currentContext": {
                "project_type": "web_api",
                "team_size": 3,
                "complexity": "medium"
            },
            "excludePatterns": []
        });

        let request = create_test_request("vibe/pattern/suggest", params);
        let result = server.handle_request(request).await;

        match result {
            Ok(response) => {
                assert!(response.is_some());
                let response = response.unwrap();
                assert!(response.result.is_some());
                assert!(response.error.is_none());

                // Verify response structure
                if let Some(result_value) = response.result {
                    assert!(result_value.get("suggestionId").is_some());
                    assert!(result_value.get("recommendedPatterns").is_some());
                    assert!(result_value.get("successProbability").is_some());
                }
            }
            Err(e) => {
                eprintln!(
                    "Unexpected error in test_pattern_suggest_basic_functionality: {:?}",
                    e
                );
                panic!("Request should not fail at transport level");
            }
        }
    }

    #[tokio::test]
    async fn test_guideline_enforce_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "enforcingAgentId": Uuid::new_v4().to_string(),
            "coordinationPlan": {
                "communication_plan": "daily_standup",
                "resource_reservations": ["src/core.rs"]
            },
            "applicableGuidelines": ["communication_first", "resource_reservation"],
            "enforcementLevel": "MODERATE",
            "allowExceptions": true
        });

        let request = create_test_request("vibe/guideline/enforce", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("enforcementId").is_some());
            assert!(result_value.get("complianceStatus").is_some());
            assert!(result_value.get("complianceScore").is_some());
        }
    }

    #[tokio::test]
    async fn test_learning_capture_basic_functionality() {
        let server = create_test_server();
        let params = json!({
            "capturingAgentId": Uuid::new_v4().to_string(),
            "coordinationSession": {
                "session_id": "coord_001",
                "participants": 3,
                "duration": "2h"
            },
            "outcomeData": {
                "success": true,
                "conflicts_resolved": 2,
                "efficiency_score": 0.85
            },
            "successMetrics": {
                "completion_time": "1.5h",
                "quality_score": 0.9
            },
            "lessonsLearned": [
                "Early communication prevented conflicts",
                "Resource reservation was effective"
            ],
            "improvementOpportunities": [
                "Better conflict prediction",
                "More granular resource management"
            ]
        });

        let request = create_test_request("vibe/learning/capture", params);
        let result = server.handle_request(request).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.is_some());

        let response = response.unwrap();
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        // Verify response structure
        if let Some(result_value) = response.result {
            assert!(result_value.get("learningRecordId").is_some());
            assert!(result_value.get("extractedPatterns").is_some());
            assert!(result_value.get("knowledgeQualityScore").is_some());
        }
    }

    // Error Handling Tests

    #[tokio::test]
    async fn test_invalid_agent_id_handling() {
        let server = create_test_server();
        let params = json!({
            "coordinatorAgentId": "invalid-uuid",
            "workSequences": [],
            "involvedAgents": [],
            "projectScopes": [],
            "resourceRequirements": {}
        });

        let request = create_test_request("vibe/schedule/coordinate", params);
        let result = server.handle_request(request).await;

        match result {
            Ok(response) => {
                assert!(response.is_some());
                let response = response.unwrap();
                assert!(response.error.is_some());
                assert!(response.result.is_none());
            }
            Err(e) => {
                // The test expects the request to be handled and return an error response,
                // not fail at the transport level. This suggests a fundamental issue.
                eprintln!(
                    "Unexpected error in test_invalid_agent_id_handling: {:?}",
                    e
                );
                panic!("Request should not fail at transport level");
            }
        }
    }

    #[tokio::test]
    async fn test_missing_parameters_handling() {
        let server = create_test_server();
        let params = json!({
            // Missing required parameters
        });

        let request = create_test_request("vibe/conflict/predict", params);
        let result = server.handle_request(request).await;

        // Should return an error for missing parameters
        assert!(result.is_err());
    }

    // Integration Tests for Coordination Workflows

    #[tokio::test]
    async fn test_coordination_workflow_integration() {
        let server = create_test_server();
        let agent_id = Uuid::new_v4().to_string();

        // 1. First, predict conflicts
        let conflict_predict_params = json!({
            "analyzerAgentId": &agent_id,
            "plannedActions": [{"action": "modify", "file": "src/main.rs"}],
            "activeWorkflows": [],
            "resourceMap": {"files": ["src/main.rs"]}
        });

        let request = create_test_request("vibe/conflict/predict", conflict_predict_params);
        let result = server.handle_request(request).await;
        assert!(result.is_ok());

        // 2. Then, reserve resources
        let resource_reserve_params = json!({
            "reservingAgentId": &agent_id,
            "resourcePaths": ["src/main.rs"],
            "reservationType": "EXCLUSIVE",
            "reservationDuration": "1h",
            "exclusiveAccess": true,
            "allowedOperations": ["read", "write"],
            "justification": "Implementing new feature"
        });

        let request = create_test_request("vibe/resource/reserve", resource_reserve_params);
        let result = server.handle_request(request).await;
        assert!(result.is_ok());

        // 3. Finally, capture learning
        let learning_capture_params = json!({
            "capturingAgentId": &agent_id,
            "coordinationSession": {"workflow": "conflict_prevention"},
            "outcomeData": {"success": true},
            "successMetrics": {"efficiency": 0.9},
            "lessonsLearned": ["Proactive conflict prediction works"],
            "improvementOpportunities": ["Faster resource reservation"]
        });

        let request = create_test_request("vibe/learning/capture", learning_capture_params);
        let result = server.handle_request(request).await;
        assert!(result.is_ok());
    }
}
