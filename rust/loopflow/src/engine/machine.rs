//! Match authored Wave startup policy against the machine executing `lf`.

use std::collections::BTreeSet;
use std::fmt;
use std::net::{IpAddr, SocketAddr, TcpListener, ToSocketAddrs};
use std::process::Command;

use crate::durable::HomeId;
use crate::engine::wave_config::WaveConfig;

/// The SSH destination by which the current foreground `lf` was reached.
/// It is invocation context, not durable Home identity.
pub(crate) const SSH_TARGET_ENV: &str = "LF_SSH_TARGET";

#[derive(Debug, Clone)]
pub(crate) struct MachineIdentity {
    owner: Option<String>,
    home_id: HomeId,
    names: BTreeSet<String>,
}

impl MachineIdentity {
    pub(crate) fn detect(home_id: HomeId) -> Self {
        let mut names = BTreeSet::from(["local".to_string(), "localhost".to_string()]);
        let hostname = gethostname::gethostname().to_string_lossy().to_string();
        insert_name_and_short(&mut names, &hostname);
        if let Ok(target) = std::env::var(SSH_TARGET_ENV) {
            insert_name_and_short(&mut names, ssh_host(&target));
        }
        Self {
            owner: current_owner(),
            home_id,
            names,
        }
    }

    #[cfg(test)]
    fn test(owner: Option<&str>, home_id: HomeId, names: &[&str]) -> Self {
        Self {
            owner: owner.map(str::to_string),
            home_id,
            names: names.iter().map(|name| normalize_name(name)).collect(),
        }
    }

    fn matches_owner(&self, expected: &str) -> bool {
        let expected = expected.trim();
        expected.is_empty() || self.owner.as_deref() == Some(expected)
    }

    fn matches_home(&self, expected: &str) -> bool {
        let expected = normalize_name(expected);
        if expected.is_empty()
            || expected == self.home_id.as_str()
            || self.names.contains(&expected)
        {
            return true;
        }
        if let Ok(address) = expected.parse::<IpAddr>() {
            return address_is_local(address);
        }
        (expected.as_str(), 0)
            .to_socket_addrs()
            .is_ok_and(|addresses| {
                addresses
                    .into_iter()
                    .any(|address| address_is_local(address.ip()))
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WaveStartDecision {
    Start,
    OtherOwner {
        expected: String,
        current: Option<String>,
    },
    OtherHome {
        expected: String,
    },
}

impl WaveStartDecision {
    pub(crate) fn should_start(&self) -> bool {
        matches!(self, Self::Start)
    }
}

impl fmt::Display for WaveStartDecision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => formatter.write_str("assigned to this user and Home"),
            Self::OtherOwner { expected, current } => write!(
                formatter,
                "owned by {expected}, not {}",
                current.as_deref().unwrap_or("the current unknown OS user")
            ),
            Self::OtherHome { expected } => {
                write!(formatter, "assigned to another Home ({expected})")
            }
        }
    }
}

pub(crate) fn wave_start_decision(
    config: Option<&WaveConfig>,
    machine: &MachineIdentity,
) -> WaveStartDecision {
    let Some(config) = config else {
        return WaveStartDecision::Start;
    };
    if let Some(owner) = config
        .owner
        .as_deref()
        .filter(|owner| !owner.trim().is_empty())
    {
        if !machine.matches_owner(owner) {
            return WaveStartDecision::OtherOwner {
                expected: owner.trim().to_string(),
                current: machine.owner.clone(),
            };
        }
    }
    if let Some(home) = config
        .home
        .as_deref()
        .filter(|home| !home.trim().is_empty())
    {
        if !machine.matches_home(home) {
            return WaveStartDecision::OtherHome {
                expected: home.trim().to_string(),
            };
        }
    }
    WaveStartDecision::Start
}

fn current_owner() -> Option<String> {
    for name in ["USER", "USERNAME"] {
        if let Ok(value) = std::env::var(name) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    Command::new("id")
        .arg("-un")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|owner| owner.trim().to_string())
        .filter(|owner| !owner.is_empty())
}

fn insert_name_and_short(names: &mut BTreeSet<String>, raw: &str) {
    let name = normalize_name(raw);
    if name.is_empty() {
        return;
    }
    names.insert(name.clone());
    if let Some((short, _)) = name.split_once('.') {
        names.insert(short.to_string());
    }
}

fn normalize_name(raw: &str) -> String {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.')
        .to_ascii_lowercase()
}

fn ssh_host(target: &str) -> &str {
    target.rsplit_once('@').map_or(target, |(_, host)| host)
}

fn address_is_local(address: IpAddr) -> bool {
    if address.is_unspecified() {
        return false;
    }
    if address.is_loopback() {
        return true;
    }
    TcpListener::bind(SocketAddr::new(address, 0)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::{wave_start_decision, MachineIdentity, WaveStartDecision};
    use crate::durable::HomeId;
    use crate::engine::wave_config::WaveConfig;

    fn config(owner: Option<&str>, home: Option<&str>) -> WaveConfig {
        WaveConfig {
            owner: owner.map(str::to_string),
            home: home.map(str::to_string),
            ..WaveConfig::default()
        }
    }

    #[test]
    fn absent_owner_and_home_start_everywhere() {
        let machine = MachineIdentity::test(Some("jack"), HomeId::new(), &["build-vm"]);
        assert_eq!(
            wave_start_decision(Some(&config(None, None)), &machine),
            WaveStartDecision::Start
        );
    }

    #[test]
    fn owner_and_home_are_independent_filters() {
        let machine = MachineIdentity::test(Some("jack"), HomeId::new(), &["build-vm"]);
        assert!(matches!(
            wave_start_decision(Some(&config(Some("casey"), Some("build-vm"))), &machine),
            WaveStartDecision::OtherOwner { .. }
        ));
        assert!(matches!(
            wave_start_decision(Some(&config(Some("jack"), Some("other-vm"))), &machine),
            WaveStartDecision::OtherHome { .. }
        ));
    }

    #[test]
    fn home_accepts_stable_id_hostname_and_loopback() {
        let home_id = HomeId::new();
        let machine = MachineIdentity::test(Some("jack"), home_id.clone(), &["build-vm"]);
        for home in [
            home_id.as_str(),
            "build-vm",
            "localhost",
            "127.0.0.1",
            "::1",
        ] {
            assert_eq!(
                wave_start_decision(Some(&config(None, Some(home))), &machine),
                WaveStartDecision::Start,
                "{home} should identify this Home"
            );
        }
    }
}
