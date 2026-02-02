use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml::Value;

use crate::error::LoadError;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "data")]
pub enum FlowItem {
    Step(Step),
    Fork {
        branches: Vec<FlowItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        synthesize: Option<String>,
    },
    Choose {
        prompt: String,
        options: HashMap<String, Vec<FlowItem>>,
    },
    LoopUntilEmpty {
        steps: Vec<FlowItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        wave: Option<String>,
        #[serde(default = "default_loop_max_iterations")]
        max_iterations: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Flow {
    pub name: String,
    pub items: Vec<FlowItem>,
}

#[derive(Debug, Clone)]
pub struct Direction {
    pub name: String,
    pub content: String,
    pub source: PathBuf,
}

pub fn load_flow(name: &str, repo: &Path) -> Result<Flow, LoadError> {
    let flow_path = find_flow_path(name, repo)?;
    let content = fs::read_to_string(&flow_path)?;
    let value: Value =
        serde_yaml::from_str(&content).map_err(|err| LoadError::InvalidFlow(err.to_string()))?;
    let items = parse_flow_items(&value)?;
    Ok(Flow {
        name: name.to_string(),
        items,
    })
}

pub fn load_step(name: &str, repo: &Path) -> Result<Step, LoadError> {
    // Try file-based lookup first (repo-local, then global)
    if let Ok(step_path) = find_step_path(name, repo) {
        let content = fs::read_to_string(&step_path)?;
        return Ok(Step {
            name: name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: parse_interactive_from_frontmatter(&content),
            content: Some(content),
        });
    }

    // Fall back to built-in steps
    if let Some(content) = crate::builtins::get_builtin_step(name) {
        return Ok(Step {
            name: name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: parse_interactive_from_frontmatter(content),
            content: Some(content.to_string()),
        });
    }

    Err(LoadError::StepNotFound(name.to_string()))
}

/// Parse the `interactive` field from YAML frontmatter.
fn parse_interactive_from_frontmatter(content: &str) -> Option<bool> {
    if !content.starts_with("---") {
        return None;
    }
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() < 3 {
        return None;
    }
    for line in parts[1].lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("interactive:") {
            let val = rest.trim().to_lowercase();
            return Some(val == "true" || val == "yes");
        }
    }
    None
}

pub fn load_direction(name: &str, repo: &Path) -> Result<Direction, LoadError> {
    let direction_path = find_direction_path(name, repo)?;
    let content = fs::read_to_string(&direction_path)?;
    Ok(Direction {
        name: name.to_string(),
        content,
        source: direction_path,
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
        Value::String(name) => Ok(FlowItem::Step(Step {
            name: name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: None,
            content: None,
        })),
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
    if let Some(fork_value) = map.get(key("fork")) {
        return parse_fork_value(fork_value);
    }
    if let Some(choose_value) = map.get(key("choose")) {
        return parse_choose_value(choose_value);
    }
    if let Some(loop_value) = map.get(key("loop_until_empty")) {
        return parse_loop_value(loop_value);
    }
    Err(LoadError::InvalidFlow(
        "flow item mapping must include step, fork, choose, or loop_until_empty".to_string(),
    ))
}

fn parse_step_value(value: &Value) -> Result<Step, LoadError> {
    match value {
        Value::String(name) => Ok(Step {
            name: name.to_string(),
            model: None,
            directions: Vec::new(),
            interactive: None,
            content: None,
        }),
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
    let branches_value = map
        .get(key("branches"))
        .ok_or_else(|| LoadError::InvalidFlow("fork missing branches".to_string()))?;
    let branches = match branches_value {
        Value::Sequence(seq) => seq.iter().map(parse_flow_item).collect::<Result<_, _>>()?,
        _ => {
            return Err(LoadError::InvalidFlow(
                "fork branches must be list".to_string(),
            ))
        }
    };
    let synthesize = map
        .get(key("synthesize"))
        .and_then(|val| val.as_str())
        .map(|val| val.to_string());
    Ok(FlowItem::Fork {
        branches,
        synthesize,
    })
}

fn parse_choose_value(value: &Value) -> Result<FlowItem, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("choose must be mapping".to_string()))?;
    let prompt = map
        .get(key("prompt"))
        .and_then(|val| val.as_str())
        .ok_or_else(|| LoadError::InvalidFlow("choose missing prompt".to_string()))?
        .to_string();
    let options_value = map
        .get(key("options"))
        .ok_or_else(|| LoadError::InvalidFlow("choose missing options".to_string()))?;
    let options_map = options_value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("choose options must be mapping".to_string()))?;
    let mut options = HashMap::new();
    for (k, v) in options_map {
        let k = k
            .as_str()
            .ok_or_else(|| LoadError::InvalidFlow("choose option key must be string".to_string()))?
            .to_string();
        let items = parse_flow_items(v)?;
        options.insert(k, items);
    }
    Ok(FlowItem::Choose { prompt, options })
}

fn parse_loop_value(value: &Value) -> Result<FlowItem, LoadError> {
    let map = value
        .as_mapping()
        .ok_or_else(|| LoadError::InvalidFlow("loop_until_empty must be mapping".to_string()))?;
    let steps_value = map
        .get(key("steps"))
        .ok_or_else(|| LoadError::InvalidFlow("loop_until_empty missing steps".to_string()))?;
    let steps = parse_flow_items(steps_value)?;
    let wave = map
        .get(key("wave"))
        .and_then(|val| val.as_str())
        .map(|val| val.to_string());
    let max_iterations = map
        .get(key("max_iterations"))
        .and_then(|val| val.as_i64())
        .map(|val| val.max(1) as usize)
        .unwrap_or_else(default_loop_max_iterations);
    Ok(FlowItem::LoopUntilEmpty {
        steps,
        wave,
        max_iterations,
    })
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

fn default_loop_max_iterations() -> usize {
    100
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
        let builtins = [
            "debug",
            "implement",
            "design",
            "review",
            "iterate",
            "polish",
            "lint",
        ];

        for name in builtins {
            let result = load_step(name, tmp.path());
            assert!(
                result.is_ok(),
                "builtin '{}' step should be found: {:?}",
                name,
                result.err()
            );
        }
    }
}
