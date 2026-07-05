use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

/// One cron line from GOAL.md frontmatter: `crons: [{flow, schedule}]`.
/// The wave's resident mind reads these and opens a system turn when a
/// schedule comes due (`crate::wave::mind`) — no daemon poller, no table.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct WaveCronDef {
    pub flow: String,
    pub schedule: String,
}

/// The Asana project a wave's roadmap lives in, from `pm.asana_project` in GOAL.md.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq)]
pub struct WavePmConfig {
    #[serde(default)]
    pub asana_project: Option<String>,
}

/// Intent read from `wave/<name>/GOAL.md` frontmatter during wave creation.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WaveConfig {
    pub flow: Option<String>,
    pub goal: Option<String>,
    pub mode: Option<String>,
    pub primary_flow: Option<String>,
    pub crons: Option<Vec<WaveCronDef>>,
    pub workers: Option<u32>,
    pub serialized: Option<bool>,
    pub area: Option<Vec<String>>,
    pub direction: Option<Vec<String>>,
    pub metrics: Option<Vec<String>>,
    pub agent: Option<String>,
    pub step_agents: Option<HashMap<String, String>>,
    pub pm: Option<WavePmConfig>,
    /// The wave's mind vendor (`codex` default; `claude`, `opencode`) — read
    /// by the RESIDENT (`crate::wave::resident::resolve_mind_vendor`).
    pub mind: Option<String>,
    /// The safety valve: `paused: true` in GOAL.md frontmatter tells the wave
    /// listener to refuse to START turns (message→turn, heartbeat, cron)
    /// while keeping the channel serving and queueing. File-first and re-read
    /// live (`crate::wave::runtime::WaveRuntime::paused`), not the registry
    /// row.
    pub paused: Option<bool>,
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
    let mut config = match split_frontmatter(&content) {
        Some((frontmatter, _)) => match serde_yaml_ng::from_str::<WaveConfig>(&frontmatter) {
            Ok(config) => config,
            Err(err) => {
                warn!(path = %path.display(), error = %err, "invalid wave goal frontmatter");
                return None;
            }
        },
        None => WaveConfig::default(),
    };
    config.goal = config.goal.or_else(|| Some(name.to_string()));
    config.flow = None;
    config.serialized = None;
    config.area = None;
    config.direction = None;
    Some(config)
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

fn remove_or_set_step_agents(
    map: &mut Mapping,
    step_agents: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let key = Value::String("step_agents".to_string());
    match step_agents {
        Some(step_agents) if !step_agents.is_empty() => {
            map.insert(
                key,
                serde_yaml_ng::to_value(step_agents)
                    .map_err(|err| format!("failed to encode step_agents: {err}"))?,
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
    step_agents: Option<HashMap<String, String>>,
) -> Result<(), String> {
    update_wave_goal_config(repo, name, |map| {
        remove_or_set_string(map, "agent", agent);
        remove_or_set_step_agents(map, step_agents)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_wave_config_parses_goal_frontmatter() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\nprimary_flow: build\nmode: manual\nworkers: 3\nmetrics:\n  - tests pass\n  - docs updated\narea: ['.']\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.goal.as_deref(), Some("scan"));
        assert_eq!(config.primary_flow.as_deref(), Some("build"));
        assert_eq!(config.mode.as_deref(), Some("manual"));
        assert_eq!(config.workers, Some(3));
        assert_eq!(
            config.metrics,
            Some(vec!["tests pass".to_string(), "docs updated".to_string()])
        );
        assert_eq!(config.area, None);
    }

    #[test]
    fn read_wave_config_parses_pm_block() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("GOAL.md"),
            "---\npm:\n  asana_project: \"1234567890\"\n---\nDrive the work.\n",
        )
        .expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        let pm = config.pm.expect("pm config should exist");
        assert_eq!(pm.asana_project.as_deref(), Some("1234567890"));
    }

    /// Crons live in GOAL.md frontmatter — the resident mind's schedule
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
            "---\nprimary_flow: build\narea: ['.']\n---\nDrive the work.\n",
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
            config.step_agents,
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
            "---\nprimary_flow: build\narea: ['.']\nagent: codex:o3\nstep_agents:\n  implement: claude:sonnet\n---\nDrive the work.\n",
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
        assert!(config.step_agents.is_none());
        assert_eq!(config.primary_flow.as_deref(), Some("build"));
        assert_eq!(config.area, None);
    }
}
