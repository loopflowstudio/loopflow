use crate::engine::{Flow, LoadError, Step};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_yaml_ng::Value;
use tracing::debug;

const SKILL_FILE_NAME: &str = "SKILL.md";

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
    let step_error = match discover_step(repo, name) {
        Ok(step) => return Ok(Target::Step(step)),
        Err(err) => err,
    };

    if !matches!(
        step_error.downcast_ref::<LoadError>(),
        Some(LoadError::StepNotFound(_))
    ) {
        return Err(step_error);
    }

    match crate::engine::load_flow(name, repo) {
        Ok(flow) => Ok(Target::Flow(flow)),
        Err(LoadError::FlowNotFound(_)) => Err(anyhow::anyhow!(
            "step or flow not found: {name}. Run `lf --list` to see available steps."
        )),
        Err(err) => Err(err.into()),
    }
}

// =============================================================================
// Built-in step metadata for formatted listing
// =============================================================================

pub const BUILTIN_CATEGORIES: &[(&str, &[&str])] = &[
    ("Setup", &["init"]),
    (
        "Planning & Design",
        &["design", "explore", "refine", "kickoff", "5whys"],
    ),
    (
        "Implementation",
        &["implement", "iterate", "expand", "reduce", "compress"],
    ),
    (
        "Quality",
        &[
            "demo", "code-review", "research", "polish", "lint", "debug", "ci-fix", "gate",
        ],
    ),
    ("Scan", &["scan/scan-report", "scan/scan-plan"]),
    ("Git", &["commit", "rebase", "pr", "land"]),
    (
        "Ops",
        &[
            "ingest",
            "split-wave",
            "update-wave",
            "synthesize",
            "validate",
            "release",
        ],
    ),
];

pub fn builtin_descriptions() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("init", "Set up loopflow in this repo"),
        ("design", "Plan what to build"),
        ("split-wave", "Split a large wave into child waves"),
        ("explore", "Investigate current diff"),
        ("implement", "Build from design doc"),
        ("iterate", "Improve code on branch"),
        ("expand", "Explore ambitious extensions"),
        ("reduce", "Simplify while preserving behavior"),
        ("demo", "Experience-first walkthrough of changes"),
        ("code-review", "Walk through structural and architectural decisions"),
        ("research", "Map the territory, understand what exists"),
        ("polish", "Fix issues, run tests"),
        ("lint", "Run linter, fix issues"),
        ("debug", "Fix errors from clipboard"),
        ("ci-fix", "Fix latest CI failures for current PR"),
        ("commit", "Commit with generated message"),
        ("rebase", "Rebase onto main"),
        ("pr", "Open or update PR with generated description"),
        ("land", "Land PR, rotate worktree"),
        ("refine", "Iteratively refine text"),
        ("compress", "Simplify touched code"),
        ("gate", "Ship-ready check with reviewer docs"),
        ("5whys", "Root cause analysis on a bug fix"),
        ("kickoff", "Elaborate design with alternatives"),
        ("ingest", "Pick wave item, move to scratch/"),
        ("update-wave", "Create, update, or delete wave state"),
        ("synthesize", "Combine multiple perspectives"),
        ("validate", "Validate flows, steps, and directions"),
        ("release", "Run one-shot release workflow"),
        ("scan/scan-report", "Scan deps and APIs for issues"),
        ("scan/scan-plan", "Turn scan report into action plan"),
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
    /// Agent Skills cache sourced by `npx skills`
    Npx,
}

/// Discover external skill sources (config, npx cache, superpowers, rams).
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

    // 2. npx skill cache in repo-local .agents/skills
    if !seen_prefixes.contains("npx") {
        if let Some(repo_root) = repo {
            let cache_dir = repo_root.join(".agents/skills");
            sources.push(SkillSource {
                name: "npx skills".to_string(),
                prefix: "npx".to_string(),
                path: Some(cache_dir.clone()),
                skills: discover_npx_cache_skills(&cache_dir),
                kind: SkillSourceKind::Npx,
            });
            seen_prefixes.insert("npx".to_string());
        }
    }

    // 3. Auto-detect superpowers
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

    // 4. Auto-detect rams
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

fn discover_npx_cache_skills(cache_dir: &Path) -> Vec<String> {
    if !cache_dir.exists() {
        return Vec::new();
    }

    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_file = path.join(SKILL_FILE_NAME);
            if !skill_file.is_file() || has_loopflow_marker(&skill_file) {
                continue;
            }

            if let Some(name) = path.file_name() {
                skills.push(name.to_string_lossy().to_string());
            }
        }
    }

    skills.sort();
    skills
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
                let skill_file = path.join(SKILL_FILE_NAME);
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

/// Discover a step by name, checking skills first, then repo → global → builtins
/// via `load_step`.
pub fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    if name.contains(':') {
        if let Some(step) = find_skill(name, Some(repo)) {
            return Ok(step);
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

        if source.kind == SkillSourceKind::Npx {
            return find_npx_skill(name, skill_name, repo, source.path.as_deref());
        }

        if !source
            .skills
            .iter()
            .any(|candidate| candidate == skill_name)
        {
            continue;
        }

        let prompt_path = find_skill_prompt_path(source, skill_name)?;
        return load_skill_from_path(name, &prompt_path);
    }

    None
}

fn find_npx_skill(
    qualified_name: &str,
    skill_name: &str,
    repo: Option<&Path>,
    cache_path: Option<&Path>,
) -> Option<Step> {
    let repo_root = repo?;
    let cache_dir = cache_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root.join(".agents/skills"));

    if let Some(path) = find_cached_npx_skill(&cache_dir, skill_name) {
        return load_skill_from_path(qualified_name, &path);
    }

    if run_npx_add(skill_name, repo_root) {
        if let Some(path) = find_cached_npx_skill(&cache_dir, skill_name) {
            return load_skill_from_path(qualified_name, &path);
        }
    }

    let found = run_npx_find(skill_name, repo_root)?;
    if !run_npx_add(&found, repo_root) {
        return None;
    }

    // After `npx skills add owner/repo@skill`, the skill lands at .agents/skills/<skill>/
    let skill_from_qualified = found.split_once('@').map(|(_, skill)| skill);

    find_cached_npx_skill(&cache_dir, skill_name)
        .or_else(|| skill_from_qualified.and_then(|s| find_cached_npx_skill(&cache_dir, s)))
        .or_else(|| find_cached_npx_skill(&cache_dir, &found))
        .and_then(|path| load_skill_from_path(qualified_name, &path))
}

fn find_cached_npx_skill(cache_dir: &Path, skill_name: &str) -> Option<PathBuf> {
    for candidate in [
        Some(cache_dir.join(skill_name).join(SKILL_FILE_NAME)),
        Some(
            cache_dir
                .join(skill_name.replace('/', "-"))
                .join(SKILL_FILE_NAME),
        ),
        skill_name
            .split('/')
            .next_back()
            .map(|last_component| cache_dir.join(last_component).join(SKILL_FILE_NAME)),
    ]
    .into_iter()
    .flatten()
    {
        if candidate.is_file() && !has_loopflow_marker(&candidate) {
            return Some(candidate);
        }
    }

    let normalized = normalize_skill_name(skill_name);
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let skill_file = path.join(SKILL_FILE_NAME);
        if !skill_file.is_file() || has_loopflow_marker(&skill_file) {
            continue;
        }

        let Some(dir_name) = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
        else {
            continue;
        };
        if dir_name == skill_name || normalize_skill_name(&dir_name) == normalized {
            return Some(skill_file);
        }
    }

    None
}

fn load_skill_from_path(name: &str, prompt_path: &Path) -> Option<Step> {
    let content = std::fs::read_to_string(prompt_path).ok()?;

    Some(Step {
        name: name.to_string(),
        content: Some(content),
        agent: None,
        default_agent: None,
        directions: Vec::new(),
        action_style: None,
        interactive: Some(true),
        fast_path: None,
    })
}

fn run_npx_add(skill_name: &str, repo_root: &Path) -> bool {
    // First `--yes` is for npx itself; trailing `--yes` is for the skills CLI.
    // Without the latter, `skills add` can open an interactive selector and
    // return without writing `.agents/skills/<name>/SKILL.md`.
    match run_npx(repo_root, &["--yes", "skills", "add", skill_name, "--yes"]) {
        Ok(output) => {
            if output.status.success() {
                return true;
            }
            debug!(
                skill = skill_name,
                code = ?output.status.code(),
                stderr = %String::from_utf8_lossy(&output.stderr),
                "npx skills add failed"
            );
            false
        }
        Err(error) => {
            debug!(skill = skill_name, error = %error, "failed to run npx skills add");
            false
        }
    }
}

fn run_npx_find(skill_name: &str, repo_root: &Path) -> Option<String> {
    let output = match run_npx(repo_root, &["--yes", "skills", "find", skill_name]) {
        Ok(output) => output,
        Err(error) => {
            debug!(skill = skill_name, error = %error, "failed to run npx skills find");
            return None;
        }
    };

    if !output.status.success() {
        debug!(
            skill = skill_name,
            code = ?output.status.code(),
            stderr = %String::from_utf8_lossy(&output.stderr),
            "npx skills find failed"
        );
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let result = parse_npx_find_output(&stdout);
    debug!(
        skill = skill_name,
        stdout_len = stdout.len(),
        ?result,
        "npx skills find output"
    );
    result
}

fn run_npx(repo_root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new(npx_binary())
        .args(args)
        .current_dir(repo_root)
        .output()
}

fn parse_npx_find_output(stdout: &str) -> Option<String> {
    let stripped = strip_ansi(stdout);
    for token in stripped.split_whitespace() {
        // Skip placeholder templates like <owner/repo@skill>
        if token.starts_with('<') && token.ends_with('>') {
            continue;
        }
        let cleaned = token.trim_matches(|c: char| {
            c.is_ascii_punctuation() && c != '/' && c != '-' && c != '_' && c != '.' && c != '@'
        });
        if cleaned.is_empty() {
            continue;
        }
        // owner/repo@skill format from `npx skills find`
        if let Some(hint) = normalize_qualified_skill(cleaned) {
            return Some(hint);
        }
        if let Some(rest) = cleaned.strip_prefix("https://github.com/") {
            return normalize_repo_hint(rest);
        }
        if let Some(rest) = cleaned.strip_prefix("github.com/") {
            return normalize_repo_hint(rest);
        }
        if is_repo_hint(cleaned) {
            return normalize_repo_hint(cleaned);
        }
    }
    None
}

/// Recognize `owner/repo@skill` format and return the full string.
fn normalize_qualified_skill(value: &str) -> Option<String> {
    let (repo_part, skill_part) = value.split_once('@')?;
    if is_repo_hint(repo_part) && !skill_part.is_empty() && skill_part.chars().all(is_skill_char) {
        Some(value.to_string())
    } else {
        None
    }
}

fn normalize_repo_hint(value: &str) -> Option<String> {
    let trimmed = value.trim_end_matches(".git").trim_matches('/');
    is_repo_hint(trimmed).then(|| trimmed.to_string())
}

/// Strip ANSI escape sequences (e.g. `\x1b[38;5;145m`) from a string.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn is_repo_hint(token: &str) -> bool {
    let mut parts = token.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(repo) = parts.next() else {
        return false;
    };
    if parts.next().is_some() {
        return false;
    }
    !owner.is_empty()
        && !repo.is_empty()
        && owner.chars().all(is_skill_char)
        && repo.chars().all(is_skill_char)
}

fn is_skill_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')
}

fn npx_binary() -> String {
    std::env::var("LF_NPX_BIN").unwrap_or_else(|_| "npx".to_string())
}

fn has_loopflow_marker(skill_path: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(skill_path) else {
        return false;
    };
    let Some((frontmatter, _body)) = split_frontmatter(&content) else {
        return false;
    };
    let Ok(value) = serde_yaml_ng::from_str::<Value>(&frontmatter) else {
        return false;
    };
    let Some(map) = value.as_mapping() else {
        return false;
    };
    map.get(Value::String("loopflow".to_string()))
        .and_then(Value::as_bool)
        == Some(true)
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
                        let skill_file = path.join(SKILL_FILE_NAME);
                        if skill_file.is_file() {
                            return Some(skill_file);
                        }
                    }
                }
            }
            None
        }
        SkillSourceKind::Npx => None,
    }
}

/// List repo-local steps (.lf/steps/, .claude/commands/).
pub fn list_user_steps(repo: &Path) -> Vec<String> {
    list_md_stems(&[repo.join(".lf/steps"), repo.join(".claude/commands")])
}

/// List global steps (~/.lf/steps/, ~/.claude/commands/).
pub fn list_global_steps() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    list_md_stems(&[home.join(".lf/steps"), home.join(".claude/commands")])
}

/// Collect sorted, deduplicated `.md` file stems from the given directories.
fn list_md_stems(dirs: &[PathBuf]) -> Vec<String> {
    let mut names = HashSet::new();
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(name) = path.file_stem() {
                        names.insert(name.to_string_lossy().to_string());
                    }
                }
            }
        }
    }
    let mut sorted: Vec<_> = names.into_iter().collect();
    sorted.sort();
    sorted
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
    directions.extend(
        crate::engine::builtins::builtin_direction_group_names()
            .into_iter()
            .map(|name| name.to_string()),
    );

    if let Some(repo) = repo {
        directions.extend(list_user_direction_names(repo));
    }

    let mut list: Vec<_> = directions.into_iter().collect();
    list.sort();
    list
}

fn list_user_direction_names(repo: &Path) -> HashSet<String> {
    let directions_dir = repo.join(".lf/directions");
    let mut names: HashSet<String> = list_md_stems(std::slice::from_ref(&directions_dir))
        .into_iter()
        .collect();
    let Ok(entries) = std::fs::read_dir(&directions_dir) else {
        return names;
    };

    let mut group_dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(group_name) = path.file_name() {
                names.insert(group_name.to_string_lossy().to_string());
            }
            group_dirs.push(path);
        }
    }

    names.extend(list_md_stems(&group_dirs));
    names
}

// =============================================================================
// Built-in flow metadata for formatted listing
// =============================================================================

// Generated by build.rs from the flows/ directory structure.
pub use crate::engine::builtins::BUILTIN_FLOW_CATEGORIES;

/// Derive flow descriptions from YAML content at runtime.
pub fn builtin_flow_descriptions() -> HashMap<String, String> {
    crate::engine::builtins::builtin_flow_entries()
        .map(|(name, content)| {
            let desc = format_flow_description(content);
            (name.to_string(), desc)
        })
        .collect()
}

/// Format a flow's YAML content as a human-readable step chain (e.g., "implement → compress → gate").
fn format_flow_description(yaml_content: &str) -> String {
    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(yaml_content) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    let names = extract_flow_summary(&value);
    names.join(" → ")
}

/// Extract step names from a flow value, formatting forks as "fork(step×N)".
fn extract_flow_summary(value: &serde_yaml_ng::Value) -> Vec<String> {
    let serde_yaml_ng::Value::Sequence(seq) = value else {
        return Vec::new();
    };

    let mut names = Vec::new();
    for item in seq {
        match item {
            serde_yaml_ng::Value::String(s) => names.push(s.clone()),
            serde_yaml_ng::Value::Mapping(map) => {
                // Fork: { fork: { step: "reduce", drafts: [...] } }
                if let Some(serde_yaml_ng::Value::Mapping(fork_map)) =
                    map.get(serde_yaml_ng::Value::String("fork".into()))
                {
                    let step = fork_map
                        .get(serde_yaml_ng::Value::String("step".into()))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let count = fork_map
                        .get(serde_yaml_ng::Value::String("drafts".into()))
                        .and_then(|v| v.as_sequence())
                        .map(|s| s.len())
                        .unwrap_or(0);
                    names.push(format!("fork({step}×{count})"));
                }
            }
            _ => {}
        }
    }
    names
}

/// All builtin flow names (from BUILTIN_FLOW_CATEGORIES).
pub fn builtin_flows() -> HashSet<String> {
    BUILTIN_FLOW_CATEGORIES
        .iter()
        .flat_map(|(_, flows)| flows.iter().map(|f| (*f).to_string()))
        .collect()
}

// =============================================================================
// Flow discovery and step chain extraction
// =============================================================================

#[derive(Debug)]
pub struct FlowInfo {
    pub name: String,
    pub step_names: Vec<String>,
}

/// List user-defined flows with their step names for display.
pub fn list_user_flows(repo: &Path) -> Vec<FlowInfo> {
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

    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(&content) {
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

fn extract_step_names_from_value(value: &serde_yaml_ng::Value) -> Vec<String> {
    let mut names = Vec::new();

    match value {
        serde_yaml_ng::Value::String(s) => {
            names.push(s.clone());
        }
        serde_yaml_ng::Value::Sequence(seq) => {
            for item in seq {
                names.extend(extract_step_names_from_value(item));
            }
        }
        serde_yaml_ng::Value::Mapping(map) => {
            // Check for "steps" key first (common flow structure)
            if let Some(steps) = map.get(serde_yaml_ng::Value::String("steps".to_string())) {
                return extract_step_names_from_value(steps);
            }
            // Check for "step" key (step definition)
            if let Some(step) = map.get(serde_yaml_ng::Value::String("step".to_string())) {
                if let serde_yaml_ng::Value::String(name) = step {
                    names.push(name.clone());
                } else if let serde_yaml_ng::Value::Mapping(step_map) = step {
                    if let Some(serde_yaml_ng::Value::String(name)) =
                        step_map.get(serde_yaml_ng::Value::String("name".to_string()))
                    {
                        names.push(name.clone());
                    }
                }
            }
            // Check for fork structures
            if let Some(fork) = map.get(serde_yaml_ng::Value::String("fork".to_string())) {
                names.push("[fork]".to_string());
                if let serde_yaml_ng::Value::Mapping(fork_map) = fork {
                    if let Some(branches) =
                        fork_map.get(serde_yaml_ng::Value::String("branches".to_string()))
                    {
                        let branch_names = extract_step_names_from_value(branches);
                        if !branch_names.is_empty() {
                            // Just show first step of fork for simplicity
                            names.push(format!("{}…", branch_names[0]));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    names
}
