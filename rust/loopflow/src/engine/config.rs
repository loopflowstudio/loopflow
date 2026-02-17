//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo).
//! Repo config overrides global. Additive keys (context, exclude, summaries) combine.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::error::LoadError;

/// Keys that combine lists from global + repo config.
const ADDITIVE_KEYS: &[&str] = &["context", "exclude", "skill_sources", "summaries"];

/// Token budgets for prompt sections.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetConfig {
    #[serde(default = "default_area_budget")]
    pub area: usize,
    #[serde(default = "default_docs_budget")]
    pub docs: usize,
    #[serde(default = "default_diff_budget")]
    pub diff: usize,
}

fn default_area_budget() -> usize {
    50000
}
fn default_docs_budget() -> usize {
    30000
}
fn default_diff_budget() -> usize {
    20000
}

/// Summary configuration for a specific path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    pub path: String,
    #[serde(default)]
    pub tokens: Option<usize>,
    #[serde(default = "default_summary_model")]
    pub model: String,
}

fn default_summary_model() -> String {
    "gemini".to_string()
}

/// External skill library configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSourceConfig {
    pub name: String,
    pub prefix: String,
    pub path: String,
}

/// Branch naming configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchNameConfig {
    #[serde(default = "default_branch_schema", alias = "schema")]
    pub schema_: String,
}

fn default_branch_schema() -> String {
    "{user}.{name}.{timestamp}.{words}".to_string()
}

impl Default for BranchNameConfig {
    fn default() -> Self {
        Self {
            schema_: default_branch_schema(),
        }
    }
}

/// Autoprune configuration.
#[derive(Debug, Clone, Serialize, Default)]
pub struct AutopruneConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
}

fn default_poll_interval() -> u64 {
    60
}

/// Intermediate representation for deserializing `autoprune: true` or `autoprune: { ... }`.
#[derive(Deserialize)]
#[serde(untagged)]
enum AutopruneRaw {
    Bool(bool),
    Config {
        #[serde(default)]
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

/// IDE configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeConfig {
    #[serde(default = "default_true")]
    pub warp: bool,
    #[serde(default = "default_true")]
    pub cursor: bool,
    #[serde(default)]
    pub workspace: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for IdeConfig {
    fn default() -> Self {
        Self {
            warp: true,
            cursor: true,
            workspace: None,
        }
    }
}

/// Main configuration struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Model in format backend:variant (e.g., claude:opus, codex)
    #[serde(default = "default_agent_model")]
    pub agent_model: String,

    /// Skip permissions; Codex also disables sandboxing
    #[serde(default)]
    pub yolo: bool,

    /// Enable Chrome integration for Claude Code
    #[serde(default)]
    pub chrome: bool,

    /// Auto-push after commits
    #[serde(default)]
    pub push: bool,

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

    /// IDE configuration
    #[serde(default)]
    pub ide: IdeConfig,

    /// Tasks that default to interactive mode
    #[serde(default)]
    pub interactive: Vec<String>,

    /// Include bundled LOOPFLOW.md
    #[serde(default = "default_true")]
    pub include_loopflow_doc: bool,

    /// Include reports/, wave/, scratch/, and root .md files
    #[serde(default = "default_true")]
    pub lfdocs: bool,

    /// Include raw branch diff
    #[serde(default)]
    pub diff: bool,

    /// Include full content of files touched by branch
    #[serde(default = "default_true")]
    pub diff_files: bool,

    /// Include clipboard content by default
    #[serde(default)]
    pub paste: bool,

    /// Default directions for all tasks
    #[serde(default)]
    pub direction: Option<Vec<String>>,

    /// Default area for parent doc inclusion
    #[serde(default)]
    pub area: Option<String>,

    /// Summaries to include
    #[serde(default)]
    pub summaries: Vec<SummaryConfig>,

    /// Default token budget for summaries
    #[serde(default = "default_summary_tokens")]
    pub summary_tokens: usize,

    /// External skill libraries
    #[serde(default)]
    pub skill_sources: Vec<SkillSourceConfig>,

    /// Branch naming schema
    #[serde(default)]
    pub branch_names: Option<BranchNameConfig>,

    /// Command to check if lint passes (e.g. "cargo fmt --check && cargo clippy -- -D warnings")
    #[serde(default)]
    pub lint: Option<String>,

    /// Command to run tests (e.g. "cargo test --all")
    #[serde(default)]
    pub test: Option<String>,

    /// Autoprune configuration
    #[serde(default)]
    pub autoprune: AutopruneConfig,

    /// Token budgets
    #[serde(default)]
    pub budgets: BudgetConfig,

    /// Model for RLM sub-agents (default: same as agent_model)
    #[serde(default)]
    pub rlm_model: Option<String>,

    /// Max concurrent RLM sub-agents
    #[serde(default = "default_rlm_max_parallel")]
    pub rlm_max_parallel: usize,

    /// Max RLM recursion depth
    #[serde(default = "default_rlm_max_depth")]
    pub rlm_max_depth: usize,
}

fn default_agent_model() -> String {
    "claude:opus".to_string()
}

fn default_land() -> String {
    "gh".to_string()
}

fn default_summary_tokens() -> usize {
    5000
}

fn default_rlm_max_parallel() -> usize {
    10
}

fn default_rlm_max_depth() -> usize {
    3
}

impl Default for Config {
    fn default() -> Self {
        Self {
            agent_model: default_agent_model(),
            yolo: false,
            chrome: false,
            push: false,
            pr: false,
            land: default_land(),
            context: Vec::new(),
            exclude: Vec::new(),
            ide: IdeConfig::default(),
            interactive: Vec::new(),
            include_loopflow_doc: true,
            lfdocs: true,
            diff: false,
            diff_files: true,
            paste: false,
            direction: None,
            area: None,
            summaries: Vec::new(),
            summary_tokens: default_summary_tokens(),
            skill_sources: Vec::new(),
            branch_names: None,
            lint: None,
            test: None,
            autoprune: AutopruneConfig::default(),
            budgets: BudgetConfig::default(),
            rlm_model: None,
            rlm_max_parallel: default_rlm_max_parallel(),
            rlm_max_depth: default_rlm_max_depth(),
        }
    }
}

/// Parse model string like 'claude:opus' into (backend, variant).
///
/// Applies smart defaults when no variant is specified:
/// - claude -> opus (Claude Opus 4.5)
/// - gemini -> 2.5-pro (Gemini 2.5 Pro)
/// - codex -> None (let Codex CLI pick its default)
/// - opencode -> None (let OpenCode use its own config)
pub fn parse_model(model: &str) -> (String, Option<String>) {
    let parts: Vec<&str> = model.splitn(2, ':').collect();
    let backend = parts[0].to_string();
    let variant = if parts.len() > 1 {
        Some(parts[1].to_string())
    } else {
        match backend.as_str() {
            "claude" => Some("opus".to_string()),
            "gemini" => Some("2.5-pro".to_string()),
            _ => None,
        }
    };
    (backend, variant)
}

/// Load YAML file, returning None if not present or empty.
fn load_yaml_file(path: &Path) -> Result<Option<serde_yaml::Value>, LoadError> {
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(path)?;
    if content.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_yaml::Value = serde_yaml::from_str(&content).map_err(|e| {
        LoadError::InvalidFlow(format!("YAML parse error in {}: {}", path.display(), e))
    })?;
    Ok(Some(value))
}

/// Merge global and repo config. Repo wins for scalars, additive keys combine.
fn merge_config_values(
    global: Option<serde_yaml::Value>,
    repo: Option<serde_yaml::Value>,
) -> serde_yaml::Value {
    match (global, repo) {
        (None, None) => serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
        (Some(g), None) => g,
        (None, Some(r)) => r,
        (
            Some(serde_yaml::Value::Mapping(mut global_map)),
            Some(serde_yaml::Value::Mapping(repo_map)),
        ) => {
            for (key, value) in repo_map {
                let key_str = key.as_str().unwrap_or("");
                if ADDITIVE_KEYS.contains(&key_str) {
                    // Combine lists
                    if let Some(serde_yaml::Value::Sequence(mut global_seq)) =
                        global_map.remove(&key)
                    {
                        if let serde_yaml::Value::Sequence(repo_seq) = value {
                            global_seq.extend(repo_seq);
                            global_map.insert(key, serde_yaml::Value::Sequence(global_seq));
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
            serde_yaml::Value::Mapping(global_map)
        }
        (_, Some(r)) => r,
    }
}

/// Load config, merging global (~/.lf/config.yaml) with repo (.lf/config.yaml).
pub fn load_config(repo_root: Option<&Path>) -> Result<Option<Config>, LoadError> {
    let global_path = dirs::home_dir()
        .map(|h| h.join(".lf").join("config.yaml"))
        .unwrap_or_else(|| PathBuf::from(".lf/config.yaml"));

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

    let config: Config = serde_yaml::from_value(merged)
        .map_err(|e| LoadError::InvalidFlow(format!("Config validation error: {}", e)))?;

    Ok(Some(config))
}

/// Get config or default if no config files exist.
pub fn load_config_or_default(repo_root: Option<&Path>) -> Config {
    load_config(repo_root).ok().flatten().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // parse_model tests
    // ==========================================================================

    #[test]
    fn parse_model_with_variant() {
        let (backend, variant) = parse_model("claude:opus");
        assert_eq!(backend, "claude");
        assert_eq!(variant, Some("opus".to_string()));
    }

    #[test]
    fn parse_model_with_complex_variant() {
        let (backend, variant) = parse_model("gemini:2.5-pro");
        assert_eq!(backend, "gemini");
        assert_eq!(variant, Some("2.5-pro".to_string()));
    }

    #[test]
    fn parse_model_claude_default() {
        let (backend, variant) = parse_model("claude");
        assert_eq!(backend, "claude");
        assert_eq!(variant, Some("opus".to_string()));
    }

    #[test]
    fn parse_model_codex_no_default() {
        let (backend, variant) = parse_model("codex");
        assert_eq!(backend, "codex");
        assert_eq!(variant, None);
    }

    #[test]
    fn parse_model_gemini_default() {
        let (backend, variant) = parse_model("gemini");
        assert_eq!(backend, "gemini");
        assert_eq!(variant, Some("2.5-pro".to_string()));
    }

    #[test]
    fn parse_model_opencode_no_default() {
        let (backend, variant) = parse_model("opencode");
        assert_eq!(backend, "opencode");
        assert_eq!(variant, None);
    }

    #[test]
    fn parse_model_opencode_with_variant() {
        let (backend, variant) = parse_model("opencode:anthropic/claude-sonnet");
        assert_eq!(backend, "opencode");
        assert_eq!(variant, Some("anthropic/claude-sonnet".to_string()));
    }

    #[test]
    fn parse_model_unknown_backend() {
        let (backend, variant) = parse_model("unknown");
        assert_eq!(backend, "unknown");
        assert_eq!(variant, None);
    }

    #[test]
    fn parse_model_unknown_with_variant() {
        let (backend, variant) = parse_model("custom:model-v2");
        assert_eq!(backend, "custom");
        assert_eq!(variant, Some("model-v2".to_string()));
    }

    // ==========================================================================
    // Default config tests
    // ==========================================================================

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.agent_model, "claude:opus");
        assert!(!config.yolo);
        assert!(config.lfdocs);
        assert!(config.diff_files);
        assert!(!config.diff);
        assert!(!config.chrome);
        assert!(!config.push);
        assert!(!config.pr);
        assert_eq!(config.land, "gh");
        assert!(config.context.is_empty());
        assert!(config.exclude.is_empty());
        assert!(config.interactive.is_empty());
        assert!(config.direction.is_none());
        assert!(config.area.is_none());
    }

    #[test]
    fn default_ide_config() {
        let ide = IdeConfig::default();
        assert!(ide.warp);
        assert!(ide.cursor);
        assert!(ide.workspace.is_none());
    }

    #[test]
    fn default_autoprune_config() {
        // Note: Default trait gives zeros, serde deserialize gives proper defaults
        let autoprune = AutopruneConfig::default();
        assert!(!autoprune.enabled);
        // Default trait uses 0, serde uses default_poll_interval (60)
        assert_eq!(autoprune.poll_interval_seconds, 0);
    }

    #[test]
    fn default_budget_config() {
        // Note: Default trait gives zeros, serde deserialize gives proper defaults
        let budgets = BudgetConfig::default();
        // Default trait uses 0, serde uses default functions
        assert_eq!(budgets.area, 0);
        assert_eq!(budgets.docs, 0);
        assert_eq!(budgets.diff, 0);
    }

    #[test]
    fn autoprune_config_from_empty_yaml() {
        // When deserialized from YAML, gets proper defaults
        let yaml = "autoprune: {}\n";
        let config: Config = serde_yaml::from_str(yaml).expect("parse");
        assert!(!config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 60);
    }

    #[test]
    fn budget_config_from_empty_yaml() {
        // When deserialized from YAML, gets proper defaults
        let yaml = "budgets: {}\n";
        let config: Config = serde_yaml::from_str(yaml).expect("parse");
        assert_eq!(config.budgets.area, 50000);
        assert_eq!(config.budgets.docs, 30000);
        assert_eq!(config.budgets.diff, 20000);
    }

    // ==========================================================================
    // YAML parsing tests
    // ==========================================================================

    #[test]
    fn config_from_yaml_basic() {
        let yaml = r#"
agent_model: codex:o3
yolo: true
chrome: true
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.agent_model, "codex:o3");
        assert!(config.yolo);
        assert!(config.chrome);
    }

    #[test]
    fn config_from_yaml_context_as_list() {
        let yaml = r#"
context:
  - src/
  - tests/
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.context, vec!["src/", "tests/"]);
    }

    #[test]
    fn config_from_yaml_exclude_as_list() {
        let yaml = r#"
exclude:
  - "*.log"
  - build/
  - node_modules/
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.exclude, vec!["*.log", "build/", "node_modules/"]);
    }

    #[test]
    fn config_from_yaml_direction_as_list() {
        let yaml = r#"
direction:
  - architect
  - concise
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(
            config.direction,
            Some(vec!["architect".to_string(), "concise".to_string()])
        );
    }

    #[test]
    fn config_from_yaml_interactive_list() {
        let yaml = r#"
interactive:
  - design
  - iterate
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.interactive, vec!["design", "iterate"]);
    }

    #[test]
    fn config_from_yaml_ide_settings() {
        let yaml = r#"
ide:
  warp: false
  cursor: true
  workspace: myproject.code-workspace
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(!config.ide.warp);
        assert!(config.ide.cursor);
        assert_eq!(
            config.ide.workspace,
            Some("myproject.code-workspace".to_string())
        );
    }

    #[test]
    fn config_from_yaml_ide_partial() {
        let yaml = r#"
ide:
  cursor: false
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(config.ide.warp); // default true
        assert!(!config.ide.cursor);
        assert!(config.ide.workspace.is_none());
    }

    #[test]
    fn config_from_yaml_autoprune_bool_true() {
        let yaml = "autoprune: true\n";
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 60); // default
    }

    #[test]
    fn config_from_yaml_autoprune_bool_false() {
        let yaml = "autoprune: false\n";
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(!config.autoprune.enabled);
    }

    #[test]
    fn config_from_yaml_autoprune_object() {
        let yaml = r#"
autoprune:
  enabled: true
  poll_interval_seconds: 120
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 120);
    }

    #[test]
    fn config_from_yaml_autoprune_object_partial() {
        let yaml = r#"
autoprune:
  enabled: true
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(config.autoprune.enabled);
        assert_eq!(config.autoprune.poll_interval_seconds, 60); // default
    }

    #[test]
    fn config_from_yaml_budgets() {
        let yaml = r#"
budgets:
  area: 100000
  docs: 50000
  diff: 30000
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.budgets.area, 100000);
        assert_eq!(config.budgets.docs, 50000);
        assert_eq!(config.budgets.diff, 30000);
    }

    #[test]
    fn config_from_yaml_budgets_partial() {
        let yaml = r#"
budgets:
  area: 80000
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.budgets.area, 80000);
        assert_eq!(config.budgets.docs, 30000); // default
        assert_eq!(config.budgets.diff, 20000); // default
    }

    #[test]
    fn config_from_yaml_summaries() {
        let yaml = r#"
summaries:
  - path: src/
    tokens: 5000
    model: claude
  - path: tests/
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.summaries.len(), 2);
        assert_eq!(config.summaries[0].path, "src/");
        assert_eq!(config.summaries[0].tokens, Some(5000));
        assert_eq!(config.summaries[0].model, "claude");
        assert_eq!(config.summaries[1].path, "tests/");
        assert_eq!(config.summaries[1].model, "gemini"); // default
    }

    #[test]
    fn config_from_yaml_skill_sources() {
        let yaml = r#"
skill_sources:
  - name: superpowers
    prefix: sp
    path: ~/.lf/skills/superpowers
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(config.skill_sources.len(), 1);
        assert_eq!(config.skill_sources[0].name, "superpowers");
        assert_eq!(config.skill_sources[0].prefix, "sp");
    }

    #[test]
    fn config_from_yaml_branch_names() {
        let yaml = r#"
branch_names:
  schema: "{user}.{name}.{date}_{ts}"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert!(config.branch_names.is_some());
        assert_eq!(
            config.branch_names.as_ref().unwrap().schema_,
            "{user}.{name}.{date}_{ts}"
        );
    }

    #[test]
    fn config_from_yaml_lint_and_test() {
        let yaml = r#"
lint: "cargo fmt --check && cargo clippy -- -D warnings"
test: "cargo test --all"
"#;
        let config: Config = serde_yaml::from_str(yaml).expect("parse config");
        assert_eq!(
            config.lint,
            Some("cargo fmt --check && cargo clippy -- -D warnings".to_string())
        );
        assert_eq!(config.test, Some("cargo test --all".to_string()));
    }

    // ==========================================================================
    // Invalid YAML tests
    // ==========================================================================

    #[test]
    fn config_from_yaml_invalid_type() {
        let yaml = "summary_tokens: not_a_number\n";
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn config_from_yaml_invalid_syntax() {
        let yaml = "foo: [invalid\n";
        let result: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    // ==========================================================================
    // Config merge tests
    // ==========================================================================

    #[test]
    fn merge_config_values_repo_overrides_scalar() {
        let global: serde_yaml::Value = serde_yaml::from_str(
            r#"
agent_model: claude:opus
yolo: false
"#,
        )
        .unwrap();

        let repo: serde_yaml::Value = serde_yaml::from_str(
            r#"
agent_model: codex
"#,
        )
        .unwrap();

        let merged = merge_config_values(Some(global), Some(repo));
        let config: Config = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.agent_model, "codex");
        assert!(!config.yolo); // preserved from global
    }

    #[test]
    fn merge_config_values_additive_keys_combine() {
        let global: serde_yaml::Value = serde_yaml::from_str(
            r#"
context:
  - global.md
exclude:
  - "*.log"
"#,
        )
        .unwrap();

        let repo: serde_yaml::Value = serde_yaml::from_str(
            r#"
context:
  - local.md
exclude:
  - build/
"#,
        )
        .unwrap();

        let merged = merge_config_values(Some(global), Some(repo));
        let config: Config = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.context, vec!["global.md", "local.md"]);
        assert_eq!(config.exclude, vec!["*.log", "build/"]);
    }

    #[test]
    fn merge_config_values_global_only() {
        let global: serde_yaml::Value = serde_yaml::from_str("agent_model: claude:opus\n").unwrap();

        let merged = merge_config_values(Some(global), None);
        let config: Config = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.agent_model, "claude:opus");
    }

    #[test]
    fn merge_config_values_repo_only() {
        let repo: serde_yaml::Value = serde_yaml::from_str("agent_model: codex\n").unwrap();

        let merged = merge_config_values(None, Some(repo));
        let config: Config = serde_yaml::from_value(merged).unwrap();

        assert_eq!(config.agent_model, "codex");
    }

    #[test]
    fn merge_config_values_both_none() {
        let merged = merge_config_values(None, None);
        let config: Config = serde_yaml::from_value(merged).unwrap();

        // Should get defaults
        assert_eq!(config.agent_model, "claude:opus");
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
        std::fs::write(&config_path, "agent_model: codex\n").expect("write config");

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
        std::fs::write(
            config_dir.join("config.yaml"),
            "agent_model: codex\nyolo: true\n",
        )
        .expect("write config");

        let result = load_config(Some(temp.path()));
        assert!(result.is_ok());
        let config = result.unwrap().expect("config should exist");
        assert_eq!(config.agent_model, "codex");
        assert!(config.yolo);
    }

    #[test]
    fn load_config_or_default_exists() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let config_dir = temp.path().join(".lf");
        std::fs::create_dir_all(&config_dir).expect("create .lf dir");
        std::fs::write(config_dir.join("config.yaml"), "agent_model: codex\n")
            .expect("write config");

        let config = load_config_or_default(Some(temp.path()));
        assert_eq!(config.agent_model, "codex");
    }
}
