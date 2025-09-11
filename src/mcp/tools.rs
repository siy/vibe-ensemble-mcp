use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use crate::{error::Result, server::AppState};
use super::types::{CallToolRequest, CallToolResponse, Tool, ToolContent};

#[async_trait]
pub trait ToolHandler: Send + Sync {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse>;
    fn definition(&self) -> Tool;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn ToolHandler>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register<T: ToolHandler + 'static>(&mut self, tool: T) {
        let name = tool.definition().name.clone();
        self.tools.insert(name, Box::new(tool));
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn ToolHandler>> {
        self.tools.get(name)
    }

    pub fn list_tools(&self) -> Vec<Tool> {
        self.tools.values().map(|tool| tool.definition()).collect()
    }

    pub async fn call_tool(
        &self,
        state: &AppState,
        request: CallToolRequest,
    ) -> Result<CallToolResponse> {
        match self.get_tool(&request.name) {
            Some(tool) => tool.call(state, request.arguments).await,
            None => Ok(CallToolResponse {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: format!("Tool '{}' not found", request.name),
                }],
                is_error: Some(true),
            }),
        }
    }
}

pub fn create_success_response(message: &str) -> CallToolResponse {
    CallToolResponse {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: message.to_string(),
        }],
        is_error: None,
    }
}

pub fn create_error_response(error: &str) -> CallToolResponse {
    CallToolResponse {
        content: vec![ToolContent {
            content_type: "text".to_string(),
            text: error.to_string(),
        }],
        is_error: Some(true),
    }
}

// Utility function to extract and validate parameters
pub fn extract_param<T>(arguments: &Option<Value>, key: &str) -> Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    match arguments {
        Some(Value::Object(map)) => {
            match map.get(key) {
                Some(value) => serde_json::from_value(value.clone())
                    .map_err(|e| crate::error::AppError::BadRequest(
                        format!("Invalid parameter '{}': {}", key, e)
                    )),
                None => Err(crate::error::AppError::BadRequest(
                    format!("Missing required parameter '{}'", key)
                )),
            }
        }
        _ => Err(crate::error::AppError::BadRequest(
            "Arguments must be an object".to_string()
        )),
    }
}

pub fn extract_optional_param<T>(arguments: &Option<Value>, key: &str) -> Result<Option<T>>
where
    T: for<'de> serde::Deserialize<'de>,
{
    match arguments {
        Some(Value::Object(map)) => {
            match map.get(key) {
                Some(value) if !value.is_null() => {
                    let parsed: T = serde_json::from_value(value.clone())
                        .map_err(|e| crate::error::AppError::BadRequest(
                            format!("Invalid parameter '{}': {}", key, e)
                        ))?;
                    Ok(Some(parsed))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}