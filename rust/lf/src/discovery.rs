use anyhow::Result;
use loopflow_engine::Step;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// =============================================================================
// Built-in step metadata for formatted listing
// =============================================================================

pub const BUILTIN_CATEGORIES: &[(&str, &[&str])] = &[
    ("Setup", &["init"]),
    ("Planning & Design", &["design", "explore", "refine"]),
    (
        "Implementation",
        &["implement", "iterate", "expand", "reduce"],
    ),
    ("Quality", &["review", "polish", "lint", "debug"]),
    ("Git", &["commit", "rebase"]),
];

pub fn builtin_descriptions() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("init", "Set up loopflow in this repo"),
        ("design", "Plan what to build"),
        ("explore", "Investigate current diff"),
        ("implement", "Build from design doc"),
        ("iterate", "Improve code on branch"),
        ("expand", "Explore ambitious extensions"),
        ("reduce", "Simplify while preserving behavior"),
        ("review", "Assess code, write verdict"),
        ("polish", "Fix issues, run tests"),
        ("lint", "Run linter, fix issues"),
        ("debug", "Fix errors from clipboard"),
        ("commit", "Commit with generated message"),
        ("rebase", "Rebase onto main"),
        ("refine", "Iteratively refine text"),
    ])
}

/// All builtin step names (from BUILTIN_CATEGORIES).
pub fn builtin_steps() -> HashSet<String> {
    BUILTIN_CATEGORIES
        .iter()
        .flat_map(|(_, steps)| steps.iter().map(|s| (*s).to_string()))
        .collect()
}

/// Check if a step is interactive by loading it via the engine.
pub fn is_step_interactive(repo: &Path, name: &str) -> bool {
    loopflow_engine::load_step(name, repo)
        .map(|s| s.interactive.unwrap_or(false))
        .unwrap_or(false)
}

// =============================================================================
// External skill sources
// =============================================================================

#[derive(Debug)]
pub struct SkillSource {
    pub name: String,
    pub prefix: String,
    pub skills: Vec<String>,
}

/// Discover external skill sources (superpowers, rams).
pub fn discover_skill_sources(repo: Option<&Path>) -> Vec<SkillSource> {
    let mut sources = Vec::new();
    let mut seen_prefixes = HashSet::new();

    // Check for superpowers
    let sp_paths = [
        repo.map(|r| r.join("superpowers")),
        dirs::home_dir().map(|h| h.join(".superpowers")),
        dirs::home_dir().map(|h| h.join("superpowers")),
    ];

    for path in sp_paths.into_iter().flatten() {
        if seen_prefixes.contains("sp") {
            break;
        }
        if path.exists() {
            let skills = discover_superpowers_skills(&path);
            if !skills.is_empty() {
                sources.push(SkillSource {
                    name: "superpowers".to_string(),
                    prefix: "sp".to_string(),
                    skills,
                });
                seen_prefixes.insert("sp".to_string());
            }
        }
    }

    // Check for rams at ~/.claude/commands/rams.md
    if !seen_prefixes.contains("rams") {
        if let Some(home) = dirs::home_dir() {
            let rams_path = home.join(".claude/commands/rams.md");
            if rams_path.exists() {
                sources.push(SkillSource {
                    name: "rams.ai".to_string(),
                    prefix: "rams".to_string(),
                    skills: vec!["rams".to_string()],
                });
            }
        }
    }

    sources
}

fn discover_superpowers_skills(source_path: &Path) -> Vec<String> {
    let skills_dir = source_path.join("skills");
    if !skills_dir.exists() {
        return Vec::new();
    }

    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skill_file = path.join("SKILL.md");
                if skill_file.exists() {
                    if let Some(name) = path.file_name() {
                        let name = normalize_skill_name(&name.to_string_lossy());
                        skills.push(name);
                    }
                }
            }
        }
    }

    skills.sort();
    skills
}

fn normalize_skill_name(dir_name: &str) -> String {
    dir_name.to_lowercase().replace('_', "-")
}

/// List all external skills as (prefixed_name, source_name) tuples.
pub fn list_all_skills(sources: &[SkillSource]) -> Vec<(String, String)> {
    let mut skills = Vec::new();
    for source in sources {
        for skill_name in &source.skills {
            let prefixed = format!("{}:{}", source.prefix, skill_name);
            skills.push((prefixed, source.name.clone()));
        }
    }
    skills.sort();
    skills
}

// =============================================================================
// Step discovery (user, global, builtin)
// =============================================================================

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

/// List repo-local steps (.lf/steps/, .claude/commands/).
pub fn list_user_steps(repo: &Path) -> Vec<String> {
    let mut steps = HashSet::new();

    for dir in [repo.join(".lf/steps"), repo.join(".claude/commands")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(name) = path.file_stem() {
                        steps.insert(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    let mut steps: Vec<_> = steps.into_iter().collect();
    steps.sort();
    steps
}

/// List global steps (~/.lf/steps/, ~/.claude/commands/).
pub fn list_global_steps() -> Vec<String> {
    let mut steps = HashSet::new();

    if let Some(home) = dirs::home_dir() {
        for dir in [home.join(".lf/steps"), home.join(".claude/commands")] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map(|e| e == "md").unwrap_or(false) {
                        if let Some(name) = path.file_stem() {
                            steps.insert(name.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
    }

    let mut steps: Vec<_> = steps.into_iter().collect();
    steps.sort();
    steps
}

/// Structured result from list_all_steps.
pub type StepListResult = (Vec<String>, Vec<String>, Vec<String>, Vec<(String, String)>);

/// Return (user_steps, global_steps, builtin_only_steps, external_skills).
///
/// User steps include any that override builtins or globals.
/// Global steps are from ~/.claude/commands/ not overridden by repo-local.
/// Builtin-only steps are builtins not overridden by user or global steps.
/// External skills are (prefixed_name, source_name) tuples from skill sources.
pub fn list_all_steps(repo: Option<&Path>) -> StepListResult {
    let builtins = builtin_steps();
    let user: HashSet<String> = repo
        .map(|r| list_user_steps(r).into_iter().collect())
        .unwrap_or_default();
    let global: HashSet<String> = list_global_steps().into_iter().collect();

    let sources = discover_skill_sources(repo);
    let external_skills = list_all_skills(&sources);

    // Collect skill names that are handled by external sources (to exclude from global)
    let mut external_skill_names = HashSet::new();
    for source in &sources {
        // Single-file skills like rams are named after the file
        if source.skills.len() == 1 && source.skills[0] == source.prefix {
            external_skill_names.insert(source.skills[0].clone());
        }
    }

    // Global steps not overridden by repo-local or handled by external sources
    let global_only: Vec<String> = global
        .difference(&user)
        .filter(|s| !external_skill_names.contains(*s))
        .cloned()
        .collect();

    // Builtins not overridden by user or global
    let builtin_only: Vec<String> = builtins
        .difference(&user)
        .filter(|s| !global.contains(*s))
        .cloned()
        .collect();

    let mut user_sorted: Vec<_> = user.into_iter().collect();
    user_sorted.sort();

    let mut global_sorted = global_only;
    global_sorted.sort();

    let mut builtin_sorted = builtin_only;
    builtin_sorted.sort();

    (user_sorted, global_sorted, builtin_sorted, external_skills)
}


// =============================================================================
// Flow discovery and step chain extraction
// =============================================================================

#[derive(Debug)]
pub struct FlowInfo {
    pub name: String,
    pub step_names: Vec<String>,
}

/// List flows with their step names for display.
pub fn list_flows_with_steps(repo: &Path) -> Vec<FlowInfo> {
    let mut flows = Vec::new();
    let flows_dir = repo.join(".lf/flows");

    if let Ok(entries) = std::fs::read_dir(flows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_stem() {
                let ext = path.extension().map(|e| e.to_string_lossy().to_string());
                if matches!(ext.as_deref(), Some("yaml") | Some("yml") | Some("json")) {
                    let name = name.to_string_lossy().to_string();
                    let step_names = extract_flow_step_names(&path);
                    flows.push(FlowInfo { name, step_names });
                }
            }
        }
    }

    flows.sort_by(|a, b| a.name.cmp(&b.name));
    flows
}

/// Extract step names from a flow file for display in the chain.
fn extract_flow_step_names(path: &Path) -> Vec<String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let value: serde_yaml::Value = match serde_yaml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    extract_step_names_from_value(&value)
}

fn extract_step_names_from_value(value: &serde_yaml::Value) -> Vec<String> {
    let mut names = Vec::new();

    match value {
        serde_yaml::Value::String(s) => {
            names.push(s.clone());
        }
        serde_yaml::Value::Sequence(seq) => {
            for item in seq {
                names.extend(extract_step_names_from_value(item));
            }
        }
        serde_yaml::Value::Mapping(map) => {
            // Check for "steps" key first (common flow structure)
            if let Some(steps) = map.get(serde_yaml::Value::String("steps".to_string())) {
                return extract_step_names_from_value(steps);
            }
            // Check for "step" key (step definition)
            if let Some(step) = map.get(serde_yaml::Value::String("step".to_string())) {
                if let serde_yaml::Value::String(name) = step {
                    names.push(name.clone());
                } else if let serde_yaml::Value::Mapping(step_map) = step {
                    if let Some(serde_yaml::Value::String(name)) =
                        step_map.get(serde_yaml::Value::String("name".to_string()))
                    {
                        names.push(name.clone());
                    }
                }
            }
            // Check for fork/choose/loop structures
            if let Some(fork) = map.get(serde_yaml::Value::String("fork".to_string())) {
                names.push("[fork]".to_string());
                if let serde_yaml::Value::Mapping(fork_map) = fork {
                    if let Some(branches) =
                        fork_map.get(serde_yaml::Value::String("branches".to_string()))
                    {
                        let branch_names = extract_step_names_from_value(branches);
                        if !branch_names.is_empty() {
                            // Just show first step of fork for simplicity
                            names.push(format!("{}…", branch_names[0]));
                        }
                    }
                }
            }
            if map.contains_key(serde_yaml::Value::String("choose".to_string())) {
                names.push("[choose]".to_string());
            }
            if map.contains_key(serde_yaml::Value::String("loop_until_empty".to_string())) {
                names.push("[loop]".to_string());
            }
        }
        _ => {}
    }

    names
}

