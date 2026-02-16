use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::PathBuf;
use tracing::warn;

const DEFAULT_AUTH_BASE_URL: &str = "https://auth.loopflow.studio";
const DEFAULT_EXECUTOR_IMAGE: &str = "loopflow/agent:latest";

/// Auth config from `~/.lf/lfd.yaml`.
///
/// `provider` selects the auth strategy:
/// - `"local"` (default): loopback only, no auth for local connections
/// - `"static"`: validate against a pre-shared token
/// - `"loopflow.studio"`: register with loopflow.studio, validate via API
#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_provider")]
    pub provider: String,
    pub token: Option<String>,
    #[serde(default = "default_base_url")]
    pub base_url: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            token: None,
            base_url: default_base_url(),
        }
    }
}

fn default_provider() -> String {
    "local".to_string()
}

fn default_base_url() -> String {
    DEFAULT_AUTH_BASE_URL.to_string()
}

#[derive(Debug, Clone, Default)]
pub struct LfdConfig {
    pub mode: Mode,
    pub service_manager: ServiceManager,
    pub runtime_backend: RuntimeBackend,
    pub storage: StorageType,
    pub auth: AuthConfig,
    pub executor: ExecutorConfig,
    pub github: GitHubConfig,
}

impl LfdConfig {
    pub fn load() -> Result<Self> {
        let path = config_path();
        let mut config: RawLfdConfig = match std::fs::read_to_string(&path) {
            Ok(content) => serde_yaml::from_str(&content)
                .with_context(|| format!("invalid lfd config at {}", path.display()))?,
            Err(err) if err.kind() == ErrorKind::NotFound => RawLfdConfig::default(),
            Err(err) => {
                warn!(
                    path = %path.display(),
                    error = %err,
                    "failed reading lfd config, using defaults"
                );
                RawLfdConfig::default()
            }
        };

        config.apply_env_overrides()?;
        config.resolve()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawLfdConfig {
    #[serde(default)]
    mode: Mode,
    service_manager: Option<ServiceManager>,
    runtime_backend: Option<RuntimeBackend>,
    storage: Option<StorageType>,
    #[serde(default)]
    auth: AuthConfig,
    #[serde(default)]
    executor: RawExecutorConfig,
    #[serde(default)]
    github: GitHubConfig,
}

impl RawLfdConfig {
    fn apply_env_overrides(&mut self) -> Result<()> {
        if let Ok(value) = std::env::var("LFD_MODE") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.mode = Mode::parse(trimmed)?;
            }
        }

        if let Ok(value) = std::env::var("LFD_AUTH_PROVIDER") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.auth.provider = trimmed.to_string();
            }
        }

        if let Ok(value) = std::env::var("LFD_AUTH_TOKEN") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.auth.token = Some(trimmed.to_string());
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

        Ok(())
    }

    fn resolve(self) -> Result<LfdConfig> {
        if self.service_manager.is_some() {
            bail!("`service_manager` is managed by `mode`; remove this key");
        }
        if self.runtime_backend.is_some() {
            bail!("`runtime_backend` is managed by `mode`; remove this key");
        }
        if self.storage.is_some() {
            bail!("`storage` is managed by `mode`; remove this key");
        }
        if self.executor.r#type.is_some() {
            bail!("`executor.type` is managed by `mode`; remove this key");
        }

        let profile = ModeProfile::for_mode(self.mode);

        Ok(LfdConfig {
            mode: self.mode,
            service_manager: profile.service_manager,
            runtime_backend: profile.runtime_backend,
            storage: profile.storage,
            auth: self.auth,
            executor: ExecutorConfig {
                r#type: profile.executor_type,
                image: self.executor.image,
                credentials: self.executor.credentials,
            },
            github: self.github,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ModeProfile {
    service_manager: ServiceManager,
    runtime_backend: RuntimeBackend,
    storage: StorageType,
    executor_type: ExecutorType,
}

impl ModeProfile {
    fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Native => Self {
                service_manager: ServiceManager::default_for_os(),
                runtime_backend: RuntimeBackend::Native,
                storage: StorageType::Sqlite,
                executor_type: ExecutorType::Local,
            },
            Mode::Container => Self {
                service_manager: ServiceManager::default_for_os(),
                runtime_backend: RuntimeBackend::Compose,
                storage: StorageType::Postgres,
                executor_type: ExecutorType::Docker,
            },
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
pub enum Mode {
    #[default]
    Native,
    Container,
}

impl Mode {
    fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "native" => Ok(Self::Native),
            "container" => Ok(Self::Container),
            _ => bail!("invalid LFD_MODE value '{raw}'; expected 'native' or 'container'"),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Container => "container",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceManager {
    Launchd,
    Systemd,
}

impl ServiceManager {
    fn default_for_os() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::Systemd
        }

        #[cfg(not(target_os = "linux"))]
        {
            Self::Launchd
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Launchd => "launchd",
            Self::Systemd => "systemd",
        }
    }
}

impl Default for ServiceManager {
    fn default() -> Self {
        Self::default_for_os()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBackend {
    #[default]
    Native,
    Compose,
}

impl RuntimeBackend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Compose => "compose",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StorageType {
    #[default]
    Sqlite,
    Postgres,
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

#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    pub r#type: ExecutorType,
    pub image: String,
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

#[derive(Debug, Clone, Deserialize)]
struct RawExecutorConfig {
    r#type: Option<ExecutorType>,
    #[serde(default = "default_executor_image")]
    image: String,
    #[serde(default)]
    credentials: ExecutorCredentialsConfig,
}

impl Default for RawExecutorConfig {
    fn default() -> Self {
        Self {
            r#type: None,
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
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        vars: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn snapshot(vars: &[&'static str]) -> Self {
            Self {
                vars: vars
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.vars {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    #[test]
    fn mode_native_is_default() {
        let config = RawLfdConfig::default().resolve().expect("default resolves");
        assert_eq!(config.mode, Mode::Native);
        assert_eq!(config.runtime_backend, RuntimeBackend::Native);
        assert_eq!(config.storage, StorageType::Sqlite);
        assert_eq!(config.executor.r#type, ExecutorType::Local);
    }

    #[test]
    fn mode_container_sets_container_profile() {
        let config: RawLfdConfig = serde_yaml::from_str("mode: container").expect("yaml parses");
        let resolved = config.resolve().expect("container resolves");

        assert_eq!(resolved.mode, Mode::Container);
        assert_eq!(resolved.runtime_backend, RuntimeBackend::Compose);
        assert_eq!(resolved.storage, StorageType::Postgres);
        assert_eq!(resolved.executor.r#type, ExecutorType::Docker);
    }

    #[test]
    fn explicit_runtime_override_in_yaml_is_rejected() {
        let raw = r#"
mode: container
runtime_backend: native
"#;
        let config: RawLfdConfig = serde_yaml::from_str(raw).expect("yaml parses");
        let err = config.resolve().expect_err("override should fail");
        assert_eq!(
            err.to_string(),
            "`runtime_backend` is managed by `mode`; remove this key"
        );
    }

    #[test]
    fn explicit_executor_type_in_yaml_is_rejected() {
        let raw = r#"
executor:
  type: docker
"#;
        let config: RawLfdConfig = serde_yaml::from_str(raw).expect("yaml parses");
        let err = config
            .resolve()
            .expect_err("executor type override should fail");
        assert_eq!(
            err.to_string(),
            "`executor.type` is managed by `mode`; remove this key"
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
        let result = serde_yaml::from_str::<RawLfdConfig>(raw);
        assert!(result.is_err());
    }

    #[test]
    fn env_overrides_allowed_fields() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&[
            "LFD_MODE",
            "LFD_AUTH_PROVIDER",
            "LFD_AUTH_TOKEN",
            "LFD_EXECUTOR_IMAGE",
            "LFD_GITHUB_WEBHOOK_SECRET",
            "LFD_GITHUB_TOKEN",
        ]);
        std::env::set_var("LFD_MODE", "container");
        std::env::set_var("LFD_AUTH_PROVIDER", "static");
        std::env::set_var("LFD_AUTH_TOKEN", "env-token-456");
        std::env::set_var("LFD_EXECUTOR_IMAGE", "loopflow/agent:env");
        std::env::set_var("LFD_GITHUB_WEBHOOK_SECRET", "env-secret");
        std::env::set_var("LFD_GITHUB_TOKEN", "ghp_env");

        let mut config = RawLfdConfig::default();
        config.apply_env_overrides().expect("overrides apply");
        let resolved = config.resolve().expect("resolved");

        assert_eq!(resolved.mode, Mode::Container);
        assert_eq!(resolved.storage, StorageType::Postgres);
        assert_eq!(resolved.executor.r#type, ExecutorType::Docker);
        assert_eq!(resolved.auth.provider, "static");
        assert_eq!(resolved.auth.token, Some("env-token-456".to_string()));
        assert_eq!(resolved.executor.image, "loopflow/agent:env");
        assert_eq!(resolved.github.webhook_secret, "env-secret");
        assert_eq!(resolved.github.token, Some("ghp_env".to_string()));
    }

    #[test]
    fn invalid_mode_env_override_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_MODE"]);
        std::env::set_var("LFD_MODE", "invalid");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("invalid mode should fail");

        assert_eq!(
            err.to_string(),
            "invalid LFD_MODE value 'invalid'; expected 'native' or 'container'"
        );
    }

    #[test]
    fn load_invalid_yaml_returns_error() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&[
            "LFD_MODE",
            "LFD_AUTH_PROVIDER",
            "LFD_AUTH_TOKEN",
            "LFD_EXECUTOR_IMAGE",
            "LFD_GITHUB_WEBHOOK_SECRET",
            "LFD_GITHUB_TOKEN",
        ]);
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
