#[derive(Debug, Clone)]
pub struct Config {
    pub database_path: String,
    pub host: String,
    pub port: u16,
    pub no_respawn: bool,
    pub permission_mode: String,
}

impl Config {
    pub fn database_url(&self) -> String {
        format!("sqlite:{}?mode=rwc", self.database_path)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn validate_permission_mode(&self) -> Result<(), String> {
        use crate::permissions::PermissionMode;
        self.permission_mode.parse::<PermissionMode>()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}
