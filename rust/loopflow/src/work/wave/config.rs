use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::path::{Path, PathBuf};
use tracing::warn;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WaveConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid wave config in {path}: {source}")]
    Parse {
        path: std::path::PathBuf,
        source: serde_yaml_ng::Error,
    },
}

/// The Linear Initiative linked to this Wave.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct WaveLinearConfig {
    #[serde(deserialize_with = "crate::id::deserialize_uuid_option", default)]
    pub initiative: Option<String>,
}

/// One existing Discord guild text channel bound to this Wave's chat.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "provider", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum WaveChatConfig {
    Local,
    Discord {
        guild_id: String,
        channel_id: String,
    },
}

/// Keys only a Wave's `config.yaml` carries; every other key is shared
/// `Config` layered over the repository's (see `crate::config::load_wave_config`).
const WAVE_ONLY_KEYS: [&str; 3] = ["id", "linear", "chat"];

/// The Wave's own identity and bindings from its `config.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WaveConfig {
    pub id: Option<crate::id::WaveId>,
    pub linear: Option<WaveLinearConfig>,
    /// One external presentation binding. Discord is the only supported
    /// provider and remains a concrete variant rather than a registry.
    pub chat: Option<WaveChatConfig>,
}

/// Read the Wave's identity and bindings.
pub fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig> {
    match try_read_wave_config(repo, name) {
        Ok(config) => config,
        Err(err) => {
            warn!(error = %err, "failed to read wave config");
            None
        }
    }
}

/// Read the Wave's identity and bindings and surface a malformed file to
/// callers that must fail closed before starting external side effects.
pub(crate) fn try_read_wave_config(
    repo: &Path,
    name: &str,
) -> Result<Option<WaveConfig>, WaveConfigError> {
    let Some((path, mapping)) = read_wave_file(repo, name)? else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(Value::Mapping(mapping))
        .map(Some)
        .map_err(|source| WaveConfigError::Parse { path, source })
}

/// Read only the external chat binding, so malformed unrelated Wave policy
/// cannot prevent an independent bridge from reading its binding.
pub(crate) fn try_read_wave_chat_config(
    repo: &Path,
    name: &str,
) -> Result<Option<WaveChatConfig>, WaveConfigError> {
    let Some((path, mapping)) = read_wave_file(repo, name)? else {
        return Ok(None);
    };
    let Some(chat) = mapping.get("chat") else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(chat.clone())
        .map(Some)
        .map_err(|source| WaveConfigError::Parse { path, source })
}

/// The Wave's shared `Config` values and the file they came from, without
/// its own identity and bindings.
pub(crate) fn read_wave_layer(
    repo: &Path,
    name: &str,
) -> Result<Option<(PathBuf, Value)>, WaveConfigError> {
    let Some((path, mut mapping)) = read_wave_file(repo, name)? else {
        return Ok(None);
    };
    for key in WAVE_ONLY_KEYS {
        mapping.remove(key);
    }
    Ok(Some((path, Value::Mapping(mapping))))
}

/// The graph the Wave's own `config.yaml` selects for its Tasks.
pub(crate) fn wave_graph(repo: &Path, name: &str) -> Result<Option<String>, WaveConfigError> {
    Ok(read_wave_layer(repo, name)?.and_then(|(_, layer)| {
        ["graph", "workflow"]
            .iter()
            .find_map(|key| layer.get(*key)?.as_str().map(str::to_owned))
    }))
}

/// The Wave's `config.yaml`, or else its GOAL.md frontmatter.
///
/// Released lf reads Wave settings from GOAL.md frontmatter, so a checkout keeps
/// that frontmatter while either build may run in it; a Wave without
/// `config.yaml` reads the keys `config.yaml` holds from it. LOO-640 removes
/// this fallback once a release reading `config.yaml` is installed.
fn read_wave_file(repo: &Path, name: &str) -> Result<Option<(PathBuf, Mapping)>, WaveConfigError> {
    let path = config_path(repo, name);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let mapping = parse_mapping(&path, &content)?;
            return Ok(Some((path, mapping)));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => return Err(WaveConfigError::Read { path, source }),
    }
    let path = goal_path(repo, name);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(WaveConfigError::Read { path, source }),
    };
    let Some((frontmatter, _)) = split_frontmatter(&content) else {
        return Ok(Some((path, Mapping::new())));
    };
    let mut mapping = parse_mapping(&path, &frontmatter)?;
    let shared = crate::config::wave_config_keys();
    mapping.retain(|key, _| {
        key.as_str()
            .is_some_and(|key| WAVE_ONLY_KEYS.contains(&key) || shared.contains(key))
    });
    Ok(Some((path, mapping)))
}

fn parse_mapping(path: &Path, content: &str) -> Result<Mapping, WaveConfigError> {
    serde_yaml_ng::from_str::<Option<Mapping>>(content)
        .map(Option::unwrap_or_default)
        .map_err(|source| WaveConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
}

/// One-line Wave objective for status, PM, and API projections.
///
/// The checkout goal is the source of truth. The summary is the first paragraph of
/// `## Objective`, falling back to the first prose paragraph when that section
/// is absent.
pub fn read_wave_summary(repo: &Path, name: &str) -> std::io::Result<String> {
    match std::fs::read_to_string(goal_path(repo, name)) {
        Ok(content) => Ok(wave_summary(&content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub(crate) fn wave_summary(content: &str) -> String {
    let body = split_frontmatter(content)
        .map(|(_, body)| body)
        .unwrap_or_else(|| content.to_string());
    let objective = markdown_section(&body, "Objective");
    let summary = first_paragraph(&objective);
    if summary.is_empty() {
        first_prose_paragraph(&body)
    } else {
        summary
    }
}

fn split_frontmatter(content: &str) -> Option<(String, String)> {
    if !content.starts_with("---") {
        return None;
    }
    let mut parts = content.splitn(3, "---");
    let _ = parts.next();
    let frontmatter = parts.next()?;
    let rest = parts.next()?;
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();
    Some((frontmatter.to_string(), body))
}

fn goal_path(repo: &Path, name: &str) -> PathBuf {
    repo.join(".lf/wave").join(name).join("GOAL.md")
}

/// The committed file holding a Wave's shared setup.
pub fn config_path(repo: &Path, name: &str) -> PathBuf {
    repo.join(".lf/wave").join(name).join("config.yaml")
}

fn markdown_section(content: &str, heading: &str) -> String {
    let marker = format!("## {heading}");
    let mut in_section = false;
    let mut lines = Vec::new();
    for line in content.lines() {
        if line.trim() == marker {
            in_section = true;
            continue;
        }
        if in_section && line.trim_start().starts_with("## ") {
            break;
        }
        if in_section {
            lines.push(line);
        }
    }
    lines.join("\n").trim().to_string()
}

fn first_paragraph(content: &str) -> String {
    content
        .split("\n\n")
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|paragraph| !paragraph.is_empty())
        .unwrap_or_default()
}

fn first_prose_paragraph(content: &str) -> String {
    content
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .filter(|paragraph| !paragraph.starts_with('#'))
        .map(|paragraph| paragraph.split_whitespace().collect::<Vec<_>>().join(" "))
        .next()
        .unwrap_or_default()
}

/// Update the Wave's `config.yaml`. A Wave without one starts from the
/// values its GOAL.md frontmatter carried, so moving loses nothing.
pub(crate) fn update_wave_config(
    repo: &Path,
    name: &str,
    update: impl FnOnce(&mut Mapping) -> Result<(), String>,
) -> Result<(), String> {
    let directory = repo.join(".lf/wave").join(name);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;
    let mut mapping = read_wave_file(repo, name)
        .map_err(|error| error.to_string())?
        .map(|(_, mapping)| mapping)
        .unwrap_or_default();
    update(&mut mapping)?;
    let path = config_path(repo, name);
    let content = serde_yaml_ng::to_string(&Value::Mapping(mapping))
        .map_err(|error| format!("failed to render {}: {error}", path.display()))?;
    std::fs::write(&path, content)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        read_wave_config, read_wave_layer, read_wave_summary, try_read_wave_chat_config,
        try_read_wave_config, update_wave_config, WaveChatConfig, WaveConfigError,
    };
    use std::path::Path;

    struct ConfigRepo {
        repo: tempfile::TempDir,
    }

    impl ConfigRepo {
        fn new() -> Self {
            let repo = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(repo.path().join(".lf/wave/scan")).unwrap();
            Self { repo }
        }
        fn path(&self) -> &Path {
            self.repo.path()
        }
        fn write(&self, content: &str) -> std::io::Result<()> {
            std::fs::write(self.path().join(".lf/wave/scan/GOAL.md"), content)
        }
        fn write_config(&self, content: &str) {
            std::fs::write(self.path().join(".lf/wave/scan/config.yaml"), content).unwrap();
        }
        fn layer(&self) -> serde_yaml_ng::Value {
            read_wave_layer(self.path(), "scan").unwrap().unwrap().1
        }
    }

    #[test]
    fn authored_waves_use_only_the_lf_directory() {
        let repo = ConfigRepo::new();
        let unrelated = repo.path().join("wave/scan");
        std::fs::create_dir_all(&unrelated).unwrap();
        let original = "linear:\n  initiative: unrelated\n";
        std::fs::write(unrelated.join("config.yaml"), original).unwrap();
        assert!(read_wave_config(repo.path(), "scan").is_none());

        repo.write_config("agent: codex\n");
        update_wave_config(repo.path(), "scan", |map| {
            map.insert("agent".into(), "claude".into());
            Ok(())
        })
        .unwrap();
        assert_eq!(repo.layer()["agent"].as_str(), Some("claude"));
        assert_eq!(
            std::fs::read_to_string(unrelated.join("config.yaml")).unwrap(),
            original
        );
    }

    #[test]
    fn config_yaml_wins_over_goal_frontmatter() {
        let repo = ConfigRepo::new();
        repo.write("---\nowner: jack\nagent: codex\nid: 6155f18a-1b7f-418c-9af3-8d6fa5ce4989\n---\nDrive the work.\n")
            .unwrap();
        assert_eq!(repo.layer()["agent"].as_str(), Some("codex"));
        assert!(repo.layer().get("owner").is_none());
        assert!(repo.layer().get("id").is_none());
        assert!(read_wave_config(repo.path(), "scan").unwrap().id.is_some());

        repo.write_config("agent: claude\n");
        assert_eq!(repo.layer()["agent"].as_str(), Some("claude"));
        assert!(read_wave_config(repo.path(), "scan").unwrap().id.is_none());

        // An empty file still replaces the old frontmatter.
        for empty in ["", " \n", "# No overrides\n", "null\n"] {
            repo.write_config(empty);
            assert!(repo.layer().as_mapping().unwrap().is_empty());
            assert!(read_wave_config(repo.path(), "scan").unwrap().id.is_none());
        }
    }

    #[test]
    fn first_edit_carries_frontmatter_into_config_yaml() {
        let repo = ConfigRepo::new();
        let body = "\n## Objective\n\nKeep tokens λ.\n";
        repo.write(&format!(
            "---\nagent: codex\nlinear:\n  initiative: lin-123\n---\n{body}"
        ))
        .unwrap();
        update_wave_config(repo.path(), "scan", |map| {
            map.insert("graph".into(), "code".into());
            Ok(())
        })
        .unwrap();
        let goal = std::fs::read_to_string(repo.path().join(".lf/wave/scan/GOAL.md")).unwrap();
        assert!(goal.ends_with(body));
        assert_eq!(repo.layer()["agent"].as_str(), Some("codex"));
        assert_eq!(repo.layer()["graph"].as_str(), Some("code"));
        assert_eq!(
            read_wave_config(repo.path(), "scan")
                .unwrap()
                .linear
                .unwrap()
                .initiative
                .as_deref(),
            Some("lin-123")
        );
    }

    #[test]
    fn read_wave_summary_prefers_the_objective() {
        let temp = ConfigRepo::new();
        temp.write(
            "---\nagent: codex\n---\n\n## Objective\n\nKeep the system\nboring.\n\n## Process\n\nDo the work.\n",
        )
        .expect("write");

        assert_eq!(
            read_wave_summary(temp.path(), "scan").expect("summary"),
            "Keep the system boring."
        );
    }

    #[test]
    fn discord_chat_config_is_typed_and_invalid_bindings_fail_closed() {
        let temp = ConfigRepo::new();
        temp.write_config("chat:\n  provider: discord\n  guild_id: guild\n  channel_id: channel\n");
        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert!(matches!(
            config.chat,
            Some(WaveChatConfig::Discord { guild_id, channel_id })
                if guild_id == "guild" && channel_id == "channel"
        ));

        // `deny_unknown_fields` rejects a stale `machine_id`, so the binding fails closed.
        temp.write_config(
            "chat:\n  provider: discord\n  machine_id: home_1\n  guild_id: guild\n  channel_id: channel\n",
        );
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        temp.write_config("chat:\n  provider: discord\n  guild_id: guild\n");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        temp.write_config("chat:\n  provider: local\n");
        assert!(matches!(
            try_read_wave_config(temp.path(), "scan")
                .expect("local config")
                .and_then(|config| config.chat),
            Some(WaveChatConfig::Local)
        ));

        temp.write_config("agent: [not-a-string]\n");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Ok(None)
        ));
    }

    #[test]
    fn read_wave_config_returns_none_for_missing() {
        let temp = ConfigRepo::new();
        assert!(read_wave_config(temp.path(), "nonexistent").is_none());
    }
}
