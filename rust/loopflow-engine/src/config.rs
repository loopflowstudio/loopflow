//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo).
//! Repo config overrides global. Additive keys (context, exclude, summaries) combine.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::LoadError;

/// Keys that combine lists from global + repo config.
const ADDITIVE_KEYS: &[&str] = &["context", "exclude", "skill_sources", "summaries"];

/// Token budgets for prompt sections.
#[derive(Debug, Clone, Deserialize, Default)]
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
pub struct SkillSourceConfig {
    pub name: String,
    pub prefix: String,
    pub path: String,
}

/// Branch naming configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct BranchNameConfig {
    #[serde(default = "default_branch_schema", alias = "schema")]
    pub schema_: String,
}

fn default_branch_schema() -> String {
    "{name}".to_string()
}

impl Default for BranchNameConfig {
    fn default() -> Self {
        Self {
            schema_: default_branch_schema(),
        }
    }
}

/// Autoprune configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct AutopruneConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_seconds: u64,
}

fn default_poll_interval() -> u64 {
    60
}

/// IDE configuration.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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

    /// Include reports/, roadmap/, scratch/, and root .md files
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

    /// Command to check if lint passes
    #[serde(default)]
    pub lint_check: Option<String>,

    /// Autoprune configuration
    #[serde(default)]
    pub autoprune: AutopruneConfig,

    /// Token budgets
    #[serde(default)]
    pub budgets: BudgetConfig,
}

fn default_agent_model() -> String {
    "claude:opus".to_string()
}

fn default_land() -> String {
    "gh".to_string()
}

fn default_summary_tokens() -> usize {
    10000
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
            lint_check: None,
            autoprune: AutopruneConfig::default(),
            budgets: BudgetConfig::default(),
        }
    }
}

/// Parse model string like 'claude:opus' into (backend, variant).
///
/// Applies smart defaults when no variant is specified:
/// - claude -> opus (Claude Opus 4.5)
/// - gemini -> 2.5-pro (Gemini 2.5 Pro)
/// - codex -> None (let Codex CLI pick its default)
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

    #[test]
    fn parse_model_with_variant() {
        let (backend, variant) = parse_model("claude:opus");
        assert_eq!(backend, "claude");
        assert_eq!(variant, Some("opus".to_string()));
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
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.agent_model, "claude:opus");
        assert!(!config.yolo);
        assert!(config.lfdocs);
        assert!(config.diff_files);
        assert!(!config.diff);
    }
}
