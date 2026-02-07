use axum::extract::Query;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::http::ApiResult;

#[derive(Deserialize)]
pub struct ListFlowsQuery {
    repo: Option<String>,
}

#[derive(Debug, Serialize)]
struct StepSummary {
    name: String,
}

#[derive(Debug, Serialize)]
struct FlowSummary {
    name: String,
    steps: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FlowsResult {
    flows: Vec<FlowSummary>,
    steps: Vec<StepSummary>,
}

#[derive(Debug, Serialize)]
struct FlowsResponse {
    ok: bool,
    result: FlowsResult,
}

pub async fn list_flows_handler(Query(query): Query<ListFlowsQuery>) -> ApiResult<FlowsResponse> {
    let repo = query.repo.unwrap_or_default();
    let repo_path = PathBuf::from(repo);

    let flows = list_flows(&repo_path);
    let steps = list_steps(&repo_path);

    Ok(Json(FlowsResponse {
        ok: true,
        result: FlowsResult { flows, steps },
    }))
}

fn list_flows(repo: &Path) -> Vec<FlowSummary> {
    let mut flows: HashMap<String, Vec<String>> = HashMap::new();

    for (name, steps) in list_repo_flows(repo) {
        flows.insert(name, steps);
    }

    for name in loopflow_engine::builtins::builtin_flow_names() {
        let name = name.to_string();
        if flows.contains_key(&name) {
            continue;
        }
        if let Some(steps) = load_flow_steps(&name, repo) {
            flows.insert(name, steps);
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
        if let Some(steps) = load_flow_steps(name, repo) {
            result.push((name.to_string(), steps));
        }
    }

    result
}

fn load_flow_steps(name: &str, repo: &Path) -> Option<Vec<String>> {
    let flow = loopflow_engine::flow::load_flow(name, repo).ok()?;
    let items = loopflow_engine::flow::expand_flow(&flow, repo).ok()?;
    Some(extract_step_names(&items))
}

fn extract_step_names(items: &[loopflow_engine::flow::ConcreteItem]) -> Vec<String> {
    let mut names = Vec::new();
    for item in items {
        match item {
            loopflow_engine::flow::ConcreteItem::Step(step) => {
                names.push(step.step.name.clone());
            }
            loopflow_engine::flow::ConcreteItem::Fork(fork) => {
                for branch in &fork.branches {
                    names.push(branch.step.name.clone());
                }
            }
        }
    }
    names
}

fn list_steps(repo: &Path) -> Vec<StepSummary> {
    let mut names: HashSet<String> = loopflow_engine::builtins::builtin_step_names()
        .into_iter()
        .map(|name| name.to_string())
        .collect();

    let steps_dir = repo.join(".lf/steps");
    if let Ok(entries) = std::fs::read_dir(steps_dir) {
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

    let mut list: Vec<StepSummary> = names.into_iter().map(|name| StepSummary { name }).collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}
