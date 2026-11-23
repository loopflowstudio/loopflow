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

use super::{permits_peer, same_device_path, Configuration, TailscaleIdentity};
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

#[derive(Clone)]
pub(in crate::server) struct Tailscale {
    client: Client,
    base: Url,
    token: Option<Arc<str>>,
}

pub(in crate::server) struct HostStatus {
    pub owner: TailscaleIdentity,
    pub dns: String,
    pub addresses: Vec<IpAddr>,
}

impl HostStatus {
    fn parse(status: &Value, full: bool) -> Result<Self> {
        let owner = super::owner_from_status(status, full)?;
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
            .context("Tailscale has no addresses")?
            .iter()
            .map(|ip| {
                ip.as_str()
                    .context("Invalid Tailscale address")?
                    .parse()
                    .context("Invalid Tailscale address")
            })
            .collect::<Result<Vec<IpAddr>>>()?;
        anyhow::ensure!(!addresses.is_empty(), "Tailscale has no addresses");
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

impl Tailscale {
    pub(in crate::server) async fn discover() -> Result<(Self, HostStatus)> {
        // An explicitly selected daemon also isolates private Machine fixtures.
        let candidates =
            if let Some(socket) = std::env::var_os("TS_SOCKET").filter(|s| !s.is_empty()) {
                vec![Endpoint::Unix(socket.into())]
            } else {
                #[cfg(target_os = "macos")]
                {
                    macos::endpoints()
                }
                #[cfg(not(target_os = "macos"))]
                {
                    vec![Endpoint::Unix("/var/run/tailscale/tailscaled.sock".into())]
                }
            };
        let mut installed = false;
        for endpoint in candidates {
            let api = Self::new(endpoint)?;
            let Ok(status) = api.status(true).await else {
                continue;
            };
            installed = true;
            if status["BackendState"] == "Running" {
                return Ok((api, HostStatus::parse(&status, true)?));
            }
        }
        #[cfg(target_os = "macos")]
        {
            installed |= macos::app_installed();
        }
        installed |=
            loopflow_execution::os_process::which_on_path(Path::new("tailscale")).is_some();
        anyhow::bail!(if installed {
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
                Some(token.into()),
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
            Some(token) => request.basic_auth("", Some(token.as_ref())),
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

    async fn status(&self, full: bool) -> Result<Value> {
        let mut url = self.base.join("status")?;
        if !full {
            url.set_query(Some("peers=false"));
        }
        Self::response(self.get(url))
            .await?
            .json()
            .await
            .context("Tailscale returned an unsupported status")
    }

    pub(in crate::server) async fn status_without_peers(&self) -> Result<HostStatus> {
        HostStatus::parse(&self.status(false).await?, false)
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
        let mut url = self.base.join("cert/")?;
        url.path_segments_mut()
            .expect("LocalAPI is HTTP")
            .pop_if_empty()
            .push(dns);
        url.set_query(Some("type=pair&min_validity=24h"));
        let pem = Self::response(self.get(url).timeout(Duration::from_secs(50)))
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
