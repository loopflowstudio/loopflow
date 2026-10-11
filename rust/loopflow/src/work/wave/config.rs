use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use tracing::warn;

use crate::prompt::context_budget::BudgetKey;

#[derive(Debug, thiserror::Error)]
pub(crate) enum WaveConfigError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
    #[error("invalid wave goal frontmatter in {path}: {source}")]
    Parse {
        path: std::path::PathBuf,
        source: serde_yaml_ng::Error,
    },
}

/// One cron line from GOAL.md frontmatter: `crons: [{flow, schedule}]`.
/// `lf wave cron sync` installs these schedules on the placed Machine.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WaveCronDef {
    pub flow: String,
    pub schedule: String,
}

/// The Linear Initiative representing a wave, from `pm.*` in GOAL.md.
///
/// `provider` and `linear_team` remain decodable only as repository-Team
/// migration sentinels. Normal PM authority reads provider and Team from the
/// repository's `.lf/config.yaml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct WavePmConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub linear_initiative: Option<String>,
    #[serde(default)]
    pub linear_team: Option<String>,
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

/// Machine policy read from the checkout Wave goal frontmatter.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WaveConfig {
    pub id: Option<crate::id::WaveId>,
    #[serde(default)]
    pub context_budgets: BTreeMap<BudgetKey, usize>,
    pub crons: Option<Vec<WaveCronDef>>,
    pub agent: Option<String>,
    pub skill_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    /// One external presentation binding. Discord is the only supported
    /// provider and remains a concrete variant rather than a registry.
    pub chat: Option<WaveChatConfig>,
}

fn parse_wave_config(content: &str) -> Result<WaveConfig, serde_yaml_ng::Error> {
    match split_frontmatter(content) {
        Some((frontmatter, _)) => serde_yaml_ng::from_str(&frontmatter),
        None => Ok(WaveConfig::default()),
    }
}

/// Read Wave intent from its checkout goal frontmatter.
pub fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig> {
    match try_read_wave_config(repo, name) {
        Ok(config) => config,
        Err(err) => {
            warn!(error = %err, "failed to read wave config");
            None
        }
    }
}

/// Read wave machine policy and surface malformed frontmatter to callers that
/// must fail closed before starting external side effects.
pub(crate) fn try_read_wave_config(
    repo: &Path,
    name: &str,
) -> Result<Option<WaveConfig>, WaveConfigError> {
    let path = goal_path(repo, name);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(WaveConfigError::Read { path, source }),
    };
    parse_wave_config(&content)
        .map(Some)
        .map_err(|source| WaveConfigError::Parse { path, source })
}

/// Read only the external chat binding, so malformed unrelated Wave policy
/// cannot prevent an independent bridge from reading its binding.
pub(crate) fn try_read_wave_chat_config(
    repo: &Path,
    name: &str,
) -> Result<Option<WaveChatConfig>, WaveConfigError> {
    let path = goal_path(repo, name);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(WaveConfigError::Read { path, source }),
    };
    let Some((frontmatter, _)) = split_frontmatter(&content) else {
        return Ok(None);
    };
    let value = serde_yaml_ng::from_str::<Value>(&frontmatter).map_err(|source| {
        WaveConfigError::Parse {
            path: path.clone(),
            source,
        }
    })?;
    let Some(chat) = value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String("chat".to_string())))
    else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(chat.clone())
        .map(Some)
        .map_err(|source| WaveConfigError::Parse { path, source })
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

fn goal_path(repo: &Path, name: &str) -> std::path::PathBuf {
    repo.join("wave").join(name).join("GOAL.md")
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

/// Update checkout Wave frontmatter, preserving its objective body.
pub(crate) fn update_wave_goal_config(
    repo: &Path,
    name: &str,
    update: impl FnOnce(&mut Mapping) -> Result<(), String>,
) -> Result<(), String> {
    let path = goal_path(repo, name);
    let content = std::fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let (mut value, body) = match split_frontmatter(&content) {
        Some((frontmatter, body)) => {
            let value = serde_yaml_ng::from_str::<Value>(&frontmatter)
                .map_err(|error| format!("invalid yaml in {}: {error}", path.display()))?;
            (value, body)
        }
        None => (Value::Mapping(Mapping::new()), content),
    };
    let map = value.as_mapping_mut().ok_or_else(|| {
        format!(
            "wave goal frontmatter at {} must be a mapping",
            path.display()
        )
    })?;
    update(map)?;

    let frontmatter = serde_yaml_ng::to_string(&value)
        .map_err(|error| format!("failed to render wave goal frontmatter: {error}"))?;
    std::fs::write(&path, format!("---\n{frontmatter}---\n{body}"))
        .map_err(|error| format!("failed to write {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{
        read_wave_config, read_wave_summary, try_read_wave_chat_config, try_read_wave_config,
        update_wave_goal_config, WaveChatConfig, WaveConfigError,
    };
    use std::path::Path;

    struct ConfigRepo {
        repo: tempfile::TempDir,
    }

    impl ConfigRepo {
        fn new() -> Self {
            let repo = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(repo.path().join("wave/scan")).unwrap();
            Self { repo }
        }
        fn path(&self) -> &Path {
            self.repo.path()
        }
        fn write(&self, content: &str) -> std::io::Result<()> {
            std::fs::write(self.path().join("wave/scan/GOAL.md"), content)
        }
    }

    #[test]
    fn frontmatter_edits_do_not_create_missing_goals() {
        let repo = ConfigRepo::new();
        assert!(update_wave_goal_config(repo.path(), "scan", |_| Ok(())).is_err());
        assert!(!repo.path().join("wave/scan/GOAL.md").exists());
    }

    #[test]
    fn read_wave_config_parses_machine_frontmatter() {
        let temp = ConfigRepo::new();
        temp.write(
            "---\nowner: jack\nhome: build.example.com\nagent: codex\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.agent.as_deref(), Some("codex"));
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
    fn read_wave_config_parses_linear_pm_block() {
        let temp = ConfigRepo::new();
        temp.write(
            "---\npm:\n  provider: linear\n  linear_initiative: \"lin-123\"\n  linear_team: \"team-prd\"\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        let pm = config.pm.expect("pm config should exist");
        assert_eq!(pm.provider.as_deref(), Some("linear"));
        assert_eq!(pm.linear_initiative.as_deref(), Some("lin-123"));
        assert_eq!(pm.linear_team.as_deref(), Some("team-prd"));
    }

    #[test]
    fn discord_chat_config_is_typed_and_invalid_bindings_fail_closed() {
        let temp = ConfigRepo::new();
        temp.write(
            "---\nchat:\n  provider: discord\n  guild_id: guild\n  channel_id: channel\n---\nDrive the work.\n",
        )
        .expect("write");
        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert!(matches!(
            config.chat,
            Some(WaveChatConfig::Discord { guild_id, channel_id })
                if guild_id == "guild" && channel_id == "channel"
        ));
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Ok(Some(WaveChatConfig::Discord { guild_id, channel_id }))
                if guild_id == "guild" && channel_id == "channel"
        ));

        // The binding is local: `machine_id` is no longer a Discord-config field.
        // `deny_unknown_fields` rejects it, so a stale GOAL.md fails closed.
        temp.write(
            "---\nchat:\n  provider: discord\n  machine_id: home_11111111111111111111111111111111\n  guild_id: guild\n  channel_id: channel\n---\nDrive the work.\n",
        )
        .expect("write stale machine_id");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        // A missing required field still fails closed.
        temp.write("---\nchat:\n  provider: discord\n  guild_id: guild\n---\nDrive the work.\n")
            .expect("write missing channel");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        temp.write("---\nchat:\n  provider: local\n---\nDrive the work.\n")
            .expect("write local chat");
        assert!(matches!(
            try_read_wave_config(temp.path(), "scan")
                .expect("local config")
                .and_then(|config| config.chat),
            Some(WaveChatConfig::Local)
        ));

        temp.write("---\nowner: [not-a-string]\n---\nDrive the work.\n")
            .expect("write unrelated invalid policy");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Ok(None)
        ));
    }

    /// Crons live in GOAL.md frontmatter, the schedule source. Legacy `triggers:` keys are simply unknown fields now.
    #[test]
    fn read_wave_config_parses_crons_and_ignores_legacy_triggers() {
        let temp = ConfigRepo::new();
        temp.write(
            "---\ncrons:\n  - flow: wave-polish\n    schedule: '0 0 0 * * Mon *'\ntriggers:\n  signal: wave\n  source: infra\n  source_repo: /tmp/source\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        let crons = config.crons.expect("crons parse from frontmatter");
        assert_eq!(crons.len(), 1);
        assert_eq!(crons[0].flow, "wave-polish");
        assert_eq!(crons[0].schedule, "0 0 0 * * Mon *");
    }

    #[test]
    fn read_wave_config_returns_none_for_missing() {
        let temp = ConfigRepo::new();
        assert!(read_wave_config(temp.path(), "nonexistent").is_none());
    }

    #[test]
    fn frontmatter_edits_preserve_the_authored_body_and_unrelated_fields() {
        let repo = ConfigRepo::new();
        let body = "\n## Objective\n\nKeep tokens λ.\n";
        repo.write(&format!("---\nagent: codex\n---\n{body}"))
            .unwrap();
        update_wave_goal_config(repo.path(), "scan", |map| {
            map.insert(
                "pm".into(),
                serde_yaml_ng::from_str("linear_initiative: lin-123").unwrap(),
            );
            Ok(())
        })
        .unwrap();
        let content = std::fs::read_to_string(repo.path().join("wave/scan/GOAL.md")).unwrap();
        assert!(content.ends_with(body));
        let config = read_wave_config(repo.path(), "scan").unwrap();
        assert_eq!(config.agent.as_deref(), Some("codex"));
        assert_eq!(
            config.pm.unwrap().linear_initiative.as_deref(),
            Some("lin-123")
        );
    }
}
