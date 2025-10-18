use anyhow::{Context, Result};
use reqwest::Client;
use tracing::{debug, info};

const JBCT_CODER_URL: &str =
    "https://raw.githubusercontent.com/siy/coding-technology/main/jbct-coder.md";

pub struct JbctGitHubClient {
    client: Client,
}

impl JbctGitHubClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Fetch the latest jbct-coder.md from GitHub
    pub async fn fetch_jbct_coder(&self) -> Result<String> {
        info!("Fetching jbct-coder.md from GitHub");
        debug!("URL: {}", JBCT_CODER_URL);

        let response = self
            .client
            .get(JBCT_CODER_URL)
            .send()
            .await
            .context("Failed to fetch jbct-coder.md from GitHub")?;

        if !response.status().is_success() {
            anyhow::bail!("GitHub returned error status: {}", response.status());
        }

        let content = response
            .text()
            .await
            .context("Failed to read response body")?;

        info!(
            "Successfully fetched jbct-coder.md ({} bytes)",
            content.len()
        );

        Ok(content)
    }
}

impl Default for JbctGitHubClient {
    fn default() -> Self {
        Self::new()
    }
}
