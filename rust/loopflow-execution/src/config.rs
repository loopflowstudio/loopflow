//! Configuration loading for loopflow.
//!
//! Loads config from `~/.lf/config.yaml` (global), `.lf/config.yaml` (repo) and,
//! for work in a Wave, `.lf/wave/<name>/config.yaml`. Settings resolve by source,
//! then scope and target; see `settings` for precedence and provenance.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::LoadError;
use loopflow_harness::HarnessKind;

pub mod catalog;
mod definitions;
pub mod environment;
pub mod files;
pub mod keys;
pub mod migrate;
pub mod placement;
pub mod reader;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum CodexServiceTier {
    #[serde(alias = "standard")]
    Default,
    #[serde(alias = "fast")]
    Priority,
}

impl CodexServiceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Priority => "priority",
        }
    }
}

/// Resolve a display name from personal Loopflow configuration or Git's
/// user.name in `directory`.
fn load_user_name(directory: &Path) -> Result<Option<String>, LoadError> {
    if let Some(name) = load_global_config()?
        .and_then(|config| config.user.name)
        .as_deref()
        .and_then(loopflow_model::person::normalize_name)
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
        .and_then(loopflow_model::person::normalize_name))
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

/// Claude model families a launch can name with `claude:<family>` and a
/// scoped Claude limit can name in its `rate_limit_type` suffix.
pub const CLAUDE_MODEL_FAMILIES: [&str; 4] = ["fable", "opus", "sonnet", "haiku"];

/// The family Claude Code runs when a launch names no model.
pub const CLAUDE_DEFAULT_MODEL_FAMILY: &str = "fable";

/// An explicit invocation-wide choice; definitions and defaults never set it.
pub const YOLO_OVERRIDE_ENV: &str = "LF_YOLO_OVERRIDE";
pub const CHROME_OVERRIDE_ENV: &str = "LF_CHROME_OVERRIDE";

pub const AGENT_OVERRIDE_ENV: &str = "LF_AGENT_OVERRIDE";

pub fn agent_override() -> Option<String> {
    settings::explicit_agent().ok().flatten()
}

/// How a recorded conversation continues under the invocation's agent choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resume {
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
    let is_installed = |agent: HarnessKind| agent == HarnessKind::Codex;
    #[cfg(not(test))]
    let is_installed = |kind: HarnessKind| crate::harness::launcher(kind).installed();
    installed_agent(is_installed)
}

fn installed_agent(is_installed: impl Fn(HarnessKind) -> bool) -> Option<&'static str> {
    let mut installed = HarnessKind::ALL
        .into_iter()
        .filter(|agent| is_installed(*agent))
        .map(HarnessKind::as_str);
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
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
    // Remove in task_c118380f31f14d07ba28669499efa395 after LOO-823 ships:
    // released lf still reads this shared config key. It no longer has an effect.
    #[serde(default, rename = "end_detached_after", skip_serializing)]
    #[schemars(skip)]
    _end_detached_after: Option<String>,
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

/// `auto: on`: each Task turns auto on as it takes up its Graph, unless
/// someone set it first. It has no other value; leave it out for none.
#[derive(schemars::JsonSchema, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoDefault {
    On,
}

/// Repository merge policy; GitHub's allowed methods remain authoritative.
#[derive(schemars::JsonSchema, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum MergeMethod {
    Merge,
    Squash,
    Rebase,
}

/// The repository's tracker. Without one the plan is in Loopflow's store,
/// on this Machine's hub when it has one. Every tracker names the team that
/// owns the repository's Tasks.
#[derive(schemars::JsonSchema, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "RawTrackerConfig", into = "RawTrackerConfig")]
pub enum TrackerConfig {
    /// The Linear Team's ID; `lf wave plan connect` finds or creates it.
    Linear { team: Option<String> },
    /// The Asana team's gid or URL. Tasks live in the team project named
    /// after the repository; Wave Projects are Asana projects in the team.
    Asana { team: String },
    /// `provider: git`, which the released `lf` still writes and reads: no
    /// tracker here. LOO-973 removes it once that release is gone.
    ReleasedGit,
}

#[derive(schemars::JsonSchema, Clone, Serialize, Deserialize)]
// Released shape until LOO-784 writes canonical spellings.
#[serde(tag = "provider", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(!tag)]
enum RawTrackerConfig {
    /// Read as no tracker; its keys are accepted and ignored.
    Git {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        remote: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shared: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        personal: bool,
    },
    /// The Linear Team's ID; `lf wave plan connect` finds or creates it.
    Linear {
        #[serde(
            default,
            deserialize_with = "loopflow_model::id::deserialize_uuid_option"
        )]
        team: Option<String>,
    },
    /// The Asana team's gid or URL. Tasks live in the team project named
    /// after the repository; Wave Projects are Asana projects in the team.
    Asana { team: String },
}

impl From<RawTrackerConfig> for TrackerConfig {
    fn from(raw: RawTrackerConfig) -> Self {
        match raw {
            RawTrackerConfig::Git { .. } => Self::ReleasedGit,
            RawTrackerConfig::Linear { team } => Self::Linear { team },
            RawTrackerConfig::Asana { team } => Self::Asana { team },
        }
    }
}

impl From<TrackerConfig> for RawTrackerConfig {
    fn from(config: TrackerConfig) -> Self {
        match config {
            TrackerConfig::ReleasedGit => Self::Git {
                remote: None,
                shared: None,
                personal: false,
            },
            TrackerConfig::Linear { team } => Self::Linear { team },
            TrackerConfig::Asana { team } => Self::Asana { team },
        }
    }
}

impl TrackerConfig {
    /// The tracker's product name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Linear { .. } => "Linear",
            Self::Asana { .. } => "Asana",
            Self::ReleasedGit => unreachable!("planning_transport reads provider: git as None"),
        }
    }
}

impl Config {
    /// Linear or Asana; `None` when the plan is in Loopflow's store.
    pub fn planning_transport(&self) -> Option<TrackerConfig> {
        self.planning
            .clone()
            .filter(|tracker| *tracker != TrackerConfig::ReleasedGit)
    }
}

pub use catalog::{Config, ContextConfig, ForwardConfig};

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
    /// Resolve authored preferences, or the sole installed harness: an `agent`
    /// choice, else the first of [`Config::agent_order`].
    ///
    /// # Errors
    /// Returns `AgentSelection` when no preference resolves and installation is ambiguous or absent.
    pub fn agent(&self) -> Result<&str, LoadError> {
        select_agent(
            self.agent
                .as_deref()
                .or(self.default_agent.as_deref())
                .or_else(|| self.agents.first().map(String::as_str)),
        )
    }

    /// What Loopflow tries, in order: an `agent` choice alone, else
    /// `default_agent` followed by the rest of `agents`. A `default_agent`
    /// the list lacks is added in front for this context only. Entries are
    /// `harness[:model]` as written; a bare harness is its own entry.
    pub fn agent_order(&self) -> Vec<String> {
        if let Some(agent) = &self.agent {
            return vec![agent.clone()];
        }
        self.default_agent
            .iter()
            .chain(
                self.agents
                    .iter()
                    .filter(|entry| Some(*entry) != self.default_agent.as_ref()),
            )
            .cloned()
            .collect()
    }

    /// Resolve launch choices before provider effects, including configured models.
    pub fn resolve_agents(&self) -> Result<(Agent, Vec<Agent>), LoadError> {
        let first = Agent::parse(self.agent()?)?;
        let mut fallback: Vec<Agent> = self
            .fallback_agents()
            .iter()
            .map(|agent| Agent::parse(agent).map(|agent| agent.with_configured_model(self)))
            .collect::<Result<_, _>>()?;
        if fallback.is_empty() && first.model.is_none() {
            fallback.extend(
                first
                    .harness
                    .harness()
                    .model_rungs()
                    .iter()
                    .map(|model| Agent {
                        harness: first.harness,
                        model: Some((*model).to_owned()),
                    }),
            );
        }
        let first = first.with_configured_model(self);
        fallback.retain(|agent| agent != &first);
        Ok((first, fallback))
    }

    /// The agents headless work may move to when a usage limit refuses its
    /// own: the rest of [`Config::agent_order`]. An `agent` choice has none.
    pub fn fallback_agents(&self) -> Vec<String> {
        self.agent_order().into_iter().skip(1).collect()
    }
}

/// The entries of a comma-separated agent list, such as `claude,codex`.
pub(crate) fn agent_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split(',')
        .map(str::trim)
        .filter(|agent| !agent.is_empty())
}

/// A selected harness and its provider model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agent {
    pub harness: HarnessKind,
    pub model: Option<String>,
}

impl Agent {
    pub fn parse(value: &str) -> Result<Self, LoadError> {
        let (name, model) = parse_agent(value);
        let harness =
            HarnessKind::parse(&name).ok_or_else(|| LoadError::InvalidAgent(value.to_owned()))?;
        Ok(Self { harness, model })
    }

    pub fn with_configured_model(mut self, config: &Config) -> Self {
        if self.harness == HarnessKind::Claude && self.model.is_none() {
            self.model = config.claude.model.clone();
        }
        self
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.harness.as_str())?;
        if let Some(model) = &self.model {
            write!(f, ":{model}")?;
        }
        Ok(())
    }
}

/// Parse agent string like 'claude:opus' into (harness, model). An empty
/// model (`claude:`) leaves the choice to the provider.
pub fn parse_agent(agent: &str) -> (String, Option<String>) {
    match agent.split_once(':') {
        Some((harness, model)) => (
            harness.to_string(),
            Some(model.trim())
                .filter(|model| !model.is_empty())
                .map(str::to_string),
        ),
        None => (agent.to_string(), None),
    }
}

/// Load YAML file, returning None if not present or empty.
fn load_yaml_file(path: &Path) -> Result<Option<serde_yaml_ng::Value>, LoadError> {
    let content = match crate::observe::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if content.trim().is_empty() {
        return Ok(None);
    }
    let mut value = serde_yaml_ng::Value::Mapping(reader::parse(&content, path));
    keys::fold(&mut value);
    normalize_context(&mut value);
    Ok(Some(value))
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
    // Released lf wrote scratch size targets here; they selected nothing and
    // are gone. A file that still sets them loads without them.
    if let Some(budgets) = context.get_mut("budgets").and_then(Value::as_mapping_mut) {
        for retired in ["scratch_tokens", "scratch_bytes"] {
            budgets.remove(retired);
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
    Ok(Some(resolved(repo_root, wave)?.config.clone()))
}

/// Settings for work in a checkout and Wave. Served readings share one value
/// until a file it came from changes; a bare command resolves for itself.
pub fn resolved(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Arc<settings::ResolvedSettings>, LoadError> {
    resolved_with(repo_root, repo_root, wave, &settings::outside()?)
}

/// [`resolved`] for a Task: checked-in files come from its checkout, your own
/// file from its repository, which is there before the checkout and after it.
pub fn resolved_for_task(
    checkout: &Path,
    repository: &Path,
    wave: Option<&str>,
) -> Result<Arc<settings::ResolvedSettings>, LoadError> {
    resolved_with(
        Some(checkout),
        Some(repository),
        wave,
        &settings::outside()?,
    )
}

/// [`resolved`] inside a computation whose key already holds `outside`.
/// `repository` is the checkout your settings file is found from.
pub(crate) fn resolved_with(
    repo_root: Option<&Path>,
    repository: Option<&Path>,
    wave: Option<&str>,
    outside: &settings::Outside,
) -> Result<Arc<settings::ResolvedSettings>, LoadError> {
    // A Wave named by ID is found by walking the checkout, which no reading
    // observes: the shared value is kept under the name.
    let wave = selected_wave(repo_root, wave)?;
    let key = crate::observe::Derived::Settings {
        checkout: repo_root.map(Path::to_owned),
        repository: repository.map(Path::to_owned),
        wave: wave.clone(),
        outside: outside.clone(),
    };
    outside.shared(key, || {
        let layers = file_layers(
            repo_root,
            repository,
            wave.as_deref(),
            &outside.home,
            outside.remote.as_deref(),
        )?;
        settings::resolve_with(&layers, None, outside).map(Arc::new)
    })
}

fn selected_wave(
    repo_root: Option<&Path>,
    wave: Option<&str>,
) -> Result<Option<String>, LoadError> {
    match (repo_root, wave) {
        (Some(repo), Some(wave)) => files::wave_name(repo, wave)
            .map(Some)
            .map_err(|error| LoadError::InvalidGraph(error.to_string())),
        (_, wave) => Ok(wave.map(str::to_owned)),
    }
}

/// The repository's tracker, resolved once per bounded planning hook.
pub fn load_planning_transport(repo_root: &Path) -> Result<Option<TrackerConfig>, LoadError> {
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
    let wave = selected_wave(repo_root, wave)?;
    file_layers(
        repo_root,
        repo_root,
        wave.as_deref(),
        &crate::store::lf_home_dir(),
        remote::incoming_text().as_deref(),
    )
}

/// The checkout's `.lf/config.yaml`, the named Wave's own file, then your
/// settings files under `home` and beside `repository`.
fn file_layers(
    repo_root: Option<&Path>,
    repository: Option<&Path>,
    wave: Option<&str>,
    home: &Path,
    remote: Option<&str>,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
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
    layers.extend(personal_file_layers(repository, wave, home, remote)?);
    Ok(layers)
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
    home: &Path,
    remote: Option<&str>,
) -> Result<Vec<ConfigDeclarations>, LoadError> {
    if let Some(profile) = remote {
        return remote::layers(profile, home, repo_root, wave)
            .map_err(|error| LoadError::InvalidGraph(error.to_string()));
    }
    let mut layers = Vec::new();
    let path = home.join("config.yaml");
    if let Some(value) = load_yaml_file(&path)? {
        layers.push(ConfigDeclarations {
            source: ConfigSource::Personal,
            scope: settings::SettingScope::Global,
            path,
            value,
        });
    }
    if let Some(repo) = repo_root {
        // Where the personal file lives depends on whether `repo` is a
        // checkout yet: a worktree that appears later moves it.
        crate::observe::exists(&repo.join(".git"));
        let path = files::private_path(repo)
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

pub(crate) fn global_config_path() -> PathBuf {
    crate::store::lf_home_dir().join("config.yaml")
}

/// Load only the repository-owned config, without inheriting user-global values.
///
/// Linear Team bindings use this path; personal transport defaults do not assign
/// a repository to a Team.
pub fn load_repo_config(repo_root: &Path) -> Result<Option<Config>, LoadError> {
    let path = repo_root.join(".lf/config.yaml");
    let Some(mut value) = load_yaml_file(&path)? else {
        return Ok(None);
    };
    settings::filter_file_preferences(
        &mut value,
        &path,
        ConfigSource::Repository,
        settings::SettingScope::Repository,
    );
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
    fn private_wave_reads_skip_malformed_entries_but_validate_authored_identity() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo.path().join(".lf/config.local.yaml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let document = serde_yaml_ng::from_str("waves: {}\n").unwrap();
        assert!(
            super::private_wave_layers(repo.path(), Some("missing"), &path, &document)
                .unwrap()
                .is_empty()
        );
        let wave = repo.path().join(".lf/wave/broken");
        std::fs::create_dir_all(&wave).unwrap();
        std::fs::write(wave.join("config.yaml"), "id: [invalid\n").unwrap();
        assert!(
            super::private_wave_layers(repo.path(), Some("broken"), &path, &document)
                .unwrap()
                .is_empty()
        );
        std::fs::write(wave.join("config.yaml"), "id: invalid\n").unwrap();
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
        assert_eq!(
            installed_agent(|agent| agent == HarnessKind::Claude),
            Some("claude")
        );
        assert_eq!(installed_agent(|_| true), None);
        assert_eq!(installed_agent(|_| false), None);
    }

    #[test]
    fn resolved_choices_keep_configured_models_and_fallback_order() {
        let mut config = Config {
            agent: Some("claude".into()),
            ..Config::default()
        };
        config.claude.model = Some("sonnet".into());
        let (agent, fallback) = config.resolve_agents().unwrap();
        assert_eq!(agent.to_string(), "claude:sonnet");
        assert_eq!(
            fallback.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["claude:opus", "claude:haiku"]
        );
        config.agent = Some("claude:fable".into());
        assert!(config.resolve_agents().unwrap().1.is_empty());
        config.agent = None;
        config.default_agent = Some("codex".into());
        config.agents = vec!["codex".into(), "claude".into(), "pi:fixture/model".into()];
        let (_, fallback) = config.resolve_agents().unwrap();
        assert_eq!(
            fallback.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["claude:sonnet", "pi:fixture/model"]
        );
        config.agent = Some("unknown:model".into());
        assert!(matches!(
            config.resolve_agents(),
            Err(LoadError::InvalidAgent(_))
        ));
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
        for harness in HarnessKind::ALL.map(HarnessKind::as_str) {
            assert_eq!(parse_agent(harness), (harness.to_string(), None));
        }
    }

    #[test]
    fn an_empty_model_defers_to_the_provider() {
        assert_eq!(parse_agent("claude:"), ("claude".to_string(), None));
        assert_eq!(Agent::parse("codex: ").unwrap().to_string(), "codex");
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
            Some(TrackerConfig::Linear {
                team: Some(team.to_string())
            })
        );
    }

    #[test]
    fn config_parses_planning_team() {
        let yaml = "planning:\n  provider: asana\n  team: \"9876543210\"\n";
        let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
        assert_eq!(
            config.planning_transport(),
            Some(TrackerConfig::Asana {
                team: "9876543210".to_string(),
            })
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
        assert!(config.yolo);
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
    fn default_agent_leads_the_agents_order_and_a_choice_pins_one() {
        let order = |yaml: &str| {
            let config: Config = serde_yaml_ng::from_str(yaml).expect("parse config");
            (config.agent().unwrap().to_owned(), config.agent_order())
        };
        assert_eq!(
            order("agents: [claude:opus, codex, claude:fable]"),
            (
                "claude:opus".into(),
                vec!["claude:opus".into(), "codex".into(), "claude:fable".into()]
            )
        );
        // An entry the list has moves up; one it lacks is added in front.
        assert_eq!(
            order("agents: [claude:opus, codex, claude:fable]\ndefault_agent: claude:fable").1,
            ["claude:fable", "claude:opus", "codex"]
        );
        assert_eq!(
            order("agents: [claude, codex]\ndefault_agent: codex:sol").1,
            ["codex:sol", "claude", "codex"]
        );
        assert_eq!(order("default_agent: claude").1, ["claude"]);
        let pinned: Config =
            serde_yaml_ng::from_str("agents: [claude, codex]\ndefault_agent: codex\nagent: pi")
                .unwrap();
        assert_eq!(pinned.agent_order(), ["pi"]);
        assert!(pinned.fallback_agents().is_empty());
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
    fn obsolete_detached_limit_is_readable_but_not_written() {
        for value in ["2m", "off"] {
            let config: super::SessionConfig = serde_yaml_ng::from_str(&format!(
                "terminal: Ghostty\nend_detached_after: {value}\n"
            ))
            .unwrap();
            assert_eq!(config.terminal.as_deref(), Some("Ghostty"));
            assert!(!serde_yaml_ng::to_string(&config)
                .unwrap()
                .contains("end_detached_after"));
        }
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
    fn skill_settings_merge_per_skill_and_a_wave_layer_ignores_repository_keys() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
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
        std::fs::write(&wave, "workflow: review\n").unwrap();
        assert_eq!(
            load_wave_config(Some(repo.path()), Some("infra"))
                .unwrap()
                .unwrap()
                .graph
                .as_deref(),
            Some("review")
        );
        std::fs::write(&wave, "planning: {provider: git}\n").unwrap();
        assert!(load_wave_config(Some(repo.path()), Some("infra")).is_ok());
        std::fs::write(&wave, "agnet: claude\n").unwrap();
        assert_eq!(
            load_wave_config(Some(repo.path()), Some("infra"))
                .unwrap()
                .unwrap()
                .agent()
                .unwrap(),
            "codex"
        );
        assert!(load_config(Some(repo.path())).is_ok());
        let repo_config = repo.path().join(".lf/config.yaml");
        std::fs::write(&repo_config, "session: {terminal: ghostty}\n").unwrap();
        assert!(load_config(Some(repo.path()))
            .unwrap()
            .unwrap()
            .session
            .terminal
            .is_none());
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
    use super::{Config, TrackerConfig};

    #[test]
    fn a_released_git_tracker_reads_as_no_tracker() {
        let config: Config =
            serde_yaml_ng::from_str("planning: {provider: linear, team: team}").unwrap();
        assert_eq!(
            config.planning_transport(),
            Some(TrackerConfig::Linear {
                team: Some("team".into())
            })
        );
        assert_eq!(Config::default().planning_transport(), None);
        for released in [
            "planning:\n  provider: git\n",
            "planning:\n  provider: git\n  personal: true\n",
            "planning:\n  provider: git\n  remote: upstream\n  shared: team\n",
        ] {
            let config: Config = serde_yaml_ng::from_str(released).unwrap();
            assert_eq!(config.planning_transport(), None, "{released}");
        }
        assert!(serde_yaml_ng::from_str::<Config>(
            "planning:\n  provider: linear\n  shared: employees\n"
        )
        .is_err());
    }
}
