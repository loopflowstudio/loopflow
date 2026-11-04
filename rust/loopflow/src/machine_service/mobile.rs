//! Installed Tailscale owns identity, routing and certificate issuance.
//! The Machine exposes HTTPS only on its tailnet addresses.
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use super::wire::{Operation, Reply};
use anyhow::{Context, Result};
use fs2::FileExt;
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_rustls::{rustls, TlsAcceptor};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Owner {
    pub node: String,
    pub user: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Configuration {
    pub enabled: bool,
    pub endpoint: String,
    pub owner: Owner,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Status {
    pub configuration: Option<Configuration>,
    pub ready: bool,
    pub error: Option<String>,
}

pub fn status() -> Result<Status> {
    match super::client::connect(Operation::MobileStatus) {
        Ok((_, response)) => match response.result {
            Reply::MobileStatus { status } => Ok(status),
            Reply::Error { message, .. } => anyhow::bail!(message),
            _ => anyhow::bail!("Update the Machine service to read mobile readiness"),
        },
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
    error.downcast_ref::<std::io::Error>().is_some_and(|io| {
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
    let value =
        crate::config::read_global_config_at(&crate::store::lf_home_dir().join("config.yaml"))?;
    value
        .get("mobile")
        .cloned()
        .map(serde_yaml_ng::from_value)
        .transpose()
        .context("Read mobile setup")
}

pub(super) fn save(configuration: &Configuration) -> Result<()> {
    crate::config::update_global_config_at(&crate::store::lf_home_dir(), |value| {
        let mapping = value
            .as_mapping_mut()
            .context("Machine configuration is not a mapping")?;
        mapping.insert(
            serde_yaml_ng::Value::String("mobile".into()),
            serde_yaml_ng::to_value(configuration)?,
        );
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
    let (_, response) = super::client::connect(Operation::ConfigureRemote { configuration })?;
    match response.result {
        Reply::Remote { configuration } => Ok(configuration),
        Reply::Error { message, .. } => anyhow::bail!(message),
        _ => anyhow::bail!("Update the Machine service before enabling mobile access"),
    }
}

pub fn enable() -> Result<Configuration> {
    let _lock = lock()?;
    let host = Host::discover()?;
    let owner = host.identity()?.0;
    let previous = configured()?;
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
    super::installation::ensure()?;
    configure(Some(proposed))?.context("The Machine did not start its mobile listener")
}

pub(super) fn save_disabled() -> Result<Option<Configuration>> {
    let mut configuration = configured()?;
    if let Some(config) = &mut configuration {
        config.enabled = false;
        save(config)?;
    }
    Ok(configuration)
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

    pub(super) fn identity(&self) -> Result<(Owner, Vec<IpAddr>)> {
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

    pub(super) fn permits(&self, peer: SocketAddr, owner: &Owner) -> Result<bool> {
        let who = self.json(&["whois", "--json", "--proto=tcp", &peer.to_string()])?;
        Ok(permits_peer(&who, owner))
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
            Command::new(&self.binary).args(args),
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

fn owner_from_status(status: &Value) -> Result<Owner> {
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
    Ok(Owner {
        node: node["ID"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Tailscale did not identify this Machine")?
            .into(),
        user,
    })
}

fn permits_peer(who: &Value, owner: &Owner) -> bool {
    // Network identity belongs to the device, not its local OS accounts. A
    // same-host connection cannot prove which account opened it; use the private
    // Unix socket on this Machine instead. Phones arrive as a distinct node.
    who["Node"]["StableID"]
        .as_str()
        .is_some_and(|id| !id.is_empty() && id != owner.node)
        && who["UserProfile"]["ID"].as_u64() == Some(owner.user)
        && who["Node"]["User"].as_u64() == Some(owner.user)
        && !who["Node"]["Tags"]
            .as_array()
            .is_some_and(|tags| !tags.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{owner_from_status, permits_peer, Configuration};
    use serde_json::json;

    #[test]
    fn admission_requires_a_different_user_owned_device_of_the_saved_owner() {
        let mut status = json!({"BackendState":"Running", "Self":{"ID":"host", "UserID":1}, "User":{"1":{"LoginName":"owner@example.com"}}});
        let owner = owner_from_status(&status).unwrap();
        let mut who = json!({"Node":{"StableID":"phone", "User":1}, "UserProfile":{"ID":1}});
        assert!(permits_peer(&who, &owner));
        who["Node"]["StableID"] = json!("host");
        assert!(!permits_peer(&who, &owner));
        who["Node"]["StableID"] = json!("phone");
        who["Node"]["Tags"] = json!(["tag:ci"]);
        assert!(!permits_peer(&who, &owner));
        who["Node"]["Tags"] = json!([]);
        who["UserProfile"]["ID"] = json!(2);
        assert!(!permits_peer(&who, &owner));
        assert!(!permits_peer(&json!({}), &owner));
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
        let value: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/machine/mobile_configuration.json"
        ))
        .unwrap();
        let config: Configuration = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(config.port().unwrap(), 8443);
        assert_eq!(serde_json::to_value(config).unwrap(), value);
    }
}
