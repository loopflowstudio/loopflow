use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::engine::error::LoadError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Skill {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_agent: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl Skill {
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            agent: None,
            default_agent: None,
            directions: Vec::new(),
            action_style: None,
            interactive: None,
            content: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum Step {
    Skill(Skill),
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
    pub steps: Vec<Skill>,
    pub description: String,
    #[serde(default)]
    pub direction: Vec<String>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteSkill {
    pub skill: Skill,
    pub flow_parents: Vec<String>,
}

impl ConcreteSkill {
    pub fn display_path(&self) -> String {
        let mut parts = self.flow_parents.clone();
        parts.push(self.skill.name.clone());
        parts.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteXor {
    pub router: Option<String>,
    pub paths: HashMap<String, XorPath>,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteOp {
    pub item: Op,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcreteStep {
    Skill(ConcreteSkill),
    Op(ConcreteOp),
    Xor(ConcreteXor),
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub name: String,
    pub content: String,
    pub source: PathBuf,
}

pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    load_flow_inner(name, repo, true)
}

pub fn available_flow_names(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = crate::engine::builtins::builtin_flow_names()
        .into_iter()
        .map(ToOwned::to_owned)
        .collect();
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
        "{}\n\n{}\n\n<lf:goal-context>\nAvailable flows:\n{}\n</lf:goal-context>",
        goal.prompt.trim(),
        memory,
        flows,
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
                    items: vec![Step::Skill(Skill::named(name))],
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
    expand_with_chain(flow, repo, vec![flow.name.clone()], 0)
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
        return skill_from_content(name, &content);
    }

    Err(LoadError::SkillNotFound(name.to_string()))
}

pub(crate) fn load_skill_from_path(name: &str, skill_path: &Path) -> Result<Skill, LoadError> {
    let content = fs::read_to_string(skill_path)?;
    skill_from_content(name, &content)
}

#[derive(Debug, Default)]
struct SkillFrontmatter {
    agent: Option<String>,
    default_agent: Option<String>,
    directions: Vec<String>,
    action_style: Option<String>,
    interactive: Option<bool>,
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
        directions: frontmatter.directions,
        action_style: frontmatter.action_style,
        interactive: frontmatter.interactive,
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
    let interactive = map.get(key("interactive")).and_then(|val| val.as_bool());
    SkillFrontmatter {
        agent,
        default_agent,
        directions: parse_directions_field(map),
        action_style,
        interactive,
    }
}

fn parse_directions_field(map: &serde_yaml_ng::Mapping) -> Vec<String> {
    let directions = parse_string_list(map.get(key("directions")));
    if directions.is_empty() {
        parse_string_list(map.get(key("direction")))
    } else {
        directions
    }
}

pub fn load_direction(name: &str, repo: &Path) -> Result<Direction, LoadError> {
    let (content, source) = match find_direction_path(name, repo) {
        Ok(direction_path) => (fs::read_to_string(&direction_path)?, direction_path),
        Err(LoadError::DirectionNotFound(_)) => {
            if let Some(builtin) = crate::engine::builtins::get_builtin_direction(name) {
                (
                    builtin.to_string(),
                    PathBuf::from(format!("builtin:{name}")),
                )
            } else if let Some(content) = load_agent_skill(name, repo) {
                (
                    content,
                    repo.join(format!(".agents/skills/{name}/SKILL.md")),
                )
            } else {
                return Err(LoadError::DirectionNotFound(name.to_string()));
            }
        }
        Err(err) => return Err(err),
    };
    Ok(Direction {
        name: name.to_string(),
        content,
        source,
    })
}

/// Expand direction names, resolving groups to their member directions.
/// User groups (.lf/directions/{name}/ directory) are checked first, then builtin groups.
/// Non-group names pass through unchanged. Deduplicates while preserving order.
pub fn expand_direction_names(names: &[String], repo: &Path) -> Vec<String> {
    let mut expanded = Vec::new();
    let mut seen = HashSet::new();
    let mut queue: VecDeque<String> = names.iter().cloned().collect();
    while let Some(name) = queue.pop_front() {
        if !seen.insert(name.clone()) {
            continue;
        }
        match resolve_direction_group(&name, repo) {
            Some(members) => {
                for member in members {
                    queue.push_back(member);
                }
            }
            None => expanded.push(name),
        }
    }
    expanded
}

/// Check whether `name` is a direction group (user-defined directory or builtin group).
fn resolve_direction_group(name: &str, repo: &Path) -> Option<Vec<String>> {
    let user_members = markdown_stems_in_dir(&repo.join(".lf/directions").join(name));
    if !user_members.is_empty() {
        return Some(user_members);
    }

    crate::engine::builtins::builtin_direction_group(name)
        .map(|members| members.iter().map(|member| (*member).to_string()).collect())
}

fn markdown_stems_in_dir(dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut stems = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "md") {
            if let Some(stem) = path.file_stem() {
                stems.push(stem.to_string_lossy().to_string());
            }
        }
    }
    stems.sort();
    stems
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
    // Accept both "gstack/sprint" and "gstack-sprint"
    let splits: Vec<(&str, &str)> = name
        .split_once('/')
        .into_iter()
        .chain(name.split_once('-'))
        .collect();
    for (prefix, flow_name) in splits {
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

fn find_direction_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    let path = repo.join(".lf/directions").join(format!("{name}.md"));
    if path.exists() {
        return Ok(path);
    }

    let directions_dir = repo.join(".lf/directions");
    if let Ok(entries) = fs::read_dir(&directions_dir) {
        for entry in entries.flatten() {
            let dir_path = entry.path();
            if dir_path.is_dir() {
                let candidate = dir_path.join(format!("{name}.md"));
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }

    Err(LoadError::DirectionNotFound(name.to_string()))
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
        Value::String(name) => Ok(Step::Skill(Skill::named(name))),
        Value::Mapping(map) => parse_flow_mapping(map),
        _ => Err(LoadError::InvalidFlow(
            "flow item must be string or mapping".to_string(),
        )),
    }
}

fn parse_flow_mapping(map: &serde_yaml_ng::Mapping) -> Result<Step, LoadError> {
    if let Some(skill_value) = map.get(key("step")) {
        return Ok(Step::Skill(parse_skill_value(skill_value)?));
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

fn parse_skill_value(value: &Value) -> Result<Skill, LoadError> {
    match value {
        Value::String(name) => Ok(Skill::named(name)),
        Value::Mapping(map) => {
            let name = match map.get(key("name")) {
                Some(Value::String(name)) => name.to_string(),
                _ => {
                    return Err(LoadError::InvalidFlow(
                        "step mapping missing name".to_string(),
                    ))
                }
            };
            let agent = parse_optional_string(map, "agent");
            let default_agent = parse_optional_string(map, "default_agent");
            let action_style = parse_optional_string(map, "action_style");
            let interactive = map.get(key("interactive")).and_then(|val| val.as_bool());
            let directions = parse_directions_field(map);
            Ok(Skill {
                name,
                agent,
                default_agent,
                directions,
                action_style,
                interactive,
                content: None,
            })
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
        let skill = parse_optional_string(path_map, "step");
        let skills = parse_xor_path_skills(path_map, key_str, kind_prefix)?;

        let target_count = usize::from(flow.is_some())
            + usize::from(skill.is_some())
            + usize::from(!skills.is_empty());
        if target_count > 1 {
            return Err(LoadError::InvalidFlow(format!(
                "{kind_prefix} path '{key_str}' cannot have more than one of flow, step, or steps"
            )));
        }

        let description = parse_optional_string(path_map, "description").ok_or_else(|| {
            LoadError::InvalidFlow(format!(
                "{kind_prefix} path '{key_str}' must have description"
            ))
        })?;

        let direction = parse_directions_field(path_map);

        paths.insert(
            key_str.to_string(),
            XorPath {
                flow,
                skill,
                steps: skills,
                description,
                direction,
            },
        );
    }

    let router = parse_optional_string(map, "router");

    Ok(XorDef { router, paths })
}

/// Validate that flows referenced by xor paths can be loaded and expanded.
fn validate_xor_paths(xor_def: &XorDef, repo: &Path) -> Result<(), LoadError> {
    for path in xor_def.paths.values() {
        load_xor_path_items(path, repo)?;
    }
    Ok(())
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
        "\nWrite your choice to `scratch/route-xor.md`.\n\
         First line must be exactly: `path: <key>`\n\
         Then explain your reasoning briefly.\n",
    );
    suffix
}

pub fn read_xor_verdict(verdict_path: &Path, xor_def: &ConcreteXor) -> Result<String, String> {
    let content = fs::read_to_string(verdict_path)
        .map_err(|err| format!("xor verdict not found at {}: {err}", verdict_path.display()))?;

    let first_line = content
        .lines()
        .next()
        .ok_or_else(|| "xor verdict file is empty".to_string())?;

    let selected = first_line
        .strip_prefix("path:")
        .map(|s| s.trim().to_string())
        .ok_or_else(|| {
            format!("xor verdict first line must start with 'path:', got: {first_line}")
        })?;

    if !xor_def.paths.contains_key(&selected) {
        let valid_keys: Vec<&String> = xor_def.paths.keys().collect();
        return Err(format!(
            "unknown xor path: {selected}, expected one of: {valid_keys:?}"
        ));
    }

    Ok(selected)
}

pub fn load_xor_path_items(or_path: &XorPath, repo: &Path) -> Result<Vec<ConcreteStep>, LoadError> {
    if let Some(ref flow_name) = or_path.flow {
        let flow = load_flow(flow_name, repo)?;
        return expand_flow(&flow, repo);
    }

    if let Some(ref skill_name) = or_path.skill {
        let skill = load_skill(skill_name, repo)?;
        return Ok(vec![ConcreteStep::Skill(ConcreteSkill {
            skill,
            flow_parents: Vec::new(),
        })]);
    }

    if !or_path.steps.is_empty() {
        return Ok(or_path
            .steps
            .iter()
            .map(|skill| {
                ConcreteStep::Skill(ConcreteSkill {
                    skill: resolve_skill_reference(skill, repo),
                    flow_parents: Vec::new(),
                })
            })
            .collect());
    }

    Ok(Vec::new())
}

fn expand_branch_def(
    branch_def: &XorDef,
    repo: &Path,
    chain: &[String],
) -> Result<ConcreteXor, LoadError> {
    validate_xor_paths(branch_def, repo)?;
    Ok(ConcreteXor {
        router: branch_def.router.clone(),
        paths: branch_def.paths.clone(),
        flow_parents: chain.to_vec(),
    })
}

fn parse_xor_path_skills(
    map: &serde_yaml_ng::Mapping,
    path_name: &str,
    kind: &str,
) -> Result<Vec<Skill>, LoadError> {
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
            Value::String(name) => Ok(Skill::named(name)),
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

fn parse_string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(value)) => vec![value.to_string()],
        Some(Value::Sequence(seq)) => seq
            .iter()
            .filter_map(|val| val.as_str().map(|item| item.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_optional_string(map: &serde_yaml_ng::Mapping, field: &str) -> Option<String> {
    map.get(key(field))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn resolve_skill_reference(skill: &Skill, repo: &Path) -> Skill {
    if skill.content.is_some() {
        return skill.clone();
    }

    let Ok(mut resolved) = load_skill(&skill.name, repo) else {
        return skill.clone();
    };

    if let Some(agent) = &skill.agent {
        resolved.agent = Some(agent.clone());
    }
    if let Some(default_agent) = &skill.default_agent {
        resolved.default_agent = Some(default_agent.clone());
    }
    if !skill.directions.is_empty() {
        resolved.directions = skill.directions.clone();
    }
    if let Some(action_style) = &skill.action_style {
        resolved.action_style = Some(action_style.clone());
    }
    if let Some(interactive) = skill.interactive {
        resolved.interactive = Some(interactive);
    }

    resolved
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
            Step::Skill(skill) => {
                // A plain string in flow YAML is parsed as Skill, but it might
                // actually be a sub-flow name. If the skill has no inline content,
                // check if a flow with this name exists and expand it.
                if let Some(nested) = try_load_multi_skill_flow(skill, repo, &chain) {
                    items.extend(expand_with_chain(
                        &nested,
                        repo,
                        chain_with(&chain, &skill.name),
                        depth + 1,
                    )?);
                    continue;
                }
                items.push(ConcreteStep::Skill(ConcreteSkill {
                    skill: resolve_skill_reference(skill, repo),
                    flow_parents: chain.clone(),
                }));
            }
            Step::FlowRef(name) => {
                if chain.contains(name) {
                    return Err(LoadError::InvalidFlow(format!(
                        "flow cycle detected: {} -> {name}",
                        chain.join(" ")
                    )));
                }
                let nested = load_flow(name, repo)?;
                items.extend(expand_with_chain(
                    &nested,
                    repo,
                    chain_with(&chain, name),
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
                    branch_def, repo, &chain,
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
            .map(|i| !matches!(i, Step::Skill(s) if s.name == skill_name))
            .unwrap_or(false)
}

fn try_load_multi_skill_flow(skill: &Skill, repo: &Path, chain: &[String]) -> Option<Flow> {
    if skill.content.is_some() || chain.contains(&skill.name) {
        return None;
    }

    // Strict resolution inside an expanding flow: a bare skill name like `review`
    // must not auto-escalate to `gstack/review`. Only exact-key matches.
    let flow = load_flow_strict(&skill.name, repo).ok()?;
    is_multi_skill_flow(&flow, &skill.name).then_some(flow)
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
    use super::*;
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
        assert_eq!(skill.interactive, Some(false));
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
default_agent: gemini:2.5-pro
---
# Fast Skill
Do it quickly.
"#,
        )
        .unwrap();

        let skill = load_skill("fast", tmp.path()).unwrap();
        assert_eq!(skill.default_agent, Some("gemini:2.5-pro".to_string()));
    }

    #[test]
    fn load_skill_parses_frontmatter_interactive() {
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
        assert_eq!(skill.interactive, Some(true));
    }

    #[test]
    fn load_skill_parses_frontmatter_action_style() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("design.md"),
            r#"---
action_style: exploratory
---
# Design Skill
Design the feature.
"#,
        )
        .unwrap();

        let skill = load_skill("design", tmp.path()).unwrap();
        assert_eq!(skill.action_style.as_deref(), Some("exploratory"));
    }

    #[test]
    fn load_skill_includes_frontmatter_directions() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("careful.md"),
            r#"---
directions:
  - thorough
  - tested
---
# Careful Skill
Be careful.
"#,
        )
        .unwrap();

        let skill = load_skill("careful", tmp.path()).unwrap();
        assert_eq!(skill.directions, vec!["thorough", "tested"]);
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
    fn load_direction_finds_repo_direction() {
        let tmp = TempDir::new().unwrap();
        let directions = tmp.path().join(".lf/directions");
        fs::create_dir_all(&directions).unwrap();
        fs::write(directions.join("focus.md"), "Stay focused.").unwrap();
        let result = load_direction("focus", tmp.path());
        assert!(
            result.is_ok(),
            "repo direction should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_direction_not_found_error() {
        let tmp = TempDir::new().unwrap();

        let result = load_direction("nonexistent", tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
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
    fn load_direction_falls_back_to_agent_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".agents/skills/empathy");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: empathy\n---\nDesign with empathy.",
        )
        .unwrap();

        let direction = load_direction("empathy", tmp.path()).unwrap();
        assert_eq!(direction.name, "empathy");
        assert!(direction.content.contains("Design with empathy."));
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
            for item in expanded.unwrap() {
                match item {
                    ConcreteStep::Skill(skill) => {
                        let result = load_skill(&skill.skill.name, tmp.path());
                        assert!(
                            result.is_ok(),
                            "builtin flow '{}' references missing skill '{}': {:?}",
                            name,
                            skill.skill.name,
                            result.err()
                        );
                    }
                    ConcreteStep::Op(ops) => {
                        assert!(
                            !ops.item.command.is_empty(),
                            "builtin flow '{}' contains empty ops command",
                            name
                        );
                    }
                    ConcreteStep::Xor(branch) => {
                        for (path_key, path) in &branch.paths {
                            if let Some(ref flow_name) = path.flow {
                                let result = load_flow(flow_name, tmp.path());
                                assert!(
                                    result.is_ok(),
                                    "builtin flow '{}' branch path '{}' references missing flow '{}': {:?}",
                                    name, path_key, flow_name, result.err()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn parse_skill_mapping_accepts_plural_directions_key() {
        let yaml = r#"
- step:
    name: implement
    directions: [designer, product-engineer]
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::Skill(skill) => {
                assert_eq!(skill.name, "implement");
                assert_eq!(skill.directions, vec!["designer", "product-engineer"]);
            }
            other => panic!("expected Skill, got {other:?}"),
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
- op: pr land --create-pr
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::Op(item) => {
                assert_eq!(item.command, "pr");
                assert_eq!(item.args, vec!["land", "--create-pr"]);
            }
            other => panic!("expected Ops item, got {other:?}"),
        }
    }

    #[test]
    fn expand_direction_names_passes_through_non_groups() {
        let tmp = TempDir::new().unwrap();
        let result = expand_direction_names(&["security".to_string()], tmp.path());
        assert_eq!(result, vec!["security"]);
    }

    #[test]
    fn expand_direction_names_expands_user_group() {
        let tmp = TempDir::new().unwrap();
        let group_dir = tmp.path().join(".lf/directions/mygroup");
        fs::create_dir_all(&group_dir).unwrap();
        fs::write(group_dir.join("alpha.md"), "Alpha direction").unwrap();
        fs::write(group_dir.join("beta.md"), "Beta direction").unwrap();

        let result = expand_direction_names(&["mygroup".to_string()], tmp.path());
        assert_eq!(result, vec!["alpha", "beta"]);
    }

    #[test]
    fn expand_direction_names_accepts_retired_builtin_group_name() {
        let tmp = TempDir::new().unwrap();
        let group_dir = tmp.path().join(".lf/directions/craft");
        fs::create_dir_all(&group_dir).unwrap();
        fs::write(group_dir.join("custom.md"), "Custom craft").unwrap();

        let result = expand_direction_names(&["craft".to_string()], tmp.path());
        assert_eq!(result, vec!["custom"]);
    }

    #[test]
    fn expand_direction_names_deduplicates() {
        let tmp = TempDir::new().unwrap();
        let group_dir = tmp.path().join(".lf/directions/mygroup");
        fs::create_dir_all(&group_dir).unwrap();
        fs::write(group_dir.join("alpha.md"), "Alpha").unwrap();

        let result =
            expand_direction_names(&["alpha".to_string(), "mygroup".to_string()], tmp.path());
        assert_eq!(result, vec!["alpha"]);
    }

    #[test]
    fn expand_direction_names_recursive_group() {
        let tmp = TempDir::new().unwrap();
        let group_dir = tmp.path().join(".lf/directions/quality");
        let nested_group_dir = tmp.path().join(".lf/directions/craft");
        fs::create_dir_all(&group_dir).unwrap();
        fs::create_dir_all(&nested_group_dir).unwrap();
        fs::write(group_dir.join("craft.md"), "Craft direction").unwrap();
        fs::write(group_dir.join("extra.md"), "Extra direction").unwrap();
        fs::write(nested_group_dir.join("care.md"), "Care direction").unwrap();
        fs::write(nested_group_dir.join("clarity.md"), "Clarity direction").unwrap();

        let result = expand_direction_names(&["quality".to_string()], tmp.path());
        assert!(!result.contains(&"craft".to_string()));
        assert!(result.contains(&"care".to_string()));
        assert!(result.contains(&"clarity".to_string()));
        assert!(result.contains(&"extra".to_string()));
    }

    #[test]
    fn find_direction_path_searches_subdirectories() {
        let tmp = TempDir::new().unwrap();
        let sub_dir = tmp.path().join(".lf/directions/mygroup");
        fs::create_dir_all(&sub_dir).unwrap();
        fs::write(sub_dir.join("nested.md"), "Nested direction").unwrap();

        let result = find_direction_path("nested", tmp.path());
        assert!(result.is_ok());
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
        step: gate
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
    fn parse_xor_with_direction_override() {
        let yaml = r#"
- xor:
    paths:
      careful:
        flow: build
        description: "Build carefully"
        direction: [care, clarity]
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();

        match &items[0] {
            Step::Xor(branch) => {
                let careful = &branch.paths["careful"];
                assert_eq!(careful.direction, vec!["care", "clarity"]);
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
        step: gate
        description: "invalid"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_flow_items(&value);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("flow") && err.contains("step"),
            "expected error about both flow and step, got: {err}"
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
        assert_eq!(tune.steps[0].name, "implement");
        assert_eq!(tune.steps[1].name, "review");
    }

    #[test]
    fn parse_xor_rejects_multiple_targets() {
        let yaml = r#"
- xor:
    paths:
      bad:
        description: "invalid"
        step: implement
        steps:
          - review
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let err = parse_flow_items(&value).unwrap_err().to_string();
        assert!(err.contains("flow, step, or steps"));
    }

    #[test]
    fn expand_xor_keeps_concrete_xor() {
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-or".to_string(),
            items: vec![
                Step::Skill(Skill::named("gate")),
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
                                direction: Vec::new(),
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
        let mut paths = HashMap::new();
        paths.insert(
            "zeta".to_string(),
            XorPath {
                flow: None,
                skill: None,
                steps: Vec::new(),
                description: "Last".to_string(),
                direction: Vec::new(),
            },
        );
        paths.insert(
            "alpha".to_string(),
            XorPath {
                flow: None,
                skill: None,
                steps: Vec::new(),
                description: "First".to_string(),
                direction: Vec::new(),
            },
        );

        let suffix = build_xor_routing_suffix(&ConcreteXor {
            router: None,
            paths,
            flow_parents: Vec::new(),
        });

        let alpha = suffix.find("**alpha**").unwrap();
        let zeta = suffix.find("**zeta**").unwrap();
        assert!(alpha < zeta, "paths should be listed in sorted order");
    }

    #[test]
    fn read_xor_verdict_rejects_unknown_path() {
        let tmp = TempDir::new().unwrap();
        let verdict = tmp.path().join("route-xor.md");
        fs::write(&verdict, "path: missing\n").unwrap();

        let mut paths = HashMap::new();
        paths.insert(
            "known".to_string(),
            XorPath {
                flow: None,
                skill: None,
                steps: Vec::new(),
                description: "Known".to_string(),
                direction: Vec::new(),
            },
        );
        let err = read_xor_verdict(
            &verdict,
            &ConcreteXor {
                router: None,
                paths,
                flow_parents: Vec::new(),
            },
        )
        .expect_err("unknown path should fail");

        assert!(err.contains("unknown xor path"));
    }

    #[test]
    fn load_xor_path_items_allows_silence_path() {
        let tmp = TempDir::new().unwrap();
        let items = load_xor_path_items(
            &XorPath {
                flow: None,
                skill: None,
                steps: Vec::new(),
                description: "Silence".to_string(),
                direction: Vec::new(),
            },
            tmp.path(),
        )
        .unwrap();

        assert!(
            items.is_empty(),
            "silence path should not expand into items"
        );
    }

    #[test]
    fn load_xor_path_items_expands_inline_skills() {
        let tmp = TempDir::new().unwrap();
        let items = load_xor_path_items(
            &XorPath {
                flow: None,
                skill: None,
                steps: vec![Skill::named("design"), Skill::named("gate")],
                description: "Inline skills".to_string(),
                direction: Vec::new(),
            },
            tmp.path(),
        )
        .unwrap();

        assert_eq!(items.len(), 2);
        match &items[0] {
            ConcreteStep::Skill(skill) => assert_eq!(skill.skill.name, "design"),
            other => panic!("expected skill, got {other:?}"),
        }
        match &items[1] {
            ConcreteStep::Skill(skill) => assert_eq!(skill.skill.name, "gate"),
            other => panic!("expected skill, got {other:?}"),
        }
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
