//! Installed Tailscale owns identity, routing and certificate issuance.
//! The Machine exposes HTTPS only on its tailnet addresses.
use std::fs;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::time::Duration;

use super::wire::{Operation, ServerOperation};
use anyhow::{Context, Result};
use fs2::FileExt;
use loopflow_execution::machine::client::Connection;

mod tailscale;
pub(super) use tailscale::Tailscale;

pub use loopflow_execution::machine::mobile::{Configuration, Status, TailscaleIdentity};

pub fn status() -> Result<Status> {
    match Connection::open().and_then(|mut connection| {
        connection.request(Operation::Server(ServerOperation::MobileStatus))
    }) {
        Ok(answer) => serde_json::from_value(answer.result)
            .context("Update the Loopflow server to read mobile readiness"),
        Err(error) if service_is_stopped(&error) => {
            let configuration = configured()?;
            let error = configuration
                .as_ref()
                .filter(|config| config.enabled)
                .map(|_| {
                    "The Machine service is not running. Choose Set up again to start it.".into()
                });
            Ok(Status {
                configuration,
                ready: false,
                error,
            })
        }
        Err(error) => Err(error),
    }
}

fn service_is_stopped(error: &anyhow::Error) -> bool {
    error
        .root_cause()
        .downcast_ref::<std::io::Error>()
        .is_some_and(|io| {
            matches!(
                io.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
            )
        })
}

pub fn configured() -> Result<Option<Configuration>> {
    let value = loopflow_execution::config::files::read(&configuration_path())?;
    configuration(&value)
}

fn configuration_path() -> PathBuf {
    loopflow_execution::store::lf_home_dir().join("mobile.yaml")
}

fn configuration(value: &serde_yaml_ng::Value) -> Result<Option<Configuration>> {
    if value.as_mapping().is_some_and(|mapping| mapping.is_empty()) {
        return Ok(None);
    }
    serde_yaml_ng::from_value(value.clone())
        .map(Some)
        .context("Read mobile setup")
}

pub(super) fn save(configuration: &Configuration) -> Result<()> {
    loopflow_execution::config::files::update(&configuration_path(), |value| {
        *value = serde_yaml_ng::to_value(configuration)?;
        Ok(())
    })
}

fn lock() -> Result<fs::File> {
    let home = loopflow_execution::store::lf_home_dir();
    fs::create_dir_all(&home)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .open(home.join("mobile-setup.lock"))?;
    lock.lock_exclusive()?;
    Ok(lock)
}

fn configure(configuration: Option<Configuration>) -> Result<Option<Configuration>> {
    let mut connection = Connection::open()?;
    // Initial Tailscale certificate issuance can outlast an ordinary read.
    connection.set_read_timeout(Some(Duration::from_secs(60)))?;
    let answer = connection.request(Operation::Server(ServerOperation::MobileConfigure {
        configuration,
    }))?;
    serde_json::from_value(answer.result["configuration"].clone())
        .context("Update the Loopflow server before enabling mobile access")
}

pub fn enable() -> Result<Configuration> {
    let _lock = lock()?;
    let (_, host) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(Tailscale::discover())?;
    let owner = host.owner;
    let value = loopflow_execution::config::files::read(&configuration_path())?;
    // Explicit setup replaces obsolete mobile settings; file read/parse errors
    // still surface instead of discarding an unreadable document.
    let previous = configuration(&value).ok().flatten();
    let port = previous
        .as_ref()
        .filter(|c| c.owner == owner)
        .and_then(|c| c.port().ok())
        .unwrap_or(0);
    let proposed = Configuration {
        enabled: true,
        endpoint: format!("https://{}:{port}", host.dns),
        owner,
    };
    loopflow_execution::installation::server::ensure()?;
    configure(Some(proposed))?.context("The Machine did not start its mobile listener")
}

pub(super) fn save_disabled() -> Result<Option<Configuration>> {
    loopflow_execution::config::files::update(&configuration_path(), |value| {
        let mut saved = configuration(value).ok().flatten();
        if let Some(config) = &mut saved {
            config.enabled = false;
            *value = serde_yaml_ng::to_value(config)?;
        } else {
            *value = serde_yaml_ng::Value::Mapping(Default::default());
        }
        Ok(saved)
    })
}

pub fn disable() -> Result<()> {
    // A live manager serializes setup and can cancel certificate issuance
    // even while the setup CLI holds the lock for offline/startup coordination.
    match configure(None) {
        Ok(_) => Ok(()),
        Err(error) if service_is_stopped(&error) => {
            let _lock = lock()?;
            save_disabled()?;
            // Startup binds the local socket before reading saved approval.
            // If it raced our first probe, stop that live snapshot too. A later
            // startup will read the disabled setting without opening a listener.
            match configure(None) {
                Ok(_) => Ok(()),
                Err(error) if service_is_stopped(&error) => Ok(()),
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[allow(dead_code)] // The transport binary also drives certificate, peer and request controls.
#[path = "../../tests/support/tailscale.rs"]
pub(super) mod localapi_fixture;

#[cfg(test)]
mod tests {
    use super::Configuration;

    #[test]
    fn configuration_round_trips_the_approved_owner() {
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../loopflow-execution/tests/fixtures/dto/server/remote.json"
        ))
        .unwrap();
        let value = value["result"]["configuration"].clone();
        let config: Configuration = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(config.port().unwrap(), 8443);
        assert_eq!(serde_json::to_value(config).unwrap(), value);
    }
}
