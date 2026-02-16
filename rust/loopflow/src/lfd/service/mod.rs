mod compose;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

use crate::lfd::config::{LfdConfig, ServiceManager};

pub fn install(force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let config = LfdConfig::load()?;
    dispatch(&config, force, Action::Install)
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
