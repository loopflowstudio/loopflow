//! A Wave's Home: a user-owned execution address — an *owner* plus a *location*.
//!
//! Home is not a host alias and not a local/remote boolean. It names *whose*
//! execution context a Wave runs in and *where* that context lives:
//!
//! - `jack@local` — the canonical local form (owner `jack`, this machine).
//! - `ssh://jack@host[:port]` — the canonical remote form, reachable over SSH.
//! - `jack@host` — human shorthand that normalizes to `ssh://jack@host`.
//!
//! The owner is required and is distinct from credentials: it says who the Home
//! belongs to, not how to authenticate — credentials still ride the SSH and
//! Doppler surfaces. Reachability is never enforced at parse time: a public IP,
//! public/private DNS name, or Tailscale address all describe the same SSH
//! location, and whether it answers is *operational evidence* (see
//! [`HomeState`]), not a property of the address.
//!
//! One [`WaveHome::parse`] funnel accepts every form (Postel: liberal in); one
//! [`WaveHome::to_string`] formatter emits the canonical form (strict out).
//! DNS, IPv4, bracketed IPv6, and optional ports all flow through the same pair.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The pinned/inherited home a launched child carries. Read before GOAL.md so a
/// child routes to the home its Session was launched with.
pub const WAVE_HOME_ENV: &str = "LF_WAVE_HOME";

/// Set on the remote hop by the router. Its presence means "you are already the
/// home host; run the command locally" — the single break in the forward loop.
pub const HOME_ROUTED_ENV: &str = "LF_HOME_ROUTED";

/// A Wave's Home: an owner plus the location its execution context lives at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaveHome {
    owner: String,
    location: HomeLocation,
}

/// Where a Home's execution context lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeLocation {
    /// This machine.
    Local,
    /// One SSH location. `host` is a DNS name or IP; `port` defaults to ssh's.
    Remote { host: HomeHost, port: Option<u16> },
}

/// A remote location's host: a DNS name or a numeric IP. IPv6 is stored numeric
/// and always rendered bracketed in the canonical URI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeHost {
    Name(String),
    Ip(IpAddr),
}

impl HomeHost {
    /// The bare host as `ssh` wants it in a `user@host` destination — no
    /// brackets, since the ssh CLI takes an unbracketed IPv6 there.
    fn as_ssh_host(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Ip(ip) => ip.to_string(),
        }
    }

    /// The host as it appears in the canonical URI — IPv6 bracketed.
    fn as_uri_host(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Ip(IpAddr::V4(v4)) => v4.to_string(),
            Self::Ip(IpAddr::V6(v6)) => format!("[{v6}]"),
        }
    }
}

impl WaveHome {
    /// Parse any authored/shorthand form. `None` for anything unrecognized, so a
    /// typo fails loudly at the read site rather than silently routing wrong.
    ///
    /// The owner is mandatory: a bare `local` or `ssh://host` (no owner) is
    /// rejected — there is no implicit current-user Home.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        // The `ssh://` scheme is optional on input; it is always emitted on
        // output for the remote form.
        let body = raw.strip_prefix("ssh://").unwrap_or(raw);
        let (owner, rest) = body.split_once('@')?;
        let owner = valid_owner(owner)?;
        if rest.eq_ignore_ascii_case("local") {
            return Some(Self {
                owner,
                location: HomeLocation::Local,
            });
        }
        let (host, port) = parse_host_port(rest)?;
        Some(Self {
            owner,
            location: HomeLocation::Remote { host, port },
        })
    }

    pub fn local(owner: impl Into<String>) -> Option<Self> {
        Some(Self {
            owner: valid_owner(&owner.into())?,
            location: HomeLocation::Local,
        })
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn location(&self) -> &HomeLocation {
        &self.location
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.location, HomeLocation::Remote { .. })
    }

    /// The `user@host` destination for `ssh`, when remote.
    pub fn ssh_destination(&self) -> Option<String> {
        match &self.location {
            HomeLocation::Local => None,
            HomeLocation::Remote { host, .. } => {
                Some(format!("{}@{}", self.owner, host.as_ssh_host()))
            }
        }
    }

    /// The SSH port, when remote and explicitly set.
    pub fn ssh_port(&self) -> Option<u16> {
        match &self.location {
            HomeLocation::Remote { port, .. } => *port,
            HomeLocation::Local => None,
        }
    }
}

/// A valid Home owner: a username-shaped token, distinct from any credential.
fn valid_owner(owner: &str) -> Option<String> {
    let owner = owner.trim();
    if owner.is_empty() {
        return None;
    }
    let ok = owner
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    ok.then(|| owner.to_string())
}

/// Parse the `host[:port]` (or `[ipv6][:port]`) location tail. Bracketed IPv6 is
/// the only accepted IPv6 form — an unbracketed multi-colon token is ambiguous
/// with a port and is rejected.
fn parse_host_port(rest: &str) -> Option<(HomeHost, Option<u16>)> {
    if let Some(inner) = rest.strip_prefix('[') {
        let (v6, after) = inner.split_once(']')?;
        let ip: Ipv6Addr = v6.parse().ok()?;
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?.parse::<u16>().ok()?),
        };
        return Some((HomeHost::Ip(IpAddr::V6(ip)), port));
    }
    match rest.matches(':').count() {
        0 => Some((parse_host(rest)?, None)),
        1 => {
            let (host, port) = rest.rsplit_once(':')?;
            Some((parse_host(host)?, Some(port.parse::<u16>().ok()?)))
        }
        // Unbracketed IPv6 is ambiguous with host:port — require brackets.
        _ => None,
    }
}

/// A host token: an IPv4 literal or a DNS name. (Bracketed IPv6 is handled by
/// the caller.)
fn parse_host(token: &str) -> Option<HomeHost> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }
    if let Ok(v4) = token.parse::<Ipv4Addr>() {
        return Some(HomeHost::Ip(IpAddr::V4(v4)));
    }
    let ok = token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'));
    ok.then(|| HomeHost::Name(token.to_string()))
}

impl fmt::Display for WaveHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            HomeLocation::Local => write!(f, "{}@local", self.owner),
            HomeLocation::Remote { host, port } => {
                write!(f, "ssh://{}@{}", self.owner, host.as_uri_host())?;
                if let Some(port) = port {
                    write!(f, ":{port}")?;
                }
                Ok(())
            }
        }
    }
}

impl FromStr for WaveHome {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw).ok_or_else(|| format!("invalid wave home: {raw:?}"))
    }
}

// Serde rides the canonical string so the pinned/inherited value and the
// authored GOAL.md value are always the same bytes — no second shape to drift.
impl Serialize for WaveHome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for WaveHome {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

// -- Wire types -----------------------------------------------------------

/// Wire projection of a Home address: the canonical string plus the structured
/// owner/location a surface needs to render and navigate without reparsing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveHomeDto {
    /// Canonical address, e.g. `jack@local` or `ssh://jack@host:22`.
    pub address: String,
    pub owner: String,
    pub location: HomeLocationDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HomeLocationDto {
    Local,
    Ssh { host: String, port: Option<u16> },
}

impl From<&WaveHome> for WaveHomeDto {
    fn from(home: &WaveHome) -> Self {
        let location = match &home.location {
            HomeLocation::Local => HomeLocationDto::Local,
            HomeLocation::Remote { host, port } => HomeLocationDto::Ssh {
                host: host.as_uri_host(),
                port: *port,
            },
        };
        Self {
            address: home.to_string(),
            owner: home.owner.clone(),
            location,
        }
    }
}

/// A Home's observed liveness, with evidence living alongside in
/// [`HomeRuntimeDto::reason`]. `Unreachable` and `Unknown` are different facts:
/// the Home did not answer at all versus it answered but its state could not be
/// read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeState {
    /// The Home could not be reached over its address.
    Unreachable,
    /// The Home is reachable but no resident is serving the Wave.
    Stopped,
    /// A resident is serving the Wave on the Home.
    Running,
    /// The Home answered but its state could not be determined.
    Unknown,
}

/// The one contextual action a surface should offer for a Home, derived from its
/// state so the UI never has to branch on `HomeState` itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HomeActionDto {
    /// Running: open/attach to the resident at `endpoint`.
    Attach { endpoint: String },
    /// Reachable but stopped: start the Wave on `home` (its canonical address).
    Start { home: String },
    /// Unreachable or unknown: show `message`, the actionable reason.
    Reason { message: String },
}

/// A Wave's Home plus the evidence of what is happening there — the shared
/// contract a conductor surface renders. `home` is the address; `state`+`reason`
/// are the probe's evidence; `endpoint` is the attach identity when running; and
/// `action` is the single button to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeRuntimeDto {
    pub home: WaveHomeDto,
    pub state: HomeState,
    pub reason: String,
    /// Attach identity when running (the resident endpoint), else `null`.
    pub endpoint: Option<String>,
    pub action: HomeActionDto,
}

impl HomeRuntimeDto {
    /// Assemble the runtime evidence and derive the single contextual action.
    pub fn new(
        home: &WaveHome,
        state: HomeState,
        reason: String,
        endpoint: Option<String>,
    ) -> Self {
        let action = match (state, &endpoint) {
            (HomeState::Running, Some(endpoint)) => HomeActionDto::Attach {
                endpoint: endpoint.clone(),
            },
            (HomeState::Stopped, _) => HomeActionDto::Start {
                home: home.to_string(),
            },
            // Running-without-endpoint is a state we could not fully read.
            (HomeState::Running, None) | (HomeState::Unknown, _) | (HomeState::Unreachable, _) => {
                HomeActionDto::Reason {
                    message: reason.clone(),
                }
            }
        };
        Self {
            home: WaveHomeDto::from(home),
            state,
            reason,
            endpoint,
            action,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(raw: &str) -> WaveHome {
        WaveHome::parse(raw).unwrap_or_else(|| panic!("parse {raw:?}"))
    }

    #[test]
    fn canonical_forms_round_trip() {
        for raw in [
            "jack@local",
            "ssh://jack@mini-heart",
            "ssh://jack@mini.example.com:2222",
            "ssh://jack@10.0.0.5",
            "ssh://jack@10.0.0.5:22",
            "ssh://jack@[2001:db8::1]",
            "ssh://jack@[::1]:22",
        ] {
            assert_eq!(home(raw).to_string(), raw, "canonical {raw}");
        }
    }

    #[test]
    fn shorthand_normalizes_to_ssh_uri() {
        assert_eq!(home("jack@mini-heart").to_string(), "ssh://jack@mini-heart");
        assert_eq!(
            home("jack@10.0.0.5:22").to_string(),
            "ssh://jack@10.0.0.5:22"
        );
        // `ssh://jack@local` is accepted and canonicalizes to the local form.
        assert_eq!(home("ssh://jack@local").to_string(), "jack@local");
    }

    #[test]
    fn owner_is_required_and_no_implicit_user() {
        assert_eq!(WaveHome::parse("local"), None);
        assert_eq!(WaveHome::parse("ssh://mini-heart"), None);
        assert_eq!(WaveHome::parse("mini-heart"), None);
        assert_eq!(WaveHome::parse("@host"), None);
        assert_eq!(WaveHome::parse(""), None);
    }

    #[test]
    fn ipv6_must_be_bracketed_and_ports_parse() {
        // bracketed ok
        assert!(home("ssh://jack@[fe80::1]").is_remote());
        // unbracketed ipv6 is ambiguous with a port and is rejected
        assert_eq!(WaveHome::parse("ssh://jack@2001:db8::1"), None);
        // bad port
        assert_eq!(WaveHome::parse("ssh://jack@host:notaport"), None);
        assert_eq!(WaveHome::parse("ssh://jack@host:99999"), None);
    }

    #[test]
    fn ssh_destination_and_port_feed_the_transport() {
        let h = home("ssh://jack@[::1]:2222");
        assert_eq!(h.ssh_destination().as_deref(), Some("jack@::1"));
        assert_eq!(h.ssh_port(), Some(2222));

        let h = home("ssh://deploy@box.tail.ts.net");
        assert_eq!(
            h.ssh_destination().as_deref(),
            Some("deploy@box.tail.ts.net")
        );
        assert_eq!(h.ssh_port(), None);

        assert_eq!(home("jack@local").ssh_destination(), None);
    }

    #[test]
    fn dto_carries_address_owner_and_structured_location() {
        let dto = WaveHomeDto::from(&home("ssh://jack@host:22"));
        assert_eq!(dto.address, "ssh://jack@host:22");
        assert_eq!(dto.owner, "jack");
        assert_eq!(
            dto.location,
            HomeLocationDto::Ssh {
                host: "host".to_string(),
                port: Some(22),
            }
        );

        let local = WaveHomeDto::from(&home("jack@local"));
        assert_eq!(local.address, "jack@local");
        assert_eq!(local.location, HomeLocationDto::Local);
    }

    #[test]
    fn serde_uses_the_canonical_string() {
        let h = home("ssh://jack@host:22");
        let json = serde_json::to_string(&h).unwrap();
        assert_eq!(json, "\"ssh://jack@host:22\"");
        assert_eq!(serde_json::from_str::<WaveHome>(&json).unwrap(), h);
    }

    #[test]
    fn runtime_action_follows_state() {
        let h = home("ssh://jack@host");
        let running = HomeRuntimeDto::new(
            &h,
            HomeState::Running,
            "resident serving".into(),
            Some("127.0.0.1:7777".into()),
        );
        assert_eq!(
            running.action,
            HomeActionDto::Attach {
                endpoint: "127.0.0.1:7777".into()
            }
        );

        let stopped = HomeRuntimeDto::new(
            &h,
            HomeState::Stopped,
            "reachable, no resident".into(),
            None,
        );
        assert_eq!(
            stopped.action,
            HomeActionDto::Start {
                home: "ssh://jack@host".into()
            }
        );

        let unreachable = HomeRuntimeDto::new(
            &h,
            HomeState::Unreachable,
            "ssh could not connect".into(),
            None,
        );
        assert_eq!(
            unreachable.action,
            HomeActionDto::Reason {
                message: "ssh could not connect".into()
            }
        );
    }
}
