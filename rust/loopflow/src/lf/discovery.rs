use crate::engine::{Flow, Step};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

// =============================================================================
// Auto-dispatch: step or flow
// =============================================================================

#[derive(Debug)]
pub enum Target {
    Step(Step),
    Flow(Flow),
}

/// Discover a step or flow by name. Tries step lookup first, falls back to flow.
pub fn discover_target(repo: &Path, name: &str) -> Result<Target> {
    match discover_step(repo, name) {
        Ok(step) => Ok(Target::Step(step)),
        Err(_) => match crate::engine::load_flow(name, repo) {
            Ok(flow) => Ok(Target::Flow(flow)),
            Err(_) => {
                // Return the original step-not-found error for better messaging
                Err(anyhow::anyhow!(
                    "step or flow not found: {name}. Run `lf --list` to see available steps."
                ))
            }
        },
    }
}

// =============================================================================
// Built-in step metadata for formatted listing
// =============================================================================

pub const BUILTIN_CATEGORIES: &[(&str, &[&str])] = &[
    ("Setup", &["init"]),
    (
        "Planning & Design",
        &[
            "design",
            "explore",
            "refine",
            "kickoff",
            "wave-plan",
            "5whys",
        ],
    ),
    (
        "Implementation",
        &["implement", "iterate", "expand", "reduce", "compress"],
    ),
    ("Quality", &["review", "polish", "lint", "debug", "gate"]),
    ("Scan", &["scan/cves", "scan/deps", "scan/upstream"]),
    ("Git", &["commit", "rebase"]),
    (
        "Ops",
        &[
            "ingest",
            "add-to-wave",
            "update-wave",
            "consolidate",
            "synthesize",
            "validate",
        ],
    ),
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
        ("compress", "Simplify touched code"),
        ("gate", "Ship-ready check with reviewer docs"),
        ("5whys", "Root cause analysis on a bug fix"),
        ("kickoff", "Elaborate design with alternatives"),
        ("wave-plan", "Synthesize analysis into wave plan"),
        ("ingest", "Pick wave item, move to scratch/"),
        ("add-to-wave", "Promote from scratch/ to wave/"),
        ("update-wave", "Update wave status after work"),
        ("consolidate", "Reorganize scratch/ for wave"),
        ("synthesize", "Combine multiple perspectives"),
        ("validate", "Validate flows, steps, and directions"),
        ("scan/cves", "Check dependencies for CVEs"),
        ("scan/deps", "Check for major version bumps"),
        ("scan/upstream", "Check external APIs for changes"),
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
    crate::engine::load_step(name, repo)
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
    pub path: Option<PathBuf>,
    pub skills: Vec<String>,
    pub kind: SkillSourceKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSourceKind {
    /// Directory of skills (e.g., superpowers)
    Directory,
    /// Single file skill (e.g., rams)
    SingleFile,
}

/// Discover external skill sources (config, superpowers, rams).
pub fn discover_skill_sources(repo: Option<&Path>) -> Vec<SkillSource> {
    let mut sources = Vec::new();
    let mut seen_prefixes = HashSet::new();

    // 1. Config-defined sources (checked first)
    if let Some(repo_root) = repo {
        if let Ok(Some(config)) = crate::engine::load_config(Some(repo_root)) {
            for source_config in &config.skill_sources {
                let path = expand_tilde(&source_config.path);
                if !path.exists() {
                    continue;
                }
                let skills = discover_superpowers_skills(&path);
                if !skills.is_empty() {
                    sources.push(SkillSource {
                        name: source_config.name.clone(),
                        prefix: source_config.prefix.clone(),
                        path: Some(path),
                        skills,
                        kind: SkillSourceKind::Directory,
                    });
                    seen_prefixes.insert(source_config.prefix.clone());
                }
            }
        }
    }

    // 2. Auto-detect superpowers
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
                    path: Some(path),
                    skills,
                    kind: SkillSourceKind::Directory,
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
                    path: Some(rams_path.parent().unwrap_or(&home).to_path_buf()),
                    skills: vec!["rams".to_string()],
                    kind: SkillSourceKind::SingleFile,
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
    let mut name = dir_name.to_lowercase().replace('_', "-");

    // Known abbreviations
    if name == "test-driven-development" {
        return "tdd".to_string();
    }

    // Strip -ing suffix (brainstorming -> brainstorm)
    if let Some(stripped) = name.strip_suffix("ing") {
        name = stripped.to_string();
    }

    // Strip trailing -s (writing-plans -> writing-plan)
    if let Some(stripped) = name.strip_suffix('s') {
        name = stripped.to_string();
    }

    name
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

/// Discover a step by name, checking skills → repo → global → builtins.
pub fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    // Check if it's a skill reference (prefix:name)
    if name.contains(':') {
        if let Some(step) = find_skill(name, Some(repo)) {
            return Ok(step);
        }
    }

    let repo_paths = [
        repo.join(".lf/steps").join(format!("{name}.md")),
        repo.join(".claude/commands").join(format!("{name}.md")),
    ];
    for path in repo_paths {
        if path.exists() {
            return crate::engine::load_step(name, repo).map_err(Into::into);
        }
    }

    if let Some(home) = dirs::home_dir() {
        let global_paths = [
            home.join(".lf/steps").join(format!("{name}.md")),
            home.join(".claude/commands").join(format!("{name}.md")),
        ];
        for path in global_paths {
            if path.exists() {
                return crate::engine::load_step(name, &home).map_err(Into::into);
            }
        }
    }

    crate::engine::load_step(name, repo).map_err(Into::into)
}

/// Resolve a skill reference like "sp:brainstorm" to a Step.
fn find_skill(name: &str, repo: Option<&Path>) -> Option<Step> {
    let (prefix, skill_name) = name.split_once(':')?;
    let sources = discover_skill_sources(repo);

    for source in &sources {
        if source.prefix != prefix {
            continue;
        }
        if !source.skills.contains(&skill_name.to_string()) {
            continue;
        }

        let prompt_path = find_skill_prompt_path(source, skill_name)?;
        let content = std::fs::read_to_string(&prompt_path).ok()?;

        return Some(Step {
            name: name.to_string(),
            content: Some(content),
            model: None,
            directions: Vec::new(),
            interactive: Some(true),
        });
    }

    None
}

/// Find the prompt file for a skill within a source.
fn find_skill_prompt_path(source: &SkillSource, skill_name: &str) -> Option<PathBuf> {
    let source_path = source.path.as_ref()?;

    match source.kind {
        SkillSourceKind::SingleFile => {
            let candidate = source_path.join(format!("{skill_name}.md"));
            candidate.is_file().then_some(candidate)
        }
        SkillSourceKind::Directory => {
            let skills_dir = source_path.join("skills");
            if !skills_dir.exists() {
                return None;
            }
            // Walk skill directories, matching normalized names
            let entries = std::fs::read_dir(&skills_dir).ok()?;
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let normalized = normalize_skill_name(&path.file_name()?.to_string_lossy());
                    if normalized == skill_name {
                        let skill_file = path.join("SKILL.md");
                        if skill_file.is_file() {
                            return Some(skill_file);
                        }
                    }
                }
            }
            None
        }
    }
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
// Direction discovery
// =============================================================================

/// List builtin + repo directions for display.
pub fn list_directions(repo: Option<&Path>) -> Vec<String> {
    let mut directions: HashSet<String> = crate::engine::builtins::builtin_direction_names()
        .into_iter()
        .map(|name| name.to_string())
        .collect();

    if let Some(repo) = repo {
        let dir = repo.join(".lf/directions");
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(name) = path.file_stem() {
                        directions.insert(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    let mut list: Vec<_> = directions.into_iter().collect();
    list.sort();
    list
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

/// Expand ~ to home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
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
