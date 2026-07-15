//! A Wave's execution home: where its top-level `lf` commands run.
//!
//! Either `local` (this machine) or one explicit SSH target such as the Mac
//! mini. The home is authored on the Wave (GOAL.md frontmatter), inherited by
//! Project and Task launches, and used to route repo/PR/release commands. It is
//! not a daemon, an environment-selected role, or a new transport — a remote
//! home reuses the existing `lf ssh` credential-forwarding surface.
//!
//! Authored forms in `GOAL.md`:
//! - `home: local` (also the default when the field is absent)
//! - `home: ssh://<host>`
//! - `home: ssh://<host>/<repo>` (repo relative to `$HOME` on the target)
//!
//! `parse` is the single input funnel (Postel: liberal in, strict out), and
//! `Display` round-trips it. The typed value never carries a string the router
//! has to re-parse to decide a host.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The pinned/inherited home a launched child carries. Read before GOAL.md so a
/// child routes to the home its Session was launched with.
pub const WAVE_HOME_ENV: &str = "LF_WAVE_HOME";

/// Set on the remote hop by the router. Its presence means "you are already the
/// home host; run the command locally" — the single break in the forward loop.
pub const HOME_ROUTED_ENV: &str = "LF_HOME_ROUTED";

/// Where a Wave's routed commands execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaveHome {
    /// This machine — the default. Commands run in-process, as they always have.
    Local,
    /// One SSH target. `repo` is the repository path relative to `$HOME` on the
    /// host; `None` means the caller's default (`ssh::DEFAULT_REPO`).
    Ssh { host: String, repo: Option<String> },
}

impl WaveHome {
    /// Parse an authored home string. `None` for anything unrecognized, so a
    /// typo fails loudly at the read site rather than silently routing wrong.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if raw.eq_ignore_ascii_case("local") {
            return Some(Self::Local);
        }
        let rest = raw.strip_prefix("ssh://")?;
        let (host, repo) = match rest.split_once('/') {
            Some((host, repo)) => (host, Some(repo)),
            None => (rest, None),
        };
        let host = host.trim();
        if host.is_empty() || host.contains(char::is_whitespace) {
            return None;
        }
        let repo = repo
            .map(str::trim)
            .filter(|repo| !repo.is_empty())
            .map(str::to_string);
        Some(Self::Ssh {
            host: host.to_string(),
            repo,
        })
    }

    /// Whether commands for this home run on another machine.
    pub fn is_remote(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }

    /// The SSH host, when remote.
    pub fn host(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Ssh { host, .. } => Some(host),
        }
    }

    /// The target repo path relative to `$HOME`, when remote and authored.
    pub fn repo(&self) -> Option<&str> {
        match self {
            Self::Ssh { repo, .. } => repo.as_deref(),
            Self::Local => None,
        }
    }
}

impl fmt::Display for WaveHome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Ssh {
                host,
                repo: Some(repo),
            } => write!(f, "ssh://{host}/{repo}"),
            Self::Ssh { host, repo: None } => write!(f, "ssh://{host}"),
        }
    }
}

impl FromStr for WaveHome {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw).ok_or_else(|| format!("invalid wave home: {raw:?}"))
    }
}

// Serde rides the string form so the pinned/inherited value and the authored
// GOAL.md value are always the same bytes — no second shape to drift.
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

/// Wire projection for status: a tagged shape both language mirrors decode
/// cleanly, rather than reparsing a string on every consumer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WaveHomeDto {
    Local,
    Ssh { host: String, repo: Option<String> },
}

impl From<&WaveHome> for WaveHomeDto {
    fn from(home: &WaveHome) -> Self {
        match home {
            WaveHome::Local => Self::Local,
            WaveHome::Ssh { host, repo } => Self::Ssh {
                host: host.clone(),
                repo: repo.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_round_trips_every_authored_form() {
        for (raw, home) in [
            ("local", WaveHome::Local),
            (
                "ssh://mini-heart",
                WaveHome::Ssh {
                    host: "mini-heart".to_string(),
                    repo: None,
                },
            ),
            (
                "ssh://mini-heart/src/loopflow",
                WaveHome::Ssh {
                    host: "mini-heart".to_string(),
                    repo: Some("src/loopflow".to_string()),
                },
            ),
        ] {
            assert_eq!(WaveHome::parse(raw), Some(home.clone()));
            assert_eq!(home.to_string(), raw);
            assert_eq!(home.to_string().parse::<WaveHome>().unwrap(), home);
        }
    }

    #[test]
    fn local_is_case_insensitive_and_absent_forms_are_rejected() {
        assert_eq!(WaveHome::parse("Local"), Some(WaveHome::Local));
        assert_eq!(WaveHome::parse("  local  "), Some(WaveHome::Local));
        assert_eq!(WaveHome::parse(""), None);
        assert_eq!(WaveHome::parse("   "), None);
        assert_eq!(WaveHome::parse("mini-heart"), None); // no scheme
        assert_eq!(WaveHome::parse("ssh://"), None); // empty host
        assert_eq!(WaveHome::parse("ssh://a b"), None); // whitespace in host
    }

    #[test]
    fn remote_accessors_and_dto() {
        let home = WaveHome::parse("ssh://mini-heart/src/loopflow").unwrap();
        assert!(home.is_remote());
        assert_eq!(home.host(), Some("mini-heart"));
        assert_eq!(home.repo(), Some("src/loopflow"));
        assert!(!WaveHome::Local.is_remote());
        assert_eq!(WaveHome::Local.host(), None);

        assert_eq!(WaveHomeDto::from(&WaveHome::Local), WaveHomeDto::Local);
        assert_eq!(
            WaveHomeDto::from(&home),
            WaveHomeDto::Ssh {
                host: "mini-heart".to_string(),
                repo: Some("src/loopflow".to_string()),
            }
        );
    }

    #[test]
    fn serde_uses_the_string_form() {
        let home = WaveHome::parse("ssh://mini-heart").unwrap();
        let json = serde_json::to_string(&home).unwrap();
        assert_eq!(json, "\"ssh://mini-heart\"");
        let decoded: WaveHome = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, home);
    }
}
