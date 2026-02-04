use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_AUTH_BASE_URL: &str = "https://loopflow.studio";

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    pub provider: Option<String>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

fn default_base_url() -> String {
    DEFAULT_AUTH_BASE_URL.to_string()
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LfdConfig {
    #[serde(default)]
    pub auth: AuthConfig,
}

impl LfdConfig {
    pub fn load() -> Self {
        let config_path = config_path();
        if !config_path.exists() {
            return Self::default();
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(_) => return Self::default(),
        };

        serde_yaml::from_str(&content).unwrap_or_default()
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("lfd.yaml")
}
