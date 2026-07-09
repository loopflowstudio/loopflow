use anyhow::{anyhow, bail, Context, Result};
use ipnet::IpNet;
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::PathBuf;
use tracing::warn;

use secrecy::SecretString;

const DEFAULT_HTTP_MAX_JSON_BODY_BYTES: usize = 1_048_576;
const DEFAULT_HTTP_MAX_HOOK_BODY_BYTES: usize = 262_144;
const DEFAULT_HTTP_MAX_WS_FRAME_BYTES: usize = 65_536;
const DEFAULT_HTTP_MAX_WS_MESSAGE_BYTES: usize = 262_144;
const DEFAULT_HTTP_MAX_WS_QUEUE: usize = 256;
const DEFAULT_HTTP_MAX_WS_MALFORMED: u32 = 3;
const DEFAULT_HTTP_AUTH_FAILURES_PER_MINUTE: u32 = 12;

/// Auth config from `~/.lf/lfd.yaml`.
///
/// lfd uses bearer-token auth for local and remote clients. Local launches
/// generate a session token under `~/.lf/session-token`; self-hosted remote
/// deployments set `auth.token` or `LFD_AUTH_TOKEN` from Doppler or another
/// secret store.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthConfig {
    /// Optional session-token override for embedded launches and self-hosted deployments.
    pub token: Option<SecretString>,
}

#[derive(Debug, Clone)]
pub struct LfdConfig {
    pub auth: AuthConfig,
    pub github: GitHubConfig,
    pub http_security: HttpSecurityConfig,
    pub output_log_retention_days: u32,
}

impl Default for LfdConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig::default(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            output_log_retention_days: DEFAULT_OUTPUT_LOG_RETENTION_DAYS,
        }
    }
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

const DEFAULT_OUTPUT_LOG_RETENTION_DAYS: u32 = 7;

/// Unknown keys are ignored, not rejected. A key we removed still sits in
/// config files on machines that predate the removal, and `mode: native` —
/// dropped with the postgres backend in 944909ae — was enough to make `lfd`
/// panic on every start, in a crash loop, long after nothing read it.
#[derive(Debug, Clone, Default, Deserialize)]
struct RawLfdConfig {
    #[serde(default)]
    auth: AuthConfig,
    #[serde(default)]
    github: GitHubConfig,
    #[serde(default)]
    http_security: RawHttpSecurityConfig,
    #[serde(default = "default_output_log_retention_days")]
    output_log_retention_days: u32,
}

fn default_output_log_retention_days() -> u32 {
    DEFAULT_OUTPUT_LOG_RETENTION_DAYS
}

fn reject_removed_env(name: &str, replacement: &str) -> Result<()> {
    if std::env::var_os(name).is_some() {
        bail!("{name} was removed; use {replacement}");
    }
    Ok(())
}

impl RawLfdConfig {
    fn apply_env_overrides(&mut self) -> Result<()> {
        reject_removed_env("LFD_AUTH_MODE", "LFD_AUTH_TOKEN")?;
        reject_removed_env("LFD_MODE", "native lfd; mode no longer exists")?;
        reject_removed_env(
            "LFD_EXECUTOR_CREDENTIALS_ENV",
            "native lfd; docker executor credentials no longer exist",
        )?;
        reject_removed_env(
            "LFD_EXECUTOR_CREDENTIALS_MOUNTS",
            "native lfd; docker credential mounts no longer exist",
        )?;
        reject_removed_env(
            "LFD_EXECUTOR_IMAGE",
            "native lfd; executor images no longer exist",
        )?;

        if let Ok(value) = std::env::var("LFD_AUTH_TOKEN") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                self.auth.token = Some(SecretString::new(trimmed.to_string()));
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
                Some(SecretString::new(token.to_string()))
            };
        }

        Self::apply_env_override(
            &mut self.http_security.max_json_body_bytes,
            "LFD_HTTP_MAX_JSON_BODY_BYTES",
            "http_security.max_json_body_bytes",
            parse_positive_usize,
        )?;
        Self::apply_env_override(
            &mut self.http_security.max_hook_body_bytes,
            "LFD_HTTP_MAX_HOOK_BODY_BYTES",
            "http_security.max_hook_body_bytes",
            parse_positive_usize,
        )?;
        Self::apply_env_override(
            &mut self.http_security.max_ws_frame_bytes,
            "LFD_HTTP_MAX_WS_FRAME_BYTES",
            "http_security.max_ws_frame_bytes",
            parse_positive_usize,
        )?;
        Self::apply_env_override(
            &mut self.http_security.max_ws_message_bytes,
            "LFD_HTTP_MAX_WS_MESSAGE_BYTES",
            "http_security.max_ws_message_bytes",
            parse_positive_usize,
        )?;
        Self::apply_env_override(
            &mut self.http_security.max_ws_queue,
            "LFD_HTTP_MAX_WS_QUEUE",
            "http_security.max_ws_queue",
            parse_positive_usize,
        )?;
        Self::apply_env_override(
            &mut self.http_security.max_ws_malformed,
            "LFD_HTTP_MAX_WS_MALFORMED",
            "http_security.max_ws_malformed",
            parse_positive_u32,
        )?;
        Self::apply_env_override(
            &mut self.http_security.auth_failures_per_minute,
            "LFD_HTTP_AUTH_FAILURES_PER_MINUTE",
            "http_security.auth_failures_per_minute",
            parse_positive_u32,
        )?;

        if let Ok(value) = std::env::var("LFD_HTTP_TRUSTED_PROXY_CIDRS") {
            let trimmed = value.trim();
            self.http_security.trusted_proxy_cidrs = if trimmed.is_empty() {
                Vec::new()
            } else {
                trimmed
                    .split(',')
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect()
            };
        }

        Ok(())
    }

    fn apply_env_override<T>(
        target: &mut T,
        env_key: &str,
        field: &str,
        parse: fn(&str, &str, &str) -> Result<T>,
    ) -> Result<()> {
        let Ok(value) = std::env::var(env_key) else {
            return Ok(());
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        *target = parse(trimmed, env_key, field)?;
        Ok(())
    }

    fn resolve(self) -> Result<LfdConfig> {
        Ok(LfdConfig {
            auth: self.auth,
            github: self.github,
            http_security: self.http_security.resolve()?,
            output_log_retention_days: self.output_log_retention_days,
        })
    }
}

#[derive(Clone, Default, Deserialize)]
pub struct GitHubConfig {
    #[serde(default)]
    pub webhook_secret: String,
    pub token: Option<SecretString>,
}

impl std::fmt::Debug for GitHubConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let webhook_secret = if self.webhook_secret.is_empty() {
            ""
        } else {
            "[REDACTED]"
        };
        let token = self.token.as_ref().map(|_| "[REDACTED]");
        f.debug_struct("GitHubConfig")
            .field("webhook_secret", &webhook_secret)
            .field("token", &token)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSecurityConfig {
    pub max_json_body_bytes: usize,
    pub max_hook_body_bytes: usize,
    pub max_ws_frame_bytes: usize,
    pub max_ws_message_bytes: usize,
    pub max_ws_queue: usize,
    pub max_ws_malformed: u32,
    pub auth_failures_per_minute: u32,
    pub trusted_proxy_cidrs: Vec<IpNet>,
}

impl Default for HttpSecurityConfig {
    fn default() -> Self {
        Self {
            max_json_body_bytes: DEFAULT_HTTP_MAX_JSON_BODY_BYTES,
            max_hook_body_bytes: DEFAULT_HTTP_MAX_HOOK_BODY_BYTES,
            max_ws_frame_bytes: DEFAULT_HTTP_MAX_WS_FRAME_BYTES,
            max_ws_message_bytes: DEFAULT_HTTP_MAX_WS_MESSAGE_BYTES,
            max_ws_queue: DEFAULT_HTTP_MAX_WS_QUEUE,
            max_ws_malformed: DEFAULT_HTTP_MAX_WS_MALFORMED,
            auth_failures_per_minute: DEFAULT_HTTP_AUTH_FAILURES_PER_MINUTE,
            trusted_proxy_cidrs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
struct RawHttpSecurityConfig {
    max_json_body_bytes: usize,
    max_hook_body_bytes: usize,
    max_ws_frame_bytes: usize,
    max_ws_message_bytes: usize,
    max_ws_queue: usize,
    max_ws_malformed: u32,
    auth_failures_per_minute: u32,
    trusted_proxy_cidrs: Vec<String>,
}

impl Default for RawHttpSecurityConfig {
    fn default() -> Self {
        let default = HttpSecurityConfig::default();
        Self {
            max_json_body_bytes: default.max_json_body_bytes,
            max_hook_body_bytes: default.max_hook_body_bytes,
            max_ws_frame_bytes: default.max_ws_frame_bytes,
            max_ws_message_bytes: default.max_ws_message_bytes,
            max_ws_queue: default.max_ws_queue,
            max_ws_malformed: default.max_ws_malformed,
            auth_failures_per_minute: default.auth_failures_per_minute,
            trusted_proxy_cidrs: Vec::new(),
        }
    }
}

impl RawHttpSecurityConfig {
    fn resolve(self) -> Result<HttpSecurityConfig> {
        require_positive_usize(
            self.max_json_body_bytes,
            "http_security.max_json_body_bytes",
        )?;
        require_positive_usize(
            self.max_hook_body_bytes,
            "http_security.max_hook_body_bytes",
        )?;
        require_positive_usize(self.max_ws_frame_bytes, "http_security.max_ws_frame_bytes")?;
        require_positive_usize(
            self.max_ws_message_bytes,
            "http_security.max_ws_message_bytes",
        )?;
        require_positive_usize(self.max_ws_queue, "http_security.max_ws_queue")?;
        require_positive_u32(self.max_ws_malformed, "http_security.max_ws_malformed")?;
        require_positive_u32(
            self.auth_failures_per_minute,
            "http_security.auth_failures_per_minute",
        )?;

        let mut trusted_proxy_cidrs = Vec::with_capacity(self.trusted_proxy_cidrs.len());
        for cidr in self.trusted_proxy_cidrs {
            let trimmed = cidr.trim();
            if trimmed.is_empty() {
                continue;
            }
            let parsed = trimmed
                .parse::<IpNet>()
                .map_err(|err| anyhow!("invalid trusted proxy CIDR '{trimmed}': {err}"))?;
            trusted_proxy_cidrs.push(parsed);
        }

        Ok(HttpSecurityConfig {
            max_json_body_bytes: self.max_json_body_bytes,
            max_hook_body_bytes: self.max_hook_body_bytes,
            max_ws_frame_bytes: self.max_ws_frame_bytes,
            max_ws_message_bytes: self.max_ws_message_bytes,
            max_ws_queue: self.max_ws_queue,
            max_ws_malformed: self.max_ws_malformed,
            auth_failures_per_minute: self.auth_failures_per_minute,
            trusted_proxy_cidrs,
        })
    }
}

fn parse_positive_usize(raw: &str, env_key: &str, field: &str) -> Result<usize> {
    let value: usize = raw
        .parse()
        .map_err(|err| anyhow!("invalid {env_key} value '{raw}' for {field}: {err}"))?;
    if value == 0 {
        bail!("invalid {env_key} value '{raw}' for {field}: must be greater than zero");
    }
    Ok(value)
}

fn parse_positive_u32(raw: &str, env_key: &str, field: &str) -> Result<u32> {
    let value: u32 = raw
        .parse()
        .map_err(|err| anyhow!("invalid {env_key} value '{raw}' for {field}: {err}"))?;
    if value == 0 {
        bail!("invalid {env_key} value '{raw}' for {field}: must be greater than zero");
    }
    Ok(value)
}

fn require_positive_usize(value: usize, field: &str) -> Result<()> {
    if value == 0 {
        bail!("{field} must be greater than zero");
    }
    Ok(())
}

fn require_positive_u32(value: u32, field: &str) -> Result<()> {
    if value == 0 {
        bail!("{field} must be greater than zero");
    }
    Ok(())
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
    use secrecy::{ExposeSecret, SecretString};
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
    fn native_config_is_default() {
        let config = RawLfdConfig::default().resolve().expect("default resolves");

        assert!(config.auth.token.is_none());
        assert_eq!(config.http_security.max_json_body_bytes, 1_048_576);
        assert_eq!(config.http_security.max_hook_body_bytes, 262_144);
        assert_eq!(config.http_security.max_ws_frame_bytes, 65_536);
        assert_eq!(config.http_security.max_ws_message_bytes, 262_144);
        assert_eq!(config.http_security.max_ws_queue, 256);
        assert_eq!(config.http_security.max_ws_malformed, 3);
        assert_eq!(config.http_security.auth_failures_per_minute, 12);
        assert!(config.http_security.trusted_proxy_cidrs.is_empty());
    }

    /// A key we removed still sits in config files on machines that predate the
    /// removal. `mode: native` outlived the postgres backend by months and made
    /// every `lfd` start panic, in a crash loop, long after nothing read it. An
    /// obsolete key is ignored; the keys that remain still parse.
    #[test]
    fn a_removed_key_is_ignored_rather_than_bricking_the_daemon() {
        let raw = r#"
mode: native
executor:
  type: docker
output_log_retention_days: 3
"#;
        let config = serde_yaml_ng::from_str::<RawLfdConfig>(raw)
            .expect("obsolete keys must not fail the parse")
            .resolve()
            .expect("resolves");

        assert_eq!(config.output_log_retention_days, 3);
        assert!(config.auth.token.is_none());
    }

    #[test]
    fn env_overrides_allowed_fields() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&[
            "LFD_AUTH_TOKEN",
            "LFD_GITHUB_WEBHOOK_SECRET",
            "LFD_GITHUB_TOKEN",
            "LFD_HTTP_MAX_JSON_BODY_BYTES",
            "LFD_HTTP_MAX_HOOK_BODY_BYTES",
            "LFD_HTTP_MAX_WS_FRAME_BYTES",
            "LFD_HTTP_MAX_WS_MESSAGE_BYTES",
            "LFD_HTTP_MAX_WS_QUEUE",
            "LFD_HTTP_MAX_WS_MALFORMED",
            "LFD_HTTP_AUTH_FAILURES_PER_MINUTE",
            "LFD_HTTP_TRUSTED_PROXY_CIDRS",
        ]);
        std::env::set_var("LFD_AUTH_TOKEN", "env-token-456");
        std::env::set_var("LFD_GITHUB_WEBHOOK_SECRET", "env-secret");
        std::env::set_var("LFD_GITHUB_TOKEN", "ghp_env");
        std::env::set_var("LFD_HTTP_MAX_JSON_BODY_BYTES", "2097152");
        std::env::set_var("LFD_HTTP_MAX_HOOK_BODY_BYTES", "131072");
        std::env::set_var("LFD_HTTP_MAX_WS_FRAME_BYTES", "32768");
        std::env::set_var("LFD_HTTP_MAX_WS_MESSAGE_BYTES", "131072");
        std::env::set_var("LFD_HTTP_MAX_WS_QUEUE", "64");
        std::env::set_var("LFD_HTTP_MAX_WS_MALFORMED", "5");
        std::env::set_var("LFD_HTTP_AUTH_FAILURES_PER_MINUTE", "9");
        std::env::set_var("LFD_HTTP_TRUSTED_PROXY_CIDRS", "127.0.0.1/32,10.0.0.0/8");

        let mut config = RawLfdConfig::default();
        config.apply_env_overrides().expect("overrides apply");
        let resolved = config.resolve().expect("resolved");

        assert_eq!(
            resolved
                .auth
                .token
                .as_ref()
                .map(|t| t.expose_secret().as_str()),
            Some("env-token-456")
        );
        assert_eq!(resolved.github.webhook_secret, "env-secret");
        assert_eq!(
            resolved
                .github
                .token
                .as_ref()
                .map(|token| token.expose_secret().as_str()),
            Some("ghp_env")
        );
        assert_eq!(resolved.http_security.max_json_body_bytes, 2_097_152);
        assert_eq!(resolved.http_security.max_hook_body_bytes, 131_072);
        assert_eq!(resolved.http_security.max_ws_frame_bytes, 32_768);
        assert_eq!(resolved.http_security.max_ws_message_bytes, 131_072);
        assert_eq!(resolved.http_security.max_ws_queue, 64);
        assert_eq!(resolved.http_security.max_ws_malformed, 5);
        assert_eq!(resolved.http_security.auth_failures_per_minute, 9);
        assert_eq!(resolved.http_security.trusted_proxy_cidrs.len(), 2);
    }

    #[test]
    fn removed_mode_env_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_MODE"]);
        std::env::set_var("LFD_MODE", "container");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("removed env should fail");

        assert_eq!(
            err.to_string(),
            "LFD_MODE was removed; use native lfd; mode no longer exists"
        );
    }

    #[test]
    fn removed_executor_env_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_EXECUTOR_IMAGE"]);
        std::env::set_var("LFD_EXECUTOR_IMAGE", "loopflow/agent:env");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("removed env should fail");

        assert_eq!(
            err.to_string(),
            "LFD_EXECUTOR_IMAGE was removed; use native lfd; executor images no longer exist"
        );
    }

    #[test]
    fn auth_mode_env_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_AUTH_MODE"]);
        std::env::set_var("LFD_AUTH_MODE", "local");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("removed env should fail");

        assert_eq!(
            err.to_string(),
            "LFD_AUTH_MODE was removed; use LFD_AUTH_TOKEN"
        );
    }

    #[test]
    fn invalid_http_limit_env_override_is_rejected() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&["LFD_HTTP_MAX_WS_QUEUE"]);
        std::env::set_var("LFD_HTTP_MAX_WS_QUEUE", "0");

        let mut config = RawLfdConfig::default();
        let err = config
            .apply_env_overrides()
            .expect_err("invalid HTTP limit should fail");

        assert_eq!(
            err.to_string(),
            "invalid LFD_HTTP_MAX_WS_QUEUE value '0' for http_security.max_ws_queue: must be greater than zero"
        );
    }

    #[test]
    fn invalid_trusted_proxy_cidr_is_rejected() {
        let raw = r#"
http_security:
  trusted_proxy_cidrs:
    - not-a-cidr
"#;
        let config: RawLfdConfig = serde_yaml_ng::from_str(raw).expect("yaml parses");
        let err = config.resolve().expect_err("invalid cidr should fail");
        assert!(err
            .to_string()
            .contains("invalid trusted proxy CIDR 'not-a-cidr'"));
    }

    #[test]
    fn load_invalid_yaml_returns_error() {
        let _lock = env_lock().lock().expect("env lock");
        let _guard = EnvGuard::snapshot(&[
            "LFD_AUTH_TOKEN",
            "LFD_GITHUB_WEBHOOK_SECRET",
            "LFD_GITHUB_TOKEN",
            "LFD_HTTP_MAX_JSON_BODY_BYTES",
            "LFD_HTTP_MAX_HOOK_BODY_BYTES",
            "LFD_HTTP_MAX_WS_FRAME_BYTES",
            "LFD_HTTP_MAX_WS_MESSAGE_BYTES",
            "LFD_HTTP_MAX_WS_QUEUE",
            "LFD_HTTP_MAX_WS_MALFORMED",
            "LFD_HTTP_AUTH_FAILURES_PER_MINUTE",
            "LFD_HTTP_TRUSTED_PROXY_CIDRS",
        ]);
        let tmp = tempdir().expect("tempdir");
        let lf_dir = tmp.path().join(".lf");
        std::fs::create_dir_all(&lf_dir).expect("lf dir");
        std::fs::write(lf_dir.join("lfd.yaml"), "auth: [").expect("write config");

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
    fn github_config_debug_redacts_secrets() {
        let github = GitHubConfig {
            webhook_secret: "whsec_123".to_string(),
            token: Some(SecretString::new("ghp_abc".to_string())),
        };
        let rendered = format!("{github:?}");
        assert!(rendered.contains("webhook_secret: \"[REDACTED]\""));
        assert!(rendered.contains("token: Some(\"[REDACTED]\")"));
        assert!(!rendered.contains("whsec_123"));
        assert!(!rendered.contains("ghp_abc"));
    }
}
