//! Parse the mutable route observed for a durable Home authority.
//!
//! A route is either this process's machine (`local`) or one SSH destination:
//!
//! - `local` — the stable local marker.
//! - `ssh://jack@host[:port]` — the canonical remote form, reachable over SSH.
//! - `jack@host` — human shorthand that normalizes to `ssh://jack@host`.
//!
//! The route is observation, never identity; `HomeId` remains stable when it
//! changes. Reachability is operational evidence (see [`HomeState`]).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Set on the remote hop by the router. Its presence means "you are already the
/// home host; run the command locally" — the single break in the forward loop.
pub const HOME_ROUTED_ENV: &str = "LF_HOME_ROUTED";

pub(crate) fn resolve_home_relative_repo(repo: &Path) -> Result<String, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    repo.strip_prefix(&home)
        .map_err(|_| {
            format!(
                "repo {} is outside {}; remote Home routing needs a home-relative path",
                repo.display(),
                home.display()
            )
        })?
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("repo path {} is not UTF-8", repo.display()))
}

/// The current transport route to one Home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomeRoute {
    Local,
    Ssh {
        user: String,
        host: HomeHost,
        port: Option<u16>,
    },
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

impl HomeRoute {
    /// Parse a durable route or SSH shorthand. `None` for anything unrecognized, so a
    /// typo fails loudly at the read site rather than silently routing wrong.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if raw == "local" {
            return Some(Self::Local);
        }
        // The `ssh://` scheme is optional on input; it is always emitted on
        // output for the remote form.
        let body = raw.strip_prefix("ssh://").unwrap_or(raw);
        let (user, rest) = body.split_once('@')?;
        let user = valid_user(user)?;
        let (host, port) = parse_host_port(rest)?;
        Some(Self::Ssh { user, host, port })
    }

    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    /// The `user@host` destination for `ssh`, when remote.
    pub fn ssh_destination(&self) -> Option<String> {
        match self {
            Self::Local => None,
            Self::Ssh { user, host, .. } => Some(format!("{user}@{}", host.as_ssh_host())),
        }
    }

    /// The SSH port, when remote and explicitly set.
    pub fn ssh_port(&self) -> Option<u16> {
        match self {
            Self::Ssh { port, .. } => *port,
            Self::Local => None,
        }
    }
}

fn valid_user(user: &str) -> Option<String> {
    let user = user.trim();
    if user.is_empty() {
        return None;
    }
    let ok = user
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    ok.then(|| user.to_string())
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

impl fmt::Display for HomeRoute {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => f.write_str("local"),
            Self::Ssh { user, host, port } => {
                write!(f, "ssh://{user}@{}", host.as_uri_host())?;
                if let Some(port) = port {
                    write!(f, ":{port}")?;
                }
                Ok(())
            }
        }
    }
}

impl FromStr for HomeRoute {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw).ok_or_else(|| format!("invalid Home route: {raw:?}"))
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
    /// Reachable but stopped: start the Wave on its stable Home identity.
    Start { home_id: crate::durable::HomeId },
    /// Unreachable or unknown: show `message`, the actionable reason.
    Reason { message: String },
}

/// A Wave's Home plus the evidence of what is happening there — the shared
/// contract a conductor surface renders. `home` carries authority and route; `state`+`reason`
/// are the probe's evidence; `endpoint` is the attach identity when running; and
/// `action` is the single button to show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeRuntimeDto {
    pub home: crate::durable::Home,
    pub state: HomeState,
    pub reason: String,
    /// Attach identity when running (the resident endpoint), else `null`.
    pub endpoint: Option<String>,
    pub action: HomeActionDto,
}

impl HomeRuntimeDto {
    /// Assemble the runtime evidence and derive the single contextual action.
    pub fn new(
        home: &crate::durable::Home,
        state: HomeState,
        reason: String,
        endpoint: Option<String>,
    ) -> Self {
        let action = match (state, &endpoint) {
            (HomeState::Running, Some(endpoint)) => HomeActionDto::Attach {
                endpoint: endpoint.clone(),
            },
            (HomeState::Stopped, _) => HomeActionDto::Start {
                home_id: home.id.clone(),
            },
            // Running-without-endpoint is a state we could not fully read.
            (HomeState::Running, None) | (HomeState::Unknown, _) | (HomeState::Unreachable, _) => {
                HomeActionDto::Reason {
                    message: reason.clone(),
                }
            }
        };
        Self {
            home: home.clone(),
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

    fn home(raw: &str) -> HomeRoute {
        HomeRoute::parse(raw).unwrap_or_else(|| panic!("parse {raw:?}"))
    }

    #[test]
    fn canonical_forms_round_trip() {
        for raw in [
            "local",
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
        assert_eq!(home("ssh://jack@local").to_string(), "ssh://jack@local");
    }

    #[test]
    fn ssh_user_is_required() {
        assert_eq!(home("local"), HomeRoute::Local);
        assert_eq!(HomeRoute::parse("ssh://mini-heart"), None);
        assert_eq!(HomeRoute::parse("mini-heart"), None);
        assert_eq!(HomeRoute::parse("@host"), None);
        assert_eq!(HomeRoute::parse(""), None);
    }

    #[test]
    fn ipv6_must_be_bracketed_and_ports_parse() {
        // bracketed ok
        assert!(home("ssh://jack@[fe80::1]").is_remote());
        // unbracketed ipv6 is ambiguous with a port and is rejected
        assert_eq!(HomeRoute::parse("ssh://jack@2001:db8::1"), None);
        // bad port
        assert_eq!(HomeRoute::parse("ssh://jack@host:notaport"), None);
        assert_eq!(HomeRoute::parse("ssh://jack@host:99999"), None);
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

        assert_eq!(home("local").ssh_destination(), None);
    }

    #[test]
    fn runtime_action_follows_state() {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let h = crate::durable::Home {
            id: crate::durable::HomeId::parse("home_00000000000000000000000000000001").unwrap(),
            route: "ssh://jack@host".into(),
            created_at: now,
            observed_at: now,
        };
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
                home_id: h.id.clone()
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
