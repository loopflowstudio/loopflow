//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global), `.lf/config.yaml` (repo) and,
//! for work in a Wave, `.lf/wave/<name>/config.yaml`. Settings resolve by source,
//! then scope and target; see `settings` for precedence and provenance.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

#[cfg(not(test))]
use crate::agent::check_cli_available;
use crate::error::LoadError;

pub mod api;
pub mod catalog;
mod definitions;
pub mod files;
pub mod migrate;
pub(crate) mod observation;
pub(crate) mod placement;
pub mod remote;
pub mod settings;

/// Request participant carried across foreground launches, including SSH.
/// Only a non-empty value overrides personal configuration and Git.
pub const USER_NAME_ENV: &str = "LF_USER_NAME";

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    pub name: Option<String>,
}

/// Resolve a display name from personal Loopflow configuration or Git's
/// user.name in `directory`.
fn load_user_name(directory: &Path) -> Result<Option<String>, LoadError> {
    if let Some(name) = load_global_config()?
        .and_then(|config| config.user.name)
        .as_deref()
        .and_then(normalize_user_name)
    {
        return Ok(Some(name));
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
    load_user_name(directory)
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

/// Agents Loopflow can drive; listing order grants no preference.
const KNOWN_AGENTS: [&str; 4] = ["codex", "claude", "opencode", "pi"];

/// Claude model families a launch can name with `claude:<family>` and a
/// scoped Claude limit can name in its `rate_limit_type` suffix.
pub const CLAUDE_MODEL_FAMILIES: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];

/// The models a bare `claude` step falls through to when the one it runs
/// hits a scoped limit, best first.
pub const CLAUDE_MODEL_RUNGS: [&str; 3] = ["opus", "sonnet", "haiku"];

/// An explicit invocation-wide choice; definitions and defaults never set it.
pub const YOLO_OVERRIDE_ENV: &str = "LF_YOLO_OVERRIDE";
pub const CHROME_OVERRIDE_ENV: &str = "LF_CHROME_OVERRIDE";

pub const AGENT_OVERRIDE_ENV: &str = "LF_AGENT_OVERRIDE";

pub fn agent_override() -> Option<String> {
    settings::explicit_agent().ok().flatten()
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

/// Select automatically only when exactly one supported harness is installed.
pub fn default_agent() -> Option<&'static str> {
    #[cfg(test)]
    let is_installed = |agent: &str| agent == KNOWN_AGENTS[0];
    #[cfg(not(test))]
    let is_installed = check_cli_available;
    installed_agent(is_installed)
}

fn installed_agent(is_installed: impl Fn(&str) -> bool) -> Option<&'static str> {
    let mut installed = KNOWN_AGENTS.into_iter().filter(|agent| is_installed(agent));
    let only = installed.next()?;
    installed.next().is_none().then_some(only)
}

pub(crate) fn select_agent(agent: Option<&str>) -> Result<&str, LoadError> {
    match agent {
        Some(agent) => Ok(agent),
        None => default_agent().ok_or(LoadError::AgentSelection),
    }
}

/// Authority is independent of the declaration's scope and target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSource {
    Builtin,
    Repository,
    Personal,
    Environment,
    CommandLine,
}

impl ConfigSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Repository => "repository",
            Self::Personal => "personal",
            Self::Environment => "environment",
            Self::CommandLine => "command_line",
        }
    }
}

/// Summary configuration for a specific path.
#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
pub struct SummaryConfig {
    pub path: String,
    #[serde(default)]
    pub tokens: Option<usize>,
    #[serde(default)]
    pub agent: Option<String>,
}

/// Interactive session launch configuration.
#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SessionConfig {
    /// Machine-local terminal application used to present detached sessions.
    #[serde(default)]
    pub terminal: Option<String>,
    /// How long a Session keeps its agent running after its last viewer leaves.
    #[serde(default)]
    pub end_detached_after: DetachedLimit,
}

/// How long a Session nobody views and nothing uses stays resident before its
/// agent ends: `2m`, `30m`, `24h`, or `off`. `lf resume` reopens it.
#[derive(schemars::JsonSchema, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum DetachedLimit {
    Off,
    After(std::time::Duration),
}

impl Default for DetachedLimit {
    fn default() -> Self {
        Self::After(std::time::Duration::from_secs(2 * 60))
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
#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ReleaseConfig {
    #[serde(default)]
    pub targets: HashMap<String, ReleaseTargetConfig>,
}

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
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

#[derive(schemars::JsonSchema, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ReleaseCompletion {
    Tag,
    Workflow,
    GithubRelease,
}

/// Repository merge policy; GitHub's allowed methods remain authoritative.
#[derive(schemars::JsonSchema, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// One synchronization destination for the repository's local planning.
/// Repository configuration overrides the personal default as one value.
/// Every remote provider names the team that owns the repository's Tasks.
#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(schemars::JsonSchema, Clone, Serialize, Deserialize)]
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

pub use catalog::{Config, ContextConfig};

#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutomationConfig {
    pub retries: u32,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self { retries: 1 }
    }
}

#[derive(schemars::JsonSchema, Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogConfig {
    #[serde(default)]
    pub linear_requests: bool,
}

impl Config {
    /// Resolve authored preferences, or the sole installed harness. A
    /// `default_agent` list (`claude,codex`) selects its first entry.
    ///
    /// # Errors
    /// Returns `AgentSelection` when no preference resolves and installation is ambiguous or absent.
    pub fn agent(&self) -> Result<&str, LoadError> {
        select_agent(
            self.agent
                .as_deref()
                .or_else(|| self.default_agent.as_deref().map(first_agent)),
        )
    }

    /// The agents a headless step may fall through to when every account for
    /// its own is held back: the rest of the `default_agent` list, in order.
    /// An explicit `agent` choice has none; it is followed within itself.
    pub fn fallback_agents(&self) -> Vec<String> {
        match (&self.agent, &self.default_agent) {
            (None, Some(list)) => agent_list(list).skip(1).map(str::to_owned).collect(),
            _ => Vec::new(),
        }
    }
}

/// The entries of an ordered agent preference, `harness[:model]` each.
pub fn agent_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|agent| !agent.is_empty())
}

fn first_agent(value: &str) -> &str {
    agent_list(value).next().unwrap_or(value)
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
    let content = match observation::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if content.trim().is_empty() {
        return Ok(None);
    }
    let mut value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).map_err(|e| {
        LoadError::InvalidGraph(format!("YAML parse error in {}: {}", path.display(), e))
    })?;
    fold_linear_team(&mut value);
    normalize_context(&mut value);
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

/// Temporary reader for preferences shared with the released flat format.
/// Task 807ef7 removes this after installed readers migrate.
pub(crate) fn normalize_context(value: &mut serde_yaml_ng::Value) {
    use serde_yaml_ng::{Mapping, Value};
    let Some(map) = value.as_mapping_mut() else {
        return;
    };
    let mut context = match map.remove("context") {
        Some(Value::Mapping(context)) => context,
        Some(Value::Sequence(paths)) => {
            let mut docs = map
                .remove("docs")
                .unwrap_or_else(|| Value::Sequence(vec![]));
            if let Some(docs) = docs.as_sequence_mut() {
                docs.extend(paths);
            }
            Mapping::from_iter([("docs".into(), docs)])
        }
        Some(other) => {
            map.insert("context".into(), other);
            return;
        }
        None => Mapping::new(),
    };
    for (old, new) in [
        ("docs", "docs"),
        ("exclude", "exclude"),
        ("diff", "diff"),
        ("diff_files", "diff_files"),
        ("paste", "paste"),
        ("context_budgets", "budgets"),
    ] {
        if let Some(value) = map.remove(old) {
            context.entry(new.into()).or_insert(value);
        }
    }
    if !context.is_empty() {
        map.insert("context".into(), context.into());
    }
    for name in ["skills", "waves", "repos"] {
        if let Some(targets) = map.get_mut(name).and_then(Value::as_mapping_mut) {
            for value in targets.values_mut() {
                normalize_context(value);
            }
        }
    }
    if let Some(graphs) = map.get_mut("graphs").and_then(Value::as_mapping_mut) {
        for graph in graphs.values_mut() {
            if let Some(steps) = graph.get_mut("steps").and_then(Value::as_mapping_mut) {
                for value in steps.values_mut() {
                    normalize_context(value);
                }
            }
        }
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
    settings::resolve_layers(&layers, None).map(|resolved| Some(resolved.config))
}

/// The effective planning transport, resolved once per bounded planning hook.
pub fn load_planning_transport(repo_root: &Path) -> Result<PlanningConfig, LoadError> {
    Ok(load_config(Some(repo_root))?
        .unwrap_or_default()
        .planning_transport())
}

/// One present config file's values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigDeclarations {
    pub source: ConfigSource,
    pub scope: settings::SettingScope,
    pub path: PathBuf,
    pub value: serde_yaml_ng::Value,
}

/// Collect authored repository, Wave and personal-file declarations. The resolver orders them by source, scope and target.
pub fn config_layers(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
    config_layers_with(repo_root, wave, &PersonalSettings::read(repo_root, wave))
}

fn config_layers_with(
    repo_root: Option<&Path>,
    wave: Option<&str>,
    personal_values: &PersonalSettings,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
    let wave = match (repo_root, wave) {
        (Some(repo), Some(wave)) => Some(
            files::wave_name(repo, wave)
                .map_err(|error| LoadError::InvalidGraph(error.to_string()))?,
        ),
        (_, wave) => wave.map(str::to_owned),
    };
    let wave = wave.as_deref();
    let mut layers = Vec::new();
    if let Some(repo_root) = repo_root {
        let repo_path = repo_root.join(".lf/config.yaml");
        if let Some(value) = load_yaml_file(&repo_path)? {
            layers.push(ConfigDeclarations {
                source: ConfigSource::Repository,
                scope: settings::SettingScope::Repository,
                path: repo_path,
                value,
            });
        }
        if let Some(wave) = wave {
            if let Some((path, value)) = crate::work::wave::config::read_wave_layer(repo_root, wave)
                .map_err(|error| LoadError::InvalidGraph(error.to_string()))?
            {
                layers.push(ConfigDeclarations {
                    source: ConfigSource::Repository,
                    scope: settings::SettingScope::Wave,
                    path,
                    value,
                });
            }
        }
    }
    layers.extend(
        personal_values
            .0
            .as_ref()
            .map_err(|message| LoadError::InvalidGraph(message.clone()))?
            .clone(),
    );
    Ok(layers)
}

/// Personal file declarations cached for one bounded Work reading.
#[derive(Debug, Clone)]
pub struct PersonalSettings(Result<Vec<ConfigDeclarations>, String>);

impl PersonalSettings {
    pub fn read(repo_root: Option<&Path>, wave: Option<&str>) -> Self {
        Self(personal_file_layers(repo_root, wave).map_err(|error| error.to_string()))
    }
}

pub(crate) fn private_wave_layers(
    repo: &Path,
    name: Option<&str>,
    path: &Path,
    document: &serde_yaml_ng::Value,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
    let Some(name) = name.filter(|_| document.get("waves").is_some()) else {
        return Ok(vec![]);
    };
    let mut layers = Vec::new();
    let parts: Vec<_> = name.split('/').collect();
    for end in 1..=parts.len() {
        let selected = parts[..end].join("/");
        let Some(id) = files::wave_key(repo, &selected)
            .map_err(|error| LoadError::InvalidGraph(error.to_string()))?
        else {
            continue;
        };
        if let Some(mut value) = document
            .get("waves")
            .and_then(|waves| waves.get(&id))
            .cloned()
        {
            if end < parts.len() {
                if let Some(map) = value.as_mapping_mut() {
                    map.retain(|key, _| {
                        key.as_str()
                            .is_some_and(|key| ["machine", "account"].contains(&key))
                    });
                }
            }
            layers.push(ConfigDeclarations {
                source: ConfigSource::Personal,
                scope: settings::SettingScope::Wave,
                path: format!("{}#waves.{id}", path.display()).into(),
                value,
            });
        }
    }
    Ok(layers)
}

fn personal_file_layers(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
    let wave = match (repo_root, wave) {
        (Some(repo), Some(wave)) => Some(
            files::wave_name(repo, wave)
                .map_err(|error| LoadError::InvalidGraph(error.to_string()))?,
        ),
        (_, wave) => wave.map(str::to_owned),
    };
    let wave = wave.as_deref();
    if let Some(layers) = remote::layers(repo_root, wave)
        .map_err(|error| LoadError::InvalidGraph(error.to_string()))?
    {
        return Ok(layers);
    }
    let mut layers = Vec::new();
    let path = global_config_path();
    if let Some(value) = load_yaml_file(&path)? {
        if value.get("repos").is_some() {
            return Err(LoadError::InvalidGraph(format!(
                "{}: repos entries need migration to lf.config.local.yaml; run `lf config migrate`",
                path.display()
            )));
        }
        layers.push(ConfigDeclarations {
            source: ConfigSource::Personal,
            scope: settings::SettingScope::Global,
            path,
            value,
        });
    }
    if let Some(repo) = repo_root {
        let path = files::private_path(repo)
            .map_err(|error| LoadError::InvalidGraph(error.to_string()))?;
        files::diagnose_private_file(&path)
            .map_err(|error| LoadError::InvalidGraph(error.to_string()))?;
        if let Some(mut value) = load_yaml_file(&path)? {
            let waves = private_wave_layers(repo, wave, &path, &value)?;
            if let Some(map) = value.as_mapping_mut() {
                map.remove("waves");
            }
            layers.push(ConfigDeclarations {
                source: ConfigSource::Personal,
                scope: settings::SettingScope::Repository,
                path: path.clone(),
                value,
            });
            layers.extend(waves);
        }
    }
    Ok(layers)
}

/// The released name of `graph:`; LOO-637 removes it.
const GRAPH_ALIAS: &str = "workflow";

/// Keys a Wave's `config.yaml` may set: every `Config` key a repository may
/// share with its Waves.
pub fn wave_config_keys() -> BTreeSet<String> {
    catalog::DEFINITIONS
        .iter()
        .filter(|definition| {
            definition.applies(
                ConfigSource::Repository,
                settings::SettingScope::Wave,
                false,
            )
        })
        .map(|definition| definition.key.to_owned())
        .chain(["skills".into(), "graphs".into()])
        .collect()
}

/// Load only Machine-local user configuration.
pub fn load_global_config() -> Result<Option<Config>, LoadError> {
    load_wave_config(None, None)
}

/// Repository lookup belongs to the selected Machine, never repository config.
pub fn repository_root() -> Result<Option<PathBuf>, LoadError> {
    Ok(load_global_config()?.and_then(|config| config.repo_root))
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
    fn private_wave_reads_allow_missing_identity_but_report_invalid_authored_config() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path().join("lf.config.local.yaml");
        let document = serde_yaml_ng::from_str("waves: {}\n").unwrap();
        assert!(
            super::private_wave_layers(repo.path(), Some("missing"), &path, &document)
                .unwrap()
                .is_empty()
        );
        let wave = repo.path().join(".lf/wave/broken");
        std::fs::create_dir_all(&wave).unwrap();
        std::fs::write(wave.join("config.yaml"), "id: [invalid\n").unwrap();
        assert!(super::private_wave_layers(repo.path(), Some("broken"), &path, &document).is_err());
    }

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
        assert_eq!(installed_agent(|_| true), None);
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
        assert_eq!(config.agent().unwrap(), "codex");
        assert!(config.supported_harnesses.is_empty());
        assert!(!config.yolo);
        assert!(config.context.docs.is_empty());
        assert!(!config.context.diff_files);
        assert!(!config.context.diff);
        assert_eq!(config.chrome, None);
        assert!(!config.pr);
        assert_eq!(config.land, "gh");
        assert!(config.context.exclude.is_empty());
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
        assert_eq!(config.agent().unwrap(), "codex:o3");
        assert!(config.yolo);
        assert_eq!(config.chrome, Some(true));
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
  docs:
    - src/
    - tests/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.context.docs, vec!["src/", "tests/"]);
    }

    #[test]
    fn config_from_yaml_docs_as_list() {
        let yaml = r#"
context:
  docs:
    - README.md
    - docs/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(config.context.docs, vec!["README.md", "docs/"]);
    }

    #[test]
    fn config_from_yaml_exclude_as_list() {
        let yaml = r#"
context:
  exclude:
    - "*.log"
    - build/
    - node_modules/
"#;
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(
            config.context.exclude,
            vec!["*.log", "build/", "node_modules/"]
        );
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
    fn detached_sessions_end_after_two_minutes_unless_configured() {
        use super::DetachedLimit;
        use std::time::Duration;
        let parse = |yaml: &str| serde_yaml_ng::from_str::<Config>(yaml).map(|c| c.session);
        assert_eq!(
            parse("agent: claude").unwrap().end_detached_after,
            DetachedLimit::After(Duration::from_secs(120))
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
        assert_eq!(config.summaries[0].agent.as_deref(), Some("claude"));
        assert_eq!(config.summaries[1].path, "tests/");
        assert_eq!(config.summaries[1].agent, None);
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
    fn skill_settings_merge_per_skill_and_a_wave_layer_rejects_repository_keys() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".lf/wave/infra")).unwrap();
        std::fs::write(
            repo.path().join(".lf/config.yaml"),
            "agent: codex\nskills:\n  implement: {agent: codex:o3}\n  design: {agent: claude}\n",
        )
        .unwrap();
        let wave = repo.path().join(".lf/wave/infra/config.yaml");
        std::fs::write(&wave, "skills:\n  implement: {agent: claude:opus}\n").unwrap();
        let config = load_wave_config(Some(repo.path()), Some("infra"))
            .unwrap()
            .unwrap();
        assert_eq!(config.agent.as_deref(), Some("codex"));
        assert_eq!(
            super::settings::resolve(&config, Some(&crate::graph::Skill::named("implement")))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude:opus"
        );
        assert_eq!(
            super::settings::resolve(&config, Some(&crate::graph::Skill::named("design")))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude"
        );
        std::fs::write(&wave, "workflow: code\n").unwrap();
        assert_eq!(
            load_wave_config(Some(repo.path()), Some("infra"))
                .unwrap()
                .unwrap()
                .graph
                .as_deref(),
            Some("code")
        );
        for invalid in ["planning: {provider: git}\n", "agnet: claude\n"] {
            std::fs::write(&wave, invalid).unwrap();
            let error = load_wave_config(Some(repo.path()), Some("infra")).unwrap_err();
            assert!(error.to_string().contains(wave.to_str().unwrap()));
        }
        assert!(load_config(Some(repo.path())).is_ok());
        let repo_config = repo.path().join(".lf/config.yaml");
        std::fs::write(&repo_config, "session: {terminal: ghostty}\n").unwrap();
        let error = load_config(Some(repo.path())).unwrap_err().to_string();
        assert!(error.contains("does not apply"));
        assert!(error.contains(repo_config.to_str().unwrap()));
    }

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
        let _home = crate::journal::TestLedgerGuard::new();
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
        let _home = crate::journal::TestLedgerGuard::new();
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
    use super::{Config, GitPlan, PlanningConfig};

    #[test]
    fn planning_variants_keep_their_typed_defaults() {
        let config: Config =
            serde_yaml_ng::from_str("planning: {provider: linear, team: team}").unwrap();
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
