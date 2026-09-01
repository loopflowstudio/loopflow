//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo).
//! Repo config overrides global. Additive keys (docs, context, exclude, summaries) combine.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::error::LoadError;

/// Agent used when neither the caller, config, nor skill chooses one.
pub fn default_agent() -> &'static str {
    "codex"
}

/// Keys that combine lists from global + repo config.
const ADDITIVE_KEYS: &[&str] = &[
    "docs",
    "context",
    "exclude",
    "summaries",
    "supported_harnesses",
];

/// Summary configuration for a specific path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    pub path: String,
    #[serde(default)]
    pub tokens: Option<usize>,
    #[serde(default = "default_summary_agent")]
    pub agent: String,
}

fn default_summary_agent() -> String {
    default_agent().to_string()
}

/// Autoprune configuration.
#[derive(Debug, Clone, Serialize)]
pub struct AutopruneConfig {
    #[serde(default = "default_autoprune_enabled")]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
}

fn default_autoprune_enabled() -> bool {
    true
}

fn default_poll_interval() -> u64 {
    900
}

impl Default for AutopruneConfig {
    fn default() -> Self {
        Self {
            enabled: default_autoprune_enabled(),
            poll_interval_seconds: default_poll_interval(),
        }
    }
}

/// Intermediate representation for deserializing `autoprune: true` or `autoprune: { ... }`.
#[derive(Deserialize)]
#[serde(untagged)]
enum AutopruneRaw {
    Bool(bool),
    Config {
        #[serde(default = "default_autoprune_enabled")]
        enabled: bool,
        #[serde(default = "default_poll_interval")]
        poll_interval_seconds: u64,
    },
}

impl<'de> Deserialize<'de> for AutopruneConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match AutopruneRaw::deserialize(deserializer)? {
            AutopruneRaw::Bool(enabled) => Ok(Self {
                enabled,
                poll_interval_seconds: default_poll_interval(),
            }),
            AutopruneRaw::Config {
                enabled,
                poll_interval_seconds,
            } => Ok(Self {
                enabled,
                poll_interval_seconds,
            }),
        }
    }
}

/// Where interactive sessions launch.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LaunchTarget {
    #[default]
    Tui,
    Ide,
}

/// Interactive session launch configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    #[serde(default)]
    pub launch: LaunchTarget,
    /// Home-local terminal application used to present detached human sessions.
    #[serde(default)]
    pub terminal: Option<String>,
}

/// Release configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseConfig {
    #[serde(default)]
    pub targets: HashMap<String, ReleaseTargetConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReleaseTargetConfig {
    #[serde(default)]
    pub area: Vec<String>,
    #[serde(default)]
    pub tag_prefix: String,
    /// Manifest files to bump version in (auto-detected if omitted).
    #[serde(default)]
    pub manifests: Vec<String>,
    #[serde(default)]
    pub workflow: Option<String>,
    /// Commands that must pass before a release is prepared.
    #[serde(default)]
    pub verify: Vec<String>,
    /// Commands that mutate the isolated release worktree after version bumps.
    #[serde(default)]
    pub prepare: Vec<String>,
    #[serde(default)]
    pub completion: Option<ReleaseCompletion>,
    /// Repo-owned publisher command. Loopflow appends `check` or `publish` args.
    #[serde(default)]
    pub publisher: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseCompletion {
    Tag,
    Workflow,
    GithubRelease,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LinearConfig {
    #[serde(default)]
    pub team: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PmConfig {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub linear_team: Option<String>,
}

/// Main configuration struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Agent in format harness:model (e.g., claude:opus, codex)
    #[serde(default)]
    pub agent: Option<String>,

    /// Supported harnesses for model selection UI.
    #[serde(default)]
    pub supported_harnesses: Vec<String>,

    /// Skip permissions; Codex also disables sandboxing
    #[serde(default)]
    pub yolo: bool,

    /// Enable Chrome integration for Claude Code
    #[serde(default)]
    pub chrome: bool,

    /// Create PR after push
    #[serde(default)]
    pub pr: bool,

    /// Land strategy: "gh" or "local"
    #[serde(default = "default_land")]
    pub land: String,

    /// Additional context files to include
    #[serde(default)]
    pub context: Vec<String>,

    /// Glob patterns to exclude
    #[serde(default)]
    pub exclude: Vec<String>,

    /// Interactive session launch configuration
    #[serde(default)]
    pub session: SessionConfig,

    /// Docs paths, globs, or directories to include by default.
    #[serde(default)]
    pub docs: Vec<String>,

    /// Include raw branch diff
    #[serde(default)]
    pub diff: bool,

    /// Include full content of explicitly requested files.
    #[serde(default)]
    pub diff_files: bool,

    /// Include clipboard content by default
    #[serde(default)]
    pub paste: bool,

    /// Default directions for all tasks
    #[serde(default)]
    pub direction: Option<Vec<String>>,

    /// Summaries to include
    #[serde(default)]
    pub summaries: Vec<SummaryConfig>,

    /// Default token budget for summaries
    #[serde(default = "default_summary_tokens")]
    pub summary_tokens: usize,

    /// Autoprune configuration
    #[serde(default)]
    pub autoprune: AutopruneConfig,

    /// Release targets and scoping rules.
    #[serde(default)]
    pub release: ReleaseConfig,

    /// Linear configuration for PM integration spikes.
    #[serde(default)]
    pub linear: LinearConfig,

    /// PM provider selection.
    #[serde(default)]
    pub pm: Option<PmConfig>,
}

fn default_land() -> String {
    "gh".to_string()
}

fn default_summary_tokens() -> usize {
    5000
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agent: None,
            supported_harnesses: Vec::new(),
            yolo: false,
            chrome: false,
            pr: false,
            land: default_land(),
            context: Vec::new(),
            exclude: Vec::new(),
            session: SessionConfig::default(),
            docs: Vec::new(),
            diff: false,
            diff_files: false,
            paste: false,
            direction: None,
            summaries: Vec::new(),
            summary_tokens: default_summary_tokens(),
            autoprune: AutopruneConfig::default(),
            release: ReleaseConfig::default(),
            linear: LinearConfig::default(),
            pm: None,
        }
    }
}

impl Config {
    /// Return the configured agent or Loopflow's compiled default.
    pub fn agent(&self) -> &str {
        match self.agent.as_deref() {
            Some(agent) => agent,
            None => default_agent(),
        }
    }
}

/// Parse agent string like 'claude:opus' into (harness, model).
pub fn parse_agent(agent: &str) -> (String, Option<String>) {
    if let Some((harness, model)) = agent.split_once(':') {
        return (harness.to_string(), Some(model.to_string()));
    }

    (agent.to_string(), None)
}

/// Load YAML file, returning None if not present or empty.
fn load_yaml_file(path: &Path) -> Result<Option<serde_yaml_ng::Value>, LoadError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).map_err(|e| {
        LoadError::InvalidFlow(format!("YAML parse error in {}: {}", path.display(), e))
    })?;
    Ok(Some(value))
}

/// Merge global and repo config. Repo wins for scalars, additive keys combine.
fn merge_config_values(
    global: Option<serde_yaml_ng::Value>,
    repo: Option<serde_yaml_ng::Value>,
) -> serde_yaml_ng::Value {
    match (global, repo) {
        (None, None) => serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()),
        (Some(g), None) => g,
        (None, Some(r)) => r,
        (
            Some(serde_yaml_ng::Value::Mapping(mut global_map)),
            Some(serde_yaml_ng::Value::Mapping(repo_map)),
        ) => {
            for (key, value) in repo_map {
                let key_str = key.as_str().unwrap_or("");
                if ADDITIVE_KEYS.contains(&key_str) {
                    // Combine lists
                    if let Some(serde_yaml_ng::Value::Sequence(mut global_seq)) =
                        global_map.remove(&key)
                    {
                        if let serde_yaml_ng::Value::Sequence(repo_seq) = value {
                            global_seq.extend(repo_seq);
                            global_map.insert(key, serde_yaml_ng::Value::Sequence(global_seq));
                        } else {
                            global_map.insert(key, value);
                        }
                    } else {
                        global_map.insert(key, value);
                    }
                } else {
                    // Repo overrides
                    global_map.insert(key, value);
                }
            }
            serde_yaml_ng::Value::Mapping(global_map)
        }
        (_, Some(r)) => r,
    }
}

/// Load config, merging global (~/.lf/config.yaml) with repo (.lf/config.yaml).
pub fn load_config(repo_root: Option<&Path>) -> Result<Option<Config>, LoadError> {
    let global_path = global_config_path();

    let repo_path = repo_root.map(|r| r.join(".lf").join("config.yaml"));

    let global_data = load_yaml_file(&global_path)?;
    let repo_data = repo_path
        .as_ref()
        .map(|p| load_yaml_file(p))
        .transpose()?
        .flatten();

    if global_data.is_none() && repo_data.is_none() {
        return Ok(None);
    }

    let merged = merge_config_values(global_data, repo_data);

    let config: Config = serde_yaml_ng::from_value(merged)
        .map_err(|e| LoadError::InvalidFlow(format!("Config validation error: {}", e)))?;

    Ok(Some(config))
}

/// Load only Home-local user configuration.
pub fn load_global_config() -> Result<Option<Config>, LoadError> {
    let Some(value) = load_yaml_file(&global_config_path())? else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(value)
        .map(Some)
        .map_err(|error| LoadError::InvalidFlow(format!("Config validation error: {error}")))
}

fn global_config_path() -> PathBuf {
    if let Ok(home) = std::env::var("LF_HOME") {
        PathBuf::from(home).join("config.yaml")
    } else {
        dirs::home_dir()
            .map(|home| home.join(".lf/config.yaml"))
            .unwrap_or_else(|| PathBuf::from(".lf/config.yaml"))
    }
}

/// Load only the repository-owned config, without inheriting user-global values.
///
/// PM identity uses this path because a global Team binding must never become a
/// repository's authority implicitly.
pub fn load_repo_config(repo_root: &Path) -> Result<Option<Config>, LoadError> {
    let Some(value) = load_yaml_file(&repo_root.join(".lf/config.yaml"))? else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(value)
        .map(Some)
        .map_err(|error| LoadError::InvalidFlow(format!("Config validation error: {error}")))
}

/// Get config or default if no config files exist.
pub fn load_config_or_default(repo_root: Option<&Path>) -> Config {
    load_config(repo_root).ok().flatten().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // parse_agent tests
    // ==========================================================================

    #[test]
    fn parse_agent_with_model() {
        let (harness, model) = parse_agent("claude:opus");
        assert_eq!(harness, "claude");
        assert_eq!(model, Some("opus".to_string()));
    }

    #[test]
    fn parse_agent_with_complex_model() {
        let (harness, model) = parse_agent("opencode:moonshotai/kimi-k2");
        assert_eq!(harness, "opencode");
        assert_eq!(model, Some("moonshotai/kimi-k2".to_string()));
    }

    #[test]
    fn bare_harnesses_defer_model_selection_to_the_provider() {
        for harness in ["claude", "codex", "opencode"] {
            assert_eq!(parse_agent(harness), (harness.to_string(), None));
        }
    }

    #[test]
    fn parse_agent_opencode_with_model() {
        let (harness, model) = parse_agent("opencode:anthropic/claude-sonnet");
        assert_eq!(harness, "opencode");
        assert_eq!(model, Some("anthropic/claude-sonnet".to_string()));
    }

    #[test]
    fn parse_agent_unknown_harness() {
        let (harness, model) = parse_agent("unknown");
        assert_eq!(harness, "unknown");
        assert_eq!(model, None);
    }

    #[test]
    fn parse_agent_unknown_with_model() {
        let (harness, model) = parse_agent("custom:model-v2");
        assert_eq!(harness, "custom");
        assert_eq!(model, Some("model-v2".to_string()));
    }

    #[test]
    fn config_parses_linear_settings() {
        let yaml = r#"
linear:
  team: "9876543210"
"#;

        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(
            config.linear,
            LinearConfig {
                team: Some("9876543210".to_string()),
            }
        );
    }

    // ==========================================================================
    // Default config tests
    // ==========================================================================

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert!(config.agent.is_none());
        assert_eq!(config.agent(), "codex");
        assert!(config.supported_harnesses.is_empty());
        assert!(!config.yolo);
        assert!(config.docs.is_empty());
        assert!(!config.diff_files);
        assert!(!config.diff);
        assert!(!config.chrome);
        assert!(!config.pr);
        assert_eq!(config.land, "gh");
        assert!(config.context.is_empty());
        assert!(config.exclude.is_empty());
        assert_eq!(config.session.launch, LaunchTarget::Tui);
        assert!(config.direction.is_none());
        assert!(config.release.targets.is_empty());
    }

    #[test]
    fn default_session_config() {
        let session = SessionConfig::default();
        assert_eq!(session.launch, LaunchTarget::Tui);
    }

    #[test]
    fn default_autoprune_config() {
        let autoprune = AutopruneConfig::default();
        assert!(autoprune.enabled);
        assert_eq!(autoprune.poll_interval_seconds, 900);
    }

    #[test]
    fn autoprune_config_from_empty_yaml() {
        // When deserialized from YAML, gets proper defaults
        let yaml = "autoprune: {}\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 900);
    }

    // ==========================================================================
    // YAML parsing tests
    // ==========================================================================

    #[test]
    fn config_from_yaml_basic() {
        let yaml = r#"
agent: codex:o3
yolo: true
chrome: true
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.agent.as_deref(), Some("codex:o3"));
        assert_eq!(config.agent(), "codex:o3");
        assert!(config.yolo);
        assert!(config.chrome);
    }

    #[test]
    fn config_from_yaml_supported_harnesses() {
        let yaml = r#"
supported_harnesses:
  - claude
  - codex
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.supported_harnesses, vec!["claude", "codex"]);
    }

    #[test]
    fn config_from_yaml_context_as_list() {
        let yaml = r#"
context:
  - src/
  - tests/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.context, vec!["src/", "tests/"]);
    }

    #[test]
    fn config_from_yaml_docs_as_list() {
        let yaml = r#"
docs:
  - README.md
  - docs/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.docs, vec!["README.md", "docs/"]);
    }

    #[test]
    fn config_from_yaml_exclude_as_list() {
        let yaml = r#"
exclude:
  - "*.log"
  - build/
  - node_modules/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.exclude, vec!["*.log", "build/", "node_modules/"]);
    }

    #[test]
    fn config_from_yaml_direction_as_list() {
        let yaml = r#"
direction:
  - architect
  - concise
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(
            config.direction,
            Some(vec!["architect".to_string(), "concise".to_string()])
        );
    }

    #[test]
    fn config_from_yaml_session_launch_tui() {
        let yaml = r#"
session:
  launch: tui
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.session.launch, LaunchTarget::Tui);
    }

    #[test]
    fn config_from_yaml_session_launch_ide() {
        let yaml = r#"
session:
  launch: ide
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.session.launch, LaunchTarget::Ide);
    }

    #[test]
    fn config_from_yaml_session_terminal() {
        let yaml = r#"
session:
  terminal: Ghostty
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.session.terminal.as_deref(), Some("Ghostty"));
    }

    #[test]
    fn config_from_yaml_autoprune_bool_true() {
        let yaml = "autoprune: true\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 900);
    }

    #[test]
    fn config_from_yaml_autoprune_bool_false() {
        let yaml = "autoprune: false\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert!(!config.autoprune.enabled);
    }

    #[test]
    fn config_from_yaml_autoprune_object() {
        let yaml = r#"
autoprune:
  enabled: true
  poll_interval_seconds: 120
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 120);
    }

    #[test]
    fn config_from_yaml_autoprune_object_partial() {
        let yaml = r#"
autoprune:
  enabled: true
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 900);
    }

    #[test]
    fn config_from_yaml_summaries() {
        let yaml = r#"
summaries:
  - path: src/
    tokens: 5000
    agent: claude
  - path: tests/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.summaries.len(), 2);
        assert_eq!(config.summaries[0].path, "src/");
        assert_eq!(config.summaries[0].tokens, Some(5000));
        assert_eq!(config.summaries[0].agent, "claude");
        assert_eq!(config.summaries[1].path, "tests/");
        assert_eq!(config.summaries[1].agent, default_agent());
    }

    #[test]
    fn config_from_yaml_release_targets() {
        let yaml = r#"
release:
  targets:
    cli:
      area:
        - packages/cli/
      tag_prefix: cli/
      manifests:
        - packages/cli/package.json
      workflow: .github/workflows/release-cli.yml
      verify:
        - scripts/check-release
      prepare:
        - scripts/prepare-release {version}
      completion: github-release
      publisher:
        - doppler
        - run
        - --
        - python
        - scripts/publish.py
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        let target = config.release.targets.get("cli").expect("cli target");
        assert_eq!(target.area, vec!["packages/cli/"]);
        assert_eq!(target.tag_prefix, "cli/");
        assert_eq!(target.manifests, vec!["packages/cli/package.json"]);
        assert_eq!(
            target.workflow.as_deref(),
            Some(".github/workflows/release-cli.yml")
        );
        assert_eq!(target.verify, vec!["scripts/check-release"]);
        assert_eq!(target.prepare, vec!["scripts/prepare-release {version}"]);
        assert_eq!(target.completion, Some(ReleaseCompletion::GithubRelease));
        assert_eq!(
            target.publisher,
            vec!["doppler", "run", "--", "python", "scripts/publish.py"]
        );
    }

    // ==========================================================================
    // Invalid YAML tests
    // ==========================================================================

    #[test]
    fn config_from_yaml_invalid_type() {
        let yaml = "summary_tokens: not_a_number\n";
        let result: Result<Config, _> = serde_yaml_ng::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn config_from_yaml_invalid_syntax() {
        let yaml = "foo: [invalid\n";
        let result: Result<Config, _> = serde_yaml_ng::from_str(yaml);
        assert!(result.is_err());
    }

    // ==========================================================================
    // Config merge tests
    // ==========================================================================

    #[test]
    fn merge_config_values_repo_overrides_scalar() {
        let global: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
agent: claude:opus
yolo: false
"#,
        )
        .unwrap();

        let repo: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
agent: codex
"#,
        )
        .unwrap();

        let merged = merge_config_values(Some(global), Some(repo));
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();

        assert_eq!(config.agent.as_deref(), Some("codex"));
        assert!(!config.yolo); // preserved from global
    }

    #[test]
    fn merge_config_values_additive_keys_combine() {
        let global: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
context:
  - global.md
docs:
  - README.md
exclude:
  - "*.log"
"#,
        )
        .unwrap();

        let repo: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
context:
  - local.md
docs:
  - docs/
exclude:
  - build/
"#,
        )
        .unwrap();

        let merged = merge_config_values(Some(global), Some(repo));
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();

        assert_eq!(config.context, vec!["global.md", "local.md"]);
        assert_eq!(config.docs, vec!["README.md", "docs/"]);
        assert_eq!(config.exclude, vec!["*.log", "build/"]);
    }

    #[test]
    fn merge_config_values_supported_harnesses_combine() {
        let global: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
supported_harnesses:
  - claude
"#,
        )
        .unwrap();

        let repo: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
supported_harnesses:
  - codex
"#,
        )
        .unwrap();

        let merged = merge_config_values(Some(global), Some(repo));
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();
        assert_eq!(config.supported_harnesses, vec!["claude", "codex"]);
    }

    #[test]
    fn merge_config_values_global_only() {
        let global: serde_yaml_ng::Value = serde_yaml_ng::from_str("agent: claude:opus\n").unwrap();

        let merged = merge_config_values(Some(global), None);
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();

        assert_eq!(config.agent.as_deref(), Some("claude:opus"));
    }

    #[test]
    fn merge_config_values_repo_only() {
        let repo: serde_yaml_ng::Value = serde_yaml_ng::from_str("agent: codex\n").unwrap();

        let merged = merge_config_values(None, Some(repo));
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();

        assert_eq!(config.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn merge_config_values_both_none() {
        let merged = merge_config_values(None, None);
        let config: Config = serde_yaml_ng::from_value(merged).unwrap();

        // Should get defaults
        assert!(config.agent.is_none());
    }

    // ==========================================================================
    // File-based config loading tests
    // Note: These tests may be affected by the user's global ~/.lf/config.yaml
    // ==========================================================================

    #[test]
    fn load_yaml_file_empty_returns_none() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&config_path, "").expect("write empty config");

        let result = load_yaml_file(&config_path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn load_yaml_file_whitespace_only_returns_none() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&config_path, "   \n\n  ").expect("write whitespace");

        let result = load_yaml_file(&config_path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn load_yaml_file_missing_returns_none() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_path = temp.path().join("nonexistent.yaml");

        let result = load_yaml_file(&config_path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn load_yaml_file_valid_content() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_path = temp.path().join("config.yaml");
        std::fs::write(&config_path, "agent: codex\n").expect("write config");

        let result = load_yaml_file(&config_path);
        assert!(result.is_ok());
        let value = result.unwrap().expect("should have value");
        assert!(value.is_mapping());
    }

    #[test]
    fn load_config_basic() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_dir = temp.path().join(".lf");
        std::fs::create_dir_all(&config_dir).expect("create .lf dir");
        std::fs::write(config_dir.join("config.yaml"), "agent: codex\nyolo: true\n")
            .expect("write config");

        let result = load_config(Some(temp.path()));
        assert!(result.is_ok());
        let config = result.unwrap().expect("config should exist");
        assert_eq!(config.agent.as_deref(), Some("codex"));
        assert!(config.yolo);
    }

    #[test]
    fn load_config_or_default_exists() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_dir = temp.path().join(".lf");
        std::fs::create_dir_all(&config_dir).expect("create .lf dir");
        std::fs::write(config_dir.join("config.yaml"), "agent: codex\n").expect("write config");

        let config = load_config_or_default(Some(temp.path()));
        assert_eq!(config.agent.as_deref(), Some("codex"));
    }

    #[test]
    fn load_repo_config_reads_repository_pm_authority() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_dir = temp.path().join(".lf");
        std::fs::create_dir_all(&config_dir).expect("create .lf dir");
        std::fs::write(
            config_dir.join("config.yaml"),
            "pm:\n  provider: linear\n  linear_team: team-loo\n",
        )
        .expect("write config");

        let config = load_repo_config(temp.path()).unwrap().unwrap();
        let pm = config.pm.unwrap();
        assert_eq!(pm.provider.as_deref(), Some("linear"));
        assert_eq!(pm.linear_team.as_deref(), Some("team-loo"));
    }
}
