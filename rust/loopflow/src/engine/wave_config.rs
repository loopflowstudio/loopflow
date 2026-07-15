use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

use crate::engine::wave_home::WaveHome;

/// One cron line from GOAL.md frontmatter: `crons: [{flow, schedule}]`.
/// The wave's resident loop reads these and opens a system pass when a
/// schedule comes due (`crate::flowloop::wave`) — no daemon poller, no table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WaveCronDef {
    pub flow: String,
    pub schedule: String,
}

/// The Linear Initiative representing a wave, from `pm.*` in GOAL.md.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct WavePmConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub linear_initiative: Option<String>,
}

/// Machine policy read from `wave/<name>/GOAL.md` frontmatter.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WaveConfig {
    pub crons: Option<Vec<WaveCronDef>>,
    pub agent: Option<String>,
    pub skill_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    /// The safety valve: `paused: true` in GOAL.md frontmatter tells the wave
    /// listener to refuse to START turns (message→turn, heartbeat, cron)
    /// while keeping the channel serving and queueing. File-first and re-read
    /// live (`crate::wave::runtime::WaveRuntime::paused`), not the registry
    /// row.
    pub paused: Option<bool>,
    /// The wave's execution Home: a user-owned address such as `jack@local`
    /// (the default form) or `ssh://jack@host[:port]`. Authored here, parsed via
    /// [`WaveHome`], and used to resolve/probe/start and to route top-level `lf`
    /// commands. Absent means the current user's local Home.
    #[serde(default)]
    pub home: Option<String>,
}

impl WaveConfig {
    /// The authored Home, when present and parseable. `None` means the field is
    /// absent or malformed; the caller supplies the default owner (see
    /// [`read_wave_home`]).
    pub fn home_authored(&self) -> Option<WaveHome> {
        self.home.as_deref().and_then(WaveHome::parse)
    }
}

/// Read the wave's execution Home straight from `GOAL.md`. When no Home is
/// authored (or it is malformed), default to the current user's local Home
/// (`<git-user>@local`). The single read site for resolve/probe/start, launch
/// inheritance, and routing — keyed by the wave name (identity), never a string
/// parsed from a branch or path.
pub fn read_wave_home(repo: &Path, name: &str) -> WaveHome {
    read_wave_config(repo, name)
        .and_then(|config| config.home_authored())
        .unwrap_or_else(|| default_local_home(repo))
}

/// The current user's local Home — the default when no Home is authored. Owner
/// is the git user (falling back to `$USER`, then `user`); it names who the Home
/// belongs to, not a credential.
pub fn default_local_home(repo: &Path) -> WaveHome {
    let owner = crate::engine::naming::git_user(repo).unwrap_or_else(|_| "user".to_string());
    WaveHome::local(owner).unwrap_or_else(|| {
        WaveHome::local("user").expect("literal owner 'user' is always a valid Home owner")
    })
}

/// Read wave intent from `wave/<name>/GOAL.md` frontmatter.
pub fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig> {
    let path = goal_path(repo, name);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to read wave config");
            return None;
        }
    };
    Some(match split_frontmatter(&content) {
        Some((frontmatter, _)) => match serde_yaml_ng::from_str::<WaveConfig>(&frontmatter) {
            Ok(config) => config,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "invalid wave goal frontmatter");
                return None;
            }
        },
        None => WaveConfig::default(),
    })
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
            "---\nagent: codex\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
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
            "---\npm:\n  provider: linear\n  linear_initiative: \"lin-123\"\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        let pm = config.pm.expect("pm config should exist");
        assert_eq!(pm.provider.as_deref(), Some("linear"));
        assert_eq!(pm.linear_initiative.as_deref(), Some("lin-123"));
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
}
