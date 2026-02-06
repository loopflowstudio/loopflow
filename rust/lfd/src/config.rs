use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_AUTH_BASE_URL: &str = "https://auth.loopflow.studio";

/// Auth config from `~/.lf/lfd.yaml`.
///
/// loopflow.studio is hard-coded as the auth provider. Auth is required
/// automatically when binding to a non-loopback address — there's no toggle.
/// `base_url` exists only for dev/staging overrides.
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            base_url: default_base_url(),
        }
    }
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
