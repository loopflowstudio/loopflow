mod compose;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
mod onboarding;

use std::path::Path;

use anyhow::Result;

use crate::lfd::config::{LfdConfig, ServiceManager};

const SERVICE_ENV_KEYS: &[&str] = &[
    "LFD_HTTP_ADDR",
    "LFD_AUTH_TOKEN",
    "LFD_MODE",
    "LFD_DB_PATH",
    "LFD_DATABASE_URL",
    "LFD_MAX_SLOTS",
    "LFD_GITHUB_WEBHOOK_SECRET",
    "LFD_GITHUB_TOKEN",
    "LFD_EXECUTOR_CREDENTIALS_ENV",
    "LFD_EXECUTOR_CREDENTIALS_MOUNTS",
    "LFD_EXECUTOR_IMAGE",
    "LFD_EXECUTOR_AGENT_TIMEOUT",
    "LFD_EXECUTOR_LIMITS_MEMORY",
    "LFD_EXECUTOR_LIMITS_MEMORY_SWAP",
    "LFD_EXECUTOR_LIMITS_CPU_QUOTA",
    "LFD_EXECUTOR_LIMITS_PIDS_LIMIT",
    "LFD_HTTP_MAX_JSON_BODY_BYTES",
    "LFD_HTTP_MAX_HOOK_BODY_BYTES",
    "LFD_HTTP_MAX_WS_FRAME_BYTES",
    "LFD_HTTP_MAX_WS_MESSAGE_BYTES",
    "LFD_HTTP_MAX_WS_QUEUE",
    "LFD_HTTP_MAX_WS_MALFORMED",
    "LFD_HTTP_AUTH_FAILURES_PER_MINUTE",
    "LFD_HTTP_TRUSTED_PROXY_CIDRS",
];

pub(super) fn service_environment(path_env: &str) -> Vec<(String, String)> {
    let mut values = vec![("PATH".to_string(), path_env.to_string())];
    for name in SERVICE_ENV_KEYS {
        let Ok(value) = std::env::var(name) else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        values.push((name.to_string(), trimmed.to_string()));
    }
    values
}

pub(super) fn contains_sensitive_service_env(values: &[(String, String)]) -> bool {
    values.iter().any(|(name, _)| {
        let name = name.to_ascii_uppercase();
        name.contains("TOKEN") || name.contains("SECRET") || name.contains("KEY")
    })
}

pub(super) fn write_service_file(path: &Path, content: &str, sensitive: bool) -> Result<()> {
    std::fs::write(path, content)?;
    if sensitive {
        let mut permissions = std::fs::metadata(path)?.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o600);
        }
        std::fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

pub fn install(force: bool, no_interactive: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, force, Action::Install)?;
    onboarding::run_install_onboarding(no_interactive)?;
    Ok(())
}

pub fn uninstall() -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, false, Action::Uninstall)
}

pub fn start(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, force, Action::Start)
}

pub fn stop() -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, false, Action::Stop)
}

pub fn status() -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, false, Action::Status)
}

enum Action {
    Install,
    Uninstall,
    Start,
    Stop,
    Status,
}

fn dispatch(
    config: &LfdConfig,
    force: bool,
    action: Action,
) -> Result<(), Box<dyn std::error::Error>> {
    match config.service_manager {
        ServiceManager::Launchd => {
            #[cfg(target_os = "macos")]
            {
                match action {
                    Action::Install => macos::install(config, force),
                    Action::Uninstall => macos::uninstall(config),
                    Action::Start => macos::start(config, force),
                    Action::Stop => macos::stop(config),
                    Action::Status => macos::status(config),
                }
                .map_err(Into::into)
            }

            #[cfg(not(target_os = "macos"))]
            {
                Err(format!(
                    "mode resolved service_manager={} on this OS; use a mode compatible with Linux",
                    config.service_manager.as_str()
                )
                .into())
            }
        }
        ServiceManager::Systemd => {
            #[cfg(target_os = "linux")]
            {
                match action {
                    Action::Install => linux::install(config, force),
                    Action::Uninstall => linux::uninstall(config),
                    Action::Start => linux::start(config, force),
                    Action::Stop => linux::stop(config),
                    Action::Status => linux::status(config),
                }
                .map_err(Into::into)
            }

            #[cfg(not(target_os = "linux"))]
            {
                Err(format!(
                    "mode resolved service_manager={} on this OS; use a mode compatible with macOS",
                    config.service_manager.as_str()
                )
                .into())
            }
        }
    }
}
