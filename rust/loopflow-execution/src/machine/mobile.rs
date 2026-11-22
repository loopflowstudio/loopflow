//! Mobile endpoint values carried by the Machine protocol.
use serde::{Deserialize, Serialize};

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

use anyhow::Context;

impl Configuration {
    pub fn port(&self) -> anyhow::Result<u16> {
        reqwest::Url::parse(&self.endpoint)?
            .port_or_known_default()
            .context("Missing HTTPS port")
    }
}
