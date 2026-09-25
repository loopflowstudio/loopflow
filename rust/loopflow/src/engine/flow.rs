use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::engine::error::LoadError;

static RETIRED_INTERACTIVE_WARNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl Skill {
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agent: None,
            default_agent: None,
            action_style: None,
            content: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OccurrencePolicy {
    /// Stable identity for this occurrence inside an authored flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Whether this occurrence requires a present User.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub human: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<RepeatPolicy>,
}

/// The deciding occurrence can return to a preceding node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepeatPolicy {
    pub from: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillStep {
    #[serde(flatten)]
    pub skill: Skill,
    #[serde(flatten)]
    pub policy: OccurrencePolicy,
}

impl SkillStep {
    fn named(name: &str) -> Self {
        Self {
            skill: Skill::named(name),
            policy: OccurrencePolicy::default(),
        }
    }
}

impl OccurrencePolicy {
    fn validate(&self) -> Result<(), LoadError> {
        if self.id.as_deref().is_some_and(|id| id.trim().is_empty()) {
            return Err(LoadError::InvalidFlow(
                "step id cannot be empty".to_string(),
            ));
        }
        if self.human && self.id.is_none() {
            return Err(LoadError::InvalidFlow(
                "review steps require a stable id".to_string(),
            ));
        }
        if self.human && self.repeat.is_some() {
            return Err(LoadError::InvalidFlow(
                "human steps return feedback; put the backward edge on a following loop-decide step".to_string(),
            ));
        }
        if let Some(repeat) = &self.repeat {
            if self.id.is_none() || repeat.from.trim().is_empty() {
                return Err(LoadError::InvalidFlow(
                    "repeat requires a deciding step with an id and a from node".to_string(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum Step {
    Skill(SkillStep),
    Op(Op),
    FlowRef(String),
    Xor(XorDef),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Op {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
}

impl Op {
    pub fn display_name(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "op: {}", self.display_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XorDef {
    /// Optional router skill. If set, this skill runs first and writes the
    /// verdict. Path descriptions are appended to the skill's prompt as routing
    /// instructions. If absent, a generic routing agent is used.
    pub router: Option<String>,
    pub paths: HashMap<String, XorPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct XorPath {
    pub flow: Option<String>,
    pub skill: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<SkillStep>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Flow {
    pub name: String,
    pub items: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Goal {
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalRenderContext {
    pub flows: Vec<String>,
    pub memory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteSkill {
    pub skill: Skill,
    pub policy: OccurrencePolicy,
    pub flow_parents: Vec<String>,
}

impl ConcreteSkill {
    pub fn display_path(&self) -> String {
        let mut parts = self.flow_parents.clone();
        parts.push(self.skill.name.clone());
        parts.join(" ")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteXor {
    pub router: Skill,
    pub paths: HashMap<String, ConcretePath>,
    pub flow_parents: Vec<String>,
}

/// A captured branch body; unresolved authored paths cannot decode as this type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConcretePath {
    pub description: String,
    pub steps: Vec<ConcreteStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConcreteOp {
    pub item: Op,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConcreteStep {
    Skill(ConcreteSkill),
    Op(ConcreteOp),
    Xor(ConcreteXor),
}

pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    load_flow_inner(name, repo, true)
}

pub fn available_flow_names(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = crate::engine::builtins::builtin_flow_names()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
    names.extend(repo_flow_names(repo));
    names.sort();
    names.dedup();
    names
}

pub(crate) fn repo_flow_names(repo: &Path) -> Vec<String> {
    let mut names = Vec::new();
    collect_flow_names(&repo.join(".lf/flows"), None, &mut names);
    names.sort();
    names.dedup();
    names
}

pub fn load_goal(name: &str, repo: &Path) -> Result<Goal, LoadError> {
    if let Ok(goal_path) = find_goal_path(name, repo) {
        let content = fs::read_to_string(goal_path)?;
        let prompt = split_frontmatter(&content)
            .map(|(_, body)| body)
            .unwrap_or(content);
        return Ok(Goal { prompt });
    }

    if let Some(key) = crate::engine::builtins::resolve_builtin_goal(name) {
        let prompt = crate::engine::builtins::get_builtin_goal(key)
            .expect("resolve_builtin_goal returned a known key");
        return Ok(Goal {
            prompt: prompt.to_string(),
        });
    }

    Err(LoadError::GoalNotFound(name.to_string()))
}

/// The one wave-memory injector. Both the wave agent's goal seed
/// ([`render_goal`]) and ambient context assembly
/// ([`crate::engine::prompt::format_content_sections`]) emit memory through
/// this, so it appears under one tag — and at most once per prompt (assembly
/// skips it when the task message already carries the tag).
///
/// `None` when the memory is empty: an absent section costs zero tokens.
pub fn wave_memory_section(memory: &str) -> Option<String> {
    let trimmed = memory.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(format!("<lf:wave-memory>\n{trimmed}\n</lf:wave-memory>"))
}

/// Sections run stable → volatile so providers can prefix-cache the front of
/// the seed: goal prompt and flow list rarely change between passes, while
/// MEMORY.md is rewritten every pass — it goes last so a memory edit doesn't
/// invalidate the cacheable bytes ahead of it.
pub fn render_goal(goal: &Goal, ctx: &GoalRenderContext) -> String {
    let flows = if ctx.flows.is_empty() {
        "No flows are available.".to_string()
    } else {
        ctx.flows
            .iter()
            .map(|flow| format!("- {flow}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let memory = wave_memory_section(&ctx.memory).unwrap_or_else(|| {
        "<lf:wave-memory>\nNo wave memory is recorded.\n</lf:wave-memory>".to_string()
    });

    format!(
        "{}\n\n<lf:goal-context>\nAvailable flows:\n{}\n</lf:goal-context>\n\n{}",
        goal.prompt.trim(),
        flows,
        memory,
    )
}

/// Like `load_flow`, but resolves only exact-name matches in the builtin
/// catalog — no bare-name fallback across namespaced flows. Used when
/// expanding an already-loaded flow so a bare skill name like `review` can't
/// accidentally expand into `gstack/review`.
pub fn load_flow_strict(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    load_flow_inner(name, repo, false)
}

fn load_flow_inner(name: &str, repo: &Path, allow_bare_fallback: bool) -> Result<Flow, LoadError> {
    let (resolved_name, content) = match find_flow_path(name, repo) {
        Ok(flow_path) => (name.to_string(), fs::read_to_string(&flow_path)?),
        Err(LoadError::FlowNotFound(_)) => {
            let builtin_key = if allow_bare_fallback {
                crate::engine::builtins::resolve_builtin_flow(name)
            } else {
                crate::engine::builtins::get_builtin_flow(name).map(|_| {
                    // Re-look up the exact key by querying with same name.
                    // This is a static &str with the same lifetime as the map.
                    // Safe: we already know the key exists.
                    name_as_static_key(name).unwrap_or(name)
                })
            };

            if let Some(key) = builtin_key {
                let builtin = crate::engine::builtins::get_builtin_flow(key)
                    .expect("builtin flow lookup should succeed");
                (key.to_string(), builtin.to_string())
            } else if load_skill(name, repo).is_ok() {
                // Auto-wrap a skill name as a single-skill flow.
                return Ok(Flow {
                    name: name.to_string(),
                    items: vec![Step::Skill(SkillStep::named(name))],
                });
            } else {
                return Err(LoadError::FlowNotFound(name.to_string()));
            }
        }
        Err(err) => return Err(err),
    };
    let value: Value =
        serde_yaml_ng::from_str(&content).map_err(|err| LoadError::InvalidFlow(err.to_string()))?;
    let items = parse_flow_items(&value)?;
    Ok(Flow {
        name: resolved_name,
        items,
    })
}

/// Get the `&'static str` key matching `name` from the builtin flow map.
fn name_as_static_key(name: &str) -> Option<&'static str> {
    crate::engine::builtins::builtin_flow_names()
        .into_iter()
        .find(|k| *k == name)
}

pub fn expand_flow(flow: &Flow, repo: &Path) -> Result<Vec<ConcreteStep>, LoadError> {
    let items = expand_with_chain(flow, repo, vec![flow.name.clone()], 0)?;
    let mut ids = HashSet::new();
    validate_occurrence_ids(&items, &mut ids)?;
    validate_repeats(&items)?;
    Ok(items)
}

pub(crate) fn validate_repeats(items: &[ConcreteStep]) -> Result<(), LoadError> {
    for (index, item) in items.iter().enumerate() {
        if let ConcreteStep::Xor(branch) = item {
            for path in branch.paths.values() {
                validate_repeats(&path.steps)?;
            }
        }
        let ConcreteStep::Skill(skill) = item else {
            continue;
        };
        skill.policy.validate()?;
        let Some(repeat) = &skill.policy.repeat else {
            continue;
        };
        if !items[..index].iter().any(|item| {
            matches!(item, ConcreteStep::Skill(s) if s.policy.id.as_deref() == Some(&repeat.from))
        }) {
            return Err(LoadError::InvalidFlow(format!(
                "repeat from {:?} must name a preceding node", repeat.from
            )));
        }
    }
    Ok(())
}

fn validate_occurrence_ids(
    items: &[ConcreteStep],
    ids: &mut HashSet<String>,
) -> Result<(), LoadError> {
    for item in items {
        match item {
            ConcreteStep::Skill(skill) => {
                if let Some(id) = skill.policy.id.as_ref() {
                    if !ids.insert(id.clone()) {
                        return Err(LoadError::InvalidFlow(format!(
                            "flow occurrence id {id:?} is not unique after expansion"
                        )));
                    }
                }
            }
            ConcreteStep::Xor(branch) => {
                for path in branch.paths.values() {
                    validate_occurrence_ids(&path.steps, ids)?;
                }
            }
            ConcreteStep::Op(_) => {}
        }
    }
    Ok(())
}

pub fn human_occurrence_ids(flow: &Flow, repo: &Path) -> Result<Vec<String>, LoadError> {
    fn collect(items: &[ConcreteStep]) -> Vec<String> {
        let mut human = Vec::new();
        for item in items {
            match item {
                ConcreteStep::Skill(skill) if skill.policy.human => human.push(
                    skill
                        .policy
                        .id
                        .clone()
                        .expect("validated human occurrence has an id"),
                ),
                ConcreteStep::Xor(branch) => {
                    for path in branch.paths.values() {
                        human.extend(collect(&path.steps));
                    }
                }
                ConcreteStep::Skill(_) | ConcreteStep::Op(_) => {}
            }
        }
        human
    }

    Ok(collect(&expand_flow(flow, repo)?))
}

pub fn load_skill(name: &str, repo: &Path) -> Result<Skill, LoadError> {
    // Try file-based lookup first (repo-local, then global)
    if let Ok(skill_path) = find_skill_path(name, repo) {
        return load_skill_from_path(name, &skill_path);
    }

    // Fall back to built-in skills — exact match, then unique bare-name match
    // across namespaces.
    if let Some(key) = crate::engine::builtins::resolve_builtin_skill(name) {
        let content = crate::engine::builtins::get_builtin_skill(key)
            .expect("resolve_builtin_skill returned a known key");
        return skill_from_content(key, content);
    }

    // Fall back to .agents/skills/<name>/SKILL.md (user-installed, not loopflow-injected)
    if let Some(content) = load_agent_skill(name, repo) {
        warn_retired_interactive(name, &content);
        return skill_from_content(name, &content);
    }

    Err(LoadError::SkillNotFound(name.to_string()))
}

pub(crate) fn load_skill_from_path(name: &str, skill_path: &Path) -> Result<Skill, LoadError> {
    let content = fs::read_to_string(skill_path)?;
    warn_retired_interactive(name, &content);
    skill_from_content(name, &content)
}

fn warn_retired_interactive(name: &str, content: &str) {
    let has_interactive = split_frontmatter(content).is_some_and(|(frontmatter, _)| {
        serde_yaml_ng::from_str::<Value>(&frontmatter)
            .ok()
            .and_then(|value| value.as_mapping().cloned())
            .is_some_and(|map| map.contains_key(key("interactive")))
    });
    if has_interactive && !RETIRED_INTERACTIVE_WARNING.swap(true, Ordering::Relaxed) {
        eprintln!(
            "warning: skill {name:?} uses retired `interactive` frontmatter; direct TTY and --batch now select the launch surface"
        );
    }
}

#[derive(Debug, Default)]
struct SkillFrontmatter {
    agent: Option<String>,
    default_agent: Option<String>,
    action_style: Option<String>,
}

fn parse_skill_frontmatter(content: &str) -> Result<(SkillFrontmatter, String), LoadError> {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return Ok((SkillFrontmatter::default(), content.to_string()));
    };

    let value: Value = serde_yaml_ng::from_str(&frontmatter)
        .map_err(|err| LoadError::InvalidSkill(err.to_string()))?;
    Ok((parse_frontmatter_value(&value), body))
}

fn skill_from_content(name: &str, content: &str) -> Result<Skill, LoadError> {
    let (frontmatter, body) = parse_skill_frontmatter(content)?;
    Ok(Skill {
        name: name.to_string(),
        agent: frontmatter.agent,
        default_agent: frontmatter.default_agent,
        action_style: frontmatter.action_style,
        content: Some(body),
    })
}

pub(crate) fn split_frontmatter(content: &str) -> Option<(String, String)> {
    if !content.starts_with("---") {
        return None;
    }
    let mut parts = content.splitn(3, "---");
    let _ = parts.next();
    let frontmatter = parts.next()?;
    let rest = parts.next()?;
    let body = rest.strip_prefix('\n').unwrap_or(rest).to_string();
    Some((frontmatter.to_string(), body))
}

fn parse_frontmatter_value(value: &Value) -> SkillFrontmatter {
    let map = match value.as_mapping() {
        Some(map) => map,
        None => return SkillFrontmatter::default(),
    };

    let agent = parse_optional_string(map, "agent");
    let default_agent = parse_optional_string(map, "default_agent");
    let action_style = parse_optional_string(map, "action_style");
    SkillFrontmatter {
        agent,
        default_agent,
        action_style,
    }
}

fn first_existing_path(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    paths.into_iter().find(|path| path.exists())
}

fn paths_with_extensions(dir: &Path, name: &str, extensions: &[&str]) -> Vec<PathBuf> {
    extensions
        .iter()
        .map(|extension| dir.join(format!("{name}.{extension}")))
        .collect()
}

fn markdown_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.md"))
}

fn collect_flow_names(dir: &Path, prefix: Option<&str>, names: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && prefix.is_none() {
            let Some(child_prefix) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            collect_flow_names(&path, Some(child_prefix), names);
            continue;
        }

        if !path
            .extension()
            .is_some_and(|ext| ext == "yaml" || ext == "yml" || ext == "json")
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        match prefix {
            Some(prefix) => names.push(format!("{prefix}/{stem}")),
            None => names.push(stem.to_string()),
        }
    }
}

fn find_flow_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    // 1. Repo-local flows
    if let Some(path) = first_existing_path(paths_with_extensions(
        &repo.join(".lf/flows"),
        name,
        &["yaml", "yml", "json"],
    )) {
        return Ok(path);
    }

    // 2. Namespaced flows in subdirectories (.lf/flows/gstack/sprint.yaml)
    if let Some((prefix, flow_name)) = name.split_once('/') {
        if let Some(path) = first_existing_path(paths_with_extensions(
            &repo.join(".lf/flows").join(prefix),
            flow_name,
            &["yaml", "yml"],
        )) {
            return Ok(path);
        }
    }

    Err(LoadError::FlowNotFound(name.to_string()))
}

fn find_skill_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    // Namespaced skills: "team/review" → <dir>/.lf/skills/team/review.md
    // Check repo first, then home (so users can override namespaced builtins).
    if let Some((prefix, skill_name)) = name.split_once('/') {
        let repo_ns = markdown_path(&repo.join(".lf/skills").join(prefix), skill_name);
        if repo_ns.exists() {
            return Ok(repo_ns);
        }
        if let Some(home) = home_dir() {
            let home_ns = markdown_path(&home.join(".lf/skills").join(prefix), skill_name);
            if home_ns.exists() {
                return Ok(home_ns);
            }
        }
    }

    // 1. Check repo-local paths
    if let Some(path) = first_existing_path([
        markdown_path(&repo.join(".lf/skills"), name),
        markdown_path(&repo.join(".claude/commands"), name),
    ]) {
        return Ok(path);
    }

    // 2. Check global paths
    if let Some(home) = home_dir() {
        if let Some(path) = first_existing_path([
            markdown_path(&home.join(".lf/skills"), name),
            markdown_path(&home.join(".claude/commands"), name),
        ]) {
            return Ok(path);
        }
    }

    Err(LoadError::SkillNotFound(name.to_string()))
}

/// The editable file that supplied a file-backed skill. Built-in skills return
/// `None`: their embedded content has no source file in an installed binary.
pub fn find_skill_source_path(name: &str, repo: &Path) -> Option<PathBuf> {
    if let Ok(path) = find_skill_path(name, repo) {
        return Some(path);
    }
    if crate::engine::builtins::resolve_builtin_skill(name).is_some() {
        return None;
    }
    let path = agent_skill_path(name, repo);
    path.is_file().then_some(path)
}

fn find_goal_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    let wave_goal = repo.join("wave").join(name).join("GOAL.md");
    if exact_path_exists(&wave_goal) {
        return Ok(wave_goal);
    }

    if let Some((prefix, goal_name)) = name.split_once('/') {
        let repo_ns = markdown_path(&repo.join(".lf/goals").join(prefix), goal_name);
        if repo_ns.exists() {
            return Ok(repo_ns);
        }
        if let Some(home) = home_dir() {
            let home_ns = markdown_path(&home.join(".lf/goals").join(prefix), goal_name);
            if home_ns.exists() {
                return Ok(home_ns);
            }
        }
    }

    if let Some(path) = first_existing_path([markdown_path(&repo.join(".lf/goals"), name)]) {
        return Ok(path);
    }

    if let Some(home) = home_dir() {
        if let Some(path) = first_existing_path([markdown_path(&home.join(".lf/goals"), name)]) {
            return Ok(path);
        }
    }

    Err(LoadError::GoalNotFound(name.to_string()))
}

fn exact_path_exists(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(file_name) = path.file_name() else {
        return false;
    };
    std::fs::read_dir(parent).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .any(|entry| entry.file_name() == file_name)
    })
}

/// Load a skill from `.agents/skills/<name>/SKILL.md` if it exists.
fn load_agent_skill(name: &str, repo: &Path) -> Option<String> {
    fs::read_to_string(agent_skill_path(name, repo)).ok()
}

fn agent_skill_path(name: &str, repo: &Path) -> PathBuf {
    repo.join(".agents/skills").join(name).join("SKILL.md")
}

// -----------------------------------------------------------------------------
// YAML parsing helpers
// -----------------------------------------------------------------------------

fn key(s: &str) -> Value {
    Value::String(s.to_string())
}

fn parse_flow_items(value: &Value) -> Result<Vec<Step>, LoadError> {
    match value {
        Value::Sequence(seq) => seq.iter().map(parse_flow_item).collect(),
        Value::Mapping(map) => {
            if let Some(skills) = map.get(key("steps")) {
                return parse_flow_items(skills);
            }
            Err(LoadError::InvalidFlow(
                "flow root must be a list".to_string(),
            ))
        }
        _ => Err(LoadError::InvalidFlow(
            "flow root must be a list".to_string(),
        )),
    }
}

fn parse_flow_item(value: &Value) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step::Skill(SkillStep::named(name))),
        Value::Mapping(map) => parse_flow_mapping(map),
        _ => Err(LoadError::InvalidFlow(
            "flow item must be string or mapping".to_string(),
        )),
    }
}

fn parse_flow_mapping(map: &serde_yaml_ng::Mapping) -> Result<Step, LoadError> {
    if let Some(skill_value) = map.get(key("step")) {
        if map.len() != 1 {
            return Err(LoadError::InvalidFlow(
                "step metadata belongs inside the step mapping".to_string(),
            ));
        }
        return Ok(Step::Skill(parse_skill_value(skill_value)?));
    }
    if ["id", "human"]
        .iter()
        .any(|field| map.contains_key(key(field)))
    {
        return Err(LoadError::InvalidFlow(
            "occurrence metadata are valid only on skill nodes".to_string(),
        ));
    }
    if let Some(flow_value) = map.get(key("flow")) {
        return parse_flow_ref_value(flow_value);
    }
    if let Some(op_value) = map.get(key("op")) {
        return parse_op_value(op_value, "op");
    }
    if let Some(xor_value) = map.get(key("xor")) {
        return parse_xor_value(xor_value);
    }
    Err(LoadError::InvalidFlow(
        "flow item mapping must include step, op, flow, or xor".to_string(),
    ))
}

fn parse_op_value(value: &Value, field_name: &str) -> Result<Step, LoadError> {
    let raw = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidFlow(format!("{field_name} value must be string")))?
        .trim();

    if raw.is_empty() {
        return Err(LoadError::InvalidFlow(format!(
            "{field_name} value must include a command"
        )));
    }

    let mut parts = raw.split_whitespace();
    let command = parts
        .next()
        .ok_or_else(|| {
            LoadError::InvalidFlow(format!("{field_name} value must include a command"))
        })?
        .to_string();
    let args = parts.map(ToString::to_string).collect();

    Ok(Step::Op(Op { command, args }))
}

fn parse_skill_value(value: &Value) -> Result<SkillStep, LoadError> {
    match value {
        Value::String(name) => Ok(SkillStep::named(name)),
        Value::Mapping(map) => {
            if map.contains_key(key("feedback")) || map.contains_key(key("interactive")) {
                return Err(LoadError::InvalidFlow(
                    "feedback and interactive flow metadata are retired; use stable id plus human"
                        .to_string(),
                ));
            }
            let mut step: SkillStep =
                serde_yaml_ng::from_value(value.clone()).map_err(|error| {
                    LoadError::InvalidFlow(format!("invalid step mapping: {error}"))
                })?;
            step.policy.validate()?;
            step.skill.content = None;
            Ok(step)
        }
        _ => Err(LoadError::InvalidFlow(
            "step value must be string or mapping".to_string(),
        )),
    }
}

fn parse_xor_value(value: &Value) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("xor must be mapping".to_string()))?;
    Ok(Step::Xor(parse_xor_def(map, "xor")?))
}

fn parse_xor_def(map: &serde_yaml_ng::Mapping, kind: &str) -> Result<XorDef, LoadError> {
    let kind_prefix = if kind.is_empty() { "xor" } else { kind };

    let paths_value = map
        .get(key("paths"))
        .ok_or_else(|| LoadError::InvalidFlow(format!("{kind_prefix} must have paths")))?;
    let paths_map = paths_value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow(format!("{kind_prefix} paths must be mapping")))?;

    if paths_map.is_empty() {
        return Err(LoadError::InvalidFlow(format!(
            "{kind_prefix} must have at least one path"
        )));
    }

    let mut paths = HashMap::new();
    for (path_key, path_value) in paths_map {
        let key_str = path_key.as_str().ok_or_else(|| {
            LoadError::InvalidFlow(format!("{kind_prefix} path key must be string"))
        })?;
        let path_map = path_value.as_mapping().ok_or_else(|| {
            LoadError::InvalidFlow(format!("{kind_prefix} path '{key_str}' must be mapping"))
        })?;

        let flow = parse_optional_string(path_map, "flow");
        let skill = parse_optional_string(path_map, "skill");
        let skills = parse_xor_path_skills(path_map, key_str, kind_prefix)?;

        let target_count = usize::from(flow.is_some())
            + usize::from(skill.is_some())
            + usize::from(!skills.is_empty());
        if target_count > 1 {
            return Err(LoadError::InvalidFlow(format!(
                "{kind_prefix} path '{key_str}' cannot have more than one of flow, skill, or steps"
            )));
        }

        let description = parse_optional_string(path_map, "description").ok_or_else(|| {
            LoadError::InvalidFlow(format!(
                "{kind_prefix} path '{key_str}' must have description"
            ))
        })?;

        paths.insert(
            key_str.to_string(),
            XorPath {
                flow,
                skill,
                steps: skills,
                description,
            },
        );
    }

    let router = parse_optional_string(map, "router");

    Ok(XorDef { router, paths })
}

pub fn build_xor_routing_suffix(xor_def: &ConcreteXor) -> String {
    let mut suffix = String::from(
        "## Routing\n\nAfter completing your analysis, choose one of these paths:\n\n",
    );
    let mut keys: Vec<&String> = xor_def.paths.keys().collect();
    keys.sort();
    for key in &keys {
        let path = &xor_def.paths[*key];
        suffix.push_str(&format!("- **{key}**: {}\n", path.description));
    }
    suffix.push_str(
        "\nRecord your choice with `lf flow route PATH`, replacing PATH with one listed key.\n\
         Explain your reasoning briefly. Your final prose alone does not select a path.\n",
    );
    suffix
}

fn expand_xor_path(
    path: &XorPath,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<Vec<ConcreteStep>, LoadError> {
    if let Some(flow_name) = &path.flow {
        let flow = load_flow(flow_name, repo)?;
        check_flow_cycle(chain, &flow.name)?;
        return expand_with_chain(&flow, repo, chain_with(chain, &flow.name), depth + 1);
    }

    if let Some(skill_name) = &path.skill {
        return Ok(vec![ConcreteStep::Skill(ConcreteSkill {
            skill: load_skill(skill_name, repo)?,
            policy: OccurrencePolicy::default(),
            flow_parents: chain.to_vec(),
        })]);
    }

    path.steps
        .iter()
        .map(|step| {
            Ok(ConcreteStep::Skill(ConcreteSkill {
                skill: resolve_skill_reference(&step.skill, repo)?,
                policy: step.policy.clone(),
                flow_parents: chain.to_vec(),
            }))
        })
        .collect()
}

fn expand_branch_def(
    branch_def: &XorDef,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteXor, LoadError> {
    let router = match &branch_def.router {
        Some(name) => load_skill(name, repo)?,
        None => Skill {
            name: "xor-route".to_string(),
            agent: None,
            default_agent: Some("claude:sonnet".to_string()),
            action_style: None,
            content: Some(
                "Read the preceding findings and choose the right path forward. \
                 Record the choice with `lf flow route PATH` using a listed path key."
                    .to_string(),
            ),
        },
    };
    let paths = branch_def
        .paths
        .iter()
        .map(|(name, path)| {
            Ok((
                name.clone(),
                ConcretePath {
                    description: path.description.clone(),
                    steps: expand_xor_path(path, repo, chain, depth)?,
                },
            ))
        })
        .collect::<Result<_, LoadError>>()?;
    Ok(ConcreteXor {
        router,
        paths,
        flow_parents: chain.to_vec(),
    })
}

fn parse_xor_path_skills(
    map: &serde_yaml_ng::Mapping,
    path_name: &str,
    kind: &str,
) -> Result<Vec<SkillStep>, LoadError> {
    let Some(value) = map.get(key("steps")) else {
        return Ok(Vec::new());
    };

    let Value::Sequence(items) = value else {
        return Err(LoadError::InvalidFlow(format!(
            "{kind} path '{path_name}' skills must be a list"
        )));
    };

    items
        .iter()
        .map(|item| match item {
            Value::String(name) => Ok(SkillStep::named(name)),
            Value::Mapping(skill_map) => {
                if let Some(skill_value) = skill_map.get(key("step")) {
                    return parse_skill_value(skill_value);
                }
                parse_skill_value(item)
            }
            _ => Err(LoadError::InvalidFlow(format!(
                "{kind} path '{path_name}' skills must contain only skill items"
            ))),
        })
        .collect()
}

fn parse_flow_ref_value(value: &Value) -> Result<Step, LoadError> {
    let name = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidFlow("flow ref must be string".to_string()))?;
    Ok(Step::FlowRef(name.to_string()))
}

fn parse_optional_string(map: &serde_yaml_ng::Mapping, field: &str) -> Option<String> {
    map.get(key(field))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn resolve_skill_reference(skill: &Skill, repo: &Path) -> Result<Skill, LoadError> {
    if skill.content.is_some() {
        return Ok(skill.clone());
    }

    let mut resolved = load_skill(&skill.name, repo)?;

    if let Some(agent) = &skill.agent {
        resolved.agent = Some(agent.clone());
    }
    if let Some(default_agent) = &skill.default_agent {
        resolved.default_agent = Some(default_agent.clone());
    }
    if let Some(action_style) = &skill.action_style {
        resolved.action_style = Some(action_style.clone());
    }
    Ok(resolved)
}

fn check_flow_cycle(chain: &[String], name: &str) -> Result<(), LoadError> {
    if chain.iter().any(|parent| parent == name) {
        return Err(LoadError::InvalidFlow(format!(
            "flow cycle detected: {} -> {name}",
            chain.join(" ")
        )));
    }
    Ok(())
}

fn expand_with_chain(
    flow: &Flow,
    repo: &Path,
    chain: Vec<String>,
    depth: usize,
) -> Result<Vec<ConcreteStep>, LoadError> {
    const MAX_DEPTH: usize = 5;
    if depth > MAX_DEPTH {
        return Err(LoadError::InvalidFlow(format!(
            "flow nesting exceeds max depth {MAX_DEPTH}"
        )));
    }

    let mut items = Vec::new();
    for item in &flow.items {
        match item {
            Step::Skill(step) => {
                // A plain string in flow YAML is parsed as Skill, but it might
                // actually be a sub-flow name. If the skill has no inline content,
                // check if a flow with this name exists and expand it.
                if let Some(nested) = try_load_multi_skill_flow(&step.skill, repo, &chain)? {
                    items.extend(expand_with_chain(
                        &nested,
                        repo,
                        chain_with(&chain, &step.skill.name),
                        depth + 1,
                    )?);
                    continue;
                }
                items.push(ConcreteStep::Skill(ConcreteSkill {
                    skill: resolve_skill_reference(&step.skill, repo)?,
                    policy: step.policy.clone(),
                    flow_parents: chain.clone(),
                }));
            }
            Step::FlowRef(name) => {
                let nested = load_flow(name, repo)?;
                check_flow_cycle(&chain, &nested.name)?;
                items.extend(expand_with_chain(
                    &nested,
                    repo,
                    chain_with(&chain, &nested.name),
                    depth + 1,
                )?);
            }
            Step::Op(item) => {
                items.push(ConcreteStep::Op(ConcreteOp {
                    item: item.clone(),
                    flow_parents: chain.clone(),
                }));
            }
            Step::Xor(branch_def) => {
                items.push(ConcreteStep::Xor(expand_branch_def(
                    branch_def, repo, &chain, depth,
                )?));
            }
        }
    }

    Ok(items)
}

/// Check whether a loaded flow is a genuine multi-skill flow vs a single skill
/// auto-wrapped by `load_flow`. Returns `true` if the flow should be expanded.
fn is_multi_skill_flow(flow: &Flow, skill_name: &str) -> bool {
    flow.items.len() > 1
        || flow
            .items
            .first()
            .map(|i| !matches!(i, Step::Skill(s) if s.skill.name == skill_name))
            .unwrap_or(false)
}

fn try_load_multi_skill_flow(
    skill: &Skill,
    repo: &Path,
    chain: &[String],
) -> Result<Option<Flow>, LoadError> {
    if skill.content.is_some() {
        return Ok(None);
    }
    // A flow may run its same-named skill (for example launch-plan).
    // Only a real skill can terminate this otherwise recursive reference.
    if chain.contains(&skill.name) && load_skill(&skill.name, repo).is_ok() {
        return Ok(None);
    }

    // Strict resolution inside an expanding flow: a bare skill name like `review`
    // must not auto-escalate to `gstack/review`. Only exact-key matches.
    let flow = match load_flow_strict(&skill.name, repo) {
        Ok(flow) => flow,
        Err(LoadError::FlowNotFound(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    if !is_multi_skill_flow(&flow, &skill.name) {
        return Ok(None);
    }
    check_flow_cycle(chain, &flow.name)?;
    Ok(Some(flow))
}

fn chain_with(chain: &[String], name: &str) -> Vec<String> {
    let mut nested_chain = chain.to_vec();
    nested_chain.push(name.to_string());
    nested_chain
}

/// Home directory for global lookups. Can be overridden for testing.
fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use serde_yaml_ng::Value;

    use super::{
        build_xor_routing_suffix, expand_branch_def, expand_flow, find_skill_source_path,
        human_occurrence_ids, load_flow, load_goal, load_skill, parse_flow_items, render_goal,
        ConcretePath, ConcreteStep, ConcreteXor, Flow, Goal, GoalRenderContext, SkillStep, Step,
        XorDef, XorPath,
    };
    use crate::engine::error::LoadError;
    use tempfile::TempDir;

    #[test]
    fn load_skill_finds_repo_local_skill() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("myskill.md"), "# My Skill\nDo the thing.").unwrap();

        let skill = load_skill("myskill", tmp.path()).unwrap();
        assert_eq!(skill.name, "myskill");
        assert!(skill.content.unwrap().contains("Do the thing"));
    }

    #[test]
    fn load_goal_finds_repo_goal_override() {
        let tmp = TempDir::new().unwrap();
        let goals_dir = tmp.path().join(".lf/goals");
        fs::create_dir_all(&goals_dir).unwrap();
        fs::write(goals_dir.join("ship-roadmap.md"), "Repo goal prompt.").unwrap();

        let goal = load_goal("ship-roadmap", tmp.path()).unwrap();
        assert_eq!(goal.prompt, "Repo goal prompt.");
    }

    #[test]
    fn load_goal_prefers_wave_goal_md() {
        let tmp = TempDir::new().unwrap();
        let goals_dir = tmp.path().join(".lf/goals");
        let wave_dir = tmp.path().join("wave/goals");
        fs::create_dir_all(&goals_dir).unwrap();
        fs::create_dir_all(&wave_dir).unwrap();
        fs::write(goals_dir.join("goals.md"), "Repo goal prompt.").unwrap();
        fs::write(
            wave_dir.join("GOAL.md"),
            "---\nmetrics:\n  - tests pass\n---\nWave goal prompt.",
        )
        .unwrap();

        let goal = load_goal("goals", tmp.path()).unwrap();
        assert_eq!(goal.prompt, "Wave goal prompt.");
    }

    #[test]
    fn load_goal_ignores_legacy_goal_paths() {
        let tmp = TempDir::new().unwrap();
        let singular_dir = tmp.path().join(".lf/goal");
        let root_dir = tmp.path().join("goal");
        let wave_dir = tmp.path().join("wave/custom");
        fs::create_dir_all(&singular_dir).unwrap();
        fs::create_dir_all(&root_dir).unwrap();
        fs::create_dir_all(&wave_dir).unwrap();
        fs::write(singular_dir.join("custom.md"), "Singular goal.").unwrap();
        fs::write(root_dir.join("custom.md"), "Root goal.").unwrap();
        fs::write(wave_dir.join("goal.md"), "Lowercase wave goal.").unwrap();

        let err = load_goal("custom", tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::GoalNotFound(name) if name == "custom"));
    }

    #[test]
    fn human_skill_nodes_require_stable_ids() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("review.yaml"),
            "- step:\n    name: review-design\n    human: true\n",
        )
        .unwrap();

        let error = load_flow("review", tmp.path()).unwrap_err();
        assert!(error.to_string().contains("require a stable id"));

        fs::write(
            flows.join("retired.yaml"),
            "- step:\n    name: review-design\n    feedback: true\n",
        )
        .unwrap();
        let error = load_flow("retired", tmp.path()).unwrap_err();
        assert!(error.to_string().contains("metadata are retired"));
    }

    #[test]
    fn backward_edges_require_an_earlier_target() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        for from in ["missing", "review"] {
            fs::write(flows.join("repeat-proof.yaml"), format!(
                "- step:\n    id: implement\n    name: implement\n- step:\n    id: review\n    name: loop-decide\n    repeat:\n      from: {from}\n",
            )).unwrap();
            let result = load_flow("repeat-proof", tmp.path())
                .and_then(|flow| expand_flow(&flow, tmp.path()));
            assert!(result.unwrap_err().to_string().contains("preceding node"));
        }
    }

    #[test]
    fn human_reviews_return_feedback_to_explicit_deciding_occurrences() {
        let tmp = TempDir::new().unwrap();
        let steps = expand_flow(&load_flow("feature", tmp.path()).unwrap(), tmp.path()).unwrap();
        let edges: Vec<_> = steps
            .iter()
            .filter_map(|step| match step {
                ConcreteStep::Skill(skill) => {
                    if skill.policy.human {
                        assert!(skill.policy.repeat.is_none());
                    }
                    skill
                        .policy
                        .repeat
                        .as_ref()
                        .map(|edge| (skill.skill.name.as_str(), edge.from.as_str()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            edges,
            [("loop-decide", "implement"), ("loop-decide", "implement")]
        );
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(flows.join("invalid.yaml"), "- step: {id: implement, name: implement}\n- step: {id: demo, name: demo, human: true, repeat: {from: implement}}\n").unwrap();
        assert!(load_flow("invalid", tmp.path())
            .unwrap_err()
            .to_string()
            .contains("human steps return feedback"));
    }

    #[test]
    fn flow_steps_ignore_retired_skill_capabilities() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("fake-implementation.yaml"),
            "- step:\n    name: design\n    capabilities: [task_implementation]\n",
        )
        .unwrap();

        let flow = load_flow("fake-implementation", tmp.path()).expect("load flow");
        let steps = expand_flow(&flow, tmp.path()).expect("expand flow");

        assert!(matches!(
            &steps[0],
            ConcreteStep::Skill(skill) if skill.skill.name == "design"
        ));
    }

    #[test]
    fn nested_expansion_preserves_human_occurrence_identity() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("inner.yaml"),
            "- step:\n    id: revise\n    name: implement\n- step:\n    id: review_kickoff\n    name: review-design\n    human: true\n",
        )
        .unwrap();
        fs::write(flows.join("outer.yaml"), "- flow: inner\n").unwrap();

        let flow = load_flow("outer", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        let ConcreteStep::Skill(step) = &items[1] else {
            panic!("nested review node must expand to a skill")
        };
        assert_eq!(step.policy.id.as_deref(), Some("review_kickoff"));
        assert!(step.policy.human);
        assert_eq!(step.flow_parents, vec!["outer", "inner"]);
    }

    #[test]
    fn expanded_occurrence_ids_are_unique_and_skill_only() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(
            flows.join("inner.yaml"),
            "- step:\n    id: review_kickoff\n    name: review-design\n    human: true\n",
        )
        .unwrap();
        fs::write(
            flows.join("duplicate.yaml"),
            "- flow: inner\n- flow: inner\n",
        )
        .unwrap();
        let duplicate = load_flow("duplicate", tmp.path()).unwrap();
        assert!(expand_flow(&duplicate, tmp.path())
            .unwrap_err()
            .to_string()
            .contains("not unique after expansion"));

        fs::write(flows.join("invalid.yaml"), "- op: rebase\n  human: true\n").unwrap();
        assert!(load_flow("invalid", tmp.path())
            .unwrap_err()
            .to_string()
            .contains("valid only on skill nodes"));
    }

    #[test]
    fn render_goal_includes_flows_and_memory() {
        let goal = Goal {
            prompt: "Drive the work.".to_string(),
        };
        let rendered = render_goal(
            &goal,
            &GoalRenderContext {
                flows: vec!["build".to_string(), "qa".to_string()],
                memory: "Last loop found the docs drift.".to_string(),
            },
        );

        assert!(rendered.contains("Drive the work."));
        assert!(rendered.contains("<lf:wave-memory>"));
        assert!(rendered.contains("Last loop found the docs drift."));
        assert!(rendered.contains("- build"));
        assert!(rendered.contains("- qa"));
        // Stable → volatile: memory is rewritten between passes, so it must
        // trail the stable goal and flow list to keep the prefix cacheable.
        let memory_at = rendered.find("<lf:wave-memory>").expect("memory section");
        let flows_at = rendered.find("<lf:goal-context>").expect("flow section");
        assert!(flows_at < memory_at, "memory renders after the flow list");
    }

    #[test]
    fn render_goal_handles_empty_memory() {
        let goal = Goal {
            prompt: "Drive the work.".to_string(),
        };
        let rendered = render_goal(
            &goal,
            &GoalRenderContext {
                flows: Vec::new(),
                memory: String::new(),
            },
        );

        assert!(rendered.contains("<lf:wave-memory>\nNo wave memory is recorded."));
        assert!(rendered.contains("No flows are available."));
    }

    #[test]
    fn load_skill_rejects_legacy_colon_form() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills/gstack");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# Office Hours\nDo the thing.\n",
        )
        .unwrap();

        // The colon form is no longer accepted; users must use `/`.
        let err = load_skill("gstack:office-hours", tmp.path()).unwrap_err();
        assert!(matches!(err, LoadError::SkillNotFound(_)));
    }

    #[test]
    fn load_skill_finds_namespaced_skill_with_slash() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills/gstack");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# Office Hours\nDo the thing.\n",
        )
        .unwrap();

        let skill = load_skill("gstack/office-hours", tmp.path()).unwrap();
        assert_eq!(skill.name, "gstack/office-hours");
        assert!(skill.content.unwrap().contains("Do the thing"));
    }

    #[test]
    fn load_skill_finds_builtin_skill() {
        // Builtin skills like "debug", "implement" should be available everywhere
        let tmp = TempDir::new().unwrap();

        let result = load_skill("debug", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'debug' skill should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_skill_finds_all_builtins() {
        let tmp = TempDir::new().unwrap();
        for name in crate::engine::builtins::builtin_skill_names() {
            let result = load_skill(name, tmp.path());
            assert!(
                result.is_ok(),
                "builtin '{}' skill should be found: {:?}",
                name,
                result.err()
            );
        }
    }

    #[test]
    fn load_skill_parses_frontmatter_agent() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("fast.md"),
            r#"---
agent: claude:haiku
---
# Fast Skill
Do it quickly.
"#,
        )
        .unwrap();

        let skill = load_skill("fast", tmp.path()).unwrap();
        assert_eq!(skill.agent, Some("claude:haiku".to_string()));
    }

    #[test]
    fn load_skill_parses_frontmatter_default_agent() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("fast.md"),
            r#"---
default_agent: codex:o3
---
# Fast Skill
Do it quickly.
"#,
        )
        .unwrap();

        let skill = load_skill("fast", tmp.path()).unwrap();
        assert_eq!(skill.default_agent, Some("codex:o3".to_string()));
    }

    #[test]
    fn load_skill_ignores_retired_frontmatter_interactive() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("design.md"),
            r#"---
interactive: true
---
# Design Skill
Design the feature.
"#,
        )
        .unwrap();

        let skill = load_skill("design", tmp.path()).unwrap();
        assert_eq!(skill.name, "design");
        assert!(skill.content.unwrap().contains("Design the feature."));
    }

    #[test]
    fn load_skill_ignores_retired_capabilities_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("design.md"),
            r#"---
action_style: exploratory
capabilities: [task_implementation]
---
# Design Skill
Design the feature.
"#,
        )
        .unwrap();

        let skill = load_skill("design", tmp.path()).unwrap();
        assert_eq!(skill.action_style.as_deref(), Some("exploratory"));
        assert!(skill.content.unwrap().contains("Design the feature."));
    }

    #[test]
    fn load_skill_not_found_error_message() {
        let tmp = TempDir::new().unwrap();

        let result = load_skill("nonexistent", tmp.path());
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_flow_finds_builtin_flow() {
        let tmp = TempDir::new().unwrap();
        let result = load_flow("build", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'build' flow should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_skill_falls_back_to_agent_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".agents/skills/my-tool");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-tool\n---\nDo the thing.",
        )
        .unwrap();

        let skill = load_skill("my-tool", tmp.path()).unwrap();
        assert_eq!(skill.name, "my-tool");
        assert!(skill.content.unwrap().contains("Do the thing."));
    }

    #[test]
    fn find_skill_source_path_finds_agent_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_path = tmp.path().join(".agents/skills/my-tool/SKILL.md");
        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, "Do the thing.").unwrap();

        assert_eq!(
            find_skill_source_path("my-tool", tmp.path()),
            Some(skill_path)
        );
    }

    #[test]
    fn load_flow_expands_all_builtin_flows() {
        let tmp = TempDir::new().unwrap();
        for name in crate::engine::builtins::builtin_flow_names() {
            let flow = load_flow(name, tmp.path());
            assert!(
                flow.is_ok(),
                "builtin flow '{}' should load: {:?}",
                name,
                flow.err()
            );
            let flow = flow.unwrap();
            let expanded = expand_flow(&flow, tmp.path());
            assert!(
                expanded.is_ok(),
                "builtin flow '{}' should expand: {:?}",
                name,
                expanded.err()
            );
            assert_captured(&expanded.unwrap());
        }

        fn assert_captured(items: &[ConcreteStep]) {
            for item in items {
                match item {
                    ConcreteStep::Skill(skill) => assert!(
                        skill.skill.content.is_some(),
                        "missing captured skill {}",
                        skill.skill.name,
                    ),
                    ConcreteStep::Op(op) => assert!(!op.item.command.is_empty()),
                    ConcreteStep::Xor(branch) => {
                        assert!(branch.router.content.is_some());
                        for path in branch.paths.values() {
                            assert_captured(&path.steps);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn retired_and_step_is_rejected() {
        let yaml = r#"
- and:
    branches:
      - step: implement
      - step: review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_flow_items(&value).expect_err("and steps are retired");
        assert!(error
            .to_string()
            .contains("flow item mapping must include step, op, flow, or xor"));
    }

    #[test]
    fn unimplemented_or_step_is_rejected() {
        let yaml = r#"
- or:
    paths:
      fix:
        step: implement
        description: "Fix it"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_flow_items(&value).expect_err("or steps are not supported");
        assert!(error
            .to_string()
            .contains("flow item mapping must include step, op, flow, or xor"));
    }

    #[test]
    fn generic_loop_step_is_rejected() {
        let yaml = r#"
- loop:
    steps: [implement]
    exit:
      paths:
        done:
          description: "Done"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let error = parse_flow_items(&value).expect_err("generic loops are retired");
        assert!(error
            .to_string()
            .contains("flow item mapping must include step, op, flow, or xor"));
    }

    #[test]
    fn parse_ops_mapping_accepts_command_and_args() {
        let yaml = r#"
- op: pr land
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::Op(item) => {
                assert_eq!(item.command, "pr");
                assert_eq!(item.args, vec!["land"]);
            }
            other => panic!("expected Ops item, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_with_flow_paths() {
        let yaml = r#"
- qa
- triage
- xor:
    paths:
      fix:
        flow: qa-fix
        description: "Blocking issues found, fix before deploy"
      deploy:
        flow: deploy
        description: "Clean enough to ship"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 3);

        match &items[2] {
            Step::Xor(branch) => {
                assert_eq!(branch.paths.len(), 2);
                let fix = &branch.paths["fix"];
                assert_eq!(fix.flow.as_deref(), Some("qa-fix"));
                assert!(fix.skill.is_none());
                assert_eq!(fix.description, "Blocking issues found, fix before deploy");
                let deploy = &branch.paths["deploy"];
                assert_eq!(deploy.flow.as_deref(), Some("deploy"));
                assert_eq!(deploy.description, "Clean enough to ship");
            }
            other => panic!("expected Xor, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_with_skill_path() {
        let yaml = r#"
- xor:
    paths:
      skip:
        skill: gate
        description: "Just run gate"
      full:
        flow: build
        description: "Full build"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::Xor(branch) => {
                let skip = &branch.paths["skip"];
                assert_eq!(skip.skill.as_deref(), Some("gate"));
                assert!(skip.flow.is_none());
                let full = &branch.paths["full"];
                assert_eq!(full.flow.as_deref(), Some("build"));
                assert!(full.skill.is_none());
            }
            other => panic!("expected Xor, got {other:?}"),
        }
    }

    #[test]
    fn parse_xor_rejects_both_flow_and_skill() {
        let yaml = r#"
- xor:
    paths:
      bad:
        flow: build
        skill: gate
        description: "invalid"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_flow_items(&value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("flow") && err.contains("skill"),
            "expected error about both flow and skill, got: {err}"
        );
    }

    #[test]
    fn parse_xor_allows_silence_path() {
        let yaml = r#"
- xor:
    paths:
      silence:
        description: "no action needed"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_flow_items(&value);
        assert!(
            result.is_ok(),
            "path with only description should be valid (silence path)"
        );
    }

    #[test]
    fn parse_xor_rejects_missing_description() {
        let yaml = r#"
- xor:
    paths:
      bad:
        flow: build
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_flow_items(&value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("description"),
            "expected error about missing description, got: {err}"
        );
    }

    #[test]
    fn parse_xor_accepts_inline_skills() {
        let yaml = r#"
- xor:
    paths:
      tune:
        description: "Adjust the chord"
        steps:
          - implement
          - review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();

        let Step::Xor(xor_def) = &items[0] else {
            panic!("expected xor item");
        };
        let tune = &xor_def.paths["tune"];
        assert_eq!(tune.flow, None);
        assert_eq!(tune.skill, None);
        assert_eq!(tune.steps.len(), 2);
        assert_eq!(tune.steps[0].skill.name, "implement");
        assert_eq!(tune.steps[1].skill.name, "review");
    }

    #[test]
    fn parse_xor_rejects_multiple_targets() {
        let yaml = r#"
- xor:
    paths:
      bad:
        description: "invalid"
        skill: implement
        steps:
          - review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let err = parse_flow_items(&value).unwrap_err().to_string();
        assert!(err.contains("flow, skill, or steps"));
    }

    #[test]
    fn expand_xor_keeps_concrete_xor() {
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-or".to_string(),
            items: vec![
                Step::Skill(SkillStep::named("gate")),
                Step::Xor(XorDef {
                    router: None,
                    paths: {
                        let mut m = HashMap::new();
                        m.insert(
                            "fix".to_string(),
                            XorPath {
                                flow: Some("build".to_string()),
                                skill: None,
                                steps: Vec::new(),
                                description: "Fix it".to_string(),
                            },
                        );
                        m
                    },
                }),
            ],
        };

        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], ConcreteStep::Skill(_)));
        assert!(matches!(&items[1], ConcreteStep::Xor(_)));

        if let ConcreteStep::Xor(branch) = &items[1] {
            assert_eq!(branch.paths.len(), 1);
            assert_eq!(branch.paths["fix"].description, "Fix it");
        }
    }

    #[test]
    fn build_xor_routing_suffix_sorts_paths() {
        let tmp = TempDir::new().unwrap();
        let branch = expand_branch_def(
            &XorDef {
                router: None,
                paths: HashMap::from([
                    (
                        "zeta".to_string(),
                        XorPath {
                            flow: None,
                            skill: None,
                            steps: Vec::new(),
                            description: "Last".to_string(),
                        },
                    ),
                    (
                        "alpha".to_string(),
                        XorPath {
                            flow: None,
                            skill: None,
                            steps: Vec::new(),
                            description: "First".to_string(),
                        },
                    ),
                ]),
            },
            tmp.path(),
            &[],
            0,
        )
        .unwrap();
        let suffix = build_xor_routing_suffix(&branch);
        assert!(suffix.find("**alpha**").unwrap() < suffix.find("**zeta**").unwrap());
        assert!(suffix.contains("lf flow route PATH"));
        assert!(branch
            .router
            .content
            .unwrap()
            .contains("lf flow route PATH"));
        assert!(branch.paths.values().all(|path| path.steps.is_empty()));
    }

    #[test]
    fn xor_snapshot_preserves_all_routes_and_skills_after_source_deletion() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        let skills = tmp.path().join(".lf/skills");
        fs::create_dir_all(&flows).unwrap();
        fs::create_dir_all(&skills).unwrap();
        for (name, content) in [
            ("pin-router", "---\nagent: codex\n---\nOriginal router"),
            ("pin-chosen", "Original chosen"),
            ("pin-other", "Original other"),
        ] {
            fs::write(skills.join(format!("{name}.md")), content).unwrap();
        }
        fs::write(
            flows.join("pin-root.yaml"),
            r#"
- xor:
    router: pin-router
    paths:
      chosen:
        description: Chosen description
        skill: pin-chosen
      unchosen:
        description: Unchosen description
        flow: pin-inner
"#,
        )
        .unwrap();
        fs::write(
            flows.join("pin-inner.yaml"),
            r#"
- xor:
    router: pin-router
    paths:
      nested:
        description: Nested description
        steps:
          - step:
              name: pin-other
              id: nested-work
              agent: claude
          - step:
              name: pin-other
              id: nested-decide
              repeat: {from: nested-work}
          - step:
              name: pin-other
              id: nested-human
              human: true
      quiet:
        description: Nothing to do
"#,
        )
        .unwrap();
        let flow = load_flow("pin-root", tmp.path()).unwrap();
        let pinned = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(
            human_occurrence_ids(&flow, tmp.path()).unwrap(),
            ["nested-human"]
        );
        let saved = serde_json::to_string(&pinned).unwrap();

        fs::write(skills.join("pin-router.md"), "Changed router").unwrap();
        fs::write(skills.join("pin-other.md"), "Changed other").unwrap();
        let changed = expand_flow(&flow, tmp.path()).unwrap();
        assert_ne!(changed, pinned);
        fs::remove_dir_all(tmp.path().join(".lf")).unwrap();
        let restored: Vec<ConcreteStep> = serde_json::from_str(&saved).unwrap();
        assert_eq!(restored, pinned);
        let ConcreteStep::Xor(root) = &restored[0] else {
            panic!("root router")
        };
        assert_eq!(root.router.content.as_deref(), Some("Original router"));
        assert_eq!(root.router.agent.as_deref(), Some("codex"));
        let ConcreteStep::Skill(chosen) = &root.paths["chosen"].steps[0] else {
            panic!("chosen skill")
        };
        assert_eq!(chosen.skill.content.as_deref(), Some("Original chosen"));
        let ConcreteStep::Xor(inner) = &root.paths["unchosen"].steps[0] else {
            panic!("unchosen router")
        };
        assert_eq!(inner.router, root.router);
        assert!(inner.paths["quiet"].steps.is_empty());
        let ConcreteStep::Skill(other) = &inner.paths["nested"].steps[0] else {
            panic!("nested skill")
        };
        assert_eq!(other.skill.content.as_deref(), Some("Original other"));
        assert_eq!(other.skill.agent.as_deref(), Some("claude"));
        assert_eq!(other.flow_parents, ["pin-root", "pin-inner"]);
        assert!(build_xor_routing_suffix(inner).contains("Nested description"));
    }

    #[test]
    fn xor_expansion_rejects_cycles_and_excessive_depth() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(flows.join("pin-cycle.yaml"), "- xor:\n    paths:\n      branch:\n        description: Recursive\n        flow: pin-child\n").unwrap();
        for child in ["- flow: pin-cycle\n", "- pin-cycle\n", "- xor:\n    paths:\n      again:\n        description: Recursive\n        flow: pin-cycle\n"] {
            fs::write(flows.join("pin-child.yaml"), child).unwrap();
            let flow = load_flow("pin-cycle", tmp.path()).unwrap();
            let error = expand_flow(&flow, tmp.path()).unwrap_err().to_string();
            assert!(error.contains("cycle detected"), "{error}");
        }
        for index in 0..7 {
            let next = index + 1;
            fs::write(flows.join(format!("deep-{index}.yaml")), format!("- xor:\n    paths:\n      deeper:\n        description: Deeper\n        flow: deep-{next}\n")).unwrap();
        }
        fs::write(flows.join("deep-7.yaml"), "- implement\n").unwrap();
        let error = expand_flow(&load_flow("deep-0", tmp.path()).unwrap(), tmp.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("max depth"), "{error}");
    }

    #[test]
    fn xor_expansion_validates_nested_edges_and_human_ids() {
        let tmp = TempDir::new().unwrap();
        let flows = tmp.path().join(".lf/flows");
        fs::create_dir_all(&flows).unwrap();
        fs::write(flows.join("pin-validation.yaml"), "- step:\n    name: implement\n    id: outside\n- xor:\n    paths:\n      nested:\n        description: Nested\n        flow: pin-invalid\n").unwrap();
        for (body, expected) in [
            ("- xor:\n    paths:\n      inline:\n        description: Invalid edge\n        steps:\n          - step:\n              name: loop-decide\n              id: deciding\n              repeat: {from: outside}\n", "preceding node"),
            ("- xor:\n    paths:\n      inline:\n        description: Invalid gate\n        steps:\n          - step:\n              name: demo\n              human: true\n", "stable id"),
            ("- xor:\n    paths:\n      inline:\n        description: Duplicate gate\n        steps:\n          - step:\n              name: demo\n              id: outside\n              human: true\n", "not unique"),
        ] {
            fs::write(flows.join("pin-invalid.yaml"), body).unwrap();
            let flow = load_flow("pin-validation", tmp.path()).unwrap();
            let error = expand_flow(&flow, tmp.path()).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn xor_expansion_rejects_missing_content_in_unchosen_paths() {
        let tmp = TempDir::new().unwrap();
        for target in [
            "skill: pin-missing-skill-23952",
            "steps: [pin-missing-skill-23952]",
            "flow: pin-missing-flow-23952",
        ] {
            let value: Value = serde_yaml_ng::from_str(&format!("- xor:\n    paths:\n      empty:\n        description: Empty\n      missing:\n        description: Missing\n        {target}\n")).unwrap();
            let flow = Flow {
                name: "pin-missing".to_string(),
                items: parse_flow_items(&value).unwrap(),
            };
            assert!(expand_flow(&flow, tmp.path())
                .unwrap_err()
                .to_string()
                .contains("not found"));
        }
    }

    #[test]
    fn xor_legacy_unresolved_records_fail_without_loading_sources() {
        for router in [serde_json::Value::Null, serde_json::json!("old-router")] {
            let old = serde_json::json!({
                "router": router,
                "paths": {"branch": {"flow": "mutable-flow", "skill": null, "description": "Old"}},
                "flow_parents": ["old-flow"]
            });
            assert!(serde_json::from_value::<ConcreteXor>(old).is_err());
        }
        let unresolved = serde_json::json!({"flow": "mutable-flow", "skill": null, "steps": [], "description": "Old"});
        let error = serde_json::from_value::<ConcretePath>(unresolved)
            .unwrap_err()
            .to_string();
        assert!(error.contains("unknown field"), "{error}");
    }

    #[test]
    fn code_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("code", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 2); // implement, compress
    }

    #[test]
    fn deploy_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("deploy", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 2); // gate, op: pr land
    }
}
