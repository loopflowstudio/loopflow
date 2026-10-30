//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global), `.lf/config.yaml` (repo) and,
//! for work in a Wave, `.lf/wave/<name>/config.yaml`. Each nearer layer overrides
//! the one before; additive keys (docs, context, exclude, summaries) combine and
//! keyed maps (context_budgets, skill_agents) merge per key.

use std::collections::{BTreeMap, BTreeSet, HashMap};
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

/// Resolve a display name from personal Loopflow configuration or Git's
/// user.name in `directory`.
fn load_user_name(directory: &Path) -> Result<Option<String>, LoadError> {
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
        .arg("-C")
        .arg(directory)
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

/// A boolean this Machine sets for itself in `config.yaml` under `home`, such
/// as `logs.linear_requests`; never read from a repository. Absent, unreadable
/// or not a boolean is off.
pub(crate) fn machine_flag(home: &Path, path: &[&str]) -> bool {
    let Ok(Some(mut value)) = load_yaml_file(&home.join("config.yaml")) else {
        return false;
    };
    for key in path {
        match value.get(*key) {
            Some(child) => value = child.clone(),
            None => return false,
        }
    }
    value.as_bool().unwrap_or(false)
}

/// Resolve a direct invocation's participant before execution moves Machines.
pub fn participant_name() -> Result<Option<String>, LoadError> {
    participant_name_in(Path::new("."))
}

/// The participant as `lf` would name them running in `directory`: an app
/// resolves a repository's Git name from that repository, not where it stands.
pub fn participant_name_in(directory: &Path) -> Result<Option<String>, LoadError> {
    match std::env::var(USER_NAME_ENV) {
        Ok(name) => match normalize_user_name(&name) {
            Some(name) => Ok(Some(name)),
            None => load_user_name(directory),
        },
        Err(std::env::VarError::NotPresent) => load_user_name(directory),
        Err(error) => Err(LoadError::InvalidGraph(format!(
            "Invalid {USER_NAME_ENV}: {error}"
        ))),
    }
}

/// Your stable person key: Git's `user.email` in `repo` (your global one when
/// the checkout is absent here), lowercased. Names are display only; a
/// committed file can name a person by this key.
pub fn participant_email(repo: &Path) -> Option<String> {
    let mut command = Command::new("git");
    if repo.is_dir() {
        command.arg("-C").arg(repo);
    }
    let output = command
        .args(["config", "--get", "user.email"])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let email = std::str::from_utf8(&output.stdout).ok()?.trim();
    (!email.is_empty()).then(|| email.to_lowercase())
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

thread_local! {
    static CALL_AGENT: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

pub fn agent_override() -> Option<String> {
    CALL_AGENT
        .with(|agent| agent.borrow().clone())
        .or_else(|| std::env::var(AGENT_OVERRIDE_ENV).ok())
        .filter(|value| !value.is_empty())
}

/// Select `agent` for one call on this thread, as `lf --agent` selects it for
/// its whole process through `LF_AGENT_OVERRIDE`.
pub fn with_agent_override<T>(agent: Option<&str>, run: impl FnOnce() -> T) -> T {
    struct Restore(Option<String>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CALL_AGENT.with(|agent| *agent.borrow_mut() = self.0.take());
        }
    }
    let _restore = Restore(CALL_AGENT.with(|current| current.replace(agent.map(str::to_owned))));
    run()
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

/// Keys whose maps merge entry by entry, the nearer layer winning per entry.
const MAPPED_KEYS: &[&str] = &["context_budgets", "skill_agents"];

/// Keys only a repository or your personal config may set; a Wave file
/// naming one is an error.
const REPO_ONLY_KEYS: &[&str] = &[
    "planning",
    "release",
    "exclude",
    "merge_method",
    "land",
    "pr",
];

/// The config layer a value came from, farthest first. `Personal` is your
/// own `lf set` value for the repository or Wave; yours for every repository
/// sits with `~/.lf/config.yaml` as `Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigLayer {
    Global,
    Repo,
    Wave,
    Personal,
}

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
    /// The Linear Team's ID; `lf wave connect` finds or creates it.
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
    /// The Linear Team's ID; `lf wave connect` finds or creates it.
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

/// Main configuration struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// The graph Tasks take up without their own or their Wave's selection;
    /// `code` when unset. The released lf reads `workflow:`; LOO-637 removes
    /// that alias once the release that reads `graph:` is installed.
    #[serde(alias = "workflow")]
    pub graph: Option<String>,
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

    /// Agent for one skill by name, ahead of the skill's own choice and `agent`.
    #[serde(default)]
    pub skill_agents: BTreeMap<String, String>,

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
            graph: None,
            planning: None,
            context_budgets: Default::default(),
            context_budget_sources: Default::default(),
            agent: None,
            skill_agents: BTreeMap::new(),
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
                } else if MAPPED_KEYS.contains(&key_str) {
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
    load_wave_config(repo_root, None)
}

/// Load config for work in a Wave: global, then repo, then the Wave's
/// `config.yaml`. With no Wave this is [`load_config`].
pub fn load_wave_config(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Option<Config>, LoadError> {
    load_wave_config_with(repo_root, wave, &PersonalSettings::read(repo_root, wave))
}

/// [`load_wave_config`] with your `lf set` values already read, so a reading
/// of many Tasks reads them once per repository and Wave.
pub fn load_wave_config_with(
    repo_root: Option<&Path>,
    wave: Option<&str>,
    personal: &PersonalSettings,
) -> Result<Option<Config>, LoadError> {
    let layers = config_layers_with(repo_root, wave, personal)?;
    if layers.is_empty() {
        return Ok(None);
    }
    let mut sources = BTreeMap::new();
    let mut merged = None;
    for layer in layers {
        if let Some(budgets) = layer.value.get("context_budgets") {
            let budgets: BTreeMap<BudgetKey, usize> = serde_yaml_ng::from_value(budgets.clone())
                .map_err(|e| {
                    LoadError::InvalidGraph(format!(
                        "Invalid context_budgets in {}: {e}",
                        layer.path.display()
                    ))
                })?;
            sources.extend(
                budgets
                    .keys()
                    .map(|key| (*key, layer.path.display().to_string())),
            );
        }
        merged = Some(merge_config_values(merged, Some(layer.value)));
    }
    let merged = merged.expect("at least one layer");

    let mut config: Config = serde_yaml_ng::from_value(merged)
        .map_err(|e| LoadError::InvalidGraph(format!("Config validation error: {}", e)))?;

    config.context_budget_sources = sources;
    Ok(Some(config))
}

/// The planning transport from the global and repository config files.
/// `lf set` holds no planning values, so this opens no store: planning checks
/// resolve it once per Wave and Task inside a bounded hook.
pub fn load_planning_transport(repo_root: &Path) -> Result<PlanningConfig, LoadError> {
    let global = load_yaml_file(&global_config_path())?;
    let repo = load_yaml_file(&repo_root.join(".lf").join("config.yaml"))?;
    let config: Config = serde_yaml_ng::from_value(merge_config_values(global, repo))
        .map_err(|e| LoadError::InvalidGraph(format!("Config validation error: {e}")))?;
    Ok(config.planning_transport())
}

/// One present config file's values.
#[derive(Debug, Clone)]
pub struct ConfigLayerValues {
    pub layer: ConfigLayer,
    pub path: PathBuf,
    pub value: serde_yaml_ng::Value,
}

/// The present config layers for work in a Wave, farthest first: global
/// files, the repository's and Wave's committed config, then your `lf set`
/// values for the repository and the Wave.
pub fn config_layers(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Vec<ConfigLayerValues>, LoadError> {
    config_layers_with(repo_root, wave, &PersonalSettings::read(repo_root, wave))
}

fn config_layers_with(
    repo_root: Option<&Path>,
    wave: Option<&str>,
    personal_values: &PersonalSettings,
) -> Result<Vec<ConfigLayerValues>, LoadError> {
    let mut layers = Vec::new();
    let global_path = global_config_path();
    if let Some(value) = load_yaml_file(&global_path)? {
        layers.push(ConfigLayerValues {
            layer: ConfigLayer::Global,
            path: global_path,
            value,
        });
    }
    // Your values for every repository rank with the global file; yours for
    // the repository and then the Wave rank above every committed layer.
    let mut personal = Vec::new();
    if let Some((path, settings)) = &personal_values.0 {
        let layer = |layer, values: &BTreeMap<String, String>| {
            (!values.is_empty()).then(|| ConfigLayerValues {
                layer,
                path: path.clone(),
                value: settings_value(values),
            })
        };
        layers.extend(layer(ConfigLayer::Global, &settings.global));
        personal.extend(layer(ConfigLayer::Personal, &settings.repository));
        personal.extend(layer(ConfigLayer::Personal, &settings.wave));
    }
    let Some(repo_root) = repo_root else {
        return Ok(layers);
    };
    let repo_path = repo_root.join(".lf").join("config.yaml");
    if let Some(value) = load_yaml_file(&repo_path)? {
        layers.push(ConfigLayerValues {
            layer: ConfigLayer::Repo,
            path: repo_path,
            value,
        });
    }
    if let Some(wave) = wave {
        if let Some((path, value)) = crate::work::wave::config::read_wave_layer(repo_root, wave)
            .map_err(|error| LoadError::InvalidGraph(error.to_string()))?
        {
            validate_wave_keys(&path, &value)?;
            layers.push(ConfigLayerValues {
                layer: ConfigLayer::Wave,
                path,
                value,
            });
        }
    }
    layers.extend(personal);
    Ok(layers)
}

/// Your `lf set` values in effect for one repository and Wave, with the store
/// they came from.
#[derive(Debug, Clone)]
pub struct PersonalSettings(Option<(PathBuf, crate::store::sqlite::ScopeSettings)>);

impl PersonalSettings {
    /// The local store is optional here: absent, or older than this build, it
    /// holds none.
    pub fn read(repo_root: Option<&Path>, wave: Option<&str>) -> Self {
        Self(personal_settings(repo_root, wave))
    }
}

fn personal_settings(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Option<(PathBuf, crate::store::sqlite::ScopeSettings)> {
    let read = || -> anyhow::Result<Option<(PathBuf, crate::store::sqlite::ScopeSettings)>> {
        let path = crate::store::database_path_from_env()?;
        if !path.try_exists()? {
            return Ok(None);
        }
        // Reuse the receipt ledger when present, without retaining a new connection.
        let store = match crate::journal::retained_ledger(&path) {
            Some(store) => store,
            None => crate::store::sqlite::SqliteStore::open_read_only(&path)?,
        };
        if !store.has_scope_settings()? {
            return Ok(None);
        }
        let repo = repo_root
            .and_then(|root| crate::repository::CanonicalRepo::discover(root).ok())
            .map(|repo| repo.to_string());
        let settings = store.scope_settings(repo.as_deref(), wave)?;
        Ok(Some((path, settings)))
    };
    read().unwrap_or_else(|error| {
        tracing::warn!(%error, "your lf set values are unavailable");
        None
    })
}

fn settings_value(values: &BTreeMap<String, String>) -> serde_yaml_ng::Value {
    let mut mapping = serde_yaml_ng::Mapping::new();
    for (key, value) in values {
        match key.split_once('.') {
            Some((parent, child)) => {
                let entry = mapping
                    .entry(parent.into())
                    .or_insert_with(|| serde_yaml_ng::Mapping::new().into());
                if let Some(entry) = entry.as_mapping_mut() {
                    entry.insert(child.into(), value.as_str().into());
                }
            }
            None => {
                mapping.insert(key.as_str().into(), value.as_str().into());
            }
        }
    }
    serde_yaml_ng::Value::Mapping(mapping)
}

/// The nearest layer setting the value at `path`, such as
/// `["skill_agents", "implement"]`, with that value.
pub fn layer_setting<'a>(
    layers: &'a [ConfigLayerValues],
    path: &[&str],
) -> Option<(ConfigLayer, &'a serde_yaml_ng::Value)> {
    layers.iter().rev().find_map(|layer| {
        let mut value = &layer.value;
        for key in path {
            value = value.get(*key)?;
        }
        (!value.is_null()).then_some((layer.layer, value))
    })
}

/// The graph a Task in these layers takes up and the layer selecting it; no
/// layer means the builtin `code`. A global value (`~/.lf/config.yaml`) never
/// selects one for a repository's Tasks, and it is the farthest layer, so it is
/// simply ignored. Each layer may still say `workflow:`, the released name;
/// LOO-637 removes that alias.
pub fn graph_setting(
    layers: &[ConfigLayerValues],
) -> Result<(String, Option<ConfigLayer>), LoadError> {
    let Some((layer, value)) = layers
        .iter()
        .rev()
        .filter(|layer| layer.layer != ConfigLayer::Global)
        .find_map(|layer| {
            ["graph", GRAPH_ALIAS]
                .iter()
                .find_map(|key| layer.value.get(*key).filter(|value| !value.is_null()))
                .map(|value| (layer.layer, value))
        })
    else {
        return Ok((crate::ops::task::DEFAULT_TASK_GRAPH.to_owned(), None));
    };
    let name = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidGraph("graph must be a graph name".to_owned()))?;
    crate::task_graph::validate_graph_name(name)
        .map_err(|error| LoadError::InvalidGraph(format!("graph {name}: {error}")))?;
    Ok((name.to_owned(), Some(layer)))
}

/// The released name of `graph:`; LOO-637 removes it.
const GRAPH_ALIAS: &str = "workflow";

/// Keys a Wave's `config.yaml` may set: every `Config` key a repository may
/// share with its Waves.
pub fn wave_config_keys() -> BTreeSet<String> {
    let Ok(serde_yaml_ng::Value::Mapping(defaults)) = serde_yaml_ng::to_value(Config::default())
    else {
        unreachable!("Config serializes as a mapping");
    };
    defaults
        .keys()
        .filter_map(|key| key.as_str())
        .filter(|key| !REPO_ONLY_KEYS.contains(key))
        .map(str::to_owned)
        .collect()
}

fn validate_wave_keys(path: &Path, value: &serde_yaml_ng::Value) -> Result<(), LoadError> {
    let Some(mapping) = value.as_mapping() else {
        return Err(LoadError::InvalidGraph(format!(
            "{} must be a mapping",
            path.display()
        )));
    };
    let allowed = wave_config_keys();
    for key in mapping.keys().filter_map(|key| key.as_str()) {
        if REPO_ONLY_KEYS.contains(&key) {
            return Err(LoadError::InvalidGraph(format!(
                "{}: `{key}` belongs to the repository's .lf/config.yaml, not a Wave",
                path.display()
            )));
        }
        if !allowed.contains(key) && key != GRAPH_ALIAS {
            return Err(LoadError::InvalidGraph(format!(
                "{}: unknown key `{key}`",
                path.display()
            )));
        }
    }
    Ok(())
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

/// Read a Machine-local configuration mapping; absent or empty is an empty mapping.
pub(crate) fn read_global_config_at(path: &Path) -> anyhow::Result<serde_yaml_ng::Value> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(anyhow::Error::new(error).context(format!("Read {}", path.display())))
        }
    };
    match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text)? {
        serde_yaml_ng::Value::Null => Ok(serde_yaml_ng::Value::Mapping(Default::default())),
        value if value.is_mapping() => Ok(value),
        _ => anyhow::bail!("{} must contain a configuration mapping", path.display()),
    }
}

/// Change `home`'s config.yaml under its lock, persisting it only when `change`
/// succeeds, without exposing partial YAML.
pub(crate) fn update_global_config_at<T>(
    home: &Path,
    change: impl FnOnce(&mut serde_yaml_ng::Value) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    fs::create_dir_all(home)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("config.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    let path = home.join("config.yaml");
    let mut value = read_global_config_at(&path)?;
    let result = change(&mut value)?;
    let mut file = tempfile::NamedTempFile::new_in(home)?;
    file.write_all(serde_yaml_ng::to_string(&value)?.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(&path)?;
    Ok(result)
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
    fn skill_agents_merge_per_skill_and_a_wave_layer_rejects_repository_keys() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/wave/infra")).unwrap();
        std::fs::write(
            repo.path().join(".lf/config.yaml"),
            "agent: codex\nskill_agents:\n  implement: codex:o3\n  design: claude\n",
        )
        .unwrap();
        let wave = repo.path().join(".lf/wave/infra/config.yaml");
        std::fs::write(&wave, "skill_agents:\n  implement: claude:opus\n").unwrap();
        let layers = config_layers(Some(repo.path()), Some("infra")).unwrap();
        let config = load_wave_config(Some(repo.path()), Some("infra"))
            .unwrap()
            .unwrap();
        assert_eq!(config.agent.as_deref(), Some("codex"));
        assert_eq!(config.skill_agents["implement"], "claude:opus");
        assert_eq!(config.skill_agents["design"], "claude");
        assert_eq!(
            layer_setting(&layers, &["skill_agents", "implement"]).map(|(layer, _)| layer),
            Some(ConfigLayer::Wave)
        );
        assert_eq!(
            layer_setting(&layers, &["agent"]).map(|(layer, _)| layer),
            Some(ConfigLayer::Repo)
        );

        std::fs::write(&wave, "planning:\n  provider: git\n").unwrap();
        assert!(load_wave_config(Some(repo.path()), Some("infra")).is_err());
        std::fs::write(&wave, "agnet: claude\n").unwrap();
        assert!(load_wave_config(Some(repo.path()), Some("infra")).is_err());
        assert!(load_config(Some(repo.path())).is_ok());
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
