use anyhow::{Context, Result};
use thiserror::Error;
use tracing::{debug, warn};

use super::JbctConfig;

const JBCT_SOURCE_URL: &str = "https://pragmatica.dev/";

#[derive(Debug, Error)]
pub enum JbctParseError {
    #[error("Failed to parse JBCT frontmatter")]
    FrontmatterParse,

    #[error("JBCT version not found in document")]
    VersionNotFound,

    #[error("Core Principles section not found")]
    CorePrinciplesNotFound,

    #[error("API Usage Patterns section not found")]
    ApiPatternsNotFound,
}

pub struct JbctDocument;

impl JbctDocument {
    /// Parse jbct-coder.md content into JbctConfig
    pub fn parse(content: &str) -> Result<JbctConfig> {
        debug!("Parsing JBCT document ({} bytes)", content.len());

        let version = Self::extract_version(content)?;
        debug!("Extracted JBCT version: {}", version);

        let rules = Self::extract_rules(content)?;
        debug!("Extracted rules section ({} bytes)", rules.len());

        let patterns = Self::extract_patterns(content)?;
        debug!("Extracted patterns section ({} bytes)", patterns.len());

        Ok(JbctConfig {
            version,
            rules,
            patterns,
            source_url: JBCT_SOURCE_URL.to_string(),
        })
    }

    /// Extract version from frontmatter (e.g., "v1.6.1")
    fn extract_version(content: &str) -> Result<String> {
        // Look for "Java Backend Coding Technology v1.6.1" pattern in description
        for line in content.lines().take(20) {
            if line.contains("Java Backend Coding Technology") && line.contains(" v") {
                // Extract version like "v1.6.1"
                if let Some(version_start) = line.find(" v") {
                    let version_str = &line[version_start + 2..];
                    if let Some(version_end) = version_str.find(|c: char| c.is_whitespace()) {
                        let version = version_str[..version_end].to_string();
                        debug!("Found version in description: {}", version);
                        return Ok(version);
                    } else {
                        // Version is at end of line
                        let version = version_str.trim().to_string();
                        debug!("Found version at line end: {}", version);
                        return Ok(version);
                    }
                }
            }
        }

        warn!("Version not found in frontmatter, using default");
        Ok("unknown".to_string())
    }

    /// Extract rules section (Critical Directive + Core Principles)
    fn extract_rules(content: &str) -> Result<String> {
        let mut rules = String::new();

        // Find "## Critical Directive" section
        if let Some(critical_start) = content.find("## Critical Directive") {
            // Find next major section marker
            let critical_content = &content[critical_start..];

            // End at "## Purpose" or similar section
            let critical_end = critical_content
                .find("\n## Purpose")
                .or_else(|| critical_content.find("\n---\n\n## Purpose"))
                .unwrap_or(critical_content.len());

            rules.push_str(&critical_content[..critical_end]);
            rules.push_str("\n\n");
        }

        // Find "## Core Principles" section
        if let Some(core_start) = content.find("## Core Principles") {
            let core_content = &content[core_start..];

            // End at "## API Usage Patterns" or similar
            let core_end = core_content
                .find("\n## API Usage Patterns")
                .or_else(|| core_content.find("\n---\n\n## "))
                .unwrap_or_else(|| {
                    // Find any next major section
                    core_content[100..]
                        .find("\n## ")
                        .map(|pos| pos + 100)
                        .unwrap_or(core_content.len())
                });

            rules.push_str(&core_content[..core_end]);
        }

        if rules.is_empty() {
            return Err(JbctParseError::CorePrinciplesNotFound.into());
        }

        Ok(rules.trim().to_string())
    }

    /// Extract patterns section (API Usage + remaining content)
    fn extract_patterns(content: &str) -> Result<String> {
        // Start from "## API Usage Patterns"
        let patterns_start = content
            .find("## API Usage Patterns")
            .context("API Usage Patterns section not found")?;

        // Take everything from API Usage Patterns to end (or stop before test sections if any)
        let patterns_content = &content[patterns_start..];

        Ok(patterns_content.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_extraction() {
        let content = r#"---
name: jbct-coder
title: Java Backend Coding Technology Agent
description: Specialized agent for generating business logic code using Java Backend Coding Technology v1.6.1 with Pragmatica Lite Core 0.8.3.
---"#;

        let version = JbctDocument::extract_version(content).unwrap();
        assert_eq!(version, "1.6.1");
    }
}
