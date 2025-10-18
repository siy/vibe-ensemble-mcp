pub mod github;
pub mod parser;

pub use github::JbctGitHubClient;
pub use parser::{JbctDocument, JbctParseError};

/// JBCT version and configuration
#[derive(Debug, Clone)]
pub struct JbctConfig {
    pub version: String,
    pub rules: String,
    pub patterns: String,
    pub source_url: String,
}
