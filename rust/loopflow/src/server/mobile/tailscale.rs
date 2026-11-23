//! One connection pool for a Tailscale LocalAPI endpoint. The proof for a
//! loopback endpoint stays in memory and is sent only to that endpoint.
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Client, RequestBuilder, Url};
use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use serde_json::Value;
use tokio_rustls::{rustls, TlsAcceptor};

use super::{Configuration, TailscaleIdentity};
use crate::server::ticket::Admission;

#[cfg(target_os = "macos")]
mod macos;

const LOCAL_HOST: &str = "local-tailscaled.sock";

enum Endpoint {
    Unix(PathBuf),
    #[cfg(target_os = "macos")]
    Loopback {
        port: u16,
        token: String,
    },
}

pub(in crate::server) struct Tailscale {
    client: Client,
    base: Url,
    token: Option<String>,
}

pub(in crate::server) struct HostStatus {
    pub owner: TailscaleIdentity,
    pub dns: String,
    pub addresses: Vec<IpAddr>,
}

impl HostStatus {
    fn parse(status: &Value, require_profile: bool) -> Result<Self> {
        let owner = owner_from_status(status, require_profile)?;
        let node = &status["Self"];
        let dns = node["DNSName"].as_str().unwrap_or("").trim_end_matches('.');
        anyhow::ensure!(
            dns.ends_with(".ts.net")
                && dns.split('.').all(|label| !label.is_empty()
                    && label
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || c == b'-')),
            "Enable Tailscale MagicDNS and HTTPS for this Machine"
        );
        let addresses = node["TailscaleIPs"]
            .as_array()
            .filter(|ips| !ips.is_empty())
            .context("Tailscale has no addresses")?
            .iter()
            .map(|ip| ip.as_str()?.parse().ok())
            .collect::<Option<_>>()
            .context("Invalid Tailscale address")?;
        Ok(Self {
            owner,
            dns: dns.into(),
            addresses,
        })
    }

    pub(in crate::server) fn verify(&self, config: &Configuration) -> Result<()> {
        anyhow::ensure!(self.owner == config.owner,
            "Tailscale account changed. Restore the intended account, or choose Set up again to approve the current account.");
        anyhow::ensure!(
            Url::parse(&config.endpoint)?.host_str() == Some(&self.dns),
            "Tailscale address changed. Choose Set up again, then scan the new QR code."
        );
        Ok(())
    }
}

fn owner_from_status(status: &Value, require_profile: bool) -> Result<TailscaleIdentity> {
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
    // Peer-free status may omit User. Setup always requires the login profile.
    anyhow::ensure!(
        (!require_profile && status["User"].is_null())
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

fn permits_peer(who: &Value, owner: &TailscaleIdentity, same_device: bool) -> Option<Admission> {
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
    permitted.then(|| Admission::Remote {
        node: node.to_owned(),
        user: owner.user,
    })
}

fn endpoints() -> Vec<Endpoint> {
    if let Some(socket) = std::env::var_os("TS_SOCKET").filter(|s| !s.is_empty()) {
        return vec![Endpoint::Unix(socket.into())];
    }
    #[cfg(target_os = "macos")]
    {
        macos::endpoints()
    }
    #[cfg(not(target_os = "macos"))]
    {
        vec![Endpoint::Unix("/var/run/tailscale/tailscaled.sock".into())]
    }
}

fn installed_on_machine() -> bool {
    #[cfg(target_os = "macos")]
    if macos::app_installed() {
        return true;
    }
    loopflow_execution::os_process::which_on_path(Path::new("tailscale")).is_some()
}

impl Tailscale {
    pub(in crate::server) async fn discover() -> Result<(Self, HostStatus)> {
        Self::discover_at(endpoints(), installed_on_machine).await
    }

    async fn discover_at(
        candidates: Vec<Endpoint>,
        installed_on_machine: impl FnOnce() -> bool,
    ) -> Result<(Self, HostStatus)> {
        let mut daemon_responded = false;
        for endpoint in candidates {
            let api = Self::new(endpoint)?;
            let Ok(status) = api.json("status").await else {
                continue;
            };
            daemon_responded = true;
            if status["BackendState"] == "Running" {
                return Ok((api, HostStatus::parse(&status, true)?));
            }
        }
        anyhow::bail!(if daemon_responded || installed_on_machine() {
            "Open Tailscale and sign in on this Machine, then try again"
        } else {
            "Install and open Tailscale on this Machine, then try again"
        })
    }

    fn new(endpoint: Endpoint) -> Result<Self> {
        let builder = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10));
        let (client, base, token) = match endpoint {
            Endpoint::Unix(path) => (
                builder.unix_socket(path),
                format!("http://{LOCAL_HOST}/localapi/v0/"),
                None,
            ),
            #[cfg(target_os = "macos")]
            Endpoint::Loopback { port, token } => (
                builder,
                format!("http://127.0.0.1:{port}/localapi/v0/"),
                Some(token),
            ),
        };
        Ok(Self {
            client: client.build()?,
            base: Url::parse(&base)?,
            token,
        })
    }

    fn get(&self, url: Url) -> RequestBuilder {
        let request = self
            .client
            .get(url)
            .header(reqwest::header::HOST, LOCAL_HOST);
        match &self.token {
            Some(token) => request.basic_auth("", Some(token)),
            None => request,
        }
    }

    async fn response(request: RequestBuilder) -> Result<reqwest::Response> {
        let response = request.send().await.map_err(|_| {
            anyhow::anyhow!("Tailscale did not respond; open it and check its connection")
        })?;
        anyhow::ensure!(
            response.status() == reqwest::StatusCode::OK,
            "Tailscale LocalAPI returned HTTP {}",
            response.status().as_u16()
        );
        Ok(response)
    }

    async fn json(&self, path: &str) -> Result<Value> {
        Self::response(self.get(self.base.join(path)?))
            .await?
            .json()
            .await
            .context("Tailscale returned an unsupported status")
    }

    pub(in crate::server) async fn status_without_peers(&self) -> Result<HostStatus> {
        HostStatus::parse(&self.json("status?peers=false").await?, false)
    }

    pub(in crate::server) async fn permits(
        &self,
        peer: SocketAddr,
        owner: &TailscaleIdentity,
    ) -> Result<Option<Admission>> {
        let mut url = self.base.join("whois")?;
        url.query_pairs_mut()
            .append_pair("addr", &peer.to_string())
            .append_pair("proto", "tcp");
        let who = Self::response(self.get(url))
            .await?
            .json()
            .await
            .context("Tailscale returned an unsupported peer identity")?;
        Ok(permits_peer(&who, owner, same_device_path().exists()))
    }

    pub(in crate::server) async fn certificate(&self, dns: &str) -> Result<TlsAcceptor> {
        let url = self
            .base
            .join(&format!("cert/{dns}?type=pair&min_validity=24h"))?;
        let request = self.get(url).timeout(Duration::from_secs(50));
        let pem = Self::response(request)
            .await
            .context("Check that HTTPS certificates are enabled for this tailnet")?
            .bytes()
            .await?;
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
}

#[cfg(test)]
mod tests {
    use super::{owner_from_status, permits_peer, Endpoint, Tailscale};
    use crate::server::mobile::localapi_fixture::{approval, status, Daemon};
    use serde_json::json;
    use std::os::unix::net::UnixListener;

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
    fn discovery_skips_a_refused_socket_and_stops_at_the_first_running_host() {
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let stale = dir.path().join("stale.sock");
        drop(UnixListener::bind(&stale).unwrap());
        let first = tempfile::tempdir_in("/tmp").unwrap();
        let host = Daemon::start(first.path(), &status(1), String::new());
        let _other = Daemon::start(dir.path(), &status(1), String::new());
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let saved = approval(8443);
            let discover = || async {
                let candidates = vec![
                    Endpoint::Unix(stale.clone()),
                    Endpoint::Unix(first.path().join("tailscaled.sock")),
                    Endpoint::Unix(dir.path().join("tailscaled.sock")),
                ];
                let (_, observed) = Tailscale::discover_at(candidates, || false).await?;
                observed.verify(&saved)
            };
            discover().await.unwrap();
            host.status.send_replace(status(2));
            assert!(discover()
                .await
                .unwrap_err()
                .to_string()
                .contains("Tailscale account changed"));
            let mut no_dns = status(1);
            no_dns["Self"]["DNSName"] = json!("");
            host.status.send_replace(no_dns);
            assert!(discover()
                .await
                .unwrap_err()
                .to_string()
                .contains("Enable Tailscale MagicDNS"));
        });
    }

    #[test]
    fn setup_distinguishes_a_missing_installation_from_a_signed_out_daemon() {
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let socket = dir.path().join("tailscaled.sock");
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let message = || {
            rt.block_on(Tailscale::discover_at(
                vec![Endpoint::Unix(socket.clone())],
                || false,
            ))
            .err()
            .expect("no running host")
            .to_string()
        };
        assert_eq!(
            message(),
            "Install and open Tailscale on this Machine, then try again"
        );
        let _signed_out = Daemon::start(
            dir.path(),
            &json!({"BackendState":"NeedsLogin"}),
            String::new(),
        );
        assert_eq!(
            message(),
            "Open Tailscale and sign in on this Machine, then try again"
        );
    }
}
