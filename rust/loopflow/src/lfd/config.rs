use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;
use tracing::warn;

const DEFAULT_AUTH_BASE_URL: &str = "https://auth.loopflow.studio";
const DEFAULT_EXECUTOR_IMAGE: &str = "loopflow/agent:latest";
const DEFAULT_AGENT_TIMEOUT_SECS: u64 = 45 * 60;
const DEFAULT_EXECUTOR_MEMORY_LIMIT_BYTES: i64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_EXECUTOR_MEMORY_SWAP_LIMIT_BYTES: i64 = 4 * 1024 * 1024 * 1024;
const DEFAULT_EXECUTOR_CPU_QUOTA: i64 = 200_000;
const DEFAULT_EXECUTOR_PIDS_LIMIT: i64 = 512;

/// Auth config from `~/.lf/lfd.yaml`.
///
/// `provider` selects the auth strategy:
/// - `"local"` (default): startup session token auth (`~/.lf/session-token`)
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
            Ok(content) => serde_yaml_ng::from_str(&content)
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

        if let Ok(value) = std::env::var("LFD_EXECUTOR_AGENT_TIMEOUT") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.executor.agent_timeout =
                    parse_agent_timeout_duration(trimmed).map_err(|err| anyhow!(err))?;
            }
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_LIMITS_MEMORY") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.executor.limits.memory = parse_executor_limit_i64(
                    trimmed,
                    "LFD_EXECUTOR_LIMITS_MEMORY",
                    "executor.limits.memory",
                )?;
            }
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_LIMITS_MEMORY_SWAP") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.executor.limits.memory_swap = parse_executor_limit_i64(
                    trimmed,
                    "LFD_EXECUTOR_LIMITS_MEMORY_SWAP",
                    "executor.limits.memory_swap",
                )?;
            }
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_LIMITS_CPU_QUOTA") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.executor.limits.cpu_quota = parse_executor_limit_i64(
                    trimmed,
                    "LFD_EXECUTOR_LIMITS_CPU_QUOTA",
                    "executor.limits.cpu_quota",
                )?;
            }
        }

        if let Ok(value) = std::env::var("LFD_EXECUTOR_LIMITS_PIDS_LIMIT") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.executor.limits.pids_limit = parse_executor_limit_i64(
                    trimmed,
                    "LFD_EXECUTOR_LIMITS_PIDS_LIMIT",
                    "executor.limits.pids_limit",
                )?;
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
                agent_timeout: self.executor.agent_timeout,
                limits: self.executor.limits.resolve()?,
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
    pub agent_timeout: Duration,
    pub limits: ExecutorLimitsConfig,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            r#type: ExecutorType::Local,
            image: default_executor_image(),
            credentials: ExecutorCredentialsConfig::default(),
            agent_timeout: default_agent_timeout(),
            limits: ExecutorLimitsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorLimitsConfig {
    pub memory: i64,
    pub memory_swap: i64,
    pub cpu_quota: i64,
    pub pids_limit: i64,
}

impl Default for ExecutorLimitsConfig {
    fn default() -> Self {
        Self {
            memory: default_executor_memory_limit_bytes(),
            memory_swap: default_executor_memory_swap_limit_bytes(),
            cpu_quota: default_executor_cpu_quota(),
            pids_limit: default_executor_pids_limit(),
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
    #[serde(
        default = "default_agent_timeout",
        deserialize_with = "deserialize_duration"
    )]
    agent_timeout: Duration,
    #[serde(default)]
    limits: RawExecutorLimitsConfig,
}

impl Default for RawExecutorConfig {
    fn default() -> Self {
        Self {
            r#type: None,
            image: default_executor_image(),
            credentials: ExecutorCredentialsConfig::default(),
            agent_timeout: default_agent_timeout(),
            limits: RawExecutorLimitsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawExecutorLimitsConfig {
    #[serde(default = "default_executor_memory_limit_bytes")]
    memory: i64,
    #[serde(default = "default_executor_memory_swap_limit_bytes")]
    memory_swap: i64,
    #[serde(default = "default_executor_cpu_quota")]
    cpu_quota: i64,
    #[serde(default = "default_executor_pids_limit")]
    pids_limit: i64,
}

impl RawExecutorLimitsConfig {
    fn resolve(self) -> Result<ExecutorLimitsConfig> {
        if self.memory <= 0 {
            bail!("executor.limits.memory must be greater than zero");
        }
        if self.memory_swap <= 0 {
            bail!("executor.limits.memory_swap must be greater than zero");
        }
        if self.cpu_quota <= 0 {
            bail!("executor.limits.cpu_quota must be greater than zero");
        }
        if self.pids_limit <= 0 {
            bail!("executor.limits.pids_limit must be greater than zero");
        }

        Ok(ExecutorLimitsConfig {
            memory: self.memory,
            memory_swap: self.memory_swap,
            cpu_quota: self.cpu_quota,
            pids_limit: self.pids_limit,
        })
    }
}

impl Default for RawExecutorLimitsConfig {
    fn default() -> Self {
        Self {
            memory: default_executor_memory_limit_bytes(),
            memory_swap: default_executor_memory_swap_limit_bytes(),
            cpu_quota: default_executor_cpu_quota(),
            pids_limit: default_executor_pids_limit(),
        }
    }
}

fn default_executor_image() -> String {
    DEFAULT_EXECUTOR_IMAGE.to_string()
}

fn default_agent_timeout() -> Duration {
    Duration::from_secs(DEFAULT_AGENT_TIMEOUT_SECS)
}

fn default_executor_memory_limit_bytes() -> i64 {
    DEFAULT_EXECUTOR_MEMORY_LIMIT_BYTES
}

fn default_executor_memory_swap_limit_bytes() -> i64 {
    DEFAULT_EXECUTOR_MEMORY_SWAP_LIMIT_BYTES
}

fn default_executor_cpu_quota() -> i64 {
    DEFAULT_EXECUTOR_CPU_QUOTA
}

fn default_executor_pids_limit() -> i64 {
    DEFAULT_EXECUTOR_PIDS_LIMIT
}

fn parse_agent_timeout_duration(raw: &str) -> std::result::Result<Duration, String> {
    let trimmed = raw.trim();
    humantime::parse_duration(trimmed).map_err(|err| {
        format!(
            "invalid duration '{}' for executor.agent_timeout: {}",
            raw, err
        )
    })
}

fn parse_executor_limit_i64(raw: &str, env_key: &str, field: &str) -> Result<i64> {
    let value = raw
        .parse::<i64>()
        .map_err(|err| anyhow!("invalid {} value '{}' for {}: {}", env_key, raw, field, err))?;
    if value <= 0 {
        bail!("invalid {env_key} value '{raw}' for {field}: must be greater than zero");
    }
    Ok(value)
}

fn deserialize_duration<'de, D>(deserializer: D) -> std::result::Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum DurationValue {
        String(String),
        Seconds(u64),
    }

    let value = DurationValue::deserialize(deserializer)?;
    match value {
        DurationValue::String(raw) => {
            parse_agent_timeout_duration(raw.as_str()).map_err(serde::de::Error::custom)
        }
        DurationValue::Seconds(seconds) => Ok(Duration::from_secs(seconds)),
    }
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
    use std::time::Duration;
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
        assert_eq!(config.executor.agent_timeout, Duration::from_secs(45 * 60));
        assert_eq!(
            config.executor.limits,
            ExecutorLimitsConfig {
                memory: 4 * 1024 * 1024 * 1024,
                memory_swap: 4 * 1024 * 1024 * 1024,
                cpu_quota: 200_000,
                pids_limit: 512,
            }
        );
    }

    #[test]
    fn mode_container_sets_container_profile() {
        let config: RawLfdConfig = serde_yaml_ng::from_str("mode: container").expect("yaml parses");
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
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
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
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
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
        let result = serde_yaml_ng::from_str::<RawLfdConfig>(raw);
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
            "LFD_EXECUTOR_AGENT_TIMEOUT",
            "LFD_EXECUTOR_LIMITS_MEMORY",
            "LFD_EXECUTOR_LIMITS_MEMORY_SWAP",
            "LFD_EXECUTOR_LIMITS_CPU_QUOTA",
            "LFD_EXECUTOR_LIMITS_PIDS_LIMIT",
            "LFD_GITHUB_WEBHOOK_SECRET",
            "LFD_GITHUB_TOKEN",
        ]);
        std::env::set_var("LFD_MODE", "container");
        std::env::set_var("LFD_AUTH_PROVIDER", "static");
        std::env::set_var("LFD_AUTH_TOKEN", "env-token-456");
        std::env::set_var("LFD_EXECUTOR_IMAGE", "loopflow/agent:env");
        std::env::set_var("LFD_EXECUTOR_AGENT_TIMEOUT", "30m");
        std::env::set_var("LFD_EXECUTOR_LIMITS_MEMORY", "2147483648");
        std::env::set_var("LFD_EXECUTOR_LIMITS_MEMORY_SWAP", "2147483648");
        std::env::set_var("LFD_EXECUTOR_LIMITS_CPU_QUOTA", "100000");
        std::env::set_var("LFD_EXECUTOR_LIMITS_PIDS_LIMIT", "256");
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
        assert_eq!(
            resolved.executor.agent_timeout,
            Duration::from_secs(30 * 60)
        );
        assert_eq!(
            resolved.executor.limits,
            ExecutorLimitsConfig {
                memory: 2 * 1024 * 1024 * 1024,
                memory_swap: 2 * 1024 * 1024 * 1024,
                cpu_quota: 100_000,
                pids_limit: 256,
            }
        );
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
    fn invalid_executor_limits_env_override_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_EXECUTOR_LIMITS_MEMORY"]);
        std::env::set_var("LFD_EXECUTOR_LIMITS_MEMORY", "0");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("invalid executor limit should fail");

        assert_eq!(
            err.to_string(),
            "invalid LFD_EXECUTOR_LIMITS_MEMORY value '0' for executor.limits.memory: must be greater than zero"
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
            "LFD_EXECUTOR_AGENT_TIMEOUT",
            "LFD_EXECUTOR_LIMITS_MEMORY",
            "LFD_EXECUTOR_LIMITS_MEMORY_SWAP",
            "LFD_EXECUTOR_LIMITS_CPU_QUOTA",
            "LFD_EXECUTOR_LIMITS_PIDS_LIMIT",
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

    #[test]
    fn executor_agent_timeout_accepts_duration_string() {
        let raw = r#"
executor:
  agent_timeout: 75s
"#;
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
        let resolved = config.resolve().expect("resolved");
        assert_eq!(resolved.executor.agent_timeout, Duration::from_secs(75));
    }

    #[test]
    fn executor_limits_accept_yaml_override() {
        let raw = r#"
executor:
  limits:
    memory: 2147483648
    memory_swap: 2147483648
    cpu_quota: 100000
    pids_limit: 256
"#;
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
        let resolved = config.resolve().expect("resolved");
        assert_eq!(
            resolved.executor.limits,
            ExecutorLimitsConfig {
                memory: 2 * 1024 * 1024 * 1024,
                memory_swap: 2 * 1024 * 1024 * 1024,
                cpu_quota: 100_000,
                pids_limit: 256,
            }
        );
    }

    #[test]
    fn executor_limits_reject_non_positive_yaml_values() {
        let raw = r#"
executor:
  limits:
    pids_limit: 0
"#;
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
        let err = config.resolve().expect_err("invalid limits should fail");
        assert_eq!(
            err.to_string(),
            "executor.limits.pids_limit must be greater than zero"
        );
    }
}
