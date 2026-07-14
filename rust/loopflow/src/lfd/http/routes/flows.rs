use axum::extract::Query;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::lfd::http::ApiResult;

#[derive(Deserialize)]
pub struct ListFlowsQuery {
    repo: Option<String>,
}

#[derive(Debug, Serialize)]
struct SkillSummary {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_agent: Option<String>,
}

#[derive(Debug, Serialize)]
struct FlowSummary {
    name: String,
    steps: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FlowsResult {
    flows: Vec<FlowSummary>,
    skills: Vec<SkillSummary>,
    directions: Vec<String>,
    supported_harnesses: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct FlowsResponse {
    ok: bool,
    result: FlowsResult,
}

pub async fn list_flows_handler(Query(query): Query<ListFlowsQuery>) -> ApiResult<FlowsResponse> {
    let repo = query.repo.unwrap_or_default();
    let repo_path = PathBuf::from(repo);

    let flows = list_flows(&repo_path);
    let skills = list_skills(&repo_path);
    let directions = crate::lf::discovery::list_directions(Some(&repo_path));
    let supported_harnesses =
        crate::engine::config::load_config_or_default(Some(&repo_path)).supported_harnesses;

    Ok(Json(FlowsResponse {
        ok: true,
        result: FlowsResult {
            flows,
            skills,
            directions,
            supported_harnesses,
        },
    }))
}

fn list_flows(repo: &Path) -> Vec<FlowSummary> {
    let mut flows: HashMap<String, Vec<String>> = HashMap::new();

    for (name, skills) in list_repo_flows(repo) {
        flows.insert(name, skills);
    }

    for name in crate::engine::builtins::builtin_flow_names() {
        let name = name.to_string();
        if flows.contains_key(&name) {
            continue;
        }
        if let Some(skills) = load_flow_steps(&name, repo) {
            flows.insert(name, skills);
        }
    }

    let mut result: Vec<FlowSummary> = flows
        .into_iter()
        .map(|(name, steps)| FlowSummary { name, steps })
        .collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

fn list_repo_flows(repo: &Path) -> Vec<(String, Vec<String>)> {
    let mut result = Vec::new();
    let flows_dir = repo.join(".lf/flows");
    let Ok(entries) = std::fs::read_dir(flows_dir) else {
        return result;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(ext, "yaml" | "yml" | "json") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Some(skills) = load_flow_steps(name, repo) {
            result.push((name.to_string(), skills));
        }
    }

    result
}

pub(super) fn load_flow_steps(name: &str, repo: &Path) -> Option<Vec<String>> {
    let flow = crate::engine::flow::load_flow(name, repo).ok()?;
    let items = crate::engine::flow::expand_flow(&flow, repo).ok()?;
    Some(extract_step_names(&items))
}

fn extract_step_names(items: &[crate::engine::flow::ConcreteStep]) -> Vec<String> {
    let mut names = Vec::new();
    for item in items {
        match item {
            crate::engine::flow::ConcreteStep::Skill(skill) => {
                names.push(skill.skill.name.clone());
            }
            crate::engine::flow::ConcreteStep::Op(ops) => {
                names.push(ops.item.to_string());
            }
            crate::engine::flow::ConcreteStep::Xor(_) => {
                names.push("[xor]".to_string());
            }
        }
    }
    names
}

fn list_skills(repo: &Path) -> Vec<SkillSummary> {
    let mut names: HashSet<String> = crate::engine::builtins::builtin_skill_names()
        .into_iter()
        .map(|name| name.to_string())
        .collect();

    let skills_dir = repo.join(".lf/skills");
    if let Ok(entries) = std::fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                names.insert(name.to_string());
            }
        }
    }

    let mut list: Vec<SkillSummary> = names
        .into_iter()
        .map(|name| load_skill_summary(repo, name))
        .collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

fn load_skill_summary(repo: &Path, name: String) -> SkillSummary {
    match crate::engine::flow::load_skill(&name, repo) {
        Ok(skill) => SkillSummary {
            name,
            agent: skill.agent,
            default_agent: skill.default_agent,
        },
        Err(_) => SkillSummary {
            name,
            agent: None,
            default_agent: None,
        },
    }
}
