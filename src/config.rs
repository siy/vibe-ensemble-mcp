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
        match self.permission_mode.as_str() {
            "bypass" | "inherit" | "file" => Ok(()),
            _ => Err(format!(
                "Invalid permission mode '{}'. Valid options: bypass, inherit, file",
                self.permission_mode
            )),
        }
    }
}
