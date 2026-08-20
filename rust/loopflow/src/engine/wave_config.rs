use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

use crate::durable::HomeId;

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
/// The wave's resident loop reads these and opens a system pass when a
/// schedule comes due (`crate::flowloop::wave`) — no daemon poller, no table.
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
        #[serde(deserialize_with = "deserialize_home_id")]
        home_id: HomeId,
        guild_id: String,
        channel_id: String,
    },
}

fn deserialize_home_id<'de, D>(deserializer: D) -> Result<HomeId, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    HomeId::parse(&value).map_err(serde::de::Error::custom)
}

/// Machine policy read from `wave/<name>/GOAL.md` frontmatter.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WaveConfig {
    /// OS user allowed to start this Wave automatically. Absent means any user.
    pub owner: Option<String>,
    /// Machine allowed to start this Wave automatically. Accepts a HomeId,
    /// hostname, or IP address. Absent means any Home.
    pub home: Option<String>,
    pub crons: Option<Vec<WaveCronDef>>,
    pub agent: Option<String>,
    pub skill_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    /// One external presentation binding. Discord is the only supported
    /// provider and remains a concrete variant rather than a registry.
    pub chat: Option<WaveChatConfig>,
    /// The safety valve: `paused: true` in GOAL.md frontmatter tells the wave
    /// listener to refuse to START turns (message→turn, heartbeat, cron)
    /// while keeping the listener serving and queueing. File-first and re-read
    /// live (`crate::wave::runtime::WaveRuntime::paused`), not the registry
    /// row.
    pub paused: Option<bool>,
    /// Backup agent for disconnect-class body failures: when an opencode body
    /// goes hollow or the SSE stream disconnects, the next generation is handed
    /// to this agent instead of retrying the same flaky provider. Example:
    /// `backup_agent: claude:opus`. Absent → no auto-handoff (the body fails
    /// and the supervisor respawns the same agent if replay-safe, or stops).
    /// Re-read live from GOAL.md, not the registry row.
    #[serde(default)]
    pub backup_agent: Option<String>,
}

/// Read wave intent from `wave/<name>/GOAL.md` frontmatter.
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
    Ok(Some(match split_frontmatter(&content) {
        Some((frontmatter, _)) => serde_yaml_ng::from_str::<WaveConfig>(&frontmatter)
            .map_err(|source| WaveConfigError::Parse { path, source })?,
        None => WaveConfig::default(),
    }))
}

/// Read only the external chat binding, so malformed unrelated Wave policy
/// cannot turn listener startup into a new validation boundary.
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
/// GOAL.md remains the source of truth. The summary is the first paragraph of
/// `## Objective`, falling back to the first prose paragraph when that section
/// is absent.
pub fn read_wave_summary(repo: &Path, name: &str) -> std::io::Result<String> {
    let content = std::fs::read_to_string(goal_path(repo, name))?;
    let body = split_frontmatter(&content)
        .map(|(_, body)| body)
        .unwrap_or(content);
    let objective = markdown_section(&body, "Objective");
    let summary = first_paragraph(&objective);
    if summary.is_empty() {
        Ok(first_prose_paragraph(&body))
    } else {
        Ok(summary)
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

fn empty_goal_body(name: &str) -> String {
    format!("Run one loop iteration for the {name} wave.\n")
}

fn goal_value_from_content(path: &Path, name: &str) -> Result<(Value, String), String> {
    if !path.exists() {
        return Ok((Value::Mapping(Mapping::new()), empty_goal_body(name)));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let Some((frontmatter, body)) = split_frontmatter(&content) else {
        return Ok((Value::Mapping(Mapping::new()), content));
    };

    let value = serde_yaml_ng::from_str::<Value>(&frontmatter)
        .map_err(|err| format!("invalid yaml in {}: {err}", path.display()))?;
    Ok((value, body))
}

fn render_goal_md(value: &Value, body: &str) -> Result<String, String> {
    let rendered = serde_yaml_ng::to_string(value)
        .map_err(|err| format!("failed to render wave goal frontmatter: {err}"))?;
    Ok(format!("---\n{}---\n{}", rendered, body))
}

fn wave_config_map<'a>(value: &'a mut Value, path: &Path) -> Result<&'a mut Mapping, String> {
    value.as_mapping_mut().ok_or_else(|| {
        format!(
            "wave goal frontmatter at {} must be a mapping",
            path.display()
        )
    })
}

fn remove_or_set_string(map: &mut Mapping, field: &str, value: Option<String>) {
    let key = Value::String(field.to_string());
    match value {
        Some(value) if !value.trim().is_empty() => {
            map.insert(key, Value::String(value));
        }
        Some(_) => {
            map.remove(&key);
        }
        None => {}
    }
}

fn remove_or_set_skill_agents(
    map: &mut Mapping,
    skill_agents: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let key = Value::String("skill_agents".to_string());
    match skill_agents {
        Some(skill_agents) if !skill_agents.is_empty() => {
            map.insert(
                key,
                serde_yaml_ng::to_value(skill_agents)
                    .map_err(|err| format!("failed to encode skill_agents: {err}"))?,
            );
        }
        Some(_) => {
            map.remove(&key);
        }
        None => {}
    }
    Ok(())
}

/// Update `wave/<name>/GOAL.md` frontmatter, preserving existing body text.
pub fn update_wave_goal_config(
    repo: &Path,
    name: &str,
    update: impl FnOnce(&mut Mapping) -> Result<(), String>,
) -> Result<(), String> {
    let path = goal_path(repo, name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let (mut value, body) = goal_value_from_content(&path, name)?;
    let map = wave_config_map(&mut value, &path)?;
    update(map)?;

    let rendered = render_goal_md(&value, &body)?;
    std::fs::write(&path, rendered)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

/// Update agent fields in `wave/<name>/GOAL.md`, preserving existing frontmatter.
pub fn update_wave_agent_config(
    repo: &Path,
    name: &str,
    agent: Option<String>,
    skill_agents: Option<HashMap<String, String>>,
) -> Result<(), String> {
    update_wave_goal_config(repo, name, |map| {
        remove_or_set_string(map, "agent", agent);
        remove_or_set_skill_agents(map, skill_agents)
    })
}

/// Set authored Wave turn intent, preserving unrelated frontmatter and body.
///
/// Enabled turns are the default, so resuming removes `paused` rather than
/// persisting a redundant `paused: false` field.
pub fn update_wave_paused(repo: &Path, name: &str, paused: bool) -> Result<(), String> {
    update_wave_goal_config(repo, name, |map| {
        let key = Value::String("paused".to_string());
        if paused {
            map.insert(key, Value::Bool(true));
        } else {
            map.remove(&key);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_wave_config_parses_machine_frontmatter() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\nowner: jack\nhome: build.example.com\nagent: codex\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.owner.as_deref(), Some("jack"));
        assert_eq!(config.home.as_deref(), Some("build.example.com"));
        assert_eq!(config.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn read_wave_summary_prefers_the_objective() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
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
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
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
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\nchat:\n  provider: discord\n  home_id: home_11111111111111111111111111111111\n  guild_id: guild\n  channel_id: channel\n---\nDrive the work.\n",
        )
        .expect("write");
        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert!(matches!(
            config.chat,
            Some(WaveChatConfig::Discord { home_id, guild_id, channel_id })
                if home_id.as_str() == "home_11111111111111111111111111111111"
                    && guild_id == "guild"
                    && channel_id == "channel"
        ));
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Ok(Some(WaveChatConfig::Discord { home_id, guild_id, channel_id }))
                if home_id.as_str() == "home_11111111111111111111111111111111"
                    && guild_id == "guild"
                    && channel_id == "channel"
        ));

        fs::write(
            dir.join("GOAL.md"),
            "---\nchat:\n  provider: discord\n  home_id: home_39860354aaca640c2ccb50bf6ca609d8\n  guild_id: guild\n---\nDrive the work.\n",
        )
        .expect("write invalid");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        fs::write(
            dir.join("GOAL.md"),
            "---\nchat:\n  provider: local\n---\nDrive the work.\n",
        )
        .expect("write local chat");
        assert!(matches!(
            try_read_wave_config(temp.path(), "scan")
                .expect("local config")
                .and_then(|config| config.chat),
            Some(WaveChatConfig::Local)
        ));

        fs::write(
            dir.join("GOAL.md"),
            "---\nchat:\n  provider: discord\n  home_id: not-a-home\n  guild_id: guild\n  channel_id: channel\n---\nDrive the work.\n",
        )
        .expect("write invalid HomeId");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Err(WaveConfigError::Parse { .. })
        ));

        fs::write(
            dir.join("GOAL.md"),
            "---\npaused: not-a-boolean\n---\nDrive the work.\n",
        )
        .expect("write unrelated invalid policy");
        assert!(matches!(
            try_read_wave_chat_config(temp.path(), "scan"),
            Ok(None)
        ));
    }

    /// Crons live in GOAL.md frontmatter — the resident loop's schedule
    /// source. Legacy `triggers:` keys are simply unknown fields now.
    #[test]
    fn read_wave_config_parses_crons_and_ignores_legacy_triggers() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
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
        let temp = tempdir().expect("temp dir");
        assert!(read_wave_config(temp.path(), "nonexistent").is_none());
    }

    #[test]
    fn update_wave_agent_config_writes_agent_fields() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\narea: ['.']\n---\nDrive the work.\n",
        )
        .expect("write");

        update_wave_agent_config(
            temp.path(),
            "scan",
            Some("codex:o3".to_string()),
            Some(HashMap::from([(
                "implement".to_string(),
                "claude:sonnet".to_string(),
            )])),
        )
        .expect("update config");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.agent.as_deref(), Some("codex:o3"));
        assert_eq!(
            config.skill_agents,
            Some(HashMap::from([(
                "implement".to_string(),
                "claude:sonnet".to_string(),
            )]))
        );
    }

    #[test]
    fn update_wave_agent_config_removes_fields_on_empty_values() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\narea: ['.']\nagent: codex:o3\nskill_agents:\n  implement: claude:sonnet\n---\nDrive the work.\n",
        )
        .expect("write");

        update_wave_agent_config(
            temp.path(),
            "scan",
            Some(String::new()),
            Some(HashMap::new()),
        )
        .expect("update config");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert!(config.agent.is_none());
        assert!(config.skill_agents.is_none());
    }

    #[test]
    fn update_wave_paused_preserves_goal_and_removes_default() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("product");
        fs::create_dir_all(&dir).expect("create dir");
        let goal = dir.join("GOAL.md");
        let body = "\n## Objective\n\nShip the control room.\n";
        fs::write(
            &goal,
            format!("---\nowner: jack\nagent: codex\n---\n{body}"),
        )
        .expect("write");

        update_wave_paused(temp.path(), "product", true).expect("pause");
        let paused = fs::read_to_string(&goal).expect("read paused goal");
        assert!(paused.contains("owner: jack"));
        assert!(paused.contains("agent: codex"));
        assert!(paused.contains("paused: true"));
        assert!(paused.ends_with(body));

        update_wave_paused(temp.path(), "product", false).expect("resume");
        let resumed = fs::read_to_string(&goal).expect("read resumed goal");
        assert!(resumed.contains("owner: jack"));
        assert!(resumed.contains("agent: codex"));
        assert!(!resumed.contains("paused:"));
        assert!(resumed.ends_with(body));
    }
}
