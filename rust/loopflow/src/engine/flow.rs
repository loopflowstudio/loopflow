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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_path: Option<String>,
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
            fast_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum Step {
    Skill(Skill),
    Op(Op),
    And {
        branches: Vec<Step>,
        #[serde(skip_serializing_if = "Option::is_none")]
        synthesize: Option<String>,
    },
    FlowRef(String),
    Xor(XorDef),
    Or(OrDef),
    Loop(LoopDef),
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
pub struct LoopDef {
    pub steps: Vec<Step>,
    pub exit: XorDef,
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

/// Multi-select branch: router picks 1+ paths, which run sequentially.
/// Structurally identical to `XorDef`; only execution semantics differ.
pub type OrDef = XorDef;

/// Multi-select branch with resolved flow context.
/// Structurally identical to `ConcreteXor`; only execution semantics differ.
pub type ConcreteOr = ConcreteXor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowAction {
    RunSkill { skill: ConcreteSkill },
    RunOps { ops: ConcreteOp },
    WaitInteractive { skill: ConcreteSkill },
    And { fork: ConcreteAnd },
    Xor { branch: ConcreteXor },
    Or { branch: ConcreteOr },
    Loop { body: ConcreteLoop },
    Complete,
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
        if let Some(last) = parts.last() {
            let fork_label = format!("and/{}", self.skill.name);
            if last == &fork_label {
                return parts.join(" ");
            }
        }
        parts.push(self.skill.name.clone());
        parts.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteAndBranch {
    pub steps: Vec<ConcreteSkill>,
    pub flow_parents: Vec<String>,
    pub label: String,
    pub directions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteAnd {
    pub branches: Vec<ConcreteAndBranch>,
    pub flow_parents: Vec<String>,
    pub synthesize: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteXor {
    pub router: Option<String>,
    pub paths: HashMap<String, XorPath>,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteLoop {
    pub steps: Vec<ConcreteStep>,
    pub exit: ConcreteXor,
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
    And(ConcreteAnd),
    Xor(ConcreteXor),
    Or(ConcreteOr),
    Loop(ConcreteLoop),
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub name: String,
    pub content: String,
    pub source: PathBuf,
}

pub fn next_action(items: &[ConcreteStep], step_index: usize) -> FlowAction {
    let item = match items.get(step_index) {
        Some(item) => item,
        None => return FlowAction::Complete,
    };
    match item.clone() {
        ConcreteStep::Skill(skill) => {
            if skill.skill.interactive.unwrap_or(false) {
                FlowAction::WaitInteractive { skill }
            } else {
                FlowAction::RunSkill { skill }
            }
        }
        ConcreteStep::Op(ops) => FlowAction::RunOps { ops },
        ConcreteStep::And(fork) => FlowAction::And { fork },
        ConcreteStep::Xor(branch) => FlowAction::Xor { branch },
        ConcreteStep::Or(branch) => FlowAction::Or { branch },
        ConcreteStep::Loop(body) => FlowAction::Loop { body },
    }
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
    // across namespaces (so `office-hours` resolves to `gstack/office-hours`).
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
    fast_path: Option<String>,
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
        fast_path: frontmatter.fast_path,
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
    // Accept both kebab-case "fast-path" and snake_case "fast_path" in YAML.
    let fast_path =
        parse_optional_string(map, "fast-path").or_else(|| parse_optional_string(map, "fast_path"));

    SkillFrontmatter {
        agent,
        default_agent,
        directions: parse_directions_field(map),
        action_style,
        interactive,
        fast_path,
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
    // Namespaced skills: "gstack/office-hours" → <dir>/.lf/skills/gstack/office-hours.md
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
    let skill_path = repo.join(".agents/skills").join(name).join("SKILL.md");
    fs::read_to_string(&skill_path).ok()
}

// -----------------------------------------------------------------------------
// YAML parsing helpers
// -----------------------------------------------------------------------------

fn key(s: &str) -> Value {
    Value::String(s.to_string())
}

fn parse_flow_items(value: &Value) -> Result<Vec<Step>, LoadError> {
    parse_flow_items_with_options(value, true)
}

fn parse_flow_items_with_options(value: &Value, allow_loop: bool) -> Result<Vec<Step>, LoadError> {
    match value {
        Value::Sequence(seq) => seq
            .iter()
            .map(|item| parse_flow_item_with_options(item, allow_loop))
            .collect(),
        Value::Mapping(map) => {
            if let Some(skills) = map.get(key("steps")) {
                return parse_flow_items_with_options(skills, allow_loop);
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

fn parse_flow_item_with_options(value: &Value, allow_loop: bool) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step::Skill(Skill::named(name))),
        Value::Mapping(map) => parse_flow_mapping_with_options(map, allow_loop),
        _ => Err(LoadError::InvalidFlow(
            "flow item must be string or mapping".to_string(),
        )),
    }
}

fn parse_flow_mapping_with_options(
    map: &serde_yaml_ng::Mapping,
    allow_loop: bool,
) -> Result<Step, LoadError> {
    if let Some(skill_value) = map.get(key("step")) {
        return Ok(Step::Skill(parse_skill_value(skill_value)?));
    }
    if let Some(flow_value) = map.get(key("flow")) {
        return parse_flow_ref_value(flow_value);
    }
    if let Some(and_value) = map.get(key("and")) {
        return parse_and_value(and_value);
    }
    if let Some(op_value) = map.get(key("op")) {
        return parse_op_value(op_value, "op");
    }
    if let Some(xor_value) = map.get(key("xor")) {
        return parse_xor_value(xor_value);
    }
    if let Some(or_value) = map.get(key("or")) {
        return parse_or_value(or_value);
    }
    if let Some(loop_value) = map.get(key("loop")) {
        if !allow_loop {
            return Err(LoadError::InvalidFlow(
                "nested loop constructs are not supported".to_string(),
            ));
        }
        return parse_loop_value(loop_value);
    }
    Err(LoadError::InvalidFlow(
        "flow item mapping must include skill, op, flow, and, xor, or, or loop".to_string(),
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
                fast_path: None,
            })
        }
        _ => Err(LoadError::InvalidFlow(
            "step value must be string or mapping".to_string(),
        )),
    }
}

fn parse_and_value(value: &Value) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("and must be mapping".to_string()))?;

    // Three formats:
    // 1. Explicit branches: and: { branches: [...] }
    // 2. Skill shorthand:    and: { skill: "reduce", drafts: [...] }
    // 3. Flow shorthand:    and: { flow: "build", drafts: [...] }
    let branches = if let Some(branches_value) = map.get(key("branches")) {
        match branches_value {
            Value::Sequence(seq) => seq
                .iter()
                .map(parse_and_branch_item)
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(LoadError::InvalidFlow(
                    "and branches must be list".to_string(),
                ))
            }
        }
    } else if let Some(name_value) = map.get(key("step")).or_else(|| map.get(key("flow"))) {
        let name = name_value
            .as_str()
            .ok_or_else(|| LoadError::InvalidFlow("and skill/flow must be string".to_string()))?;
        parse_and_drafts(map, name)?
    } else {
        return Err(LoadError::InvalidFlow(
            "and must have branches, step+drafts, or flow+drafts".to_string(),
        ));
    };

    if map.get(key("select")).is_some() {
        return Err(LoadError::InvalidFlow(
            "and select modes are not supported; and always runs all branches".to_string(),
        ));
    }
    if map.get(key("prompt")).is_some() {
        return Err(LoadError::InvalidFlow(
            "and prompts are not supported; and always runs all branches".to_string(),
        ));
    }
    let synthesize = map
        .get(key("synthesize"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(Step::And {
        branches,
        synthesize,
    })
}

/// Parse and-drafts for the skill/flow shorthand format.
fn parse_and_drafts(map: &serde_yaml_ng::Mapping, name: &str) -> Result<Vec<Step>, LoadError> {
    let drafts = map
        .get(key("drafts"))
        .ok_or_else(|| LoadError::InvalidFlow("and with skill/flow requires drafts".to_string()))?;
    let drafts_seq = drafts
        .as_sequence()
        .ok_or_else(|| LoadError::InvalidFlow("and drafts must be list".to_string()))?;

    let mut branches = Vec::new();
    for draft in drafts_seq {
        let draft_map = draft
            .as_mapping()
            .ok_or_else(|| LoadError::InvalidFlow("and draft must be mapping".to_string()))?;
        let directions = parse_directions_field(draft_map);
        branches.push(Step::Skill(Skill {
            directions,
            ..Skill::named(name)
        }));
    }
    Ok(branches)
}

/// Parse an and-branch item. Unlike `parse_flow_item`, this handles
/// `direction:` as a sibling key for both `skill:` and `flow:` branches.
fn parse_and_branch_item(value: &Value) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step::Skill(Skill::named(name))),
        Value::Mapping(map) => {
            let directions = parse_directions_field(map);
            if let Some(skill_value) = map.get(key("step")) {
                let mut skill = parse_skill_value(skill_value)?;
                if !directions.is_empty() && skill.directions.is_empty() {
                    skill.directions = directions;
                }
                return Ok(Step::Skill(skill));
            }
            if let Some(flow_value) = map.get(key("flow")) {
                let name = flow_value.as_str().ok_or_else(|| {
                    LoadError::InvalidFlow("and branch flow must be string".to_string())
                })?;
                return Ok(Step::Skill(Skill {
                    directions,
                    ..Skill::named(name)
                }));
            }
            if map.get(key("and")).is_some() {
                return Err(LoadError::InvalidFlow(
                    "nested and constructs are not supported".to_string(),
                ));
            }
            Err(LoadError::InvalidFlow(
                "and branch must have step or flow".to_string(),
            ))
        }
        _ => Err(LoadError::InvalidFlow(
            "and branch must be string or mapping".to_string(),
        )),
    }
}

fn parse_xor_value(value: &Value) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("xor must be mapping".to_string()))?;
    Ok(Step::Xor(parse_xor_def(map, "xor")?))
}

fn parse_or_value(value: &Value) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("or must be mapping".to_string()))?;
    parse_xor_def(map, "or").map(Step::Or)
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

fn parse_loop_value(value: &Value) -> Result<Step, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("loop must be mapping".to_string()))?;

    let skills_value = map
        .get(key("steps"))
        .ok_or_else(|| LoadError::InvalidFlow("loop must have skills".to_string()))?;
    let skills = parse_flow_items_with_options(skills_value, false)?;
    if skills.is_empty() {
        return Err(LoadError::InvalidFlow(
            "loop must have at least one skill".to_string(),
        ));
    }

    let exit_value = map
        .get(key("exit"))
        .ok_or_else(|| LoadError::InvalidFlow("loop must have exit".to_string()))?;
    let exit_map = exit_value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("loop exit must be mapping".to_string()))?;
    let exit = parse_xor_def(exit_map, "loop exit")?;
    if !exit.paths.contains_key("done") {
        return Err(LoadError::InvalidFlow(
            "loop exit must include a 'done' path".to_string(),
        ));
    }

    Ok(Step::Loop(LoopDef {
        steps: skills,
        exit,
    }))
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
            Step::And {
                branches,
                synthesize,
            } => {
                let fork = expand_and(branches, synthesize.clone(), repo, &chain, depth)?;
                items.push(ConcreteStep::And(fork));
            }
            Step::Xor(branch_def) => {
                items.push(ConcreteStep::Xor(expand_branch_def(
                    branch_def, repo, &chain,
                )?));
            }
            Step::Or(or_def) => {
                items.push(ConcreteStep::Or(expand_branch_def(or_def, repo, &chain)?));
            }
            Step::Loop(loop_def) => {
                let skills = expand_items_with_chain(&loop_def.steps, repo, &chain, depth)?;
                items.push(ConcreteStep::Loop(ConcreteLoop {
                    steps: skills,
                    exit: expand_branch_def(&loop_def.exit, repo, &chain)?,
                    flow_parents: chain.clone(),
                }));
            }
        }
    }

    Ok(items)
}

fn expand_items_with_chain(
    flow_items: &[Step],
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<Vec<ConcreteStep>, LoadError> {
    let flow = Flow {
        name: chain.last().cloned().unwrap_or_else(|| "flow".to_string()),
        items: flow_items.to_vec(),
    };
    expand_with_chain(&flow, repo, chain.to_vec(), depth)
}

fn expand_and(
    branches: &[Step],
    synthesize: Option<String>,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteAnd, LoadError> {
    let branches = branches
        .iter()
        .map(|b| expand_and_branch(b, repo, chain, depth))
        .collect::<Result<_, _>>()?;
    Ok(ConcreteAnd {
        branches,
        flow_parents: chain.to_vec(),
        synthesize,
    })
}

fn expand_and_branch(
    branch: &Step,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteAndBranch, LoadError> {
    match branch {
        Step::Skill(skill) => {
            // A skill name in a fork branch might actually reference a flow.
            // Try loading it as a flow first (same resolution as expand_with_chain).
            if let Some(branch) = try_expand_skill_as_flow(skill, repo, chain, depth)? {
                return Ok(branch);
            }
            let resolved = resolve_skill_reference(skill, repo);
            let flow_parents = and_branch_parents(chain, &skill.name);
            Ok(ConcreteAndBranch {
                steps: vec![ConcreteSkill {
                    skill: resolved,
                    flow_parents: flow_parents.clone(),
                }],
                flow_parents,
                label: skill.name.clone(),
                directions: skill.directions.clone(),
            })
        }
        Step::FlowRef(name) => {
            let nested = load_flow(name, repo)?;
            expand_flow_ref_branch(name, &[], &nested, repo, chain, depth)
        }
        Step::Op(_) => Err(LoadError::InvalidFlow(
            "and branches cannot contain ops items".to_string(),
        )),
        Step::And { .. } => Err(LoadError::InvalidFlow(
            "and branches cannot contain nested and constructs".to_string(),
        )),
        Step::Xor(_) => Err(LoadError::InvalidFlow(
            "and branches cannot contain xor constructs".to_string(),
        )),
        Step::Or(_) => Err(LoadError::InvalidFlow(
            "and branches cannot contain or constructs".to_string(),
        )),
        Step::Loop(_) => Err(LoadError::InvalidFlow(
            "and branches cannot contain loop constructs".to_string(),
        )),
    }
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

fn and_branch_parents(chain: &[String], name: &str) -> Vec<String> {
    chain_with(chain, &format!("and/{name}"))
}

/// Try to expand a skill name as a flow reference. Returns `Some(branch)` if
/// the name resolves to a multi-skill flow, `None` if it's just a skill.
fn try_expand_skill_as_flow(
    skill: &Skill,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<Option<ConcreteAndBranch>, LoadError> {
    let Some(nested) = try_load_multi_skill_flow(skill, repo, chain) else {
        return Ok(None);
    };
    let branch =
        expand_flow_ref_branch(&skill.name, &skill.directions, &nested, repo, chain, depth)?;
    Ok(Some(branch))
}

/// Expand a flow reference into a multi-skill fork branch.
fn expand_flow_ref_branch(
    name: &str,
    directions: &[String],
    nested: &Flow,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteAndBranch, LoadError> {
    let nested_chain = chain_with(chain, name);
    let nested_items = expand_with_chain(nested, repo, nested_chain, depth + 1)?;
    let skills = extract_and_branch_skills(name, &nested_items)?;
    let flow_parents = and_branch_parents(chain, name);
    Ok(ConcreteAndBranch {
        steps: skills,
        flow_parents,
        label: name.to_string(),
        directions: directions.to_vec(),
    })
}

/// Extract concrete skills from expanded flow items for an and-branch.
/// Rejects nested and constructs — only sequential skills are allowed within branches.
fn extract_and_branch_skills(
    flow_name: &str,
    items: &[ConcreteStep],
) -> Result<Vec<ConcreteSkill>, LoadError> {
    let mut skills = Vec::new();
    for item in items {
        match item {
            ConcreteStep::Skill(s) => skills.push(s.clone()),
            ConcreteStep::Op(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "and-branch flow ref '{flow_name}' contains an ops item"
                )))
            }
            ConcreteStep::And(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "and-branch flow ref '{flow_name}' contains a nested and construct"
                )))
            }
            ConcreteStep::Xor(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "and-branch flow ref '{flow_name}' contains a xor construct"
                )))
            }
            ConcreteStep::Or(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "and-branch flow ref '{flow_name}' contains an or construct"
                )))
            }
            ConcreteStep::Loop(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "and-branch flow ref '{flow_name}' contains a loop construct"
                )))
            }
        }
    }
    if skills.is_empty() {
        return Err(LoadError::InvalidFlow(format!(
            "and-branch flow ref '{flow_name}' expands to zero skills"
        )));
    }
    Ok(skills)
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
    fn load_direction_finds_builtin_direction() {
        let tmp = TempDir::new().unwrap();
        let result = load_direction("focus", tmp.path());
        assert!(
            result.is_ok(),
            "builtin direction should be found: {:?}",
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
    fn next_action_marks_interactive_skills_as_wait() {
        let flow = Flow {
            name: "demo".to_string(),
            items: vec![Step::Skill(Skill {
                name: "design".to_string(),
                agent: None,
                default_agent: None,
                directions: Vec::new(),
                action_style: None,
                interactive: Some(true),
                content: None,
                fast_path: None,
            })],
        };

        let repo = TempDir::new().unwrap();
        let items = expand_flow(&flow, repo.path()).unwrap();
        let action = next_action(&items, 0);
        assert!(matches!(action, FlowAction::WaitInteractive { .. }));
    }

    #[test]
    fn expand_flow_resolves_interactive_from_skill_frontmatter() {
        // A bare skill name reference in a flow should pick up interactive: true
        // from the skill file's frontmatter, not remain None.
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(
            skills_dir.join("my-design.md"),
            "---\ninteractive: true\n---\nDesign it.",
        )
        .unwrap();

        let flow = Flow {
            name: "test-flow".to_string(),
            items: vec![Step::Skill(Skill::named("my-design"))],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);
        let action = next_action(&items, 0);
        assert!(
            matches!(action, FlowAction::WaitInteractive { .. }),
            "bare skill reference should resolve interactive from frontmatter"
        );
    }

    #[test]
    fn expand_flow_resolves_builtin_interactive_skill() {
        // The builtin "design" skill has interactive: true in its frontmatter.
        // A flow referencing it by name should produce WaitInteractive.
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-flow".to_string(),
            items: vec![Step::Skill(Skill::named("design"))],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        let design = items
            .iter()
            .find(|item| matches!(item, ConcreteStep::Skill(s) if s.skill.name == "design"));
        assert!(
            design.is_some(),
            "expanded flow should contain a design skill"
        );
        if let Some(ConcreteStep::Skill(skill)) = design {
            assert_eq!(
                skill.skill.interactive,
                Some(true),
                "builtin design skill should have interactive: true after expansion"
            );
        }
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
                    ConcreteStep::And(fork) => {
                        for branch in &fork.branches {
                            for skill in &branch.steps {
                                let result = load_skill(&skill.skill.name, tmp.path());
                                assert!(
                                    result.is_ok(),
                                    "builtin flow '{}' and references missing skill '{}': {:?}",
                                    name,
                                    skill.skill.name,
                                    result.err()
                                );
                            }
                        }
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
                    ConcreteStep::Or(branch) => {
                        for (path_key, path) in &branch.paths {
                            if let Some(ref flow_name) = path.flow {
                                let result = load_flow(flow_name, tmp.path());
                                assert!(
                                    result.is_ok(),
                                    "builtin flow '{}' or path '{}' references missing flow '{}': {:?}",
                                    name, path_key, flow_name, result.err()
                                );
                            }
                        }
                    }
                    ConcreteStep::Loop(loop_item) => {
                        // Loop body skills are validated during expansion
                        assert!(
                            !loop_item.steps.is_empty(),
                            "builtin flow '{}' has empty loop body",
                            name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn parse_and_skill_drafts_shorthand() {
        let yaml = r#"
- review
- and:
    step: reduce
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo
- publish
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 3);

        // Second item should be a Fork with 3 branches
        match &items[1] {
            Step::And { branches, .. } => {
                assert_eq!(branches.len(), 3);
                for branch in branches {
                    match branch {
                        Step::Skill(skill) => {
                            assert_eq!(skill.name, "reduce");
                            assert_eq!(skill.directions.len(), 1);
                        }
                        _ => panic!("expected Skill branch"),
                    }
                }
            }
            _ => panic!("expected And item"),
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
    fn next_action_returns_ops_action() {
        let items = vec![ConcreteStep::Op(ConcreteOp {
            item: Op {
                command: "rebase".to_string(),
                args: Vec::new(),
            },
            flow_parents: vec!["ship".to_string()],
        })];

        let action = next_action(&items, 0);
        assert!(
            matches!(action, FlowAction::RunOps { .. }),
            "expected RunOps action, got {action:?}"
        );
    }

    #[test]
    fn expand_direction_names_passes_through_non_groups() {
        let tmp = TempDir::new().unwrap();
        let result = expand_direction_names(&["security".to_string()], tmp.path());
        assert_eq!(result, vec!["security"]);
    }

    #[test]
    fn expand_direction_names_expands_ceo_group() {
        let tmp = TempDir::new().unwrap();
        let result = expand_direction_names(&["ceo".to_string()], tmp.path());
        assert!(result.contains(&"focus".to_string()));
        assert!(result.contains(&"immediacy".to_string()));
        assert!(result.contains(&"truth".to_string()));
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
    fn expand_direction_names_user_group_overrides_builtin_group() {
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
    fn expand_direction_names_expands_builtin_craft_group() {
        let tmp = TempDir::new().unwrap();
        let result = expand_direction_names(&["craft".to_string()], tmp.path());
        assert!(result.contains(&"care".to_string()));
        assert!(result.contains(&"clarity".to_string()));
        assert!(!result.contains(&"scale".to_string()));
        assert!(result.contains(&"simplicity".to_string()));
    }

    #[test]
    fn expand_direction_names_expands_builtin_creativity_group() {
        let tmp = TempDir::new().unwrap();
        let result = expand_direction_names(&["creativity".to_string()], tmp.path());
        assert!(result.contains(&"alive".to_string()));
        assert!(result.contains(&"musical".to_string()));
    }

    #[test]
    fn expand_direction_names_recursive_group() {
        let tmp = TempDir::new().unwrap();
        let group_dir = tmp.path().join(".lf/directions/quality");
        fs::create_dir_all(&group_dir).unwrap();
        // "craft" is a builtin group — should expand recursively
        fs::write(group_dir.join("craft.md"), "Craft direction").unwrap();
        fs::write(group_dir.join("extra.md"), "Extra direction").unwrap();

        let result = expand_direction_names(&["quality".to_string()], tmp.path());
        // "craft" should NOT appear — it should expand to its members
        assert!(!result.contains(&"craft".to_string()));
        // But its members should be present
        assert!(result.contains(&"care".to_string()));
        assert!(result.contains(&"clarity".to_string()));
        // And the non-group member should be present
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
    fn next_action_marks_missing_skills_as_complete() {
        let flow = Flow {
            name: "demo".to_string(),
            items: Vec::new(),
        };

        let repo = TempDir::new().unwrap();
        let items = expand_flow(&flow, repo.path()).unwrap();
        let action = next_action(&items, 0);
        assert!(matches!(action, FlowAction::Complete));
    }

    #[test]
    fn parse_and_flow_drafts_shorthand() {
        let yaml = r#"
- and:
    flow: build
    drafts:
      - direction: infra
      - direction: ux
      - direction: ceo
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::And { branches, .. } => {
                assert_eq!(branches.len(), 3);
                for (i, branch) in branches.iter().enumerate() {
                    match branch {
                        Step::Skill(skill) => {
                            assert_eq!(skill.name, "build");
                            assert_eq!(skill.directions.len(), 1);
                        }
                        _ => panic!("expected Skill branch at index {i}"),
                    }
                }
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn parse_and_with_custom_synthesize() {
        let yaml = r#"
- and:
    branches:
      - step: gstack/pr-review
      - step: gstack/cso
      - step: gstack/codex
    synthesize: gstack/review-synthesize
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::And {
                branches,
                synthesize,
            } => {
                assert_eq!(branches.len(), 3);
                assert_eq!(synthesize.as_deref(), Some("gstack/review-synthesize"));
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn parse_and_without_synthesize_defaults_to_none() {
        let yaml = r#"
- and:
    branches:
      - step: review
      - step: cso
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        match &items[0] {
            Step::And { synthesize, .. } => {
                assert!(synthesize.is_none());
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn parse_and_explicit_branches_with_flow_and_skill() {
        let yaml = r#"
- and:
    branches:
      - flow: build
        direction: infra
      - step: review
        direction: ux
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            Step::And { branches, .. } => {
                assert_eq!(branches.len(), 2);
                // First branch: flow ref "build" with direction "infra"
                match &branches[0] {
                    Step::Skill(skill) => {
                        assert_eq!(skill.name, "build");
                        assert_eq!(skill.directions, vec!["infra"]);
                    }
                    _ => panic!("expected Skill branch"),
                }
                // Second branch: skill "review" with direction "ux"
                match &branches[1] {
                    Step::Skill(skill) => {
                        assert_eq!(skill.name, "review");
                        assert_eq!(skill.directions, vec!["ux"]);
                    }
                    _ => panic!("expected Skill branch"),
                }
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn expand_fork_multi_skill_flow_ref() {
        let tmp = TempDir::new().unwrap();
        let flows_dir = tmp.path().join(".lf/flows");
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        // Create a multi-skill flow
        fs::write(
            flows_dir.join("multi.yaml"),
            "- skill-a\n- skill-b\n- skill-c\n",
        )
        .unwrap();
        fs::write(skills_dir.join("skill-a.md"), "Skill A").unwrap();
        fs::write(skills_dir.join("skill-b.md"), "Skill B").unwrap();
        fs::write(skills_dir.join("skill-c.md"), "Skill C").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![Step::And {
                branches: vec![
                    Step::Skill(Skill {
                        name: "multi".to_string(),
                        directions: vec!["infra".to_string()],
                        ..Skill::named("multi")
                    }),
                    Step::Skill(Skill {
                        name: "multi".to_string(),
                        directions: vec!["ux".to_string()],
                        ..Skill::named("multi")
                    }),
                ],
                synthesize: None,
            }],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            ConcreteStep::And(fork) => {
                assert_eq!(fork.branches.len(), 2);
                // Each branch should have 3 skills from the "multi" flow
                for branch in &fork.branches {
                    assert_eq!(branch.steps.len(), 3, "branch should have 3 skills");
                    assert_eq!(branch.steps[0].skill.name, "skill-a");
                    assert_eq!(branch.steps[1].skill.name, "skill-b");
                    assert_eq!(branch.steps[2].skill.name, "skill-c");
                    assert_eq!(branch.label, "multi");
                }
                assert_eq!(fork.branches[0].directions, vec!["infra"]);
                assert_eq!(fork.branches[1].directions, vec!["ux"]);
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn expand_fork_single_skill_unchanged() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        fs::write(skills_dir.join("reduce.md"), "Reduce things.").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![Step::And {
                branches: vec![
                    Step::Skill(Skill {
                        name: "reduce".to_string(),
                        directions: vec!["infra".to_string()],
                        ..Skill::named("reduce")
                    }),
                    Step::Skill(Skill {
                        name: "reduce".to_string(),
                        directions: vec!["ux".to_string()],
                        ..Skill::named("reduce")
                    }),
                ],
                synthesize: None,
            }],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            ConcreteStep::And(fork) => {
                assert_eq!(fork.branches.len(), 2);
                // Each branch should have exactly 1 skill
                for branch in &fork.branches {
                    assert_eq!(branch.steps.len(), 1, "single-skill branch");
                    assert_eq!(branch.steps[0].skill.name, "reduce");
                }
            }
            _ => panic!("expected And item"),
        }
    }

    #[test]
    fn expand_fork_rejects_nested_fork_in_flow_ref() {
        let tmp = TempDir::new().unwrap();
        let flows_dir = tmp.path().join(".lf/flows");
        let skills_dir = tmp.path().join(".lf/skills");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::create_dir_all(&skills_dir).unwrap();

        // Create a flow that contains an and construct
        fs::write(
            flows_dir.join("has-and.yaml"),
            r#"
- skill-a
- and:
    step: skill-b
    drafts:
      - direction: x
      - direction: y
"#,
        )
        .unwrap();
        fs::write(skills_dir.join("skill-a.md"), "Skill A").unwrap();
        fs::write(skills_dir.join("skill-b.md"), "Skill B").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![Step::And {
                branches: vec![Step::Skill(Skill {
                    name: "has-and".to_string(),
                    directions: vec!["infra".to_string()],
                    ..Skill::named("has-and")
                })],
                synthesize: None,
            }],
        };
        let result = expand_flow(&flow, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nested and"),
            "expected nested and error, got: {err}"
        );
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
          - step:
              name: review
              interactive: true
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
        assert_eq!(tune.steps[1].interactive, Some(true));
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
    fn next_action_returns_xor_action() {
        let items = vec![ConcreteStep::Xor(ConcreteXor {
            router: None,
            paths: {
                let mut m = HashMap::new();
                m.insert(
                    "a".to_string(),
                    XorPath {
                        flow: Some("build".to_string()),
                        skill: None,
                        steps: Vec::new(),
                        description: "Path A".to_string(),
                        direction: Vec::new(),
                    },
                );
                m
            },
            flow_parents: Vec::new(),
        })];

        let action = next_action(&items, 0);
        assert!(
            matches!(action, FlowAction::Xor { .. }),
            "expected Xor action, got {action:?}"
        );
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
        assert_eq!(items.len(), 4); // implement, compress, lint, gate
    }

    #[test]
    fn deploy_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("deploy", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 2); // gate, op: pr land
    }

    #[test]
    fn parse_or_multi_select() {
        let yaml = r#"
- or:
    router: triage
    paths:
      fix:
        flow: code
        description: "Fix blocking issues"
      refactor:
        step: compress
        description: "Clean up while we're here"
      deploy:
        description: "Ship it"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        let Step::Or(or_def) = &items[0] else {
            panic!("expected Or item, got {:?}", items[0]);
        };
        assert_eq!(or_def.router.as_deref(), Some("triage"));
        assert_eq!(or_def.paths.len(), 3);
        assert_eq!(or_def.paths["fix"].flow.as_deref(), Some("code"));
        assert_eq!(or_def.paths["refactor"].skill.as_deref(), Some("compress"));
        assert!(or_def.paths["deploy"].flow.is_none());
        assert!(or_def.paths["deploy"].skill.is_none());
    }

    #[test]
    fn expand_or_keeps_concrete_or() {
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-or".to_string(),
            items: vec![
                Step::Skill(Skill::named("gate")),
                Step::Or(OrDef {
                    router: None,
                    paths: {
                        let mut m = HashMap::new();
                        m.insert(
                            "fix".to_string(),
                            XorPath {
                                flow: None,
                                skill: Some("implement".to_string()),
                                steps: vec![],
                                description: "Fix it".to_string(),
                                direction: vec![],
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
        assert!(matches!(&items[1], ConcreteStep::Or(_)));

        if let ConcreteStep::Or(branch) = &items[1] {
            assert_eq!(branch.paths.len(), 1);
            assert_eq!(branch.paths["fix"].description, "Fix it");
        }
    }
}
