//! Typed preferences, preserving source authority until an invocation is selected.
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use super::{Config, ConfigDeclarations, ConfigSource};
use crate::error::LoadError;
use crate::graph::Skill;
use crate::work::selection::{WorkWorkspaceKey, WorkWorkspaceKind};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SettingScope {
    Global,
    Repository,
    Wave,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingTarget {
    General,
    Skill { name: String },
    GraphOccurrence { graph: String, id: Option<String> },
}

/// A Graph occurrence retains its defining graph through composition and capture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OccurrenceSettings {
    pub graph: String,
    pub id: Option<String>,
    pub source: ConfigSource,
    pub path: Option<PathBuf>,
    pub address: Option<String>,
    #[schemars(with = "serde_json::Value")]
    pub values: Value,
}

impl OccurrenceSettings {
    pub(crate) fn address(&self) -> Option<String> {
        self.id
            .as_deref()
            .map(|id| crate::graph::occurrence_address(Some(id), ""))
            .or_else(|| self.address.clone())
    }

    pub(crate) fn capture(
        graph: &str,
        id: Option<&str>,
        layer: ConfigSource,
        skill: &Skill,
    ) -> Result<Self, LoadError> {
        Ok(Self {
            graph: graph.into(),
            id: id.map(str::to_owned),
            source: layer,
            path: None,
            address: None,
            // Bare references already contain the loaded skill. Its preferences
            // remain skill defaults, not declarations of this Graph occurrence.
            values: if skill.content.is_some() {
                Value::Mapping(Default::default())
            } else {
                serde_yaml_ng::to_value(metadata(&skill_values(skill))?)
                    .expect("settings serialize")
            },
        })
    }
}

/// All candidates are ordered from lowest to highest authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SettingCandidate {
    /// Authored key; agent and default_agent retain their choice strength.
    pub key: String,
    pub locator: String,
    pub reason: String,
    pub source: ConfigSource,
    pub scope: SettingScope,
    pub target: SettingTarget,
    #[schemars(with = "serde_json::Value")]
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EffectiveSetting {
    #[schemars(with = "serde_json::Value")]
    pub value: Value,
    pub candidates: Vec<SettingCandidate>,
}

impl EffectiveSetting {
    pub fn selected(&self) -> Option<&SettingCandidate> {
        self.candidates
            .iter()
            .find(|candidate| candidate.reason == "selected")
    }
}

#[derive(Debug)]
pub struct ResolvedSettings {
    pub config: Config,
    pub settings: BTreeMap<String, EffectiveSetting>,
}

fn invalid(message: impl Into<String>) -> LoadError {
    LoadError::InvalidGraph(message.into())
}

pub fn validate_scope(
    key: &str,
    layer: ConfigSource,
    scope: SettingScope,
) -> Result<(), LoadError> {
    let key = &super::keys::typed_key(key);
    let preference = preference_path(key)?;
    let definition = super::catalog::definition(&preference.join(".")).expect("validated key");
    if !definition.applies(layer, scope, preference.len() != key.split('.').count()) {
        return Err(invalid(format!("{key} does not apply at {layer:?}/{scope:?}; inspect `lf config catalog` for supported destinations")));
    }
    Ok(())
}

/// Return the preference path after its optional skill/occurrence selector;
/// `key` may use any spelling `config::keys` accepts.
pub fn preference_path(key: &str) -> Result<Vec<String>, LoadError> {
    let typed = super::keys::typed_key(key);
    let parts: Vec<_> = typed.split('.').collect();
    preference_parts(&parts).map(|path| path.iter().map(|part| (*part).to_owned()).collect())
}

pub(crate) fn preference_parts<'a, 'b>(parts: &'a [&'b str]) -> Result<&'a [&'b str], LoadError> {
    let key = parts.join(".");
    if parts.iter().any(|part| part.is_empty()) {
        return Err(invalid(format!("invalid setting key {key}")));
    }
    let path = match parts {
        ["skills", _, rest @ ..] if !rest.is_empty() => rest,
        ["graphs", _, "steps", _, rest @ ..] if !rest.is_empty() => rest,
        ["skills" | "graphs", ..] => {
            return Err(invalid(
                "name a setting under skills.<name> or graphs.<name>.steps.<id>",
            ))
        }
        _ => parts,
    };
    let definition = super::catalog::definition(&path.join("."))
        .ok_or_else(|| invalid(format!("unknown setting {key}")))?;
    if !definition.accepts(path)
        || (path.len() != parts.len()
            && definition.applicability != super::catalog::Applicability::Launch)
    {
        return Err(invalid(format!("unknown or ineligible setting {key}")));
    }
    Ok(path)
}

/// CLI and file values use the same YAML types as configuration files.
pub fn parse_value(key: &str, text: &str) -> Result<Value, LoadError> {
    let path = preference_path(key)?;
    let path: Vec<&str> = path.iter().map(String::as_str).collect();
    let mut value: Value =
        serde_yaml_ng::from_str(text).map_err(|error| invalid(error.to_string()))?;
    if path[0] == "agents" {
        _list_agents(&mut value);
    }
    validate_value(&path, &value)?;
    Ok(value)
}

/// `agents: claude,codex` is the list `[claude, codex]`.
fn _list_agents(value: &mut Value) {
    if let Some(list) = value.as_str() {
        *value = Value::Sequence(super::agent_list(list).map(Value::from).collect());
    }
}

pub(crate) fn validate_value(path: &[&str], value: &Value) -> Result<(), LoadError> {
    if value.is_null() {
        return Err(invalid(
            "omit a file value or use unset to remove a preference",
        ));
    }
    if ["agent", "default_agent"].contains(&path[0]) {
        let agent = value
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid("agent needs harness[:model]"))?;
        // The retired `default_agent: claude,codex` spelling still reads
        // until LOO-875 removes it.
        if path[0] == "default_agent" {
            for entry in super::agent_list(agent) {
                validate_agent_policy(entry)?;
            }
        } else {
            validate_agent_policy(agent)?;
        }
    }
    let mut value = value.clone();
    if path[0] == "agents" {
        _list_agents(&mut value);
        let entries = value
            .as_sequence()
            .ok_or_else(|| invalid("agents needs a list, such as claude,codex"))?;
        for entry in entries {
            validate_agent_policy(
                entry
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .ok_or_else(|| invalid("agents entries need harness[:model]"))?,
            )?;
        }
    }
    let mut check = super::catalog::defaults();
    insert(&mut check, path, value.clone());
    let parsed = serde_yaml_ng::from_value::<Config>(check)
        .map_err(|error| invalid(format!("{}: {error}", path.join("."))))?;
    if let Some(model) = &parsed.claude.model {
        if model.trim().is_empty() {
            return Err(invalid(
                "claude.model needs a model name, such as opus or fable",
            ));
        }
    }
    if let Some(stream) = parsed.stream.as_deref() {
        if !["stable", "preview", "dev"].contains(&stream) {
            return Err(invalid("stream must be stable, preview or dev"));
        }
    }
    // The same check a launch makes, so an unusable layout fails where it is written.
    if let Some(template) = &parsed.task_layout {
        template
            .instantiate(WorkWorkspaceKey {
                repository: String::new(),
                kind: WorkWorkspaceKind::Task,
                id: String::new(),
            })
            .map_err(|error| invalid(format!("task_layout: {error}")))?;
    }
    if !["gh", "local"].contains(&parsed.land.as_str()) {
        return Err(invalid("land must be gh or local"));
    }
    if parsed.machine.tasks.is_some() && parsed.machine.capacity.is_some() {
        return Err(invalid(
            "choose machine.tasks or machine.capacity in one declaration",
        ));
    }
    for provider in parsed.account.keys() {
        if !["claude", "codex"].contains(&provider.as_str()) {
            return Err(invalid("account routes require claude or codex"));
        }
    }
    for (provider, browsers) in &parsed.auth_browsers {
        provider
            .parse::<crate::provider_auth::Provider>()
            .map_err(|error| invalid(error.to_string()))?;
        for profiles in std::iter::once(&browsers.default).chain(browsers.accounts.values()) {
            let mut seen = std::collections::HashSet::new();
            for profile in profiles {
                crate::profile::ProfileId::parse(profile).map_err(invalid)?;
                if !seen.insert(profile) {
                    return Err(invalid("browser profiles must be unique"));
                }
            }
        }
    }
    for (name, template) in &parsed.ondemands {
        if name.trim().is_empty() || name.trim() != name || parsed.pools.contains_key(name) {
            return Err(invalid(
                "pools and on-demands require distinct nonempty names",
            ));
        }
        if !template.hours.is_finite()
            || template.hours <= 0.0
            || template.hours > crate::rental::MAX_HOURS
        {
            return Err(invalid("rental hours must be positive and at most 24"));
        }
        template
            .provider
            .offer(&template.hardware)
            .map_err(invalid)?;
    }
    for accounts in parsed.accounts.values() {
        for declaration in accounts.values() {
            if let Some(date) = &declaration.paid_through {
                super::placement::parse_date(date).map_err(|error| invalid(error.to_string()))?;
            }
        }
    }
    if path[0] == "context" {
        crate::prompt::context_budget::ContextBudgets::resolve(&parsed)
            .map_err(|error| invalid(error.to_string()))?;
    }
    if path[0] == "graph" {
        crate::task_graph::validate_graph_name(value.as_str().expect("validated graph string"))
            .map_err(|error| invalid(error.to_string()))?;
    }
    Ok(())
}

fn validate_agent_policy(agent: &str) -> Result<(), LoadError> {
    let (harness, variant) = super::parse_agent(agent);
    match harness.as_str() {
        "claude" | "codex" | "pi" => return Ok(()),
        "opencode" => {}
        _ => {
            return Err(invalid(format!(
                "unsupported agent harness '{harness}': use 'claude', 'codex', 'opencode', or 'pi'"
            )));
        }
    }

    let Some(variant) = variant else {
        return Ok(());
    };
    if is_supported_opencode_model_variant(&variant) {
        return Ok(());
    }

    Err(invalid(format!(
        "unsupported OpenCode model '{}': supported variants are 'opencode/*' (excluding claude/codex families) and 'moonshotai/kimi*'",
        variant
    )))
}

fn is_supported_opencode_model_variant(variant: &str) -> bool {
    let variant = variant.trim().to_ascii_lowercase();
    let Some((provider, model_id)) = variant.split_once('/') else {
        return false;
    };
    if model_id.is_empty() {
        return false;
    }

    match provider {
        "moonshotai" => model_id.starts_with("kimi"),
        "opencode" => !model_id.contains("claude") && !model_id.contains("codex"),
        _ => false,
    }
}

pub(crate) fn insert(root: &mut Value, path: &[&str], value: Value) {
    let Some((key, rest)) = path.split_first() else {
        *root = value;
        return;
    };
    if !root.is_mapping() {
        *root = Value::Mapping(Default::default());
    }
    let map = root.as_mapping_mut().expect("mapping initialized");
    insert(
        map.entry(Value::String((*key).into()))
            .or_insert(Value::Null),
        rest,
        value,
    );
}

pub(crate) fn metadata(value: &Value) -> Result<BTreeMap<String, Value>, LoadError> {
    let mut value = value.clone();
    super::normalize_context(&mut value);
    if let Some(docs) = value
        .get_mut("context")
        .and_then(|context| context.get_mut("docs"))
    {
        if docs.is_string() {
            *docs = Value::Sequence(vec![docs.clone()]);
        }
    }
    let mut settings = BTreeMap::new();
    for key in super::catalog::DEFINITIONS
        .iter()
        .filter(|definition| definition.applicability == super::catalog::Applicability::Launch)
        .map(|definition| definition.key)
    {
        if let Some(value) = value.get(key).filter(|v| !v.is_null()) {
            let value = value.clone();
            validate_value(&[key], &value)?;
            settings.insert(key.into(), value);
        }
    }
    Ok(settings)
}

// Validate every authored candidate, even when another source shadows it.
pub(crate) fn validate_preferences(
    value: &Value,
    targeted: bool,
    strict: bool,
) -> Result<(), LoadError> {
    let mut normalized = value.clone();
    super::normalize_context(&mut normalized);
    let value = &normalized;
    let Some(map) = value.as_mapping() else {
        if value.is_null() && !strict {
            return Ok(());
        }
        return Err(invalid("settings must be a mapping"));
    };
    for (key, value) in map {
        let key = key
            .as_str()
            .ok_or_else(|| invalid("setting names must be strings"))?;
        let key = if !targeted && key == super::GRAPH_ALIAS {
            "graph"
        } else {
            key
        };
        if key == "skill_agents" {
            return Err(invalid(format!(
                "{key} is retired; use skill.<name>.agent or agent"
            )));
        }
        if !targeted && (key == "skills" || key == "graphs") {
            let targets = value
                .as_mapping()
                .ok_or_else(|| invalid(format!("{key} must be a mapping")))?;
            for (_, target) in targets {
                if key == "skills" {
                    validate_preferences(target, true, true)?;
                } else {
                    let graph = target
                        .as_mapping()
                        .ok_or_else(|| invalid("graph settings must contain steps"))?;
                    if graph.len() != 1 || !graph.contains_key("steps") {
                        return Err(invalid("graph settings must contain only steps"));
                    }
                    let steps = target["steps"]
                        .as_mapping()
                        .ok_or_else(|| invalid("steps must be a mapping"))?;
                    for (_, step) in steps {
                        validate_preferences(step, true, true)?;
                    }
                }
            }
        } else if super::catalog::definition(key).is_some_and(|definition| {
            !targeted || definition.applicability == super::catalog::Applicability::Launch
        }) {
            if !value.is_null() {
                validate_value(&[key], value)?;
            }
        } else if strict || targeted {
            return Err(invalid(format!("unknown or ineligible setting {key}")));
        } else {
            tracing::warn!(key, "unknown LF configuration key");
        }
    }
    Ok(())
}

pub fn definition_source(path: Option<&std::path::Path>) -> ConfigSource {
    let Some(path) = path else {
        return ConfigSource::Builtin;
    };
    if path.starts_with(crate::store::lf_home_dir())
        || dirs::home_dir().is_some_and(|home| {
            [".lf", ".agents", ".claude", ".codex"]
                .iter()
                .any(|dir| path.starts_with(home.join(dir)))
        })
    {
        ConfigSource::Personal
    } else {
        ConfigSource::Repository
    }
}

pub(super) fn skill_values(skill: &Skill) -> Value {
    let mut values = serde_yaml_ng::to_value(&skill.settings).expect("settings serialize");
    if let Some(agent) = &skill.agent {
        insert(&mut values, &["agent"], agent.clone().into());
    }
    if let Some(docs) = &skill.docs {
        insert(
            &mut values,
            &["docs"],
            serde_yaml_ng::to_value(docs).expect("docs serialize"),
        );
    }
    super::normalize_context(&mut values);
    values
}

fn leaves(value: &Value, path: &mut Vec<String>, result: &mut Vec<(Vec<String>, Value)>) {
    if value.is_null() {
        return;
    }
    if super::catalog::definition(&path.join(".")).is_some_and(|definition| {
        definition.key.split('.').count() == path.len()
            && definition.combination == super::catalog::Combination::Replace
    }) {
        result.push((path.clone(), value.clone()));
    } else if let Some(map) = value.as_mapping() {
        for (key, value) in map {
            if let Some(key) = key.as_str() {
                path.push(key.into());
                leaves(value, path, result);
                path.pop();
            }
        }
    } else {
        result.push((path.clone(), value.clone()));
    }
}

// File mistakes must not stop unrelated work. Keep usable preferences;
// explicit setters and invocation overrides remain strict.
pub(super) fn filter_file_preferences(
    value: &mut Value,
    file: &std::path::Path,
    source: ConfigSource,
    scope: SettingScope,
) {
    if !value.is_mapping() && !value.is_null() {
        super::reader::warn(file, None, "skipping settings that are not a mapping");
        *value = Value::Mapping(Default::default());
    }
    _normalize_preferences(value);
    _filter_file_settings(value, &mut Vec::new(), file, source, scope);
}

fn _normalize_preferences(value: &mut Value) {
    super::normalize_context(value);
    if let Some(map) = value.as_mapping_mut() {
        if let Some(graph) = map.remove(super::GRAPH_ALIAS) {
            map.entry(Value::String("graph".into())).or_insert(graph);
        }
    }
}

fn _filter_file_settings(
    value: &mut Value,
    path: &mut Vec<String>,
    file: &std::path::Path,
    source: ConfigSource,
    scope: SettingScope,
) {
    let Some(map) = value.as_mapping_mut() else {
        return;
    };
    map.retain(|key, value| {
        let Some(key) = key.as_str() else {
            super::reader::warn_setting(file, path, "setting names must be strings");
            return false;
        };
        path.push(key.to_owned());
        let parts: Vec<_> = path.iter().map(String::as_str).collect();
        let selector = matches!(
            parts.as_slice(),
            ["skills"]
                | ["skills", _]
                | ["graphs"]
                | ["graphs", _]
                | ["graphs", _, "steps"]
                | ["graphs", _, "steps", _]
        );
        let graph_selector = matches!(parts.as_slice(), ["graphs", _]);
        let preference = match parts.as_slice() {
            ["skills", _, preference @ ..] | ["graphs", _, "steps", _, preference @ ..] => {
                preference
            }
            parts => parts,
        };
        let preference: Vec<String> = preference.iter().map(|part| (*part).to_owned()).collect();
        let definition = super::catalog::definition(&preference.join("."));
        let known = selector
            || definition.is_some_and(|definition| {
                definition.accepts(&preference.iter().map(String::as_str).collect::<Vec<_>>())
            });
        if known && !selector && value.is_null() {
            path.pop();
            return false;
        }
        let mut reason = (!known).then_some("unknown setting");
        if known {
            _filter_file_settings(value, path, file, source, scope);
            if selector {
                if !value.is_mapping() {
                    reason = Some("settings must be a mapping");
                } else if graph_selector && value.get("steps").is_none() {
                    reason = Some("Graph settings need a steps mapping");
                }
            } else {
                let preference: Vec<_> = preference.iter().map(String::as_str).collect();
                if definition.is_some_and(|definition| definition.key == preference.join(".")) {
                    if validate_scope(&path.join("."), source, scope).is_err() {
                        reason = Some("setting does not apply here");
                    } else if validate_value(&preference, value).is_err() {
                        reason = Some("invalid value");
                    }
                }
            }
        }
        if let Some(reason) = reason {
            super::reader::warn_setting(file, path, reason);
        }
        path.pop();
        reason.is_none()
    });
}

/// What to write instead of the retired `default_agent: claude,codex`, when
/// `value` is that spelling. LOO-875 deletes this with the spelling.
pub(crate) fn old_default_agent_warning(value: &str) -> Option<String> {
    let entries: Vec<&str> = super::agent_list(value).collect();
    let first = entries.first().filter(|_| value.contains(','))?;
    Some(format!(
        "default_agent: {value} is the old spelling; write agents: [{}] and default_agent: {first}",
        entries.join(", ")
    ))
}

/// Read the retired `default_agent: claude,codex` as `agents: [claude, codex]`
/// with `default_agent: claude`, and say where to write the replacement.
/// LOO-875 deletes this once `agents` has shipped.
fn _split_agent_list(values: &mut serde_yaml_ng::Mapping, locator: &str) {
    let Some(list) = values
        .get("default_agent")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        return;
    };
    let Some(warning) = old_default_agent_warning(&list) else {
        return;
    };
    // One read resolves a file several times; say it once per file.
    static WARNED: std::sync::Mutex<BTreeSet<String>> = std::sync::Mutex::new(BTreeSet::new());
    if WARNED
        .lock()
        .expect("warned files lock")
        .insert(locator.to_owned())
    {
        super::reader::warn(std::path::Path::new(locator), None, &warning);
    }
    let entries: Vec<Value> = super::agent_list(&list).map(Value::from).collect();
    values.insert("default_agent".into(), entries[0].clone());
    if !values.contains_key("agents") {
        values.insert("agents".into(), Value::Sequence(entries));
    }
}

/// Resolve agent choices before fallbacks; within each key use source, scope, target.
pub fn resolve_layers(
    layers: &[ConfigDeclarations],
    skill: Option<&Skill>,
) -> Result<ResolvedSettings, LoadError> {
    // Each block shares provenance; specificity orders blocks within a source/scope.
    let mut candidates = Vec::new();
    let mut defaults = serde_yaml_ng::to_value(Config::default()).expect("Config serializes");
    if let Some(agent) = super::default_agent() {
        insert(&mut defaults, &["default_agent"], agent.into());
    }
    insert(
        &mut defaults,
        &["graph"],
        crate::ops::task::DEFAULT_TASK_GRAPH.into(),
    );
    insert(
        &mut defaults,
        &["planning"],
        serde_yaml_ng::to_value(super::PlanningConfig::default()).expect("planning serializes"),
    );
    for key in crate::prompt::context_budget::BudgetKey::ALL {
        insert(
            &mut defaults,
            &["context", "budgets", key.name()],
            key.default_limit().into(),
        );
    }
    candidates.push(SettingCandidate {
        key: String::new(),
        reason: String::new(),
        source: ConfigSource::Builtin,
        scope: SettingScope::Global,
        locator: "builtin".into(),
        target: SettingTarget::General,
        value: defaults,
    });
    if let Some(skill) = skill {
        candidates.extend(
            super::definitions::candidates(skill).map_err(|error| invalid(error.to_string()))?,
        );
    }
    let invocation = [
        ConfigDeclarations {
            source: ConfigSource::Environment,
            scope: SettingScope::Global,
            path: "environment".into(),
            value: environment()?,
        },
        ConfigDeclarations {
            source: ConfigSource::CommandLine,
            scope: SettingScope::Global,
            path: "CLI".into(),
            value: invocation_values(),
        },
    ];
    for layer in layers.iter().chain(&invocation) {
        let mut layer = layer.clone();
        if matches!(
            layer.source,
            ConfigSource::Personal | ConfigSource::Repository
        ) {
            filter_file_preferences(&mut layer.value, &layer.path, layer.source, layer.scope);
        } else {
            _normalize_preferences(&mut layer.value);
        }
        if layer.value.is_null() {
            continue;
        }
        validate_preferences(&layer.value, false, true)
            .map_err(|error| invalid(format!("{}: {error}", layer.path.display())))?;
        if let Some(values) = layer.value.as_mapping() {
            for key in values.keys().filter_map(Value::as_str) {
                if !["skills", "graphs"].contains(&key) && preference_path(key).is_ok() {
                    validate_scope(key, layer.source, layer.scope)
                        .map_err(|error| invalid(format!("{}: {error}", layer.path.display())))?;
                }
            }
        }
        // Scope orders scopes within one source; authored repo metadata shares repo scope.
        let mut candidate = SettingCandidate {
            key: String::new(),
            reason: String::new(),
            source: layer.source,
            scope: layer.scope,
            locator: layer.path.display().to_string(),
            target: SettingTarget::General,
            value: Value::Null,
        };
        if let Some(skill) = skill {
            let portable = crate::definition::name::portable_name(&skill.name);
            if let Some(value) = layer
                .value
                .get("skills")
                .and_then(|skills| skills.get(&skill.name).or_else(|| skills.get(&portable)))
            {
                candidates.push(SettingCandidate {
                    target: SettingTarget::Skill {
                        name: skill.name.clone(),
                    },
                    value: value.clone(),
                    ..candidate.clone()
                });
            }
            if let Some(occurrence) = &skill.occurrence {
                if let Some(value) = occurrence.id.as_ref().and_then(|id| {
                    layer
                        .value
                        .get("graphs")?
                        .get(&occurrence.graph)?
                        .get("steps")?
                        .get(id)
                }) {
                    candidates.push(SettingCandidate {
                        target: SettingTarget::GraphOccurrence {
                            graph: occurrence.graph.clone(),
                            id: occurrence.id.clone(),
                        },
                        value: value.clone(),
                        ..candidate.clone()
                    });
                }
            }
        }
        candidate.value = layer.value.clone();
        candidates.push(candidate);
    }
    candidates
        .sort_by_key(|candidate| (candidate.source, candidate.scope, candidate.target.clone()));
    let mut explanations: BTreeMap<String, EffectiveSetting> = BTreeMap::new();
    let mut merged = Value::Mapping(Default::default());
    for mut candidate in candidates {
        let mut values = std::mem::replace(&mut candidate.value, Value::Null);
        if let Some(map) = values.as_mapping_mut() {
            if let Some(value) = map.get_mut("machine") {
                super::placement::expand_shorthand(value)
                    .map_err(|error| invalid(error.to_string()))?;
            }
            if let Some(accounts) = map.get_mut("account").and_then(Value::as_mapping_mut) {
                for value in accounts.values_mut() {
                    *value = serde_yaml_ng::to_value(
                        serde_yaml_ng::from_value::<super::placement::AccountOrder>(value.clone())
                            .map_err(|error| invalid(error.to_string()))?,
                    )
                    .expect("account order serializes");
                }
            }
            map.retain(|key, _| {
                key.as_str().is_some_and(|key| {
                    !["skills", "graphs", "repos", "waves"].contains(&key)
                        && super::catalog::definition(key).is_some()
                })
            });
            if let Some(agents) = map.get_mut("agents") {
                _list_agents(agents);
            }
            _split_agent_list(map, &candidate.locator);
        }
        let mut flat = Vec::new();
        leaves(&values, &mut Vec::new(), &mut flat);
        for (path, value) in flat {
            let key = path.join(".");
            let entry = explanations
                .entry(key.clone())
                .or_insert_with(|| EffectiveSetting {
                    value: Value::Null,
                    candidates: Vec::new(),
                });
            entry.candidates.push(SettingCandidate {
                key: key.clone(),
                value: value.clone(),
                locator: if key == "default_agent" && candidate.source == ConfigSource::Builtin {
                    "PATH (only installed harness)".into()
                } else {
                    candidate.locator.clone()
                },
                ..candidate.clone()
            });
            entry.value = if super::catalog::definition(&key).is_some_and(|definition| {
                definition.combination == super::catalog::Combination::AppendReset
            }) {
                match (std::mem::replace(&mut entry.value, Value::Null), value) {
                    (Value::Sequence(mut old), Value::Sequence(new)) if !new.is_empty() => {
                        old.extend(new);
                        Value::Sequence(old)
                    }
                    (_, value) => value,
                }
            } else {
                value
            };
            // Keep literal map keys; dotted display names cannot reconstruct an address.
            insert(
                &mut merged,
                &path.iter().map(String::as_str).collect::<Vec<_>>(),
                entry.value.clone(),
            );
        }
    }
    // A Tasks destination and a capacity select the same placement. The same
    // precedence used for all declarations selects which one applies.
    if let (Some(machine), Some(capacity)) = (
        explanations.get("machine.tasks"),
        explanations.get("machine.capacity"),
    ) {
        let priority = |setting: &EffectiveSetting| {
            let candidate = setting
                .candidates
                .last()
                .expect("setting has a declaration");
            (candidate.source, candidate.scope, candidate.target.clone())
        };
        let removed = if priority(machine) > priority(capacity) {
            "machine.capacity"
        } else {
            "machine.tasks"
        };
        explanations
            .get_mut(removed)
            .expect("matched placement")
            .value = Value::Null;
        insert(
            &mut merged,
            &removed.split('.').collect::<Vec<_>>(),
            Value::Null,
        );
    }
    // Execution and inspection share the final values, including list resets.
    let mut config: Config =
        serde_yaml_ng::from_value(merged).map_err(|error| invalid(error.to_string()))?;
    config.setting_layers = Some(layers.to_vec());
    for (key, setting) in &explanations {
        if let Some(key) = key.strip_prefix("context.budgets.") {
            let key = serde_yaml_ng::from_value(Value::String(key.into()))
                .map_err(|error| invalid(error.to_string()))?;
            if let Some(winner) = setting.candidates.last() {
                config.context_budget_sources.insert(
                    key,
                    if winner.source == ConfigSource::Builtin {
                        "default".into()
                    } else {
                        winner.locator.clone()
                    },
                );
            }
        }
    }
    // `default_agent` leads only a list declared no more specifically than
    // it: a narrower `agents` is the whole order. The sole installed harness
    // ranks below every authored list.
    let rank = |key: &str| {
        let winner = explanations.get(key)?.candidates.last()?;
        Some((winner.source, winner.scope, winner.target.clone()))
    };
    let outranked = !config.agents.is_empty() && rank("default_agent") < rank("agents");
    if outranked {
        config.default_agent = None;
    }
    // Config keeps the three authored keys distinct. Inspection's effective
    // agent includes them all: the list lowest, then every fallback, then
    // every explicit choice.
    let mut agent = EffectiveSetting {
        value: config
            .agent_order()
            .first()
            .map_or(Value::Null, |agent| agent.as_str().into()),
        candidates: Vec::new(),
    };
    for key in ["agents", "default_agent", "agent"] {
        let declared = if key == "agent" {
            explanations.remove(key)
        } else {
            explanations.get(key).cloned()
        };
        agent.candidates.extend(
            declared
                .into_iter()
                .flat_map(|setting| setting.candidates)
                .filter(|candidate| {
                    candidate.source != ConfigSource::Builtin
                        || (key == "default_agent" && config.default_agent.is_some())
                }),
        );
    }
    explanations.insert("agent".into(), agent);
    for (key, setting) in &mut explanations {
        let append = super::catalog::definition(key).is_some_and(|definition| {
            definition.combination == super::catalog::Combination::AppendReset
        });
        let reset = setting
            .candidates
            .iter()
            .rposition(|candidate| candidate.value.as_sequence().is_some_and(Vec::is_empty));
        let last = setting.candidates.len().saturating_sub(1);
        let list = setting
            .candidates
            .iter()
            .rposition(|candidate| candidate.key == "agents")
            .filter(|_| !config.agents.is_empty());
        for (index, candidate) in setting.candidates.iter_mut().enumerate() {
            candidate.reason = if super::catalog::definition(key).is_some_and(|definition| {
                definition.lifecycle == super::catalog::Lifecycle::Inactive
            }) {
                "inactive: parsed but no runtime consumer"
            } else if append && reset.is_none_or(|reset| index >= reset) {
                if candidate.value.as_sequence().is_some_and(Vec::is_empty) {
                    "clears earlier contributions"
                } else {
                    "contributes"
                }
            } else if key == "agent" && Some(index) == list {
                // The list in force; it selects when no default_agent leads it.
                if config.agent.is_some() {
                    "unused: an agent choice pins one provider"
                } else if config.default_agent.is_none() {
                    "selected"
                } else {
                    "contributes"
                }
            } else if key == "agent" && candidate.key == "default_agent" && outranked {
                "unused: a more specific agents list is the whole order"
            } else if index == last && !setting.value.is_null() {
                "selected"
            } else if key == "agent" && candidate.key == "default_agent" && config.agent.is_some() {
                "fallback: an applicable agent choice wins"
            } else if append {
                "cleared by a higher declaration"
            } else {
                "overridden by a higher declaration"
            }
            .into();
        }
    }
    Ok(ResolvedSettings {
        config,
        settings: explanations,
    })
}

pub fn resolve(config: &Config, skill: Option<&Skill>) -> Result<ResolvedSettings, LoadError> {
    fn differs(value: &mut Value, defaults: &Value, path: &str) -> bool {
        if value == defaults {
            return false;
        }
        if super::catalog::definition(path)
            .is_none_or(|definition| definition.combination == super::catalog::Combination::Merge)
        {
            if let Some(map) = value.as_mapping_mut() {
                map.retain(|key, value| {
                    let name = key.as_str().expect("configuration keys are strings");
                    let child = if path.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{path}.{name}")
                    };
                    differs(value, defaults.get(key).unwrap_or(&Value::Null), &child)
                });
                return !map.is_empty();
            }
        }
        true
    }

    if let Some(layers) = &config.setting_layers {
        return resolve_layers(layers, skill);
    }
    let mut value = serde_yaml_ng::to_value(config).expect("Config serializes");
    let defaults = serde_yaml_ng::to_value(Config::default()).expect("Config serializes");
    if !differs(&mut value, &defaults, "") {
        value = Value::Mapping(Default::default());
    }
    resolve_layers(
        &[ConfigDeclarations {
            source: ConfigSource::Repository,
            scope: SettingScope::Repository,
            path: PathBuf::from("configuration"),
            value,
        }],
        skill,
    )
}

thread_local! {
    static INVOCATION: std::cell::RefCell<Value> = const { std::cell::RefCell::new(Value::Null) };
}

#[derive(Debug)]
pub struct InvocationSettings(Value);
impl Drop for InvocationSettings {
    fn drop(&mut self) {
        INVOCATION
            .with(|current| *current.borrow_mut() = std::mem::replace(&mut self.0, Value::Null));
    }
}

pub(crate) fn replace_invocation(value: Value) -> InvocationSettings {
    InvocationSettings(INVOCATION.with(|current| current.replace(value)))
}

pub fn select_invocation(value: Value) -> InvocationSettings {
    fn overlay(current: &mut Value, value: Value) {
        if let Value::Mapping(values) = value {
            if !current.is_mapping() {
                *current = Value::Mapping(Default::default());
            }
            let current = current.as_mapping_mut().expect("mapping initialized");
            for (key, value) in values {
                overlay(current.entry(key).or_insert(Value::Null), value);
            }
        } else if !value.is_null() {
            *current = value;
        }
    }
    InvocationSettings(INVOCATION.with(|current| {
        let previous = current.borrow().clone();
        overlay(&mut current.borrow_mut(), value);
        previous
    }))
}

pub(crate) fn invocation_values() -> Value {
    INVOCATION.with(|current| current.borrow().clone())
}

pub(super) fn explicit_agent() -> Result<Option<String>, LoadError> {
    let cli = invocation_values();
    let env = environment()?;
    Ok(cli
        .get("agent")
        .or_else(|| env.get("agent"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned))
}

fn environment() -> Result<Value, LoadError> {
    if crate::journal::request::active() {
        return Ok(Value::Mapping(Default::default()));
    }
    let mut values = Value::Mapping(Default::default());
    if let Ok(text) = std::env::var("LF_SETTINGS") {
        values = serde_yaml_ng::from_str(&text)
            .map_err(|error| invalid(format!("LF_SETTINGS: {error}")))?;
        if !values.is_mapping() {
            return Err(invalid("LF_SETTINGS must be a mapping"));
        }
    }
    match std::env::var(super::USER_NAME_ENV) {
        Ok(name) => {
            if let Some(name) = super::normalize_user_name(&name) {
                insert(&mut values, &["user", "name"], name.into());
            }
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => {
            return Err(invalid(format!(
                "Invalid {}: {error}",
                super::USER_NAME_ENV
            )))
        }
    }
    if let Ok(agent) = std::env::var(super::AGENT_OVERRIDE_ENV) {
        if !agent.is_empty() {
            insert(&mut values, &["agent"], agent.into());
        }
    }
    for (name, key) in [
        (super::YOLO_OVERRIDE_ENV, "yolo"),
        (super::CHROME_OVERRIDE_ENV, "chrome"),
    ] {
        if let Ok(value) = std::env::var(name) {
            let value = match value.as_str() {
                "1" | "true" | "on" => true,
                "0" | "false" | "off" => false,
                _ => return Err(invalid(format!("{name} must be on or off"))),
            };
            insert(&mut values, &[key], value.into());
        }
    }
    validate_preferences(&values, false, true)?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_layers, select_invocation, ConfigDeclarations, ConfigSource, OccurrenceSettings,
        SettingScope,
    };
    use crate::config::CodexServiceTier;
    use crate::graph::Skill;
    use serde_yaml_ng::Value;

    fn layer(kind: ConfigSource, scope: SettingScope, text: &str) -> ConfigDeclarations {
        ConfigDeclarations {
            source: kind,
            scope,
            path: format!("{kind:?}/{scope:?}").into(),
            value: serde_yaml_ng::from_str(text).unwrap(),
        }
    }
    #[test]
    fn agents_orders_what_a_fallback_moves_through() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let order = |layers: &[ConfigDeclarations], skill: Option<&Skill>| {
            resolve_layers(layers, skill).unwrap().config.agent_order()
        };
        let personal = layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "agents: [codex, claude]\nskills: {implement: {default_agent: claude}}",
        );
        // The list outranks the sole installed harness.
        assert_eq!(
            order(std::slice::from_ref(&personal), None),
            ["codex", "claude"]
        );
        let implement = Skill::named("implement");
        assert_eq!(
            order(std::slice::from_ref(&personal), Some(&implement)),
            ["claude", "codex"]
        );
        // A narrower list replaces the wider one whole; precedence is unchanged.
        let repo = layer(
            ConfigSource::Repository,
            SettingScope::Repository,
            "agents: [pi]",
        );
        assert_eq!(order(&[personal.clone(), repo], None), ["codex", "claude"]);
        let wave = layer(
            ConfigSource::Personal,
            SettingScope::Wave,
            "agents: [claude]",
        );
        assert_eq!(order(&[personal.clone(), wave], None), ["claude"]);
        // A default_agent leads only a list no more specific than it.
        let fallback = layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "agents: [claude, codex]\ndefault_agent: claude\nskills: {review: {agents: [codex]}, compress: {default_agent: codex}}",
        );
        assert_eq!(
            order(std::slice::from_ref(&fallback), None),
            ["claude", "codex"]
        );
        assert_eq!(
            order(
                std::slice::from_ref(&fallback),
                Some(&Skill::named("compress"))
            ),
            ["codex", "claude"]
        );
        let review = Skill::named("review");
        let narrowed = resolve_layers(std::slice::from_ref(&fallback), Some(&review)).unwrap();
        assert_eq!(narrowed.config.agent_order(), ["codex"]);
        assert_eq!(narrowed.settings["agent"].value.as_str(), Some("codex"));
        let reasons = |resolved: &super::ResolvedSettings| -> Vec<(String, String)> {
            resolved.settings["agent"]
                .candidates
                .iter()
                .map(|candidate| (candidate.key.clone(), candidate.reason.clone()))
                .collect()
        };
        assert!(reasons(&narrowed).contains(&(
            "default_agent".into(),
            "unused: a more specific agents list is the whole order".into()
        )));
        assert!(reasons(&narrowed).contains(&("agents".into(), "selected".into())));
        let codex_wave = layer(
            ConfigSource::Personal,
            SettingScope::Wave,
            "agents: [codex]",
        );
        assert_eq!(order(&[fallback, codex_wave], None), ["codex"]);
        let pinned = layer(
            ConfigSource::Repository,
            SettingScope::Repository,
            "agent: pi",
        );
        let resolved = resolve_layers(&[personal, pinned], None).unwrap();
        assert_eq!(resolved.config.agent_order(), ["pi"]);
        let reasons: Vec<_> = resolved.settings["agent"]
            .candidates
            .iter()
            .map(|candidate| (candidate.key.as_str(), candidate.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("agents", "unused: an agent choice pins one provider"),
                ("agent", "selected")
            ]
        );
    }

    #[test]
    fn the_comma_spelling_of_default_agent_reads_as_agents() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let old = layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "default_agent: 'codex, claude:opus'",
        );
        let resolved = resolve_layers(&[old], None).unwrap();
        assert_eq!(resolved.config.default_agent.as_deref(), Some("codex"));
        assert_eq!(resolved.config.agents, ["codex", "claude:opus"]);
        assert_eq!(resolved.settings["agent"].value.as_str(), Some("codex"));
        assert_eq!(
            resolved.settings["agent"].selected().unwrap().key,
            "default_agent"
        );
    }

    #[test]
    fn a_comma_string_is_the_agents_list() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let saved = super::parse_value("agents", "claude:opus, codex").unwrap();
        assert_eq!(
            saved,
            serde_yaml_ng::from_str::<Value>("[claude:opus, codex]").unwrap()
        );
        assert!(super::parse_value("skill.review.agents", "codex").is_ok());
        assert!(super::parse_value("agents", "claude,not a harness").is_err());
        let file = layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "agents: codex,claude\nskills: {implement: {agents: claude}}",
        );
        assert!(super::validate_preferences(&file.value, false, true).is_ok());
        let order = |skill: Option<&Skill>| {
            resolve_layers(std::slice::from_ref(&file), skill)
                .unwrap()
                .config
                .agent_order()
        };
        assert_eq!(order(None), ["codex", "claude"]);
        assert_eq!(order(Some(&Skill::named("implement"))), ["claude"]);
    }

    #[test]
    fn saving_the_comma_spelling_of_default_agent_names_the_replacement() {
        assert_eq!(
            super::old_default_agent_warning("claude,codex").unwrap(),
            "default_agent: claude,codex is the old spelling; write agents: [claude, codex] and default_agent: claude"
        );
        assert_eq!(super::old_default_agent_warning("claude"), None);
    }

    fn occurrence() -> Skill {
        let mut skill = Skill::named("implement");
        skill.occurrence = Some(Box::new(OccurrenceSettings {
            graph: "release".into(),
            id: Some("write-code".into()),
            source: ConfigSource::Repository,
            path: None,
            address: None,
            values: Value::Null,
        }));
        skill
    }

    #[test]
    fn file_reads_ignore_unknown_preferences_without_losing_known_neighbors() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let values = "default_agent: codex\nworkflow: review\nfuture: {enabled: true}\nclaude: {model: opus, future: true}\ncontext: {docs: [README.md], future: true}\nskills: {implement: {agent: 'claude:fable', future: true}}\ngraphs: {release: {steps: {write-code: {yolo: true, future: true}}}}";
        for source in [ConfigSource::Personal, ConfigSource::Repository] {
            let declaration = layer(source, SettingScope::Global, values);
            let resolved =
                resolve_layers(std::slice::from_ref(&declaration), Some(&occurrence())).unwrap();
            assert_eq!(resolved.config.agent().unwrap(), "claude:fable");
            assert_eq!(resolved.config.graph.as_deref(), Some("review"));
            assert_eq!(resolved.config.claude.model.as_deref(), Some("opus"));
            assert_eq!(resolved.config.context.docs, ["README.md"]);
            assert!(resolved.config.yolo);
            assert!(resolved.settings.keys().all(|key| !key.contains("future")));
            assert!(declaration.value.get("future").is_some());
        }
        for values in [
            "yolo: perhaps\nfuture: true",
            "claude: {model: [], future: true}",
            "context: {diff: perhaps, docs: [README.md]}",
            "yolo: null\nclaude: {model: null}",
        ] {
            let resolved = resolve_layers(
                &[layer(ConfigSource::Personal, SettingScope::Global, values)],
                None,
            )
            .unwrap();
            assert!(!resolved.config.yolo);
            assert!(resolved.config.claude.model.is_none());
            if values.starts_with("context:") {
                assert_eq!(resolved.config.context.docs, ["README.md"]);
            }
        }
        assert!(resolve_layers(
            &[layer(
                ConfigSource::CommandLine,
                SettingScope::Global,
                "future: true"
            )],
            None,
        )
        .is_err());
    }

    #[test]
    fn claude_model_uses_normal_precedence_without_selecting_a_harness() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let layers = [
            layer(
                ConfigSource::Repository,
                SettingScope::Repository,
                "claude: {model: sonnet}",
            ),
            layer(
                ConfigSource::Personal,
                SettingScope::Global,
                "default_agent: codex\nclaude: {model: opus}",
            ),
        ];
        let resolved = resolve_layers(&layers, None).unwrap();
        assert_eq!(resolved.config.agent().unwrap(), "codex");
        assert_eq!(resolved.config.claude.model.as_deref(), Some("opus"));
        assert_eq!(
            resolved.settings["claude.model"].value.as_str(),
            Some("opus")
        );
        let mut skill = Skill::named("kickoff");
        skill.agent = Some("claude:fable".into());
        assert_eq!(
            resolve_layers(&layers, Some(&skill))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude:fable"
        );
        for invalid in ["''", "' '", "null", "{}"] {
            assert!(
                super::parse_value("claude.model", invalid).is_err(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn codex_service_tier_is_optional_and_typed() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        assert_eq!(
            resolve_layers(&[], None).unwrap().config.codex.service_tier,
            None
        );
        for (value, expected) in [
            ("default", CodexServiceTier::Default),
            ("standard", CodexServiceTier::Default),
            ("priority", CodexServiceTier::Priority),
            ("fast", CodexServiceTier::Priority),
        ] {
            assert!(super::parse_value("codex.service_tier", value).is_ok());
            let resolved = resolve_layers(
                &[layer(
                    ConfigSource::Personal,
                    SettingScope::Global,
                    &format!("codex: {{service_tier: {value}}}"),
                )],
                None,
            )
            .unwrap();
            assert_eq!(resolved.config.codex.service_tier, Some(expected));
        }
        for value in ["slow", "true", "null", "42"] {
            assert!(super::parse_value("codex.service_tier", value).is_err());
        }
    }

    #[test]
    fn codex_service_tier_uses_normal_precedence() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let layers = [
            layer(
                ConfigSource::Repository,
                SettingScope::Repository,
                "codex: {service_tier: priority}",
            ),
            layer(
                ConfigSource::Personal,
                SettingScope::Global,
                "codex: {service_tier: standard}",
            ),
            layer(
                ConfigSource::Personal,
                SettingScope::Repository,
                "codex: {service_tier: fast}",
            ),
        ];
        for (count, expected) in [
            (1, CodexServiceTier::Priority),
            (2, CodexServiceTier::Default),
            (3, CodexServiceTier::Priority),
        ] {
            assert_eq!(
                resolve_layers(&layers[..count], None)
                    .unwrap()
                    .config
                    .codex
                    .service_tier,
                Some(expected)
            );
        }
        let _environment = crate::lf::commands::graph::EnvVarGuard::set(
            "LF_SETTINGS",
            "codex: {service_tier: default}",
        );
        assert_eq!(
            resolve_layers(&layers, None)
                .unwrap()
                .config
                .codex
                .service_tier,
            Some(CodexServiceTier::Default)
        );
        let _cli =
            select_invocation(serde_yaml_ng::from_str("codex: {service_tier: fast}").unwrap());
        assert_eq!(
            resolve_layers(&layers, None)
                .unwrap()
                .config
                .codex
                .service_tier,
            Some(CodexServiceTier::Priority)
        );
    }

    #[test]
    fn choices_precede_fallbacks_before_source_scope_and_target() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let mut skill = occurrence();
        skill.occurrence.as_mut().unwrap().values =
            serde_yaml_ng::from_str("agent: claude:occurrence").unwrap();
        for (key, expected) in [("agent", "codex"), ("default_agent", "claude:occurrence")] {
            let personal = layer(
                ConfigSource::Personal,
                SettingScope::Global,
                &format!("skills: {{implement: {{{key}: codex}}}}"),
            );
            let resolved = resolve_layers(&[personal], Some(&skill)).unwrap();
            assert_eq!(resolved.config.agent().unwrap(), expected);
            assert_eq!(resolved.settings["agent"].value.as_str(), Some(expected));
            assert_eq!(
                resolved.settings["agent"].candidates.last().unwrap().key,
                "agent"
            );
        }
        skill.occurrence.as_mut().unwrap().values = Value::Null;
        let personal = layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "skills: {implement: {default_agent: codex}}",
        );
        let repo = layer(
            ConfigSource::Repository,
            SettingScope::Repository,
            "agent: claude",
        );
        assert_eq!(
            resolve_layers(&[personal.clone(), repo], Some(&skill))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude"
        );
        let repo = layer(
            ConfigSource::Repository,
            SettingScope::Repository,
            "graphs: {release: {steps: {write-code: {default_agent: claude}}}}",
        );
        let resolved = resolve_layers(&[personal, repo], Some(&skill)).unwrap();
        assert_eq!(resolved.config.agent().unwrap(), "codex");
        assert!(resolved.config.agent.is_none());
        assert_eq!(
            resolved.settings["agent"].candidates.last().unwrap().key,
            "default_agent"
        );
    }

    #[test]
    fn fallback_sources_keep_cli_environment_file_scope_and_target_order() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let layers = [
            layer(ConfigSource::Personal, SettingScope::Global,
                "graphs: {release: {steps: {write-code: {default_agent: claude:personal}}}}"),
            layer(ConfigSource::Personal, SettingScope::Global, "default_agent: codex:global"),
            layer(ConfigSource::Personal, SettingScope::Repository, "default_agent: codex:repo"),
            layer(ConfigSource::Personal, SettingScope::Wave,
                "default_agent: codex:wave\nskills: {implement: {default_agent: codex:skill}}\ngraphs: {release: {steps: {write-code: {default_agent: codex:step}}}}"),
        ];
        for (count, expected) in [
            (1, "claude:personal"),
            (2, "claude:personal"),
            (3, "codex:repo"),
            (4, "codex:step"),
        ] {
            assert_eq!(
                resolve_layers(&layers[..count], Some(&occurrence()))
                    .unwrap()
                    .config
                    .agent()
                    .unwrap(),
                expected
            );
        }
        let _environment = crate::lf::commands::graph::EnvVarGuard::set(
            "LF_SETTINGS",
            "default_agent: claude:environment",
        );
        assert_eq!(
            resolve_layers(&layers, Some(&occurrence()))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude:environment"
        );
        let _cli = select_invocation(serde_yaml_ng::from_str("default_agent: codex:cli").unwrap());
        assert_eq!(
            resolve_layers(&layers, Some(&occurrence()))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "codex:cli"
        );
        let mut skill = occurrence();
        skill.agent = Some("claude:choice".into());
        assert_eq!(
            resolve_layers(&layers, Some(&skill))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "claude:choice"
        );
        let _force = select_invocation(serde_yaml_ng::from_str("agent: codex:forced").unwrap());
        assert_eq!(
            resolve_layers(&layers, Some(&skill))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "codex:forced"
        );
    }

    #[test]
    fn captured_defaults_keep_fallback_meaning_and_explicit_empty_docs() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let captured = r#"{"name":"implement","default_agent":"codex","docs":[]}"#;
        let skill: Skill = serde_json::from_str(captured).unwrap();
        assert!(skill.agent.is_none());
        let saved = serde_json::to_value(&skill).unwrap();
        assert_eq!(saved["default_agent"], "codex");
        let resolved = resolve_layers(
            &[layer(
                ConfigSource::Repository,
                SettingScope::Repository,
                "agent: claude\ndocs: [repo.md]",
            )],
            Some(&skill),
        )
        .unwrap();
        assert_eq!(resolved.config.agent().unwrap(), "claude");
        assert_eq!(skill.docs, Some(vec![]));
        assert_eq!(
            resolve_layers(&[], Some(&skill))
                .unwrap()
                .config
                .agent()
                .unwrap(),
            "codex"
        );
    }

    #[test]
    fn captured_docs_preserve_omission_reset_and_addition() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        for (declaration, expected) in [
            ("", vec!["inherited.md"]),
            (",\"docs\":[]", vec![]),
            (",\"docs\":[\"skill.md\"]", vec!["inherited.md", "skill.md"]),
            (
                ",\"docs\":[\"skill.md\"],\"context\":[\"extra.md\"]",
                vec!["inherited.md", "skill.md", "extra.md"],
            ),
        ] {
            let skill: Skill = serde_json::from_str(&format!(
                "{{\"name\":\"implement\",\"default_agent\":\"codex\"{declaration}}}"
            ))
            .unwrap();
            let saved = serde_json::to_string(&skill).unwrap();
            assert_eq!(serde_json::from_str::<Skill>(&saved).unwrap(), skill);
            assert_eq!(skill.docs.is_none(), declaration.is_empty());
            assert!(!skill.settings.contains_key("docs"));
            let resolved = resolve_layers(
                &[layer(
                    ConfigSource::Builtin,
                    SettingScope::Global,
                    "docs: [inherited.md]",
                )],
                Some(&skill),
            )
            .unwrap();
            assert_eq!(resolved.config.context.docs, expected);
        }
    }

    #[test]
    fn loaded_defaults_remain_builtin_when_selecting_a_skill() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let general = resolve_layers(&[], None).unwrap();
        let mut skill = Skill::named("kickoff");
        skill.agent = Some("claude".into());
        let selected = super::resolve(&general.config, Some(&skill)).unwrap();
        assert_eq!(selected.config.agent().unwrap(), "claude");
        assert_eq!(
            selected.settings["agent"].candidates.last().unwrap().source,
            ConfigSource::Builtin
        );
    }

    #[test]
    fn files_ignore_ineligible_settings_but_keep_applicable_ones() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        {
            let source = ConfigSource::Personal;
            assert!(resolve_layers(
                &[layer(
                    source,
                    SettingScope::Repository,
                    "session: {terminal: ghostty}"
                )],
                None,
            )
            .unwrap()
            .config
            .session
            .terminal
            .is_none());
            assert!(resolve_layers(
                &[layer(
                    source,
                    SettingScope::Wave,
                    "planning: {provider: git}"
                )],
                None,
            )
            .is_ok());
            assert!(resolve_layers(
                &[layer(
                    source,
                    SettingScope::Global,
                    "session: {terminal: ghostty}"
                )],
                None,
            )
            .is_ok());
        }
    }

    #[test]
    fn source_precedes_scope_and_target_for_each_preference() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let skill = occurrence();
        let layers = vec![
            layer(ConfigSource::Repository, SettingScope::Wave, "agent: claude\nyolo: true"),
            layer(ConfigSource::Personal, SettingScope::Global, "agent: claude\nskills: {implement: {agent: codex, chrome: true}}\ngraphs: {release: {steps: {write-code: {agent: claude:sonnet, chrome: false}}}}"),
            layer(ConfigSource::Personal, SettingScope::Repository, "agent: codex\nyolo: false"),
        ];
        let resolved = resolve_layers(&layers, Some(&skill)).unwrap();
        assert_eq!(resolved.config.agent().unwrap(), "codex");
        assert!(!resolved.config.yolo);
        assert_eq!(resolved.config.chrome, Some(false));
        let source = resolved.settings["agent"].candidates.last().unwrap();
        assert_eq!(source.locator, "Personal/Repository");
        assert_eq!(source.target, super::SettingTarget::General);
        let revealed = resolve_layers(&layers[..2], Some(&skill)).unwrap();
        assert_eq!(revealed.config.agent().unwrap(), "claude:sonnet");
        assert!(revealed.config.yolo);
    }
    #[test]
    fn cli_overrides_environment_without_exporting_the_selection() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let _guard = crate::lf::commands::graph::EnvVarGuard::set(
            crate::config::AGENT_OVERRIDE_ENV,
            "claude",
        );
        let _cli = select_invocation(serde_yaml_ng::from_str("agent: codex\nyolo: false").unwrap());
        let resolved = resolve_layers(&[], Some(&occurrence())).unwrap();
        assert_eq!(resolved.config.agent().unwrap(), "codex");
        let _nested = select_invocation(serde_yaml_ng::from_str("{}").unwrap());
        assert_eq!(
            resolve_layers(&[], None).unwrap().config.agent().unwrap(),
            "codex"
        );
        {
            let _nested =
                select_invocation(serde_yaml_ng::from_str("agent: claude:sonnet").unwrap());
            let selected = resolve_layers(&[], None).unwrap();
            assert_eq!(selected.config.agent().unwrap(), "claude:sonnet");
            assert!(!selected.config.yolo);
        }
        assert_eq!(
            resolve_layers(&[], None).unwrap().config.agent().unwrap(),
            "codex"
        );
        assert_eq!(
            resolved.settings["agent"]
                .candidates
                .last()
                .unwrap()
                .locator,
            "CLI"
        );
        assert_eq!(
            std::env::var(crate::config::AGENT_OVERRIDE_ENV).unwrap(),
            "claude"
        );
    }
    #[test]
    fn empty_lists_reset_additive_values_and_maps_merge_by_key() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let resolved = resolve_layers(
            &[
                layer(
                    ConfigSource::Repository,
                    SettingScope::Repository,
                    "docs: [a.md]\ncontext_budgets: {scratch_tokens: 100, scratch_bytes: 200}",
                ),
                layer(
                    ConfigSource::Personal,
                    SettingScope::Global,
                    "docs: [b.md]\ncontext_budgets: {scratch_tokens: 50}",
                ),
            ],
            None,
        )
        .unwrap();
        assert_eq!(resolved.config.context.docs, ["a.md", "b.md"]);
        assert_eq!(
            resolved.settings["context.budgets.scratch_tokens"].value,
            Value::from(50)
        );
        assert_eq!(
            resolved.settings["context.budgets.scratch_bytes"].value,
            Value::from(200)
        );
        let mut layers = resolved.config.setting_layers.unwrap();
        layers.push(layer(
            ConfigSource::Personal,
            SettingScope::Global,
            "docs: []",
        ));
        assert!(resolve_layers(&layers, None)
            .unwrap()
            .config
            .context
            .docs
            .is_empty());
    }
    #[test]
    fn typed_values_reject_unknown_keys_and_wrong_types() {
        for (key, value) in [
            ("graphs.release.steps.write-code.yolo", "false"),
            ("skills.implement.context.docs", "[]"),
            ("graphs.release.steps.write-code.default_agent", "codex"),
            ("automation.retries", "3"),
            ("release.targets.cli.verify", "[cargo test]"),
            ("planning", "{provider: git, shared: main}"),
        ] {
            assert!(super::parse_value(key, value).is_ok(), "{key}");
        }
        for (key, value) in [
            ("skills.implement.planning", "{}"),
            ("yolo", "perhaps"),
            ("agent", "Explore"),
            ("default_agent", "Explore"),
            ("missing", "false"),
            ("context.budgets.scratch_tokens", "0"),
            ("session", "{unknown: true}"),
        ] {
            assert!(super::parse_value(key, value).is_err(), "{key}");
        }
    }
    #[test]
    fn task_layout_is_validated_where_it_is_set() {
        let pane =
            |id: &str, content: &str| format!("{{kind: pane, id: {id}, content: {content}}}");
        let layout = |session: &str| {
            format!(
                "{{tree: {{kind: split, axis: vertical, ratio: 0.5, first: {{kind: split, axis: horizontal, ratio: 0.5, first: {}, second: {}}}, second: {}}}}}",
                pane("conversation", &format!("{{kind: session, session_id: {session}, location: null}}")),
                pane("changes", "{kind: browser, location: null, tabs: [{kind: changes, id: changes, comparison: {kind: working_tree, base: parent}}], selected: changes}"),
                pane("shell", "{kind: shell, tabs: [{id: s, location: null, lf_process_id: null}], selected: s}"),
            )
        };
        assert!(super::parse_value("task_layout", &layout("null")).is_ok());
        let captured = super::parse_value("task_layout", &layout("abc")).unwrap_err();
        assert!(
            captured.to_string().contains("cannot capture a Session"),
            "{captured}"
        );
        // A tree without Desktop's three Task panes is unusable at launch.
        let partial = format!(
            "{{tree: {}}}",
            pane(
                "shell",
                "{kind: shell, tabs: [{id: s, location: null, lf_process_id: null}], selected: s}"
            )
        );
        assert!(super::parse_value("task_layout", &partial).is_err());
    }
    #[test]
    fn composed_occurrences_keep_their_settings_and_match_launch_preparation() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/skills/probe.md",
            "---\nagent: claude\nchrome: true\n---\nProbe.",
        );
        repo.create_file(
            ".lf/graphs/release.yaml",
            "- step: {name: probe, id: write-code}\n- step: {name: probe, id: check-code}\n",
        );
        repo.create_file(
            ".lf/graphs/outer.yaml",
            "- graph: release\n- graph: release\n",
        );
        std::fs::write(home.path().join("config.yaml"), "skills: {probe: {agent: codex, chrome: true}}\ngraphs: {release: {steps: {write-code: {agent: claude:sonnet, chrome: false}}}}\n").unwrap();
        let config = crate::config::load_config(Some(repo.path()))
            .unwrap()
            .unwrap();
        let graph = crate::graph::load_authored_graph("outer", repo.path()).unwrap();
        let steps = crate::graph::compile_graph(&graph, repo.path()).unwrap();
        assert_eq!(steps.len(), 4);
        for (index, step) in steps.iter().enumerate() {
            let crate::graph::ConcreteStep::Skill(step) = step else {
                panic!("agent step")
            };
            let expected = if index % 2 == 0 {
                "claude:sonnet"
            } else {
                "codex"
            };
            let inspected = super::resolve(&config, Some(&step.skill)).unwrap();
            assert_eq!(inspected.config.agent().unwrap(), expected);
            let prepared = crate::prompt::process::prepare_process_prompt(
                &config,
                crate::prompt::process::ProcessPromptInput {
                    repo_root: repo.path().to_owned(),
                    resolved_skill: Some(step.skill.clone()),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(prepared.config.agent().unwrap(), expected);
            assert_eq!(prepared.config.chrome, Some(index % 2 != 0));
        }
        repo.create_file(
            ".lf/graphs/invalid.yaml",
            "- step: {name: probe, id: same}\n- step: {name: probe, id: same}\n",
        );
        let invalid = crate::graph::load_authored_graph("invalid", repo.path()).unwrap();
        assert!(crate::graph::compile_graph(&invalid, repo.path()).is_err());
    }

    #[test]
    fn bare_graph_references_keep_skill_defaults_at_their_own_source() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/config.yaml", "agent: codex\n");
        repo.create_file(".lf/graphs/probe.yaml", "- kickoff\n- step: kickoff\n");
        let config = crate::config::load_config(Some(repo.path()))
            .unwrap()
            .unwrap();
        let graph = crate::graph::load_authored_graph("probe", repo.path()).unwrap();
        for step in crate::graph::compile_graph(&graph, repo.path()).unwrap() {
            let crate::graph::ConcreteStep::Skill(step) = step else {
                panic!("agent step")
            };
            let resolved = super::resolve(&config, Some(&step.skill)).unwrap();
            assert_eq!(resolved.config.agent().unwrap(), "codex");
            assert_eq!(
                resolved.settings["agent"].candidates.last().unwrap().source,
                ConfigSource::Repository
            );
        }
    }
}
