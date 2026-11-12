//! Installed Tailscale owns identity, routing and certificate issuance.
//! The Machine exposes HTTPS only on its tailnet addresses.
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use super::client::Connection;
use super::wire::{Operation, ServerOperation};
use anyhow::{Context, Result};
use fs2::FileExt;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_rustls::{rustls, TlsAcceptor};

/// A device's Tailscale identity, as `tailscale status` and `whois` report it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub struct TailscaleIdentity {
    /// The device's Tailscale StableID.
    pub node: String,
    /// The Tailscale user ID that owns the device.
    pub user: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Configuration {
    pub enabled: bool,
    pub endpoint: String,
    /// The Machine owner's identity: their other devices are admitted.
    pub owner: TailscaleIdentity,
}

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
pub struct Status {
    pub configuration: Option<Configuration>,
    pub ready: bool,
    pub error: Option<String>,
}

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

impl Configuration {
    pub(super) fn port(&self) -> Result<u16> {
        reqwest::Url::parse(&self.endpoint)?
            .port_or_known_default()
            .context("Missing HTTPS port")
    }
}

pub fn configured() -> Result<Option<Configuration>> {
    let value = crate::config::files::read(&configuration_path())?;
    configuration(&value)
}

fn configuration_path() -> PathBuf {
    crate::store::lf_home_dir().join("mobile.yaml")
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
    crate::config::files::update(&configuration_path(), |value| {
        *value = serde_yaml_ng::to_value(configuration)?;
        Ok(())
    })
}

fn lock() -> Result<fs::File> {
    let home = crate::store::lf_home_dir();
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
    let host = Host::discover()?;
    let owner = host.identity()?.0;
    let value = crate::config::files::read(&configuration_path())?;
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
    crate::installation::server::ensure()?;
    configure(Some(proposed))?.context("The Machine did not start its mobile listener")
}

pub(super) fn save_disabled() -> Result<Option<Configuration>> {
    crate::config::files::update(&configuration_path(), |value| {
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
    let _lock = lock()?;
    match configure(None) {
        Ok(_) => Ok(()),
        Err(error) if service_is_stopped(&error) => {
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

#[derive(Debug, Clone)]
pub(super) struct Host {
    binary: PathBuf,
    pub dns: String,
}

impl Host {
    pub(super) fn discover() -> Result<Self> {
        let mut candidates = Vec::new();
        if let Some(binary) = crate::os_process::which_on_path(Path::new("tailscale")) {
            candidates.push(binary);
        }
        #[cfg(target_os = "macos")]
        for directory in [
            PathBuf::from("/Applications"),
            crate::installation::account_home()?.join("Applications"),
        ] {
            let Ok(entries) = fs::read_dir(directory) else {
                continue;
            };
            let mut bundles: Vec<_> = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("Tailscale") && name.ends_with(".app"))
                })
                .collect();
            bundles.sort();
            for bundle in bundles {
                for executable in ["Tailscale", "tailscale"] {
                    let binary = bundle.join("Contents/MacOS").join(executable);
                    if binary.is_file() {
                        candidates.push(binary);
                    }
                }
            }
        }
        let mut installed = false;
        for binary in candidates {
            let probe = Self {
                binary,
                dns: String::new(),
            };
            let Ok(status) = probe.json(&["status", "--json"]) else {
                continue;
            };
            installed = true;
            if status["BackendState"] != "Running" {
                continue;
            }
            let dns = status["Self"]["DNSName"]
                .as_str()
                .unwrap_or("")
                .trim_end_matches('.');
            anyhow::ensure!(
                !dns.is_empty() && dns.ends_with(".ts.net"),
                "Enable Tailscale MagicDNS and HTTPS for this Machine"
            );
            return Ok(Self {
                binary: probe.binary,
                dns: dns.into(),
            });
        }
        anyhow::bail!(if installed {
            "Open Tailscale and sign in on this Machine, then try again"
        } else {
            "Install and open Tailscale on this Machine, then try again"
        })
    }

    pub(super) fn identity(&self) -> Result<(TailscaleIdentity, Vec<IpAddr>)> {
        let status = self.json(&["status", "--json"])?;
        let owner = owner_from_status(&status)?;
        let ips = status["Self"]["TailscaleIPs"]
            .as_array()
            .context("Tailscale has no addresses")?
            .iter()
            .map(|ip| {
                ip.as_str()
                    .context("Invalid Tailscale address")?
                    .parse()
                    .context("Invalid Tailscale address")
            })
            .collect::<Result<Vec<IpAddr>>>()?;
        anyhow::ensure!(!ips.is_empty(), "Tailscale has no addresses");
        anyhow::ensure!(
            status["Self"]["DNSName"]
                .as_str()
                .map(|s| s.trim_end_matches('.'))
                == Some(&self.dns),
            "Tailscale address changed. Choose Set up again, then scan the new QR code."
        );
        Ok((owner, ips))
    }

    /// The peer's admission when it is the owner's other device; tickets bind to it.
    pub(super) fn permits(
        &self,
        peer: SocketAddr,
        owner: &TailscaleIdentity,
    ) -> Result<Option<super::ticket::Admission>> {
        let who = self.json(&["whois", "--json", "--proto=tcp", &peer.to_string()])?;
        Ok(permits_peer(&who, owner, same_device_path().exists()))
    }

    pub(super) fn certificate(&self) -> Result<TlsAcceptor> {
        // The sandboxed macOS CLI cannot write arbitrary paths. Keep the key only
        // in memory; never put it in an argument, config, error or log.
        let pem = self.run(&[
            "cert",
            "--min-validity=24h",
            "--cert-file=-",
            "--key-file=-",
            &self.dns,
        ])?;
        let certificates = CertificateDer::pem_slice_iter(&pem)
            .collect::<Result<Vec<_>, _>>()
            .context("Tailscale returned an invalid certificate")?;
        let key = PrivateKeyDer::from_pem_slice(&pem)
            .context("Tailscale returned an invalid certificate key")?;
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .context("Tailscale certificate does not match its key")?;
        Ok(TlsAcceptor::from(Arc::new(config)))
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = crate::os_process::bounded_output(
            // The macOS app otherwise chooses GUI mode under launchd.
            Command::new(&self.binary)
                .env("TAILSCALE_BE_CLI", "1")
                .args(args),
            Duration::from_secs(15),
        )
        .context("Tailscale did not respond; open it and check its connection")?;
        anyhow::ensure!(output.status.success(), "Tailscale could not complete setup. Check that it is signed in and HTTPS certificates are enabled for this tailnet");
        Ok(output.stdout)
    }

    fn json(&self, args: &[&str]) -> Result<Value> {
        serde_json::from_slice(&self.run(args)?).context("Tailscale returned an unsupported status")
    }
}

fn owner_from_status(status: &Value) -> Result<TailscaleIdentity> {
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
        status["User"][user.to_string()]["LoginName"]
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
    crate::store::lf_home_dir().join("mobile-same-device")
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
mod tests {
    use super::{owner_from_status, permits_peer, Configuration};
    use serde_json::json;

    #[test]
    fn app_executable_answers_status_without_a_terminal() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let binary = directory.path().join("Tailscale");
        std::fs::write(
            &binary,
            "#!/bin/sh\nif [ \"$TAILSCALE_BE_CLI\" = 1 ]; then\n  printf '%s' '{\"BackendState\":\"Running\"}'\nelse\n  printf '%s' 'The Tailscale GUI failed to start'\nfi\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let host = super::Host {
            binary,
            dns: "host.example.ts.net".into(),
        };
        assert_eq!(
            host.json(&["status", "--json"]).unwrap()["BackendState"],
            "Running"
        );
    }

    #[test]
    fn admission_requires_a_different_user_owned_device_of_the_saved_owner() {
        let mut status = json!({"BackendState":"Running", "Self":{"ID":"host", "UserID":1}, "User":{"1":{"LoginName":"owner@example.com"}}});
        let owner = owner_from_status(&status).unwrap();
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
        assert!(owner_from_status(&status).is_err());
        status["Self"]["Tags"] = json!([]);
        status["BackendState"] = json!("NeedsLogin");
        assert!(owner_from_status(&status).is_err());
        status["BackendState"] = json!("Running");
        status["Self"]["UserID"] = json!(2);
        assert!(owner_from_status(&status).is_err());
    }

    #[test]
    fn configuration_round_trips_the_approved_owner() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/dto/server/remote.json"))
                .unwrap();
        let value = value["result"]["configuration"].clone();
        let config: Configuration = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(config.port().unwrap(), 8443);
        assert_eq!(serde_json::to_value(config).unwrap(), value);
    }
}
