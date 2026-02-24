use serde::Deserialize;
use std::path::Path;
use tracing::warn;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct StimulusDef {
    pub kind: String,
    pub cron: Option<String>,
}

/// Config read from `wave/<name>/<name>.yaml` during wave creation.
#[derive(Debug, Deserialize)]
pub(crate) struct WaveConfig {
    pub flow: String,
    pub area: Vec<String>,
    pub stimulus: Option<StimulusDef>,
    pub direction: Option<Vec<String>>,
    #[allow(dead_code)] // Parsed from YAML for future use.
    pub owner: Option<String>,
    #[allow(dead_code)] // Parsed from YAML for future use.
    pub description: Option<String>,
}

/// Read wave config from `wave/<name>/<name>.yaml`.
pub(crate) fn read_wave_config(repo: &Path, name: &str) -> Option<WaveConfig> {
    let path = repo.join("wave").join(name).join(format!("{name}.yaml"));
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
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
        fs::write(dir.join("scan.yaml"), "flow: ship\narea: ['.']\n").expect("write");

        let config = read_wave_config(temp.path(), "scan").expect("config should parse");
        assert_eq!(config.flow, "ship");
        assert_eq!(config.area, vec!["."]);
    }

    #[test]
    fn read_wave_config_returns_none_for_missing() {
        let temp = tempdir().expect("temp dir");
        assert!(read_wave_config(temp.path(), "nonexistent").is_none());
    }
}
