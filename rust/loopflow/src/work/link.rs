//! Public Work addresses: `loopflow://<repository>[/task/<id>|/wave/<name>]`.
//!
//! An address names Work, never a Session, provider, checkout path or
//! presentation host. Opening resolves its conversation through the shared
//! owners. `?repository=<id>` qualifies a short name shared by several
//! registered repositories. `session` and `diff` are explicit CLI selectors
//! carried to Desktop; ordinary emitted links never include them.

use anyhow::{bail, ensure, Result};

use crate::durable::RepositoryId;

const SCHEME: &str = "loopflow";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkTarget {
    Repository,
    Wave(String),
    Task(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkUrl {
    /// The repository's folder name; it locates, the qualifier identifies.
    pub repository: String,
    pub qualifier: Option<RepositoryId>,
    pub target: WorkTarget,
    pub session: Option<String>,
    pub diff: bool,
}

impl WorkUrl {
    pub fn new(repository: impl Into<String>, target: WorkTarget) -> Self {
        Self {
            repository: repository.into(),
            qualifier: None,
            target,
            session: None,
            diff: false,
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        let rest = text
            .strip_prefix(SCHEME)
            .and_then(|rest| rest.strip_prefix("://"))
            .ok_or_else(|| anyhow::anyhow!("Work links start with loopflow://"))?;
        ensure!(
            !rest.chars().any(|ch| ch.is_control() || ch.is_whitespace()),
            "Work links cannot contain spaces or control characters; percent-encode them"
        );
        ensure!(!rest.contains('#'), "Work links do not accept a fragment");
        let (location, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (authority, path) = location
            .split_once('/')
            .map_or((location, ""), |(authority, path)| (authority, path));
        ensure!(
            !authority.contains('@') && !authority.contains(':'),
            "Work links do not accept credentials or a port"
        );
        let repository = component(authority, "repository name")?;
        let segments: Vec<&str> = if path.is_empty() {
            Vec::new()
        } else {
            path.split('/').collect()
        };
        let target = match segments.as_slice() {
            [] => WorkTarget::Repository,
            ["task", id] => WorkTarget::Task(component(id, "Task identifier")?),
            ["wave", name] => WorkTarget::Wave(component(name, "Wave name")?),
            _ => bail!(
                "Work links name a repository, /task/<identifier> or /wave/<name>; got /{path}"
            ),
        };
        let mut url = Self::new(repository, target);
        let mut seen = Vec::new();
        for pair in query.split('&').filter(|pair| !pair.is_empty()) {
            let (name, value) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("Work link query `{pair}` has no value"))?;
            ensure!(!seen.contains(&name), "Work link query repeats `{name}`");
            seen.push(name);
            let value = component(value, name)?;
            match name {
                "repository" => url.qualifier = Some(RepositoryId::parse(&value)?),
                "session" => url.session = Some(value),
                "diff" => {
                    ensure!(value == "true", "Work link `diff` accepts only true");
                    url.diff = true;
                }
                _ => bail!(
                    "Work links accept only repository, session and diff queries; got `{name}`"
                ),
            }
        }
        ensure!(
            matches!(url.target, WorkTarget::Task(_)) || (url.session.is_none() && !url.diff),
            "Session and Changes selectors need a Task link"
        );
        Ok(url)
    }

    /// The ordinary address: Work identity only, without explicit selectors.
    pub fn work(&self) -> Self {
        Self {
            session: None,
            diff: false,
            ..self.clone()
        }
    }
}

impl std::fmt::Display for WorkUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{SCHEME}://{}", encode(&self.repository))?;
        match &self.target {
            WorkTarget::Repository => {}
            WorkTarget::Wave(name) => write!(f, "/wave/{}", encode(name))?,
            WorkTarget::Task(id) => write!(f, "/task/{}", encode(id))?,
        }
        let mut separator = '?';
        let mut query = |f: &mut std::fmt::Formatter<'_>, name: &str, value: &str| {
            let result = write!(f, "{separator}{name}={}", encode(value));
            separator = '&';
            result
        };
        if let Some(qualifier) = &self.qualifier {
            query(f, "repository", qualifier.as_str())?;
        }
        if let Some(session) = &self.session {
            query(f, "session", session)?;
        }
        if self.diff {
            query(f, "diff", "true")?;
        }
        Ok(())
    }
}

/// One clickable identifier-and-title span for a terminal supporting OSC 8.
pub fn osc8(label: &str, url: impl std::fmt::Display) -> String {
    let label: String = label.chars().filter(|ch| !ch.is_control()).collect();
    format!("\x1b]8;;{url}\x1b\\{label}\x1b]8;;\x1b\\")
}

pub fn markdown(label: &str, url: impl std::fmt::Display) -> String {
    let label = label
        .chars()
        .filter(|ch| !ch.is_control())
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]");
    format!("[{label}]({url})")
}

fn component(raw: &str, what: &str) -> Result<String> {
    let value =
        decode(raw).ok_or_else(|| anyhow::anyhow!("{what} is not valid percent-encoded UTF-8"))?;
    ensure!(!value.is_empty(), "{what} is empty");
    ensure!(
        !value.contains(['/', '\\']) && !value.chars().any(|ch| ch.is_control()),
        "{what} cannot contain slashes or control characters"
    );
    Ok(value)
}

fn decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

/// URI component encoding: unreserved characters stay, everything else is
/// percent-encoded. `+` is never a space.
fn encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::{markdown, osc8, RepositoryId, WorkTarget, WorkUrl};

    #[test]
    fn task_wave_and_repository_addresses_round_trip() {
        let qualified = format!(
            "loopflow://my%20repo/task/LOO-1?repository={}",
            RepositoryId::new()
        );
        for text in [
            "loopflow://loopflow",
            "loopflow://loopflow/wave/product",
            "loopflow://loopflow/task/LOO-426",
            &qualified,
            "loopflow://loopflow/task/LOO-1?session=a%2Bb%26c&diff=true",
        ] {
            let url = WorkUrl::parse(text).unwrap();
            assert_eq!(url.to_string(), text);
        }
        let task = WorkUrl::parse("loopflow://loopflow/task/LOO-426").unwrap();
        assert_eq!(task.repository, "loopflow");
        assert_eq!(task.target, WorkTarget::Task("LOO-426".into()));
    }

    #[test]
    fn plus_is_literal_not_a_space() {
        let url = WorkUrl::parse("loopflow://loopflow/task/A?session=x+y").unwrap();
        assert_eq!(url.session.as_deref(), Some("x+y"));
    }

    #[test]
    fn malformed_addresses_are_rejected() {
        for text in [
            "loopflow:/loopflow",
            "https://loopflow/task/A",
            "loopflow://",
            "loopflow://loopflow/task/",
            "loopflow://loopflow/task/A/B",
            "loopflow://loopflow/task/A%2FB",
            "loopflow://loopflow/task/A%0AB",
            "loopflow://loopflow/task/A#node",
            "loopflow://user@loopflow/task/A",
            "loopflow://loopflow:80/task/A",
            "loopflow://loopflow/project/A",
            "loopflow://loopflow/task/A?repo=%2Fsrc",
            "loopflow://loopflow/task/A?session=x&session=y",
            "loopflow://loopflow/task/A?session=",
            "loopflow://loopflow/task/A?diff=1",
            "loopflow://loopflow/wave/product?diff=true",
            "loopflow://loopflow?session=s",
            "loopflow://loop flow",
        ] {
            assert!(WorkUrl::parse(text).is_err(), "{text} parsed");
        }
    }

    #[test]
    fn ordinary_address_drops_explicit_selectors() {
        let url = WorkUrl::parse("loopflow://loopflow/task/A?session=s&diff=true").unwrap();
        assert_eq!(url.work().to_string(), "loopflow://loopflow/task/A");
    }

    #[test]
    fn labels_render_as_one_span() {
        let url = WorkUrl::new("loopflow", WorkTarget::Task("LOO-489".into()));
        assert_eq!(
            markdown("LOO-489 · Enable [multitask] navigation", &url),
            "[LOO-489 · Enable \\[multitask\\] navigation](loopflow://loopflow/task/LOO-489)"
        );
        assert_eq!(
            osc8("LOO-489 · Nav\x1b", &url),
            "\x1b]8;;loopflow://loopflow/task/LOO-489\x1b\\LOO-489 · Nav\x1b]8;;\x1b\\"
        );
    }
}
