use crate::permissions::PermissionMode;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub host: String,
    pub port: u16,
    pub no_respawn: bool,
    pub permission_mode: PermissionMode,
    pub enable_websocket: bool,
    pub websocket_auth_required: bool,
    pub client_tool_timeout_secs: u64,
    pub max_concurrent_client_requests: usize,
    pub sse_echo_allowlist: HashSet<String>,
}

impl Config {
    pub fn database_url(&self) -> String {
        format!("sqlite:{}?mode=rwc", self.database_path)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn websocket_url(&self) -> String {
        format!("ws://{}:{}/ws", self.host, self.port)
    }
}
