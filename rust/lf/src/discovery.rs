use anyhow::Result;
use loopflow_engine::Step;
use std::path::Path;

/// Discover a step by name, checking repo → global.
pub fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    let repo_paths = [
        repo.join(".lf/steps").join(format!("{name}.md")),
        repo.join(".claude/commands").join(format!("{name}.md")),
    ];
    for path in repo_paths {
        if path.exists() {
            return loopflow_engine::load_step(name, repo).map_err(Into::into);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let global_paths = [
            home.join(".lf/steps").join(format!("{name}.md")),
            home.join(".claude/commands").join(format!("{name}.md")),
        ];
        for path in global_paths {
            if path.exists() {
                return loopflow_engine::load_step(name, &home).map_err(Into::into);
            }
        }
    }

    loopflow_engine::load_step(name, repo).map_err(Into::into)
}

pub fn list_steps(repo: &Path) -> Vec<String> {
    let mut steps = Vec::new();

    for dir in [repo.join(".lf/steps"), repo.join(".claude/commands")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(name) = path.file_stem() {
                        steps.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    if let Some(home) = dirs::home_dir() {
        for dir in [home.join(".lf/steps"), home.join(".claude/commands")] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Some(name) = path.file_stem() {
                            let name = name.to_string_lossy().to_string();
                            if !steps.contains(&name) {
                                steps.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    steps.sort();
    steps.dedup();
    steps
}

pub fn list_flows(repo: &Path) -> Vec<String> {
    let mut flows = Vec::new();
    let flows_dir = repo.join(".lf/flows");

    if let Ok(entries) = std::fs::read_dir(flows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                let ext = path.extension().map(|e| e.to_string_lossy().to_string());
                if matches!(ext.as_deref(), Some("yaml") | Some("yml") | Some("json")) {
                    flows.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    flows.sort();
    flows
}

pub fn list_directions(repo: &Path) -> Vec<String> {
    let mut directions = Vec::new();
    let dir = repo.join(".lf/directions");

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "md").unwrap_or(false) {
                if let Some(name) = path.file_stem() {
                    directions.push(name.to_string_lossy().to_string());
                }
            }
        }
    }

    directions.sort();
    directions
}
