use axum::extract::Query;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::engine::flow::FlowItem;
use crate::lfd::http::ApiResult;

#[derive(Deserialize)]
pub struct CatalogQuery {
    repo: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FlowEntry {
    pub name: String,
    pub category: String,
    pub source: EntrySource,
    pub items: Vec<FlowItem>,
}

#[derive(Debug, Serialize)]
pub struct StepEntry {
    pub name: String,
    pub category: String,
    pub source: EntrySource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EntrySource {
    Builtin,
    Repo,
}

#[derive(Debug, Serialize)]
pub struct CatalogResult {
    pub flows: Vec<FlowEntry>,
    pub steps: Vec<StepEntry>,
}

#[derive(Debug, Serialize)]
pub struct CatalogResponse {
    ok: bool,
    result: CatalogResult,
}

pub async fn catalog_handler(Query(query): Query<CatalogQuery>) -> ApiResult<CatalogResponse> {
    let repo_path = PathBuf::from(query.repo.unwrap_or_default());

    let flows = collect_flows(&repo_path);
    let steps = collect_steps(&repo_path);

    Ok(Json(CatalogResponse {
        ok: true,
        result: CatalogResult { flows, steps },
    }))
}

fn collect_flows(repo: &Path) -> Vec<FlowEntry> {
    let builtin_categories: HashMap<&str, String> =
        crate::engine::builtins::BUILTIN_FLOW_CATEGORIES
            .iter()
            .flat_map(|(cat, names)| names.iter().map(move |name| (*name, cat.to_string())))
            .collect();

    let repo_overrides = list_repo_yaml_stems(&repo.join(".lf/flows"));

    let mut entries: Vec<FlowEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for name in builtin_categories.keys() {
        let name = (*name).to_string();
        if seen.insert(name.clone()) {
            let source = if repo_overrides.contains(&name) {
                EntrySource::Repo
            } else {
                EntrySource::Builtin
            };
            if let Some(entry) = load_flow_entry(&name, repo, source, &builtin_categories) {
                entries.push(entry);
            }
        }
    }

    for name in &repo_overrides {
        if seen.insert(name.clone()) {
            if let Some(entry) = load_flow_entry(name, repo, EntrySource::Repo, &builtin_categories)
            {
                entries.push(entry);
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn load_flow_entry(
    name: &str,
    repo: &Path,
    source: EntrySource,
    builtin_categories: &HashMap<&str, String>,
) -> Option<FlowEntry> {
    let flow = crate::engine::flow::load_flow(name, repo).ok()?;
    let category = builtin_categories
        .get(name)
        .cloned()
        .unwrap_or_else(|| "Repo".to_string());
    Some(FlowEntry {
        name: name.to_string(),
        category,
        source,
        items: flow.items,
    })
}

fn collect_steps(repo: &Path) -> Vec<StepEntry> {
    let builtin_categories: HashMap<&str, String> =
        crate::engine::builtins::BUILTIN_STEP_CATEGORIES
            .iter()
            .flat_map(|(cat, names)| names.iter().map(move |name| (*name, cat.to_string())))
            .collect();

    let descriptions = crate::lf::discovery::builtin_descriptions();
    let repo_overrides = list_repo_md_stems(&repo.join(".lf/steps"));

    let mut entries: Vec<StepEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for name in builtin_categories.keys() {
        let name = (*name).to_string();
        if seen.insert(name.clone()) {
            let source = if repo_overrides.contains(&name) {
                EntrySource::Repo
            } else {
                EntrySource::Builtin
            };
            entries.push(build_step_entry(
                &name,
                repo,
                source,
                &builtin_categories,
                &descriptions,
            ));
        }
    }

    for name in &repo_overrides {
        if seen.insert(name.clone()) {
            entries.push(build_step_entry(
                name,
                repo,
                EntrySource::Repo,
                &builtin_categories,
                &descriptions,
            ));
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn build_step_entry(
    name: &str,
    repo: &Path,
    source: EntrySource,
    builtin_categories: &HashMap<&str, String>,
    descriptions: &HashMap<&'static str, &'static str>,
) -> StepEntry {
    let category = builtin_categories
        .get(name)
        .cloned()
        .unwrap_or_else(|| "Repo".to_string());
    let description = descriptions.get(name).map(|s| s.to_string());
    let interactive = crate::engine::flow::load_step(name, repo)
        .ok()
        .and_then(|step| step.interactive);
    StepEntry {
        name: name.to_string(),
        category,
        source,
        description,
        interactive,
    }
}

fn list_repo_yaml_stems(dir: &Path) -> HashSet<String> {
    let mut stems = HashSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return stems;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !matches!(ext, "yaml" | "yml") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.insert(stem.to_string());
        }
    }
    stems
}

fn list_repo_md_stems(dir: &Path) -> HashSet<String> {
    let mut stems = HashSet::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return stems;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            stems.insert(stem.to_string());
        }
    }
    stems
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn build_catalog() -> CatalogResult {
        let temp = TempDir::new().expect("tempdir");
        CatalogResult {
            flows: collect_flows(temp.path()),
            steps: collect_steps(temp.path()),
        }
    }

    #[test]
    fn catalog_includes_builtin_flows_with_categories_and_items() {
        let catalog = build_catalog();

        let build_flow = catalog
            .flows
            .iter()
            .find(|entry| entry.name == "build")
            .expect("build flow present");
        assert_eq!(build_flow.category, "Build");
        assert_eq!(build_flow.source, EntrySource::Builtin);
        assert!(matches!(
            build_flow.items.first(),
            Some(crate::engine::flow::FlowItem::Step(step)) if step.name == "kickoff"
        ));

        let garden_flow = catalog
            .flows
            .iter()
            .find(|entry| entry.name == "garden")
            .expect("garden flow present");
        assert_eq!(garden_flow.category, "Govern");
    }

    #[test]
    fn catalog_includes_step_categories_with_descriptions() {
        let catalog = build_catalog();

        let scan_step = catalog
            .steps
            .iter()
            .find(|entry| entry.name == "scan")
            .expect("scan step present");
        assert_eq!(scan_step.category, "Govern");
        assert_eq!(scan_step.source, EntrySource::Builtin);
        assert_eq!(
            scan_step.description.as_deref(),
            Some("Read member wave state")
        );

        let implement_step = catalog
            .steps
            .iter()
            .find(|entry| entry.name == "implement")
            .expect("implement step present");
        assert_eq!(implement_step.category, "Build");
    }
}
