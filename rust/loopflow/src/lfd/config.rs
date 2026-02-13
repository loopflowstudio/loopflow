use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::PathBuf;
use tracing::warn;

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
    #[serde(default)]
    pub github: GitHubConfig,
}

impl LfdConfig {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let mut config: Self = match std::fs::read_to_string(&path) {
            Ok(content) => serde_yaml::from_str(&content)
                .with_context(|| format!("invalid lfd config at {}", path.display()))?,
            Err(err) if err.kind() == ErrorKind::NotFound => Self::default(),
            Err(err) => {
                warn!(
                    path = %path.display(),
                    error = %err,
                    "failed reading lfd config, using defaults"
                );
                Self::default()
            }
        };

        config.apply_env_overrides();
        Ok(config)
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(value) = std::env::var("LFD_EXECUTOR_TYPE") {
            if let Some(r#type) = ExecutorType::from_env(&value) {
                self.executor.r#type = r#type;
            }
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_IMAGE") {
            if !value.trim().is_empty() {
                self.executor.image = value;
            }
        }

        if let Ok(value) = std::env::var("LFD_GITHUB_WEBHOOK_SECRET") {
            self.github.webhook_secret = value;
        }

        if let Ok(value) = std::env::var("LFD_GITHUB_TOKEN") {
            let token = value.trim();
            self.github.token = if token.is_empty() {
                None
            } else {
                Some(token.to_string())
            };
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GitHubConfig {
    #[serde(default)]
    pub webhook_secret: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExecutorType {
    #[default]
    Local,
    Docker,
}

impl ExecutorType {
    fn from_env(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "local" => Some(Self::Local),
            "docker" => Some(Self::Docker),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutorCredentialsConfig {
    #[serde(default)]
    pub env: Vec<String>,
    #[serde(default)]
    pub mounts: Vec<CredentialMount>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct CredentialMount(String);

impl CredentialMount {
    pub fn name(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for CredentialMount {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let name = value.trim();
        if name.is_empty() {
            return Err("credential mount name must not be empty".to_string());
        }
        if name.contains(':') {
            return Err(
                "credential mounts no longer accept host:container paths; use named mounts"
                    .to_string(),
            );
        }
        if name.starts_with('/') {
            return Err("credential mount name must not be an absolute path".to_string());
        }
        Ok(Self(name.to_string()))
    }
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
    use tempfile::tempdir;

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
      - claude
github:
  webhook_secret: hook-secret
  token: ghp_test
"#;
        let config: LfdConfig = serde_yaml::from_str(raw).expect("yaml should parse");
        assert_eq!(config.executor.r#type, ExecutorType::Docker);
        assert_eq!(config.executor.image, "loopflow/agent:test");
        assert_eq!(config.github.webhook_secret, "hook-secret");
        assert_eq!(config.github.token, Some("ghp_test".to_string()));
        assert_eq!(
            config.executor.credentials.env,
            vec!["ANTHROPIC_API_KEY".to_string()]
        );
        assert_eq!(
            config.executor.credentials.mounts,
            vec![CredentialMount::try_from("claude".to_string()).expect("valid mount")]
        );
    }

    #[test]
    fn raw_credential_mount_paths_are_rejected() {
        let raw = r#"
executor:
  credentials:
    mounts:
      - ~/.claude:/home/agent/.claude
"#;
        let result = serde_yaml::from_str::<LfdConfig>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn env_overrides_executor_settings() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_EXECUTOR_TYPE", "docker");
        std::env::set_var("LFD_EXECUTOR_IMAGE", "loopflow/agent:env");
        std::env::set_var("LFD_GITHUB_WEBHOOK_SECRET", "env-secret");
        std::env::set_var("LFD_GITHUB_TOKEN", "ghp_env");

        let mut config = LfdConfig::default();
        config.apply_env_overrides();

        std::env::remove_var("LFD_EXECUTOR_TYPE");
        std::env::remove_var("LFD_EXECUTOR_IMAGE");
        std::env::remove_var("LFD_GITHUB_WEBHOOK_SECRET");
        std::env::remove_var("LFD_GITHUB_TOKEN");

        assert_eq!(config.executor.r#type, ExecutorType::Docker);
        assert_eq!(config.executor.image, "loopflow/agent:env");
        assert_eq!(config.github.webhook_secret, "env-secret");
        assert_eq!(config.github.token, Some("ghp_env".to_string()));
    }

    #[test]
    fn invalid_executor_type_env_value_does_not_override_config() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_EXECUTOR_TYPE", "unknown");

        let mut config = LfdConfig::default();
        config.executor.r#type = ExecutorType::Docker;
        config.apply_env_overrides();

        std::env::remove_var("LFD_EXECUTOR_TYPE");

        assert_eq!(config.executor.r#type, ExecutorType::Docker);
    }

    #[test]
    fn load_invalid_yaml_returns_error() {
        let _guard = env_lock().lock().expect("env lock");
        let tmp = tempdir().expect("tempdir");
        let lf_dir = tmp.path().join(".lf");
        std::fs::create_dir_all(&lf_dir).expect("lf dir");
        std::fs::write(lf_dir.join("lfd.yaml"), "executor: [").expect("write config");

        let original_home = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        let result = LfdConfig::load();
        match original_home {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }

        assert!(result.is_err());
    }
}
