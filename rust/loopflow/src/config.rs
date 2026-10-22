//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global) and `.lf/config.yaml` (repo).
//! Repo config overrides global. Additive keys (docs, context, exclude, summaries) combine.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use crate::agent::check_cli_available;
use crate::error::LoadError;
use crate::prompt::context_budget::BudgetKey;

/// Request participant carried across foreground launches, including SSH.
/// Only a non-empty value overrides personal configuration and Git.
pub const USER_NAME_ENV: &str = "LF_USER_NAME";

#[derive(Debug, Deserialize)]
struct UserConfig {
    name: Option<String>,
}

/// Resolve a display name from personal Loopflow configuration or Git's user.name.
fn load_user_name() -> Result<Option<String>, LoadError> {
    if let Some(config) = load_yaml_file(&global_config_path())? {
        if let Some(user) = config.get("user").filter(|value| !value.is_null()) {
            let user: UserConfig = serde_yaml_ng::from_value(user.clone()).map_err(|error| {
                LoadError::InvalidGraph(format!("Invalid personal user config: {error}"))
            })?;
            if let Some(name) = user.name.as_deref().and_then(normalize_user_name) {
                return Ok(Some(name));
            }
        }
    }
    let Some(output) = Command::new("git")
        .args(["config", "--get", "user.name"])
        .output()
        .ok()
        .filter(|output| output.status.success())
    else {
        return Ok(None);
    };
    Ok(std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(normalize_user_name))
}

/// Resolve a direct invocation's participant before execution moves Machines.
pub fn participant_name() -> Result<Option<String>, LoadError> {
    match std::env::var(USER_NAME_ENV) {
        Ok(name) => match normalize_user_name(&name) {
            Some(name) => Ok(Some(name)),
            None => load_user_name(),
        },
        Err(std::env::VarError::NotPresent) => load_user_name(),
        Err(error) => Err(LoadError::InvalidGraph(format!(
            "Invalid {USER_NAME_ENV}: {error}"
        ))),
    }
}

pub(crate) fn normalize_user_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Agents Loopflow can drive, in the order it prefers them.
const KNOWN_AGENTS: [&str; 4] = ["codex", "claude", "opencode", "pi"];

/// An explicit invocation-wide choice; definitions and defaults never set it.
pub const YOLO_OVERRIDE_ENV: &str = "LF_YOLO_OVERRIDE";
pub const CHROME_OVERRIDE_ENV: &str = "LF_CHROME_OVERRIDE";

pub fn yolo_override() -> bool {
    std::env::var(YOLO_OVERRIDE_ENV).is_ok_and(|value| value == "1")
}

pub const AGENT_OVERRIDE_ENV: &str = "LF_AGENT_OVERRIDE";

pub fn agent_override() -> Option<String> {
    std::env::var(AGENT_OVERRIDE_ENV)
        .ok()
        .filter(|value| !value.is_empty())
}

/// How a recorded conversation continues under the invocation's agent choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resume {
    /// The same provider resumes its native history with this model.
    Native(Option<String>),
    /// Another provider starts fresh from a log of the native history.
    Import { agent: String },
}

pub(crate) fn resume_choice(provider: &str, model: Option<&str>, selected: Option<&str>) -> Resume {
    let Some(agent) = selected.map(str::to_owned) else {
        return Resume::Native(model.map(str::to_owned));
    };
    let (selected, model) = parse_agent(&agent);
    if selected == provider {
        Resume::Native(model)
    } else {
        Resume::Import { agent }
    }
}

/// Agent used when neither the caller, config, nor skill chooses one: the
/// first known agent installed on this machine.
pub fn default_agent() -> &'static str {
    #[cfg(test)]
    let is_installed = |agent: &str| agent == KNOWN_AGENTS[0];
    #[cfg(not(test))]
    let is_installed = check_cli_available;
    installed_agent(is_installed).unwrap_or(KNOWN_AGENTS[0])
}

fn installed_agent(is_installed: impl Fn(&str) -> bool) -> Option<&'static str> {
    KNOWN_AGENTS.into_iter().find(|agent| is_installed(agent))
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

/// Interactive session launch configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionConfig {
    /// Machine-local terminal application used to present detached sessions.
    #[serde(default)]
    pub terminal: Option<String>,
    /// How long a Session keeps its agent running after its last viewer leaves.
    #[serde(default)]
    pub end_detached_after: DetachedLimit,
}

/// When an unviewed Session's agent ends: `24h`, `90m`, `2d`, or `off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum DetachedLimit {
    Off,
    After(std::time::Duration),
}

impl Default for DetachedLimit {
    fn default() -> Self {
        Self::After(std::time::Duration::from_secs(24 * 60 * 60))
    }
}

impl TryFrom<String> for DetachedLimit {
    type Error = String;

    fn try_from(value: String) -> Result<Self, String> {
        if value.trim() == "off" {
            return Ok(Self::Off);
        }
        parse_duration(&value)
            .map(Self::After)
            .map_err(|error| format!("end_detached_after: {error}; or use off"))
    }
}

impl From<DetachedLimit> for String {
    fn from(limit: DetachedLimit) -> Self {
        match limit {
            DetachedLimit::Off => "off".into(),
            DetachedLimit::After(duration) => format!("{}s", duration.as_secs()),
        }
    }
}

/// Parse `30`, `10s`, `5m`, `1h` or `2d`; a bare number is seconds.
pub fn parse_duration(value: &str) -> anyhow::Result<std::time::Duration> {
    let value = value.trim();
    let (number, multiplier) = match value.char_indices().last() {
        Some((at, 's')) => (&value[..at], 1),
        Some((at, 'm')) => (&value[..at], 60),
        Some((at, 'h')) => (&value[..at], 60 * 60),
        Some((at, 'd')) => (&value[..at], 24 * 60 * 60),
        _ => (value, 1),
    };
    let amount: u64 = number.parse().map_err(|_| {
        anyhow::anyhow!("invalid duration {value:?}; use seconds, 10s, 5m, 1h, or 2d")
    })?;
    Ok(std::time::Duration::from_secs(
        amount.saturating_mul(multiplier),
    ))
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

/// Repository merge policy; GitHub's allowed methods remain authoritative.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// One synchronization destination for the repository's local planning.
/// Repository configuration overrides the personal default as one value.
/// Every remote provider names the team that owns the repository's Tasks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawPlanningConfig", into = "RawPlanningConfig")]
pub enum PlanningConfig {
    Git {
        remote: String,
        plan: GitPlan,
    },
    /// The Linear Team's ID; `lf repo connect` finds or creates it.
    Linear {
        team: Option<String>,
    },
    /// The Asana team's gid or URL. Tasks live in the team project named
    /// after the repository; Wave Projects are Asana projects in the team.
    Asana {
        team: String,
    },
    /// The Notion page's ID or URL. It holds the Tasks and Projects databases.
    Notion {
        team: String,
    },
}

/// Which ref on the planning remote carries this repository's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitPlan {
    /// `refs/loopflow/planning/shared/<name>`, read and written by everyone.
    Shared(String),
    /// `refs/loopflow/planning/users/<planning user key>`, one person's plan.
    Personal,
}

impl Default for GitPlan {
    fn default() -> Self {
        Self::Shared("main".into())
    }
}

impl GitPlan {
    /// A personal plan has no reference until its user key exists.
    pub fn reference(&self, user_key: Option<&str>) -> Option<String> {
        match self {
            Self::Shared(name) => Some(format!("refs/loopflow/planning/shared/{name}")),
            Self::Personal => user_key.map(|key| format!("refs/loopflow/planning/users/{key}")),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "snake_case", deny_unknown_fields)]
enum RawPlanningConfig {
    Git {
        #[serde(default = "default_planning_remote")]
        remote: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shared: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        personal: bool,
    },
    /// The Linear Team's ID; `lf repo connect` finds or creates it.
    Linear {
        #[serde(default, deserialize_with = "crate::id::deserialize_uuid_option")]
        team: Option<String>,
    },
    /// The Asana team's gid or URL. Tasks live in the team project named
    /// after the repository; Wave Projects are Asana projects in the team.
    Asana { team: String },
    /// The Notion page's ID or URL. It holds the Tasks and Projects databases.
    Notion { team: String },
}

impl TryFrom<RawPlanningConfig> for PlanningConfig {
    type Error = String;

    fn try_from(raw: RawPlanningConfig) -> Result<Self, Self::Error> {
        let (remote, shared, personal) = match raw {
            RawPlanningConfig::Git {
                remote,
                shared,
                personal,
            } => (remote, shared, personal),
            RawPlanningConfig::Linear { team } => return Ok(Self::Linear { team }),
            RawPlanningConfig::Asana { team } => return Ok(Self::Asana { team }),
            RawPlanningConfig::Notion { team } => return Ok(Self::Notion { team }),
        };
        let plan = match (shared, personal) {
            (Some(name), true) => {
                return Err(format!(
                    "planning sets both `shared: {name}` and `personal: true`; keep one"
                ))
            }
            (Some(name), false) => GitPlan::Shared(name),
            (None, true) => GitPlan::Personal,
            (None, false) => GitPlan::default(),
        };
        Ok(Self::Git { remote, plan })
    }
}

impl From<PlanningConfig> for RawPlanningConfig {
    fn from(config: PlanningConfig) -> Self {
        match config {
            PlanningConfig::Git { remote, plan } => {
                let (shared, personal) = match plan {
                    GitPlan::Shared(name) => (Some(name), false),
                    GitPlan::Personal => (None, true),
                };
                Self::Git {
                    remote,
                    shared,
                    personal,
                }
            }
            PlanningConfig::Linear { team } => Self::Linear { team },
            PlanningConfig::Asana { team } => Self::Asana { team },
            PlanningConfig::Notion { team } => Self::Notion { team },
        }
    }
}

fn default_planning_remote() -> String {
    "origin".into()
}

impl PlanningConfig {
    /// The remote planning provider's product name; Git planning has none.
    pub fn remote_name(&self) -> Option<&'static str> {
        match self {
            Self::Git { .. } => None,
            Self::Linear { .. } => Some("Linear"),
            Self::Asana { .. } => Some("Asana"),
            Self::Notion { .. } => Some("Notion"),
        }
    }
}

impl Default for PlanningConfig {
    fn default() -> Self {
        Self::Git {
            remote: default_planning_remote(),
            plan: GitPlan::default(),
        }
    }
}

impl Config {
    pub fn planning_transport(&self) -> PlanningConfig {
        self.planning.clone().unwrap_or_default()
    }
}

/// An ordinary lf command scheduled by explicit installation on a Machine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CronDef {
    pub name: String,
    pub command: Vec<String>,
    pub schedule: String,
}

/// Main configuration struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default Workflow for Tasks without an explicit or Wave selection.
    pub workflow: Option<String>,
    #[serde(default)]
    pub crons: Option<Vec<CronDef>>,
    #[serde(default)]
    pub planning: Option<PlanningConfig>,
    #[serde(default)]
    pub automation: AutomationConfig,
    #[serde(default)]
    pub context_budgets: BTreeMap<BudgetKey, usize>,
    #[serde(skip)]
    pub context_budget_sources: BTreeMap<BudgetKey, String>,
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

    /// Repository-only merge preference; absent chooses an allowed method.
    #[serde(default)]
    pub merge_method: Option<MergeMethod>,

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

    /// Summaries to include
    #[serde(default)]
    pub summaries: Vec<SummaryConfig>,

    /// Default token budget for summaries
    #[serde(default = "default_summary_tokens")]
    pub summary_tokens: usize,

    /// Autoprune configuration

    /// Release targets and scoping rules.
    #[serde(default)]
    pub release: ReleaseConfig,

    /// Run provider conversations in the selected account's own home rather
    /// than the provider's native one. `--shared` overrides it per launch.
    #[serde(default)]
    pub isolate: bool,

    /// Machine-local `~/.lf` disk budget, e.g. `20G`; oldest material rotates out.
    #[serde(default)]
    pub disk_budget: Option<String>,
}

fn default_land() -> String {
    "gh".to_string()
}

fn default_summary_tokens() -> usize {
    5000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AutomationConfig {
    pub retries: u32,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self { retries: 1 }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workflow: None,
            planning: None,
            crons: None,
            context_budgets: Default::default(),
            context_budget_sources: Default::default(),
            agent: None,
            supported_harnesses: Vec::new(),
            yolo: false,
            chrome: false,
            pr: false,
            land: default_land(),
            merge_method: None,
            context: Vec::new(),
            exclude: Vec::new(),
            session: SessionConfig::default(),
            automation: AutomationConfig::default(),
            docs: Vec::new(),
            diff: false,
            diff_files: false,
            paste: false,
            summaries: Vec::new(),
            summary_tokens: default_summary_tokens(),
            release: ReleaseConfig::default(),
            isolate: false,
            disk_budget: None,
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
    let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).map_err(|e| {
        LoadError::InvalidGraph(format!("YAML parse error in {}: {}", path.display(), e))
    })?;
    fold_linear_team(&mut value);
    Ok(Some(value))
}

/// Released lf reads the Linear Team from `linear.team`, so a repository
/// keeps that block while either build may run in it. Read it as Linear
/// planning's team; an explicit `planning.team` wins. LOO-560 removes this
/// once a release with `planning.team` is installed.
fn fold_linear_team(value: &mut serde_yaml_ng::Value) {
    let Some(mapping) = value.as_mapping_mut() else {
        return;
    };
    let Some(linear) = mapping.remove("linear") else {
        return;
    };
    let Some(team) = linear.get("team").cloned() else {
        return;
    };
    if let Some(planning) = mapping.get_mut("planning").and_then(|p| p.as_mapping_mut()) {
        if planning.get("provider").and_then(|p| p.as_str()) == Some("linear")
            && !planning.contains_key("team")
        {
            planning.insert("team".into(), team);
        }
    }
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
                } else if key_str == "context_budgets" {
                    let previous = global_map.remove(&key);
                    global_map.insert(key, merge_config_values(previous, Some(value)));
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

    let mut sources = BTreeMap::new();
    for (data, path) in [
        (&global_data, Some(&global_path)),
        (&repo_data, repo_path.as_ref()),
    ] {
        if let (Some(data), Some(path)) = (data, path) {
            if let Some(budgets) = data.get("context_budgets") {
                let budgets: BTreeMap<BudgetKey, usize> =
                    serde_yaml_ng::from_value(budgets.clone()).map_err(|e| {
                        LoadError::InvalidGraph(format!(
                            "Invalid context_budgets in {}: {e}",
                            path.display()
                        ))
                    })?;
                sources.extend(budgets.keys().map(|key| (*key, path.display().to_string())));
            }
        }
    }
    let merged = merge_config_values(global_data, repo_data);

    let mut config: Config = serde_yaml_ng::from_value(merged)
        .map_err(|e| LoadError::InvalidGraph(format!("Config validation error: {}", e)))?;

    config.context_budget_sources = sources;
    Ok(Some(config))
}

/// Load only Machine-local user configuration.
pub fn load_global_config() -> Result<Option<Config>, LoadError> {
    let Some(value) = load_yaml_file(&global_config_path())? else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(value)
        .map(Some)
        .map_err(|error| LoadError::InvalidGraph(format!("Config validation error: {error}")))
}

/// Repository lookup belongs to the selected Machine, never repository config.
pub fn repository_root() -> Result<Option<PathBuf>, LoadError> {
    let path = global_config_path();
    let Some(value) = load_yaml_file(&path)?.and_then(|config| config.get("repo_root").cloned())
    else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(value).map_err(|error| {
        LoadError::InvalidGraph(format!("Invalid repo_root in {}: {error}", path.display()))
    })
}

/// Persist a Machine-local configuration mapping without exposing partial YAML.
pub(crate) fn write_global_config_at(
    path: &Path,
    value: &serde_yaml_ng::Value,
) -> anyhow::Result<()> {
    let parent = path.parent().expect("config path has a parent");
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(serde_yaml_ng::to_string(value)?.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

fn global_config_path() -> PathBuf {
    crate::store::lf_home_dir().join("config.yaml")
}

/// Load only the repository-owned config, without inheriting user-global values.
///
/// Linear Team bindings use this path; personal transport defaults do not assign
/// a repository to a Team.
pub fn load_repo_config(repo_root: &Path) -> Result<Option<Config>, LoadError> {
    let Some(value) = load_yaml_file(&repo_root.join(".lf/config.yaml"))? else {
        return Ok(None);
    };
    serde_yaml_ng::from_value(value)
        .map(Some)
        .map_err(|error| LoadError::InvalidGraph(format!("Config validation error: {error}")))
}

/// Get config or default if no config files exist.
pub fn load_config_or_default(repo_root: Option<&Path>) -> Config {
    load_config(repo_root).ok().flatten().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn resume_stays_native_until_the_provider_changes() {
        use super::Resume;
        assert_eq!(
            super::resume_choice("claude", Some("haiku"), None),
            Resume::Native(Some("haiku".into()))
        );
        assert_eq!(
            super::resume_choice("claude", Some("haiku"), Some("claude:opus")),
            Resume::Native(Some("opus".into()))
        );
        assert_eq!(
            super::resume_choice("codex", None, Some("claude:opus")),
            Resume::Import {
                agent: "claude:opus".into()
            }
        );
    }

    use super::*;

    #[test]
    fn default_agent_is_the_one_installed() {
        assert_eq!(installed_agent(|agent| agent == "claude"), Some("claude"));
        assert_eq!(installed_agent(|_| true), Some("codex"));
        assert_eq!(installed_agent(|_| false), None);
    }

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
        for harness in ["claude", "codex", "opencode", "pi"] {
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
    fn repo_config_reads_the_released_linear_team() {
        let team = "3871bed0-8c95-4077-ab72-728327d615b3";
        let temp = tempfile::tempdir().expect("create temp dir");
        std::fs::create_dir_all(temp.path().join(".lf")).expect("create .lf dir");
        std::fs::write(
            temp.path().join(".lf/config.yaml"),
            format!("planning:\n  provider: linear\nlinear:\n  team: {team}\n"),
        )
        .expect("write config");
        let config = load_repo_config(temp.path()).unwrap().unwrap();
        assert_eq!(
            config.planning_transport(),
            PlanningConfig::Linear {
                team: Some(team.to_string())
            }
        );
    }

    #[test]
    fn config_parses_planning_team() {
        let yaml = "planning:\n  provider: asana\n  team: \"9876543210\"\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(
            config.planning_transport(),
            PlanningConfig::Asana {
                team: "9876543210".to_string(),
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
        assert!(config.release.targets.is_empty());
    }

    #[test]
    fn default_session_config() {
        let session = SessionConfig::default();
        assert!(session.terminal.is_none());
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
    fn config_from_yaml_session_terminal() {
        let yaml = r#"
session:
  terminal: Ghostty
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.session.terminal.as_deref(), Some("Ghostty"));
    }

    #[test]
    fn detached_sessions_end_after_a_day_unless_configured() {
        use super::DetachedLimit;
        use std::time::Duration;
        let parse = |yaml: &str| serde_yaml_ng::from_str::<Config>(yaml).map(|c| c.session);
        assert_eq!(
            parse("agent: claude").unwrap().end_detached_after,
            DetachedLimit::After(Duration::from_secs(86_400))
        );
        assert_eq!(
            parse("session: {end_detached_after: 90m}")
                .unwrap()
                .end_detached_after,
            DetachedLimit::After(Duration::from_secs(5_400))
        );
        assert_eq!(
            parse("session: {end_detached_after: 2d}")
                .unwrap()
                .end_detached_after,
            DetachedLimit::After(Duration::from_secs(172_800))
        );
        assert_eq!(
            parse("session: {end_detached_after: off}")
                .unwrap()
                .end_detached_after,
            DetachedLimit::Off
        );
        assert!(parse("session: {end_detached_after: soon}").is_err());
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
}

#[cfg(test)]
mod planning_config_tests {
    use super::{merge_config_values, Config, GitPlan, PlanningConfig};

    #[test]
    fn repository_transport_replaces_personal_provider_fields() {
        let personal = serde_yaml_ng::from_str(
            "planning:\n  provider: git\n  remote: private\n  shared: solo\n",
        )
        .unwrap();
        let repository =
            serde_yaml_ng::from_str("planning:\n  provider: linear\n  team: team\n").unwrap();
        let config: Config =
            serde_yaml_ng::from_value(merge_config_values(Some(personal), Some(repository)))
                .unwrap();
        assert_eq!(
            config.planning_transport(),
            PlanningConfig::Linear {
                team: Some("team".into())
            }
        );
        assert_eq!(
            Config::default().planning_transport(),
            PlanningConfig::Git {
                remote: "origin".into(),
                plan: GitPlan::Shared("main".into())
            }
        );
        let personal: Config =
            serde_yaml_ng::from_str("planning:\n  provider: git\n  personal: true\n").unwrap();
        assert_eq!(
            personal.planning_transport(),
            PlanningConfig::Git {
                remote: "origin".into(),
                plan: GitPlan::Personal
            }
        );
        let both = serde_yaml_ng::from_str::<Config>(
            "planning:\n  provider: git\n  shared: team\n  personal: true\n",
        )
        .unwrap_err();
        assert!(both.to_string().contains("personal: true"), "{both}");
        assert!(serde_yaml_ng::from_str::<Config>(
            "planning:\n  provider: linear\n  shared: employees\n"
        )
        .is_err());
    }
}
