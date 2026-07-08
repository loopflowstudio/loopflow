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
pub struct SkillEntry {
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
    pub skills: Vec<SkillEntry>,
}

#[derive(Debug, Serialize)]
pub struct CatalogResponse {
    ok: bool,
    result: CatalogResult,
}

pub async fn catalog_handler(Query(query): Query<CatalogQuery>) -> ApiResult<CatalogResponse> {
    let repo_path = PathBuf::from(query.repo.unwrap_or_default());

    let flows = collect_flows(&repo_path);
    let skills = collect_skills(&repo_path);

    Ok(Json(CatalogResponse {
        ok: true,
        result: CatalogResult { flows, skills },
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

fn collect_skills(repo: &Path) -> Vec<SkillEntry> {
    let builtin_categories: HashMap<&str, String> =
        crate::engine::builtins::BUILTIN_STEP_CATEGORIES
            .iter()
            .flat_map(|(cat, names)| names.iter().map(move |name| (*name, cat.to_string())))
            .collect();

    let repo_overrides = list_repo_md_stems(&repo.join(".lf/skills"));

    let mut entries: Vec<SkillEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for name in builtin_categories.keys() {
        let name = (*name).to_string();
        if seen.insert(name.clone()) {
            let source = if repo_overrides.contains(&name) {
                EntrySource::Repo
            } else {
                EntrySource::Builtin
            };
            entries.push(build_skill_entry(&name, repo, source, &builtin_categories));
        }
    }

    for name in &repo_overrides {
        if seen.insert(name.clone()) {
            entries.push(build_skill_entry(
                name,
                repo,
                EntrySource::Repo,
                &builtin_categories,
            ));
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

fn build_skill_entry(
    name: &str,
    repo: &Path,
    source: EntrySource,
    builtin_categories: &HashMap<&str, String>,
) -> SkillEntry {
    let category = builtin_categories
        .get(name)
        .cloned()
        .unwrap_or_else(|| "Repo".to_string());
    let description = {
        let d = crate::lf::discovery::builtin_skill_description(name);
        if d.is_empty() {
            None
        } else {
            Some(d)
        }
    };
    let interactive = crate::engine::flow::load_skill(name, repo)
        .ok()
        .and_then(|skill| skill.interactive);
    SkillEntry {
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
            skills: collect_skills(temp.path()),
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
            Some(crate::engine::flow::FlowItem::Skill(skill)) if skill.name == "kickoff"
        ));

        let garden_flow = catalog
            .flows
            .iter()
            .find(|entry| entry.name == "garden")
            .expect("garden flow present");
        assert_eq!(garden_flow.category, "Govern");
    }

    #[test]
    fn catalog_includes_skill_categories_with_descriptions() {
        let catalog = build_catalog();

        let scan_skill = catalog
            .skills
            .iter()
            .find(|entry| entry.name == "scan")
            .expect("scan skill present");
        assert_eq!(scan_skill.category, "Govern");
        assert_eq!(scan_skill.source, EntrySource::Builtin);
        // Description comes from scan.md's first prose line (after frontmatter).
        assert!(
            scan_skill
                .description
                .as_deref()
                .unwrap_or("")
                .starts_with("Read the territory"),
            "unexpected description: {:?}",
            scan_skill.description
        );

        let implement_skill = catalog
            .skills
            .iter()
            .find(|entry| entry.name == "implement")
            .expect("implement skill present");
        assert_eq!(implement_skill.category, "Build");
    }
}
