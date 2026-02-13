use serde::Deserialize;
use std::path::PathBuf;

const DEFAULT_AUTH_BASE_URL: &str = "https://auth.loopflow.studio";
const DEFAULT_EXECUTOR_IMAGE: &str = "loopflow/agent:latest";

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
    #[serde(default)]
    pub executor: ExecutorConfig,
}

impl LfdConfig {
    pub fn load() -> Self {
        let config_path = config_path();
        if !config_path.exists() {
            let mut config = Self::default();
            config.apply_env_overrides();
            return config;
        }

        let content = match std::fs::read_to_string(&config_path) {
            Ok(content) => content,
            Err(_) => {
                let mut config = Self::default();
                config.apply_env_overrides();
                return config;
            }
        };

        let mut config: Self = serde_yaml::from_str(&content).unwrap_or_default();
        config.apply_env_overrides();
        config
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(value) = std::env::var("LFD_EXECUTOR_TYPE") {
            self.executor.r#type = match value.as_str() {
                "docker" => ExecutorType::Docker,
                _ => ExecutorType::Local,
            };
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_IMAGE") {
            if !value.trim().is_empty() {
                self.executor.image = value;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorType {
    #[default]
    Local,
    Docker,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutorCredentialsConfig {
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub mounts: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutorConfig {
    #[serde(default)]
    pub r#type: ExecutorType,
    #[serde(default = "default_executor_image")]
    pub image: String,
    #[serde(default)]
    pub credentials: ExecutorCredentialsConfig,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            r#type: ExecutorType::Local,
            image: default_executor_image(),
            credentials: ExecutorCredentialsConfig::default(),
        }
    }
}

fn default_executor_image() -> String {
    DEFAULT_EXECUTOR_IMAGE.to_string()
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
        .join("lfd.yaml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn deserialize_executor_config_from_yaml() {
        let raw = r#"
executor:
  type: docker
  image: loopflow/agent:test
  credentials:
    env:
      - ANTHROPIC_API_KEY
    mounts:
      - ~/.claude:/home/agent/.claude
"#;
        let config: LfdConfig = serde_yaml::from_str(raw).expect("yaml should parse");
        assert_eq!(config.executor.r#type, ExecutorType::Docker);
        assert_eq!(config.executor.image, "loopflow/agent:test");
        assert_eq!(
            config.executor.credentials.env,
            vec!["ANTHROPIC_API_KEY".to_string()]
        );
        assert_eq!(
            config.executor.credentials.mounts,
            vec!["~/.claude:/home/agent/.claude".to_string()]
        );
    }

    #[test]
    fn env_overrides_executor_settings() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_EXECUTOR_TYPE", "docker");
        std::env::set_var("LFD_EXECUTOR_IMAGE", "loopflow/agent:env");

        let mut config = LfdConfig::default();
        config.apply_env_overrides();

        std::env::remove_var("LFD_EXECUTOR_TYPE");
        std::env::remove_var("LFD_EXECUTOR_IMAGE");

        assert_eq!(config.executor.r#type, ExecutorType::Docker);
        assert_eq!(config.executor.image, "loopflow/agent:env");
    }
}
