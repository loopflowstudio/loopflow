use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::engine::error::LoadError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

impl Step {
    pub fn named(name: &str) -> Self {
        Self {
            name: name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: None,
            content: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum FlowItem {
    Step(Step),
    Fork {
        branches: Vec<FlowItem>,
        #[serde(default)]
        select: ForkSelect,
        #[serde(skip_serializing_if = "Option::is_none")]
        synthesize: Option<String>,
    },
    FlowRef(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForkSelect {
    #[default]
    All,
    One,
    Prompt {
        prompt: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlowAction {
    RunStep { step: ConcreteStep },
    WaitInteractive { step: ConcreteStep },
    Fork { fork: ConcreteFork },
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
pub struct ConcreteFork {
    pub branches: Vec<ConcreteStep>,
    pub select: ForkSelect,
    pub synthesize: Option<String>,
    pub flow_parents: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConcreteItem {
    Step(ConcreteStep),
    Fork(ConcreteFork),
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
        ConcreteItem::Fork(fork) => FlowAction::Fork { fork },
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
        serde_yaml::from_str(&content).map_err(|err| LoadError::InvalidFlow(err.to_string()))?;
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
        let (frontmatter, body) = parse_step_frontmatter(&content)?;
        return Ok(Step {
            name: name.to_string(),
            model: frontmatter.model,
            directions: frontmatter.directions,
            interactive: frontmatter.interactive,
            content: Some(body),
        });
    }

    // Fall back to built-in steps
    if let Some(content) = crate::engine::builtins::get_builtin_step(name) {
        let (frontmatter, body) = parse_step_frontmatter(content)?;
        return Ok(Step {
            name: name.to_string(),
            model: frontmatter.model,
            directions: frontmatter.directions,
            interactive: frontmatter.interactive,
            content: Some(body),
        });
    }

    Err(LoadError::StepNotFound(name.to_string()))
}

#[derive(Debug, Default)]
struct StepFrontmatter {
    model: Option<String>,
    directions: Vec<String>,
    interactive: Option<bool>,
}

fn parse_step_frontmatter(content: &str) -> Result<(StepFrontmatter, String), LoadError> {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return Ok((StepFrontmatter::default(), content.to_string()));
    };

    let value: Value = serde_yaml::from_str(&frontmatter)
        .map_err(|err| LoadError::InvalidStep(err.to_string()))?;
    Ok((parse_frontmatter_value(&value), body))
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

    let model = map
        .get(key("model"))
        .and_then(|val| val.as_str())
        .map(|val| val.to_string());
    let interactive = map.get(key("interactive")).and_then(|val| val.as_bool());
    let mut directions = parse_string_list(map.get(key("directions")));
    if directions.is_empty() {
        directions = parse_string_list(map.get(key("direction")));
    }

    StepFrontmatter {
        model,
        directions,
        interactive,
    }
}

pub fn load_direction(name: &str, repo: &Path) -> Result<Direction, LoadError> {
    let (content, source) = match find_direction_path(name, repo) {
        Ok(direction_path) => (fs::read_to_string(&direction_path)?, direction_path),
        Err(LoadError::DirectionNotFound(_)) => {
            let Some(builtin) = crate::engine::builtins::get_builtin_direction(name) else {
                return Err(LoadError::DirectionNotFound(name.to_string()));
            };
            (
                builtin.to_string(),
                PathBuf::from(format!("builtin:{name}")),
            )
        }
        Err(err) => return Err(err),
    };
    Ok(Direction {
        name: name.to_string(),
        content,
        source,
    })
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
    Err(LoadError::DirectionNotFound(name.to_string()))
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

fn parse_flow_mapping(map: &serde_yaml::Mapping) -> Result<FlowItem, LoadError> {
    if let Some(step_value) = map.get(key("step")) {
        return Ok(FlowItem::Step(parse_step_value(step_value)?));
    }
    if let Some(flow_value) = map.get(key("flow")) {
        return parse_flow_ref_value(flow_value);
    }
    if let Some(fork_value) = map.get(key("fork")) {
        return parse_fork_value(fork_value);
    }
    Err(LoadError::InvalidFlow(
        "flow item mapping must include step, flow, or fork".to_string(),
    ))
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
            let model = map
                .get(key("model"))
                .and_then(|val| val.as_str())
                .map(|val| val.to_string());
            let interactive = map.get(key("interactive")).and_then(|val| val.as_bool());
            let directions = parse_string_list(map.get(key("direction")));
            Ok(Step {
                name,
                model,
                directions,
                interactive,
                content: None,
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

    // Two formats:
    // 1. Explicit branches: fork: { branches: [...] }
    // 2. Shorthand: fork: { step: "reduce", drafts: [{ direction: "..." }, ...] }
    let branches = if let Some(branches_value) = map.get(key("branches")) {
        match branches_value {
            Value::Sequence(seq) => seq.iter().map(parse_flow_item).collect::<Result<_, _>>()?,
            _ => {
                return Err(LoadError::InvalidFlow(
                    "fork branches must be list".to_string(),
                ))
            }
        }
    } else if let Some(step_value) = map.get(key("step")) {
        let step_name = step_value
            .as_str()
            .ok_or_else(|| LoadError::InvalidFlow("fork step must be string".to_string()))?;
        let drafts = map
            .get(key("drafts"))
            .ok_or_else(|| LoadError::InvalidFlow("fork with step requires drafts".to_string()))?;
        let drafts_seq = drafts
            .as_sequence()
            .ok_or_else(|| LoadError::InvalidFlow("fork drafts must be list".to_string()))?;

        let mut branches = Vec::new();
        for draft in drafts_seq {
            let draft_map = draft
                .as_mapping()
                .ok_or_else(|| LoadError::InvalidFlow("fork draft must be mapping".to_string()))?;
            let directions = parse_string_list(draft_map.get(key("direction")));
            branches.push(FlowItem::Step(Step {
                name: step_name.to_string(),
                model: None,
                directions,
                interactive: None,
                content: None,
            }));
        }
        branches
    } else {
        return Err(LoadError::InvalidFlow(
            "fork must have branches or step+drafts".to_string(),
        ));
    };

    let synthesize = map
        .get(key("synthesize"))
        .and_then(|val| val.as_str())
        .map(|val| val.to_string());
    let select = parse_fork_select(map)?;
    Ok(FlowItem::Fork {
        branches,
        select,
        synthesize,
    })
}

fn parse_flow_ref_value(value: &Value) -> Result<FlowItem, LoadError> {
    let name = value
        .as_str()
        .ok_or_else(|| LoadError::InvalidFlow("flow ref must be string".to_string()))?;
    Ok(FlowItem::FlowRef(name.to_string()))
}

fn parse_fork_select(map: &serde_yaml::Mapping) -> Result<ForkSelect, LoadError> {
    let select_value = map.get(key("select"));
    let select = match select_value.and_then(|val| val.as_str()) {
        None => ForkSelect::All,
        Some("all") => ForkSelect::All,
        Some("one") => ForkSelect::One,
        Some("prompt") => {
            let prompt = map
                .get(key("prompt"))
                .and_then(|val| val.as_str())
                .ok_or_else(|| LoadError::InvalidFlow("fork select prompt missing".to_string()))?;
            ForkSelect::Prompt {
                prompt: prompt.to_string(),
            }
        }
        Some(other) => {
            return Err(LoadError::InvalidFlow(format!(
                "unknown fork select mode: {other}"
            )))
        }
    };
    Ok(select)
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
                        // Only expand if it resolved as a multi-step flow, not
                        // if load_flow auto-wrapped a step as a single-step flow.
                        if nested.items.len() > 1
                            || nested
                                .items
                                .first()
                                .map(|i| !matches!(i, FlowItem::Step(s) if s.name == step.name))
                                .unwrap_or(false)
                        {
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
                    step: step.clone(),
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
            FlowItem::Fork {
                branches,
                select,
                synthesize,
            } => {
                let fork = expand_fork(branches, select, synthesize, repo, &chain, depth)?;
                items.push(ConcreteItem::Fork(fork));
            }
        }
    }

    Ok(items)
}

fn expand_fork(
    branches: &[FlowItem],
    select: &ForkSelect,
    synthesize: &Option<String>,
    repo: &Path,
    chain: &[String],
    depth: usize,
) -> Result<ConcreteFork, LoadError> {
    let mut expanded_branches = Vec::new();
    for branch in branches {
        let (step, label) = match branch {
            FlowItem::Step(step) => (step.clone(), step.name.clone()),
            FlowItem::FlowRef(name) => {
                let nested = load_flow(name, repo)?;
                let nested_items = expand_with_chain(&nested, repo, chain.to_vec(), depth + 1)?;
                match nested_items.as_slice() {
                    [ConcreteItem::Step(step)] => (step.step.clone(), name.clone()),
                    _ => {
                        return Err(LoadError::InvalidFlow(format!(
                            "fork flow ref {name} must expand to a single step"
                        )))
                    }
                }
            }
            FlowItem::Fork { .. } => {
                return Err(LoadError::InvalidFlow(
                    "fork branches cannot contain nested forks".to_string(),
                ))
            }
        };

        let mut flow_parents = chain.to_vec();
        flow_parents.push(format!("fork/{label}"));
        expanded_branches.push(ConcreteStep { step, flow_parents });
    }

    Ok(ConcreteFork {
        branches: expanded_branches,
        select: select.clone(),
        synthesize: synthesize.clone(),
        flow_parents: chain.to_vec(),
    })
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
    fn load_step_parses_frontmatter_model() {
        let tmp = TempDir::new().unwrap();
        let steps_dir = tmp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        fs::write(
            steps_dir.join("fast.md"),
            r#"---
model: claude:haiku
---
# Fast Step
Do it quickly.
"#,
        )
        .unwrap();

        let step = load_step("fast", tmp.path()).unwrap();
        assert_eq!(step.model, Some("claude:haiku".to_string()));
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
        let result = load_direction("product-engineer", tmp.path());
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
    fn next_action_marks_interactive_steps_as_wait() {
        let flow = Flow {
            name: "demo".to_string(),
            items: vec![FlowItem::Step(Step {
                name: "design".to_string(),
                model: None,
                directions: Vec::new(),
                interactive: Some(true),
                content: None,
            })],
        };

        let repo = TempDir::new().unwrap();
        let items = expand_flow(&flow, repo.path()).unwrap();
        let action = next_action(&items, 0);
        assert!(matches!(action, FlowAction::WaitInteractive { .. }));
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
        }
    }

    #[test]
    fn parse_fork_step_drafts_shorthand() {
        let yaml = r#"
- review
- fork:
    step: reduce
    drafts:
      - direction: infra-engineer
      - direction: designer
      - direction: product-engineer
- publish
"#;
        let value: Value = serde_yaml::from_str(yaml).unwrap();
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
}
