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
use serde_json::Value;
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

fn owner_from_status(status: &Value, full: bool) -> Result<TailscaleIdentity> {
    anyhow::ensure!(
        status["BackendState"] == "Running",
        "Sign in to Tailscale on this Machine"
    );
    let node = &status["Self"];
    anyhow::ensure!(
        !node["Tags"].as_array().is_some_and(|tags| !tags.is_empty()),
        "Mobile access requires a user-owned Tailscale Machine, not a tagged device"
    );
    let user = node["UserID"]
        .as_u64()
        .filter(|id| *id != 0)
        .context("Tailscale did not identify this Machine's owner")?;
    anyhow::ensure!(
        (!full && status["User"].is_null())
            || status["User"][user.to_string()]["LoginName"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
        "Tailscale did not identify this Machine's owner"
    );
    Ok(TailscaleIdentity {
        node: node["ID"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Tailscale did not identify this Machine")?
            .into(),
        user,
    })
}

/// Present when this Machine also admits connections from itself, which is how
/// an iOS Simulator on it arrives. Create it only on a Mac with one account.
fn same_device_path() -> PathBuf {
    loopflow_execution::store::lf_home_dir().join("mobile-same-device")
}

fn permits_peer(
    who: &Value,
    owner: &TailscaleIdentity,
    same_device: bool,
) -> Option<super::ticket::Admission> {
    // Network identity belongs to the device, not its local OS accounts. A
    // same-host connection cannot prove which account opened it, so it is
    // refused unless this Machine opted in. Phones arrive as a distinct node.
    let node = who["Node"]["StableID"]
        .as_str()
        .filter(|id| !id.is_empty() && (same_device || *id != owner.node))?;
    let permitted = who["UserProfile"]["ID"].as_u64() == Some(owner.user)
        && who["Node"]["User"].as_u64() == Some(owner.user)
        && !who["Node"]["Tags"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty());
    permitted.then(|| super::ticket::Admission::Remote {
        node: node.to_owned(),
        user: owner.user,
    })
}

#[cfg(test)]
#[allow(dead_code)] // The transport binary also drives certificate, peer and request controls.
#[path = "../../tests/support/tailscale.rs"]
mod localapi_fixture;

#[cfg(test)]
pub(super) mod tests {
    use super::{owner_from_status, permits_peer, Configuration};
    use serde_json::json;

    #[test]
    fn admission_requires_a_different_user_owned_device_of_the_saved_owner() {
        let mut status = json!({"BackendState":"Running", "Self":{"ID":"host", "UserID":1}, "User":{"1":{"LoginName":"owner@example.com"}}});
        let owner = owner_from_status(&status, true).unwrap();
        let mut who = json!({"Node":{"StableID":"phone", "User":1}, "UserProfile":{"ID":1}});
        assert!(permits_peer(&who, &owner, false).is_some());
        who["Node"]["StableID"] = json!("host");
        assert!(permits_peer(&who, &owner, false).is_none());
        assert!(permits_peer(&who, &owner, true).is_some());
        who["Node"]["StableID"] = json!("phone");
        who["Node"]["Tags"] = json!(["tag:ci"]);
        assert!(permits_peer(&who, &owner, false).is_none());
        who["Node"]["Tags"] = json!([]);
        who["UserProfile"]["ID"] = json!(2);
        assert!(permits_peer(&who, &owner, false).is_none());
        assert!(permits_peer(&json!({}), &owner, false).is_none());
        status["Self"]["Tags"] = json!(["tag:server"]);
        assert!(owner_from_status(&status, true).is_err());
        status["Self"]["Tags"] = json!([]);
        status["BackendState"] = json!("NeedsLogin");
        assert!(owner_from_status(&status, true).is_err());
        status["BackendState"] = json!("Running");
        status["Self"]["UserID"] = json!(2);
        assert!(owner_from_status(&status, true).is_err());
    }

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

    // ---- Saved setup under a private LF home ----

    use super::super::remote::{manage, Control};
    use super::{configured, save, TailscaleIdentity};
    use loopflow_execution::config::environment::EnvVarGuard;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A private `LF_HOME` holding unrelated preferences beside mobile setup.
    struct Home {
        dir: tempfile::TempDir,
        _home: EnvVarGuard,
        _socket: EnvVarGuard,
        daemon: Option<super::localapi_fixture::Daemon>,
    }

    const PREFERENCES: &str = "agent: claude\n";

    impl Home {
        fn new() -> Self {
            let dir = tempfile::tempdir_in("/tmp").unwrap();
            std::fs::write(dir.path().join("config.yaml"), PREFERENCES).unwrap();
            let _home = EnvVarGuard::set("LF_HOME", dir.path().to_str().unwrap());
            let _socket = EnvVarGuard::set(
                "TS_SOCKET",
                dir.path().join("tailscaled.sock").to_str().unwrap(),
            );
            Self {
                dir,
                _home,
                _socket,
                daemon: None,
            }
        }

        fn saved(&self) -> PathBuf {
            self.dir.path().join("mobile.yaml")
        }

        fn persisted(&self) -> serde_json::Value {
            serde_yaml_ng::from_str(&std::fs::read_to_string(self.saved()).unwrap()).unwrap()
        }

        fn tailscale(&mut self) {
            let certificate =
                rcgen::generate_simple_self_signed(vec!["fixture.tail.ts.net".into()]).unwrap();
            let pem = format!(
                "{}{}",
                certificate.signing_key.serialize_pem(),
                certificate.cert.pem()
            );
            self.daemon = Some(super::localapi_fixture::Daemon::start(
                self.dir.path(),
                &status(1),
                pem,
            ));
        }
    }

    pub(super) fn status(user: u64) -> serde_json::Value {
        json!({"BackendState":"Running", "Self":{"ID":"node1", "DNSName":"fixture.tail.ts.net.", "UserID":user, "TailscaleIPs":["127.0.0.1"]}, "User":{user.to_string():{"LoginName":"owner@example.com"}}})
    }

    pub(in crate::server) fn approval(port: u16) -> Configuration {
        Configuration {
            enabled: true,
            endpoint: format!("https://fixture.tail.ts.net:{port}"),
            owner: TailscaleIdentity {
                node: "node1".into(),
                user: 1,
            },
        }
    }

    fn with_home(body: impl FnOnce(&mut Home)) {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let mut home = Home::new();
        body(&mut home);
        assert_eq!(
            std::fs::read_to_string(home.dir.path().join("config.yaml")).unwrap(),
            PREFERENCES,
            "mobile setup touches only mobile.yaml"
        );
    }

    #[test]
    fn offline_disable_keeps_the_approval_clears_obsolete_settings_and_spares_a_damaged_file() {
        with_home(|home| {
            // With no server running, status reads the saved approval.
            save(&approval(8443)).unwrap();
            let offline = super::status().unwrap();
            assert!(offline.configuration.unwrap().enabled);
            assert!(!offline.ready);
            assert!(offline.error.is_some());

            super::disable().unwrap();
            let mut disabled = serde_json::to_value(approval(8443)).unwrap();
            disabled["enabled"] = json!(false);
            assert_eq!(
                home.persisted(),
                disabled,
                "disable keeps the owner and address"
            );
            assert!(super::status().unwrap().error.is_none());

            let obsolete = json!({"enabled":true,"endpoint":"https://obsolete.tail.ts.net:8443"});
            std::fs::write(home.saved(), serde_yaml_ng::to_string(&obsolete).unwrap()).unwrap();
            super::disable().unwrap();
            assert_eq!(
                home.persisted(),
                json!({}),
                "disable clears obsolete settings"
            );

            let damaged = "enabled: [unfinished";
            std::fs::write(home.saved(), damaged).unwrap();
            assert!(super::disable().is_err());
            assert_eq!(std::fs::read_to_string(home.saved()).unwrap(), damaged);
        });
    }

    /// The server's mobile manager for this home, and a way to ask it things.
    struct Manager {
        /// Runs the manager until the test drops it.
        _runtime: tokio::runtime::Runtime,
        requests: tokio::sync::mpsc::Sender<Control>,
    }

    pub(in crate::server) fn start_manager() -> (
        tokio::sync::mpsc::Sender<Control>,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    ) {
        let (requests, received) = tokio::sync::mpsc::channel(4);
        let task = tokio::spawn(manage(
            configured(),
            received,
            Arc::from("instance"),
            Arc::new("executable".to_owned()),
            Arc::new(tokio::sync::Semaphore::new(
                crate::server::wire::MAX_CONNECTIONS,
            )),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
        ));
        (requests, task)
    }

    impl Manager {
        fn start() -> Self {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            let (requests, _task) = {
                let _entered = runtime.enter();
                start_manager()
            };
            Self {
                _runtime: runtime,
                requests,
            }
        }

        fn status(&self) -> super::Status {
            let (reply, answer) = tokio::sync::oneshot::channel();
            self.requests
                .blocking_send(Control::Status { reply })
                .unwrap();
            answer.blocking_recv().unwrap()
        }

        /// The status once restoring the saved approval has been tried.
        fn restored(&self) -> super::Status {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                let status = self.status();
                if status.ready || status.error.is_some() {
                    return status;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "restoration never ran"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }

        fn configure(&self, configuration: Option<Configuration>) -> Option<Configuration> {
            let (reply, answer) = tokio::sync::oneshot::channel();
            self.requests
                .blocking_send(Control::Configure {
                    configuration,
                    reply,
                })
                .unwrap();
            answer.blocking_recv().unwrap().unwrap()
        }
    }

    fn port(config: &Configuration) -> u16 {
        config.port().unwrap()
    }

    #[test]
    fn restoration_keeps_the_saved_address_and_setup_moves_off_an_occupied_port() {
        with_home(|home| {
            home.tailscale();
            let occupied = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let saved = approval(occupied.local_addr().unwrap().port());
            save(&saved).unwrap();

            // Restoring never changes the address phones already have.
            let manager = Manager::start();
            let restoring = manager.restored();
            assert!(!restoring.ready);
            assert!(restoring.error.is_some());
            assert_eq!(restoring.configuration.unwrap().endpoint, saved.endpoint);

            // Explicit setup replaces obsolete settings and takes a free port
            // for the same owner.
            let obsolete = json!({"enabled":true,"endpoint":"https://obsolete.tail.ts.net:8443"});
            std::fs::write(home.saved(), serde_yaml_ng::to_string(&obsolete).unwrap()).unwrap();
            let repaired = manager.configure(Some(saved.clone())).unwrap();
            assert_ne!(port(&repaired), port(&saved));
            assert_eq!(repaired.owner, saved.owner);
            // The repaired address is already listening when setup answers.
            assert_eq!(
                std::net::TcpListener::bind(("127.0.0.1", port(&repaired)))
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::AddrInUse
            );
            let ready = manager.status();
            assert!(ready.ready);
            assert_eq!(
                serde_json::to_value(ready.configuration).unwrap(),
                serde_json::to_value(&repaired).unwrap()
            );
            assert_eq!(home.persisted(), serde_json::to_value(&repaired).unwrap());
            // Setting up the same approval again keeps the live listener.
            assert_eq!(
                manager.configure(Some(repaired.clone())).unwrap().endpoint,
                repaired.endpoint
            );

            // Turning access off closes the listener and saves it disabled.
            assert!(manager.configure(None).is_none());
            let _released = std::net::TcpListener::bind(("127.0.0.1", port(&repaired)))
                .expect("disable releases the mobile listener before replying");
            let off = manager.status();
            assert!(!off.ready);
            assert!(!off.configuration.unwrap().enabled);
            assert_eq!(home.persisted()["enabled"], false);
            drop(occupied);
        });
    }

    #[test]
    fn a_saved_approval_does_not_follow_a_new_tailscale_owner() {
        with_home(|home| {
            home.tailscale();
            let saved = approval(0);
            save(&saved).unwrap();
            home.daemon.as_ref().unwrap().status.send_replace(status(2));
            let manager = Manager::start();
            let restored = manager.restored();
            assert!(!restored.ready);
            assert!(
                restored
                    .error
                    .as_deref()
                    .unwrap()
                    .contains("Tailscale account changed"),
                "{restored:?}"
            );
            assert_eq!(restored.configuration.unwrap().owner, saved.owner);
        });
    }
}
