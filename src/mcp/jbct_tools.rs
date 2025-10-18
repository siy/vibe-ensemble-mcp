use anyhow::Context;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::database::projects::{Project, UpdateProjectRequest};
use crate::error::Result;
use crate::jbct::{JbctDocument, JbctGitHubClient};
use crate::server::AppState;

use super::tools::{create_json_error_response, create_json_success_response, ToolHandler};
use super::types::{CallToolResponse, Tool};

const JBCT_WEBSITE_URL: &str = "https://pragmatica.dev/";

#[derive(Debug, Deserialize)]
struct ConfigureJbctRequest {
    project_id: String,
}

#[derive(Debug, Deserialize)]
struct CheckJbctUpdatesRequest {
    project_id: String,
}

#[derive(Debug, Serialize)]
struct ConfigureJbctResponse {
    success: bool,
    version: String,
    url: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct CheckJbctUpdatesResponse {
    update_available: bool,
    current_version: Option<String>,
    latest_version: String,
    message: String,
}

/// Tool struct for configure_jbct_for_project
pub struct ConfigureJbctForProjectTool;

#[async_trait]
impl ToolHandler for ConfigureJbctForProjectTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = arguments
            .as_ref()
            .and_then(|args| args.get("project_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: project_id"))?
            .to_string();

        match configure_jbct_for_project_impl(&state.db, &project_id).await {
            Ok(response) => Ok(create_json_success_response(response)),
            Err(e) => Ok(create_json_error_response(&e.to_string())),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "configure_jbct_for_project".to_string(),
            description: "Configure Java Backend Coding Technology (JBCT) for a project. Fetches the latest jbct-coder.md from GitHub and applies it as project rules and patterns. See https://pragmatica.dev/ for more information.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "The project identifier (repository name)"
                    }
                },
                "required": ["project_id"]
            }),
        }
    }
}

/// Tool struct for check_jbct_updates
pub struct CheckJbctUpdatesTool;

#[async_trait]
impl ToolHandler for CheckJbctUpdatesTool {
    async fn call(&self, state: &AppState, arguments: Option<Value>) -> Result<CallToolResponse> {
        let project_id: String = arguments
            .as_ref()
            .and_then(|args| args.get("project_id"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: project_id"))?
            .to_string();

        match check_jbct_updates_impl(&state.db, &project_id).await {
            Ok(response) => Ok(create_json_success_response(response)),
            Err(e) => Ok(create_json_error_response(&e.to_string())),
        }
    }

    fn definition(&self) -> Tool {
        Tool {
            name: "check_jbct_updates".to_string(),
            description: "Check if a newer version of JBCT is available for a project that already has JBCT configured.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project_id": {
                        "type": "string",
                        "description": "The project identifier (repository name)"
                    }
                },
                "required": ["project_id"]
            }),
        }
    }
}

/// Implementation function for configure_jbct_for_project
async fn configure_jbct_for_project_impl(
    pool: &crate::database::DbPool,
    project_id: &str,
) -> anyhow::Result<Value> {
    let req = ConfigureJbctRequest {
        project_id: project_id.to_string(),
    };

    info!("Configuring JBCT for project: {}", req.project_id);

    // Check if project exists
    let project = Project::get_by_id(pool, &req.project_id)
        .await?
        .context("Project not found")?;

    if project.jbct_enabled {
        warn!("JBCT already enabled for project: {}", req.project_id);
        let version = project
            .jbct_version
            .unwrap_or_else(|| "unknown".to_string());
        return Ok(serde_json::to_value(ConfigureJbctResponse {
            success: false,
            version: version.clone(),
            url: JBCT_WEBSITE_URL.to_string(),
            message: format!(
                "JBCT is already configured for this project (version: {}). Use check_jbct_updates to check for updates.",
                version
            ),
        })?);
    }

    // Fetch latest jbct-coder.md from GitHub
    let client = JbctGitHubClient::new();
    let content = client
        .fetch_jbct_coder()
        .await
        .context("Failed to fetch JBCT document from GitHub")?;

    // Parse document
    let jbct_config = JbctDocument::parse(&content).context("Failed to parse JBCT document")?;

    info!(
        "Parsed JBCT v{} ({} bytes rules, {} bytes patterns)",
        jbct_config.version,
        jbct_config.rules.len(),
        jbct_config.patterns.len()
    );

    // Update project with JBCT configuration
    Project::update(
        pool,
        &req.project_id,
        UpdateProjectRequest {
            path: None,
            short_description: None,
            rules: Some(jbct_config.rules),
            patterns: Some(jbct_config.patterns),
            jbct_enabled: Some(true),
            jbct_version: Some(jbct_config.version.clone()),
            jbct_url: Some(jbct_config.source_url.clone()),
        },
    )
    .await
    .context("Failed to update project with JBCT configuration")?;

    info!(
        "Successfully configured JBCT v{} for project: {}",
        jbct_config.version, req.project_id
    );

    Ok(serde_json::to_value(ConfigureJbctResponse {
        success: true,
        version: jbct_config.version,
        url: JBCT_WEBSITE_URL.to_string(),
        message: format!(
            "Successfully configured Java Backend Coding Technology for this project. Learn more at {}",
            JBCT_WEBSITE_URL
        ),
    })?)
}

/// Implementation function for check_jbct_updates
async fn check_jbct_updates_impl(
    pool: &crate::database::DbPool,
    project_id: &str,
) -> anyhow::Result<Value> {
    let req = CheckJbctUpdatesRequest {
        project_id: project_id.to_string(),
    };

    info!("Checking JBCT updates for project: {}", req.project_id);

    // Check if project exists and has JBCT enabled
    let project = Project::get_by_id(pool, &req.project_id)
        .await?
        .context("Project not found")?;

    if !project.jbct_enabled {
        return Ok(serde_json::to_value(CheckJbctUpdatesResponse {
            update_available: false,
            current_version: None,
            latest_version: "N/A".to_string(),
            message:
                "JBCT is not enabled for this project. Use configure_jbct_for_project to enable it."
                    .to_string(),
        })?);
    }

    // Fetch latest version from GitHub
    let client = JbctGitHubClient::new();
    let content = client
        .fetch_jbct_coder()
        .await
        .context("Failed to fetch JBCT document from GitHub")?;

    let jbct_config = JbctDocument::parse(&content).context("Failed to parse JBCT document")?;

    let current_version = project
        .jbct_version
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let latest_version = jbct_config.version.clone();

    let update_available = current_version != latest_version;

    let message = if update_available {
        format!(
            "Update available! Current version: {}, Latest version: {}. Use configure_jbct_for_project to update.",
            current_version, latest_version
        )
    } else {
        format!(
            "Project is using the latest JBCT version: {}",
            current_version
        )
    };

    info!("{}", message);

    Ok(serde_json::to_value(CheckJbctUpdatesResponse {
        update_available,
        current_version: Some(current_version),
        latest_version,
        message,
    })?)
}
