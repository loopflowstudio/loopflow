use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::engine::error::LoadError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
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

impl Step {
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
pub enum FlowItem {
    Step(Step),
    Op(Op),
    Fork { branches: Vec<FlowItem> },
    FlowRef(String),
    Branch(BranchDef),
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
        write!(f, "ops: {}", self.display_name())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchDef {
    pub paths: HashMap<String, BranchPath>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchPath {
    pub flow: Option<String>,
    pub step: Option<String>,
    pub description: String,
    #[serde(default)]
    pub direction: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowAction {
    RunStep { step: ConcreteStep },
    RunOps { ops: ConcreteOp },
    WaitInteractive { step: ConcreteStep },
    Fork { fork: ConcreteFork },
    Branch { branch: ConcreteBranch },
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Flow {
    pub name: String,
    pub items: Vec<FlowItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteStep {
    pub step: Step,
    pub flow_parents: Vec<String>,
}

impl ConcreteStep {
    pub fn display_path(&self) -> String {
        let mut parts = self.flow_parents.clone();
        if let Some(last) = parts.last() {
            let fork_label = format!("fork/{}", self.step.name);
            if last == &fork_label {
                return parts.join(" ");
            }
        }
        parts.push(self.step.name.clone());
        parts.join(" ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteForkBranch {
    pub steps: Vec<ConcreteStep>,
    pub flow_parents: Vec<String>,
    pub label: String,
    pub directions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteFork {
    pub branches: Vec<ConcreteForkBranch>,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteBranch {
    pub paths: HashMap<String, BranchPath>,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcreteOp {
    pub item: Op,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcreteItem {
    Step(ConcreteStep),
    Op(ConcreteOp),
    Fork(ConcreteFork),
    Branch(ConcreteBranch),
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub name: String,
    pub content: String,
    pub source: PathBuf,
}

pub fn next_action(items: &[ConcreteItem], step_index: usize) -> FlowAction {
    let item = match items.get(step_index) {
        Some(item) => item,
        None => return FlowAction::Complete,
    };
    match item.clone() {
        ConcreteItem::Step(step) => {
            if step.step.interactive.unwrap_or(false) {
                FlowAction::WaitInteractive { step }
            } else {
                FlowAction::RunStep { step }
            }
        }
        ConcreteItem::Op(ops) => FlowAction::RunOps { ops },
        ConcreteItem::Fork(fork) => FlowAction::Fork { fork },
        ConcreteItem::Branch(branch) => FlowAction::Branch { branch },
    }
}

pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    let content = match find_flow_path(name, repo) {
        Ok(flow_path) => fs::read_to_string(&flow_path)?,
        Err(LoadError::FlowNotFound(_)) => {
            if let Some(builtin) = crate::engine::builtins::get_builtin_flow(name) {
                builtin.to_string()
            } else if load_step(name, repo).is_ok() {
                // Auto-wrap a step name as a single-step flow.
                return Ok(Flow {
                    name: name.to_string(),
                    items: vec![FlowItem::Step(Step::named(name))],
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
        name: name.to_string(),
        items,
    })
}

pub fn expand_flow(flow: &Flow, repo: &Path) -> Result<Vec<ConcreteItem>, LoadError> {
    expand_with_chain(flow, repo, vec![flow.name.clone()], 0)
}

pub fn load_step(name: &str, repo: &Path) -> Result<Step, LoadError> {
    // Try file-based lookup first (repo-local, then global)
    if let Ok(step_path) = find_step_path(name, repo) {
        let content = fs::read_to_string(&step_path)?;
        return step_from_content(name, &content);
    }

    // Fall back to built-in steps
    if let Some(content) = crate::engine::builtins::get_builtin_step(name) {
        return step_from_content(name, content);
    }

    // Fall back to .agents/skills/<name>/SKILL.md (user-installed, not loopflow-injected)
    if let Some(content) = load_agent_skill(name, repo) {
        return step_from_content(name, &content);
    }

    Err(LoadError::StepNotFound(name.to_string()))
}

#[derive(Debug, Default)]
struct StepFrontmatter {
    agent: Option<String>,
    default_agent: Option<String>,
    directions: Vec<String>,
    action_style: Option<String>,
    interactive: Option<bool>,
    fast_path: Option<String>,
}

fn parse_step_frontmatter(content: &str) -> Result<(StepFrontmatter, String), LoadError> {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return Ok((StepFrontmatter::default(), content.to_string()));
    };

    let value: Value = serde_yaml_ng::from_str(&frontmatter)
        .map_err(|err| LoadError::InvalidStep(err.to_string()))?;
    Ok((parse_frontmatter_value(&value), body))
}

fn step_from_content(name: &str, content: &str) -> Result<Step, LoadError> {
    let (frontmatter, body) = parse_step_frontmatter(content)?;
    Ok(Step {
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

fn split_frontmatter(content: &str) -> Option<(String, String)> {
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

fn parse_frontmatter_value(value: &Value) -> StepFrontmatter {
    let map = match value.as_mapping() {
        Some(map) => map,
        None => return StepFrontmatter::default(),
    };

    let agent = parse_optional_string(map, "agent");
    let default_agent = parse_optional_string(map, "default_agent");
    let action_style = parse_optional_string(map, "action_style");
    let interactive = map.get(key("interactive")).and_then(|val| val.as_bool());
    // Accept both kebab-case "fast-path" and snake_case "fast_path" in YAML.
    let fast_path =
        parse_optional_string(map, "fast-path").or_else(|| parse_optional_string(map, "fast_path"));

    StepFrontmatter {
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

fn find_flow_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    let candidates = [
        repo.join(".lf/flows").join(format!("{name}.yaml")),
        repo.join(".lf/flows").join(format!("{name}.yml")),
        repo.join(".lf/flows").join(format!("{name}.json")),
    ];
    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(LoadError::FlowNotFound(name.to_string()))
}

fn find_step_path(name: &str, repo: &Path) -> Result<PathBuf, LoadError> {
    // 1. Check repo-local paths
    let repo_candidates = [
        repo.join(".lf/steps").join(format!("{name}.md")),
        repo.join(".claude/commands").join(format!("{name}.md")),
    ];
    for path in repo_candidates {
        if path.exists() {
            return Ok(path);
        }
    }

    // 2. Check global paths
    if let Some(home) = home_dir() {
        let global_candidates = [
            home.join(".lf/steps").join(format!("{name}.md")),
            home.join(".claude/commands").join(format!("{name}.md")),
        ];
        for path in global_candidates {
            if path.exists() {
                return Ok(path);
            }
        }
    }

    Err(LoadError::StepNotFound(name.to_string()))
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

fn parse_flow_items(value: &Value) -> Result<Vec<FlowItem>, LoadError> {
    match value {
        Value::Sequence(seq) => seq.iter().map(parse_flow_item).collect(),
        Value::Mapping(map) => {
            if let Some(steps) = map.get(key("steps")) {
                return parse_flow_items(steps);
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

fn parse_flow_item(value: &Value) -> Result<FlowItem, LoadError> {
    match value {
        Value::String(name) => Ok(FlowItem::Step(Step::named(name))),
        Value::Mapping(map) => parse_flow_mapping(map),
        _ => Err(LoadError::InvalidFlow(
            "flow item must be string or mapping".to_string(),
        )),
    }
}

fn parse_flow_mapping(map: &serde_yaml_ng::Mapping) -> Result<FlowItem, LoadError> {
    if let Some(step_value) = map.get(key("step")) {
        return Ok(FlowItem::Step(parse_step_value(step_value)?));
    }
    if let Some(flow_value) = map.get(key("flow")) {
        return parse_flow_ref_value(flow_value);
    }
    if let Some(fork_value) = map.get(key("fork")) {
        return parse_fork_value(fork_value);
    }
    if let Some(ops_value) = map.get(key("ops")) {
        return parse_ops_value(ops_value);
    }
    if let Some(branch_value) = map.get(key("branch")) {
        return parse_branch_value(branch_value);
    }
    Err(LoadError::InvalidFlow(
        "flow item mapping must include step, ops, flow, fork, or branch".to_string(),
    ))
}

fn parse_ops_value(value: &Value) -> Result<FlowItem, LoadError> {
    let raw = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidFlow("ops value must be string".to_string()))?
        .trim();

    if raw.is_empty() {
        return Err(LoadError::InvalidFlow(
            "ops value must include a command".to_string(),
        ));
    }

    let mut parts = raw.split_whitespace();
    let command = parts
        .next()
        .ok_or_else(|| LoadError::InvalidFlow("ops value must include a command".to_string()))?
        .to_string();
    let args = parts.map(ToString::to_string).collect();

    Ok(FlowItem::Op(Op { command, args }))
}

fn parse_step_value(value: &Value) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step::named(name)),
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
            Ok(Step {
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

fn parse_fork_value(value: &Value) -> Result<FlowItem, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("fork must be mapping".to_string()))?;

    // Three formats:
    // 1. Explicit branches: fork: { branches: [...] }
    // 2. Step shorthand:    fork: { step: "reduce", drafts: [...] }
    // 3. Flow shorthand:    fork: { flow: "build", drafts: [...] }
    let branches = if let Some(branches_value) = map.get(key("branches")) {
        match branches_value {
            Value::Sequence(seq) => seq
                .iter()
                .map(parse_fork_branch_item)
                .collect::<Result<_, _>>()?,
            _ => {
                return Err(LoadError::InvalidFlow(
                    "fork branches must be list".to_string(),
                ))
            }
        }
    } else if let Some(name_value) = map.get(key("step")).or_else(|| map.get(key("flow"))) {
        let name = name_value
            .as_str()
            .ok_or_else(|| LoadError::InvalidFlow("fork step/flow must be string".to_string()))?;
        parse_fork_drafts(map, name)?
    } else {
        return Err(LoadError::InvalidFlow(
            "fork must have branches, step+drafts, or flow+drafts".to_string(),
        ));
    };

    if map.get(key("select")).is_some() {
        return Err(LoadError::InvalidFlow(
            "fork select modes are not supported; forks always run all branches".to_string(),
        ));
    }
    if map.get(key("prompt")).is_some() {
        return Err(LoadError::InvalidFlow(
            "fork prompts are not supported; forks always run all branches".to_string(),
        ));
    }
    Ok(FlowItem::Fork { branches })
}

/// Parse fork drafts for the step/flow shorthand format.
fn parse_fork_drafts(map: &serde_yaml_ng::Mapping, name: &str) -> Result<Vec<FlowItem>, LoadError> {
    let drafts = map
        .get(key("drafts"))
        .ok_or_else(|| LoadError::InvalidFlow("fork with step/flow requires drafts".to_string()))?;
    let drafts_seq = drafts
        .as_sequence()
        .ok_or_else(|| LoadError::InvalidFlow("fork drafts must be list".to_string()))?;

    let mut branches = Vec::new();
    for draft in drafts_seq {
        let draft_map = draft
            .as_mapping()
            .ok_or_else(|| LoadError::InvalidFlow("fork draft must be mapping".to_string()))?;
        let directions = parse_directions_field(draft_map);
        branches.push(FlowItem::Step(Step {
            directions,
            ..Step::named(name)
        }));
    }
    Ok(branches)
}

/// Parse a fork branch item. Unlike `parse_flow_item`, this handles
/// `direction:` as a sibling key for both `step:` and `flow:` branches.
fn parse_fork_branch_item(value: &Value) -> Result<FlowItem, LoadError> {
    match value {
        Value::String(name) => Ok(FlowItem::Step(Step::named(name))),
        Value::Mapping(map) => {
            let directions = parse_directions_field(map);
            if let Some(step_value) = map.get(key("step")) {
                let mut step = parse_step_value(step_value)?;
                if !directions.is_empty() && step.directions.is_empty() {
                    step.directions = directions;
                }
                return Ok(FlowItem::Step(step));
            }
            if let Some(flow_value) = map.get(key("flow")) {
                let name = flow_value.as_str().ok_or_else(|| {
                    LoadError::InvalidFlow("fork branch flow must be string".to_string())
                })?;
                return Ok(FlowItem::Step(Step {
                    directions,
                    ..Step::named(name)
                }));
            }
            if map.get(key("fork")).is_some() {
                return Err(LoadError::InvalidFlow(
                    "nested forks are not supported".to_string(),
                ));
            }
            Err(LoadError::InvalidFlow(
                "fork branch must have step or flow".to_string(),
            ))
        }
        _ => Err(LoadError::InvalidFlow(
            "fork branch must be string or mapping".to_string(),
        )),
    }
}

fn parse_branch_value(value: &Value) -> Result<FlowItem, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("branch must be mapping".to_string()))?;

    let paths_value = map
        .get(key("paths"))
        .ok_or_else(|| LoadError::InvalidFlow("branch must have paths".to_string()))?;
    let paths_map = paths_value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("branch paths must be mapping".to_string()))?;

    if paths_map.is_empty() {
        return Err(LoadError::InvalidFlow(
            "branch must have at least one path".to_string(),
        ));
    }

    let mut paths = HashMap::new();
    for (path_key, path_value) in paths_map {
        let key_str = path_key
            .as_str()
            .ok_or_else(|| LoadError::InvalidFlow("branch path key must be string".to_string()))?;
        let path_map = path_value.as_mapping().ok_or_else(|| {
            LoadError::InvalidFlow(format!("branch path '{key_str}' must be mapping"))
        })?;

        let flow = parse_optional_string(path_map, "flow");
        let step = parse_optional_string(path_map, "step");

        match (&flow, &step) {
            (None, None) => {
                return Err(LoadError::InvalidFlow(format!(
                    "branch path '{key_str}' must have flow or step"
                )))
            }
            (Some(_), Some(_)) => {
                return Err(LoadError::InvalidFlow(format!(
                    "branch path '{key_str}' cannot have both flow and step"
                )))
            }
            _ => {}
        }

        let description = parse_optional_string(path_map, "description").ok_or_else(|| {
            LoadError::InvalidFlow(format!("branch path '{key_str}' must have description"))
        })?;

        let direction = parse_directions_field(path_map);

        paths.insert(
            key_str.to_string(),
            BranchPath {
                flow,
                step,
                description,
                direction,
            },
        );
    }

    Ok(FlowItem::Branch(BranchDef { paths }))
}

/// Validate that flows referenced by branch paths contain only steps and ops.
/// Forks and nested branches inside branch sub-flows are not supported.
fn validate_branch_paths(branch_def: &BranchDef, repo: &Path) -> Result<(), LoadError> {
    for (key, path) in &branch_def.paths {
        let Some(ref flow_name) = path.flow else {
            continue;
        };
        let flow = load_flow(flow_name, repo)?;
        let items = expand_flow(&flow, repo)?;
        for item in &items {
            match item {
                ConcreteItem::Step(_) | ConcreteItem::Op(_) => {}
                ConcreteItem::Fork(_) => {
                    return Err(LoadError::InvalidFlow(format!(
                        "branch path '{key}' references flow '{flow_name}' which contains a fork; \
                         branch sub-flows must contain only steps and ops"
                    )));
                }
                ConcreteItem::Branch(_) => {
                    return Err(LoadError::InvalidFlow(format!(
                        "branch path '{key}' references flow '{flow_name}' which contains a nested branch; \
                         branch sub-flows must contain only steps and ops"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn parse_flow_ref_value(value: &Value) -> Result<FlowItem, LoadError> {
    let name = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidFlow("flow ref must be string".to_string()))?;
    Ok(FlowItem::FlowRef(name.to_string()))
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

fn resolve_step_reference(step: &Step, repo: &Path) -> Step {
    if step.content.is_some() {
        return step.clone();
    }

    let Ok(mut resolved) = load_step(&step.name, repo) else {
        return step.clone();
    };

    if let Some(agent) = &step.agent {
        resolved.agent = Some(agent.clone());
    }
    if let Some(default_agent) = &step.default_agent {
        resolved.default_agent = Some(default_agent.clone());
    }
    if !step.directions.is_empty() {
        resolved.directions = step.directions.clone();
    }
    if let Some(action_style) = &step.action_style {
        resolved.action_style = Some(action_style.clone());
    }
    if let Some(interactive) = step.interactive {
        resolved.interactive = Some(interactive);
    }

    resolved
}

fn expand_with_chain(
    flow: &Flow,
    repo: &Path,
    chain: Vec<String>,
    depth: usize,
) -> Result<Vec<ConcreteItem>, LoadError> {
    const MAX_DEPTH: usize = 5;
    if depth > MAX_DEPTH {
        return Err(LoadError::InvalidFlow(format!(
            "flow nesting exceeds max depth {MAX_DEPTH}"
        )));
    }

    let mut items = Vec::new();
    for item in &flow.items {
        match item {
            FlowItem::Step(step) => {
                // A plain string in flow YAML is parsed as Step, but it might
                // actually be a sub-flow name. If the step has no inline content,
                // check if a flow with this name exists and expand it.
                if step.content.is_none() && !chain.contains(&step.name) {
                    if let Ok(nested) = load_flow(&step.name, repo) {
                        if is_multi_step_flow(&nested, &step.name) {
                            let mut nested_chain = chain.clone();
                            nested_chain.push(step.name.clone());
                            items.extend(expand_with_chain(
                                &nested,
                                repo,
                                nested_chain,
                                depth + 1,
                            )?);
                            continue;
                        }
                    }
                }
                items.push(ConcreteItem::Step(ConcreteStep {
                    step: resolve_step_reference(step, repo),
                    flow_parents: chain.clone(),
                }));
            }
            FlowItem::FlowRef(name) => {
                if chain.contains(name) {
                    return Err(LoadError::InvalidFlow(format!(
                        "flow cycle detected: {} -> {name}",
                        chain.join(" ")
                    )));
                }
                let nested = load_flow(name, repo)?;
                let mut nested_chain = chain.clone();
                nested_chain.push(name.clone());
                items.extend(expand_with_chain(&nested, repo, nested_chain, depth + 1)?);
            }
            FlowItem::Op(item) => {
                items.push(ConcreteItem::Op(ConcreteOp {
                    item: item.clone(),
                    flow_parents: chain.clone(),
                }));
            }
            FlowItem::Fork { branches } => {
                let fork = expand_fork(branches, repo, &chain, depth)?;
                items.push(ConcreteItem::Fork(fork));
            }
            FlowItem::Branch(branch_def) => {
                validate_branch_paths(branch_def, repo)?;
                items.push(ConcreteItem::Branch(ConcreteBranch {
                    paths: branch_def.paths.clone(),
                    flow_parents: chain.clone(),
                }));
            }
        }
    }

    Ok(items)
}

fn expand_fork(
    branches: &[FlowItem],
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteFork, LoadError> {
    let branches = branches
        .iter()
        .map(|b| expand_fork_branch(b, repo, chain, depth))
        .collect::<Result<_, _>>()?;
    Ok(ConcreteFork {
        branches,
        flow_parents: chain.to_vec(),
    })
}

fn expand_fork_branch(
    branch: &FlowItem,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteForkBranch, LoadError> {
    match branch {
        FlowItem::Step(step) => {
            // A step name in a fork branch might actually reference a flow.
            // Try loading it as a flow first (same resolution as expand_with_chain).
            if step.content.is_none() {
                if let Some(branch) = try_expand_step_as_flow(step, repo, chain, depth)? {
                    return Ok(branch);
                }
            }
            let resolved = resolve_step_reference(step, repo);
            let mut flow_parents = chain.to_vec();
            flow_parents.push(format!("fork/{}", step.name));
            Ok(ConcreteForkBranch {
                steps: vec![ConcreteStep {
                    step: resolved,
                    flow_parents: flow_parents.clone(),
                }],
                flow_parents,
                label: step.name.clone(),
                directions: step.directions.clone(),
            })
        }
        FlowItem::FlowRef(name) => expand_flow_ref_branch(name, &[], repo, chain, depth),
        FlowItem::Op(_) => Err(LoadError::InvalidFlow(
            "fork branches cannot contain ops items".to_string(),
        )),
        FlowItem::Fork { .. } => Err(LoadError::InvalidFlow(
            "fork branches cannot contain nested forks".to_string(),
        )),
        FlowItem::Branch(_) => Err(LoadError::InvalidFlow(
            "fork branches cannot contain branch constructs".to_string(),
        )),
    }
}

/// Check whether a loaded flow is a genuine multi-step flow vs a single step
/// auto-wrapped by `load_flow`. Returns `true` if the flow should be expanded.
fn is_multi_step_flow(flow: &Flow, step_name: &str) -> bool {
    flow.items.len() > 1
        || flow
            .items
            .first()
            .map(|i| !matches!(i, FlowItem::Step(s) if s.name == step_name))
            .unwrap_or(false)
}

/// Try to expand a step name as a flow reference. Returns `Some(branch)` if
/// the name resolves to a multi-step flow, `None` if it's just a step.
fn try_expand_step_as_flow(
    step: &Step,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<Option<ConcreteForkBranch>, LoadError> {
    let Ok(nested) = load_flow(&step.name, repo) else {
        return Ok(None);
    };
    if !is_multi_step_flow(&nested, &step.name) {
        return Ok(None);
    }
    let branch = expand_flow_ref_branch(&step.name, &step.directions, repo, chain, depth)?;
    Ok(Some(branch))
}

/// Expand a flow reference into a multi-step fork branch.
fn expand_flow_ref_branch(
    name: &str,
    directions: &[String],
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteForkBranch, LoadError> {
    let nested = load_flow(name, repo)?;
    let nested_items = expand_with_chain(&nested, repo, chain.to_vec(), depth + 1)?;
    let steps = extract_fork_branch_steps(name, &nested_items)?;
    let mut flow_parents = chain.to_vec();
    flow_parents.push(format!("fork/{name}"));
    Ok(ConcreteForkBranch {
        steps,
        flow_parents,
        label: name.to_string(),
        directions: directions.to_vec(),
    })
}

/// Extract concrete steps from expanded flow items for a fork branch.
/// Rejects nested forks — only sequential steps are allowed within branches.
fn extract_fork_branch_steps(
    flow_name: &str,
    items: &[ConcreteItem],
) -> Result<Vec<ConcreteStep>, LoadError> {
    let mut steps = Vec::new();
    for item in items {
        match item {
            ConcreteItem::Step(s) => steps.push(s.clone()),
            ConcreteItem::Op(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "fork branch flow ref '{flow_name}' contains an ops item"
                )))
            }
            ConcreteItem::Fork(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "fork branch flow ref '{flow_name}' contains a nested fork"
                )))
            }
            ConcreteItem::Branch(_) => {
                return Err(LoadError::InvalidFlow(format!(
                    "fork branch flow ref '{flow_name}' contains a branch construct"
                )))
            }
        }
    }
    if steps.is_empty() {
        return Err(LoadError::InvalidFlow(format!(
            "fork branch flow ref '{flow_name}' expands to zero steps"
        )));
    }
    Ok(steps)
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
    fn load_step_finds_repo_local_step() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(steps_dir.join("mystep.md"), "# My Step\nDo the thing.").unwrap();

        let step = load_step("mystep", tmp.path()).unwrap();
        assert_eq!(step.name, "mystep");
        assert!(step.content.unwrap().contains("Do the thing"));
    }

    #[test]
    fn load_step_finds_builtin_step() {
        // Builtin steps like "debug", "implement" should be available everywhere
        let tmp = TempDir::new().unwrap();

        let result = load_step("debug", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'debug' step should be found: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_step_finds_all_builtins() {
        let tmp = TempDir::new().unwrap();
        for name in crate::engine::builtins::builtin_step_names() {
            let result = load_step(name, tmp.path());
            assert!(
                result.is_ok(),
                "builtin '{}' step should be found: {:?}",
                name,
                result.err()
            );
        }
    }

    #[test]
    fn load_step_parses_frontmatter_agent() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("fast.md"),
            r#"---
agent: claude:haiku
---
# Fast Step
Do it quickly.
"#,
        )
        .unwrap();

        let step = load_step("fast", tmp.path()).unwrap();
        assert_eq!(step.agent, Some("claude:haiku".to_string()));
    }

    #[test]
    fn load_step_parses_frontmatter_default_agent() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("fast.md"),
            r#"---
default_agent: gemini:2.5-pro
---
# Fast Step
Do it quickly.
"#,
        )
        .unwrap();

        let step = load_step("fast", tmp.path()).unwrap();
        assert_eq!(step.default_agent, Some("gemini:2.5-pro".to_string()));
    }

    #[test]
    fn load_step_parses_frontmatter_interactive() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("design.md"),
            r#"---
interactive: true
---
# Design Step
Design the feature.
"#,
        )
        .unwrap();

        let step = load_step("design", tmp.path()).unwrap();
        assert_eq!(step.interactive, Some(true));
    }

    #[test]
    fn load_step_parses_frontmatter_action_style() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("design.md"),
            r#"---
action_style: exploratory
---
# Design Step
Design the feature.
"#,
        )
        .unwrap();

        let step = load_step("design", tmp.path()).unwrap();
        assert_eq!(step.action_style.as_deref(), Some("exploratory"));
    }

    #[test]
    fn load_step_includes_frontmatter_directions() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("careful.md"),
            r#"---
directions:
  - thorough
  - tested
---
# Careful Step
Be careful.
"#,
        )
        .unwrap();

        let step = load_step("careful", tmp.path()).unwrap();
        assert_eq!(step.directions, vec!["thorough", "tested"]);
    }

    #[test]
    fn load_step_not_found_error_message() {
        let tmp = TempDir::new().unwrap();

        let result = load_step("nonexistent", tmp.path());
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("nonexistent"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_flow_finds_builtin_flow() {
        let tmp = TempDir::new().unwrap();
        let result = load_flow("ship", tmp.path());
        assert!(
            result.is_ok(),
            "builtin 'ship' flow should be found: {:?}",
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
    fn load_step_falls_back_to_agent_skills() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join(".agents/skills/my-tool");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-tool\n---\nDo the thing.",
        )
        .unwrap();

        let step = load_step("my-tool", tmp.path()).unwrap();
        assert_eq!(step.name, "my-tool");
        assert!(step.content.unwrap().contains("Do the thing."));
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
    fn next_action_marks_interactive_steps_as_wait() {
        let flow = Flow {
            name: "demo".to_string(),
            items: vec![FlowItem::Step(Step {
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
    fn expand_flow_resolves_interactive_from_step_frontmatter() {
        // A bare step name reference in a flow should pick up interactive: true
        // from the step file's frontmatter, not remain None.
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("my-design.md"),
            "---\ninteractive: true\n---\nDesign it.",
        )
        .unwrap();

        let flow = Flow {
            name: "test-flow".to_string(),
            items: vec![FlowItem::Step(Step::named("my-design"))],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);
        let action = next_action(&items, 0);
        assert!(
            matches!(action, FlowAction::WaitInteractive { .. }),
            "bare step reference should resolve interactive from frontmatter"
        );
    }

    #[test]
    fn expand_flow_resolves_builtin_interactive_step() {
        // The builtin "design" step has interactive: true in its frontmatter.
        // A flow referencing it by name should produce WaitInteractive.
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-flow".to_string(),
            items: vec![FlowItem::Step(Step::named("design"))],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        let design = items
            .iter()
            .find(|item| matches!(item, ConcreteItem::Step(s) if s.step.name == "design"));
        assert!(
            design.is_some(),
            "expanded flow should contain a design step"
        );
        if let Some(ConcreteItem::Step(step)) = design {
            assert_eq!(
                step.step.interactive,
                Some(true),
                "builtin design step should have interactive: true after expansion"
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
                    ConcreteItem::Step(step) => {
                        let result = load_step(&step.step.name, tmp.path());
                        assert!(
                            result.is_ok(),
                            "builtin flow '{}' references missing step '{}': {:?}",
                            name,
                            step.step.name,
                            result.err()
                        );
                    }
                    ConcreteItem::Fork(fork) => {
                        for branch in &fork.branches {
                            for step in &branch.steps {
                                let result = load_step(&step.step.name, tmp.path());
                                assert!(
                                    result.is_ok(),
                                    "builtin flow '{}' fork references missing step '{}': {:?}",
                                    name,
                                    step.step.name,
                                    result.err()
                                );
                            }
                        }
                    }
                    ConcreteItem::Op(ops) => {
                        assert!(
                            !ops.item.command.is_empty(),
                            "builtin flow '{}' contains empty ops command",
                            name
                        );
                    }
                    ConcreteItem::Branch(branch) => {
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
    fn parse_fork_step_drafts_shorthand() {
        let yaml = r#"
- review
- fork:
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
            FlowItem::Fork { branches, .. } => {
                assert_eq!(branches.len(), 3);
                for branch in branches {
                    match branch {
                        FlowItem::Step(step) => {
                            assert_eq!(step.name, "reduce");
                            assert_eq!(step.directions.len(), 1);
                        }
                        _ => panic!("expected Step branch"),
                    }
                }
            }
            _ => panic!("expected Fork item"),
        }
    }

    #[test]
    fn parse_step_mapping_accepts_plural_directions_key() {
        let yaml = r#"
- step:
    name: implement
    directions: [designer, product-engineer]
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            FlowItem::Step(step) => {
                assert_eq!(step.name, "implement");
                assert_eq!(step.directions, vec!["designer", "product-engineer"]);
            }
            other => panic!("expected Step, got {other:?}"),
        }
    }

    #[test]
    fn parse_ops_mapping_accepts_command_and_args() {
        let yaml = r#"
- ops: land --create-pr
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            FlowItem::Op(item) => {
                assert_eq!(item.command, "land");
                assert_eq!(item.args, vec!["--create-pr"]);
            }
            other => panic!("expected Ops item, got {other:?}"),
        }
    }

    #[test]
    fn next_action_returns_ops_action() {
        let items = vec![ConcreteItem::Op(ConcreteOp {
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
    fn next_action_marks_missing_steps_as_complete() {
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
    fn parse_fork_flow_drafts_shorthand() {
        let yaml = r#"
- fork:
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
            FlowItem::Fork { branches, .. } => {
                assert_eq!(branches.len(), 3);
                for (i, branch) in branches.iter().enumerate() {
                    match branch {
                        FlowItem::Step(step) => {
                            assert_eq!(step.name, "build");
                            assert_eq!(step.directions.len(), 1);
                        }
                        _ => panic!("expected Step branch at index {i}"),
                    }
                }
            }
            _ => panic!("expected Fork item"),
        }
    }

    #[test]
    fn parse_fork_explicit_branches_with_flow_and_step() {
        let yaml = r#"
- fork:
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
            FlowItem::Fork { branches, .. } => {
                assert_eq!(branches.len(), 2);
                // First branch: flow ref "build" with direction "infra"
                match &branches[0] {
                    FlowItem::Step(step) => {
                        assert_eq!(step.name, "build");
                        assert_eq!(step.directions, vec!["infra"]);
                    }
                    _ => panic!("expected Step branch"),
                }
                // Second branch: step "review" with direction "ux"
                match &branches[1] {
                    FlowItem::Step(step) => {
                        assert_eq!(step.name, "review");
                        assert_eq!(step.directions, vec!["ux"]);
                    }
                    _ => panic!("expected Step branch"),
                }
            }
            _ => panic!("expected Fork item"),
        }
    }

    #[test]
    fn expand_fork_multi_step_flow_ref() {
        let tmp = TempDir::new().unwrap();
        let flows_dir = tmp.path().join(".lf/flows");
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::create_dir_all(&steps_dir).unwrap();

        // Create a multi-step flow
        fs::write(
            flows_dir.join("multi.yaml"),
            "- step-a\n- step-b\n- step-c\n",
        )
        .unwrap();
        fs::write(steps_dir.join("step-a.md"), "Step A").unwrap();
        fs::write(steps_dir.join("step-b.md"), "Step B").unwrap();
        fs::write(steps_dir.join("step-c.md"), "Step C").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![FlowItem::Fork {
                branches: vec![
                    FlowItem::Step(Step {
                        name: "multi".to_string(),
                        directions: vec!["infra".to_string()],
                        ..Step::named("multi")
                    }),
                    FlowItem::Step(Step {
                        name: "multi".to_string(),
                        directions: vec!["ux".to_string()],
                        ..Step::named("multi")
                    }),
                ],
            }],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            ConcreteItem::Fork(fork) => {
                assert_eq!(fork.branches.len(), 2);
                // Each branch should have 3 steps from the "multi" flow
                for branch in &fork.branches {
                    assert_eq!(branch.steps.len(), 3, "branch should have 3 steps");
                    assert_eq!(branch.steps[0].step.name, "step-a");
                    assert_eq!(branch.steps[1].step.name, "step-b");
                    assert_eq!(branch.steps[2].step.name, "step-c");
                    assert_eq!(branch.label, "multi");
                }
                assert_eq!(fork.branches[0].directions, vec!["infra"]);
                assert_eq!(fork.branches[1].directions, vec!["ux"]);
            }
            _ => panic!("expected Fork item"),
        }
    }

    #[test]
    fn expand_fork_single_step_unchanged() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(steps_dir.join("reduce.md"), "Reduce things.").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![FlowItem::Fork {
                branches: vec![
                    FlowItem::Step(Step {
                        name: "reduce".to_string(),
                        directions: vec!["infra".to_string()],
                        ..Step::named("reduce")
                    }),
                    FlowItem::Step(Step {
                        name: "reduce".to_string(),
                        directions: vec!["ux".to_string()],
                        ..Step::named("reduce")
                    }),
                ],
            }],
        };
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 1);

        match &items[0] {
            ConcreteItem::Fork(fork) => {
                assert_eq!(fork.branches.len(), 2);
                // Each branch should have exactly 1 step
                for branch in &fork.branches {
                    assert_eq!(branch.steps.len(), 1, "single-step branch");
                    assert_eq!(branch.steps[0].step.name, "reduce");
                }
            }
            _ => panic!("expected Fork item"),
        }
    }

    #[test]
    fn expand_fork_rejects_nested_fork_in_flow_ref() {
        let tmp = TempDir::new().unwrap();
        let flows_dir = tmp.path().join(".lf/flows");
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::create_dir_all(&steps_dir).unwrap();

        // Create a flow that contains a fork
        fs::write(
            flows_dir.join("has-fork.yaml"),
            r#"
- step-a
- fork:
    step: step-b
    drafts:
      - direction: x
      - direction: y
"#,
        )
        .unwrap();
        fs::write(steps_dir.join("step-a.md"), "Step A").unwrap();
        fs::write(steps_dir.join("step-b.md"), "Step B").unwrap();

        let flow = Flow {
            name: "test".to_string(),
            items: vec![FlowItem::Fork {
                branches: vec![FlowItem::Step(Step {
                    name: "has-fork".to_string(),
                    directions: vec!["infra".to_string()],
                    ..Step::named("has-fork")
                })],
            }],
        };
        let result = expand_flow(&flow, tmp.path());
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("nested fork"),
            "expected nested fork error, got: {err}"
        );
    }

    #[test]
    fn parse_branch_with_flow_paths() {
        let yaml = r#"
- qa
- triage
- branch:
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
            FlowItem::Branch(branch) => {
                assert_eq!(branch.paths.len(), 2);
                let fix = &branch.paths["fix"];
                assert_eq!(fix.flow.as_deref(), Some("qa-fix"));
                assert!(fix.step.is_none());
                assert_eq!(fix.description, "Blocking issues found, fix before deploy");
                let deploy = &branch.paths["deploy"];
                assert_eq!(deploy.flow.as_deref(), Some("deploy"));
                assert_eq!(deploy.description, "Clean enough to ship");
            }
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    #[test]
    fn parse_branch_with_step_path() {
        let yaml = r#"
- branch:
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
            FlowItem::Branch(branch) => {
                let skip = &branch.paths["skip"];
                assert_eq!(skip.step.as_deref(), Some("gate"));
                assert!(skip.flow.is_none());
                let full = &branch.paths["full"];
                assert_eq!(full.flow.as_deref(), Some("build"));
                assert!(full.step.is_none());
            }
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    #[test]
    fn parse_branch_with_direction_override() {
        let yaml = r#"
- branch:
    paths:
      careful:
        flow: build
        description: "Build carefully"
        direction: [care, clarity]
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let items = parse_flow_items(&value).unwrap();

        match &items[0] {
            FlowItem::Branch(branch) => {
                let careful = &branch.paths["careful"];
                assert_eq!(careful.direction, vec!["care", "clarity"]);
            }
            other => panic!("expected Branch, got {other:?}"),
        }
    }

    #[test]
    fn parse_branch_rejects_both_flow_and_step() {
        let yaml = r#"
- branch:
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
    fn parse_branch_rejects_missing_flow_and_step() {
        let yaml = r#"
- branch:
    paths:
      bad:
        description: "no target"
"#;
        let value: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let result = parse_flow_items(&value);
        assert!(result.is_err());
    }

    #[test]
    fn parse_branch_rejects_missing_description() {
        let yaml = r#"
- branch:
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
    fn expand_branch_keeps_concrete_branch() {
        let tmp = TempDir::new().unwrap();
        let flow = Flow {
            name: "test-branch".to_string(),
            items: vec![
                FlowItem::Step(Step::named("gate")),
                FlowItem::Branch(BranchDef {
                    paths: {
                        let mut m = HashMap::new();
                        m.insert(
                            "fix".to_string(),
                            BranchPath {
                                flow: Some("build".to_string()),
                                step: None,
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
        assert!(matches!(&items[0], ConcreteItem::Step(_)));
        assert!(matches!(&items[1], ConcreteItem::Branch(_)));

        if let ConcreteItem::Branch(branch) = &items[1] {
            assert_eq!(branch.paths.len(), 1);
            assert_eq!(branch.paths["fix"].description, "Fix it");
        }
    }

    #[test]
    fn next_action_returns_branch_action() {
        let items = vec![ConcreteItem::Branch(ConcreteBranch {
            paths: {
                let mut m = HashMap::new();
                m.insert(
                    "a".to_string(),
                    BranchPath {
                        flow: Some("build".to_string()),
                        step: None,
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
            matches!(action, FlowAction::Branch { .. }),
            "expected Branch action, got {action:?}"
        );
    }

    #[test]
    fn qa_deploy_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("qa-deploy", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        // qa, triage, branch
        assert_eq!(items.len(), 3);
        assert!(matches!(&items[2], ConcreteItem::Branch(_)));
    }

    #[test]
    fn qa_fix_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("qa-fix", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 4); // implement, compress, lint, gate
    }

    #[test]
    fn deploy_flow_parses_and_expands() {
        let tmp = TempDir::new().unwrap();
        let flow = load_flow("deploy", tmp.path()).unwrap();
        let items = expand_flow(&flow, tmp.path()).unwrap();
        assert_eq!(items.len(), 2); // gate, update-wave
    }
}
