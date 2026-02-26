use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct StimulusDef {
    pub kind: String,
    pub cron: Option<String>,
}

/// Config read from `wave/<name>/<name>.yaml` during wave creation.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub(crate) struct WaveConfig {
    pub flow: Option<String>,
    pub area: Option<Vec<String>>,
    pub stimulus: Option<StimulusDef>,
    pub direction: Option<Vec<String>>,
    pub agent: Option<String>,
    pub step_agents: Option<HashMap<String, String>>,
}

/// Read wave config from `wave/<name>/<name>.yaml`.
pub(crate) fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig> {
    let path = repo.join("wave").join(name).join(format!("{name}.yaml"));
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return None,
        Err(err) => {
            warn!(path = %path.display(), error = %err, "failed to read wave config");
            return None;
        }
    };
    match serde_yaml_ng::from_str(&content) {
        Ok(config) => Some(config),
        Err(err) => {
            warn!(path = %path.display(), error = %err, "invalid wave config");
            None
        }
    }
}

/// Update agent fields in `wave/<name>/<name>.yaml`, preserving existing keys.
pub(crate) fn update_wave_agent_config(
    repo: &Path,
    name: &str,
    agent: Option<String>,
    step_agents: Option<HashMap<String, String>>,
) -> Result<(), String> {
    let path = repo.join("wave").join(name).join(format!("{name}.yaml"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let mut value = if path.exists() {
        let content = std::fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content)
            .map_err(|err| format!("invalid yaml in {}: {err}", path.display()))?
    } else {
        serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new())
    };

    let map = value
        .as_mapping_mut()
        .ok_or_else(|| format!("wave config at {} must be a mapping", path.display()))?;
    let agent_key = serde_yaml_ng::Value::String("agent".to_string());
    let step_agents_key = serde_yaml_ng::Value::String("step_agents".to_string());

    if let Some(agent) = agent {
        if agent.trim().is_empty() {
            map.remove(&agent_key);
        } else {
            map.insert(agent_key, serde_yaml_ng::Value::String(agent));
        }
    }

    if let Some(step_agents) = step_agents {
        if step_agents.is_empty() {
            map.remove(&step_agents_key);
        } else {
            map.insert(
                step_agents_key,
                serde_yaml_ng::to_value(step_agents)
                    .map_err(|err| format!("failed to encode step_agents: {err}"))?,
            );
        }
    }

    let rendered = serde_yaml_ng::to_string(&value)
        .map_err(|err| format!("failed to render {}: {err}", path.display()))?;
    std::fs::write(&path, rendered)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn read_wave_config_parses_yaml() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(dir.join("scan.yaml"), "flow: build\narea: ['.']\n").expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.flow.as_deref(), Some("build"));
        assert_eq!(config.area, Some(vec![".".to_string()]));
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
        fs::write(dir.join("scan.yaml"), "flow: build\narea: ['.']\n").expect("write");

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
            config
                .step_agents
                .as_ref()
                .and_then(|agents| agents.get("implement"))
                .map(String::as_str),
            Some("claude:sonnet")
        );
    }

    #[test]
    fn update_wave_agent_config_removes_fields_on_empty_values() {
        let temp = tempdir().expect("temp dir");
        let dir = temp.path().join("wave").join("scan");
        fs::create_dir_all(&dir).expect("create dir");
        fs::write(
            dir.join("scan.yaml"),
            "flow: build\narea: ['.']\nagent: codex:o3\nstep_agents:\n  implement: claude:sonnet\n",
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
        assert_eq!(config.flow.as_deref(), Some("build"));
        assert_eq!(config.area, Some(vec![".".to_string()]));
    }
}
