//! Parse the mutable route observed for a stable Home identity.
//!
//! A route is either this process's machine (`local`) or one SSH destination:
//!
//! - `local` — the stable local marker.
//! - `ssh://jack@host[:port]` — the canonical remote form, reachable over SSH.
//! - `jack@host` — readable shorthand that normalizes to `ssh://jack@host`.
//!
//! The route is observation, never identity; `HomeId` remains stable when it
//! changes. Reachability is operational evidence (see [`HomeState`]).

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;
use std::str::FromStr;

pub(crate) const SSH_CONNECT_TIMEOUT_SECS: u32 = 10;
const SSH_SERVER_ALIVE_INTERVAL_SECS: u32 = 10;
const SSH_SERVER_ALIVE_COUNT_MAX: u32 = 3;

pub(crate) fn bounded_ssh_args(dest: &str, port: Option<u16>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(port) = port {
        args.extend(["-p".to_string(), port.to_string()]);
    }
    args.extend([
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-o".to_string(),
        format!("ConnectTimeout={SSH_CONNECT_TIMEOUT_SECS}"),
        "-o".to_string(),
        format!("ServerAliveInterval={SSH_SERVER_ALIVE_INTERVAL_SECS}"),
        "-o".to_string(),
        format!("ServerAliveCountMax={SSH_SERVER_ALIVE_COUNT_MAX}"),
        dest.to_string(),
    ]);
    args
}

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
}
