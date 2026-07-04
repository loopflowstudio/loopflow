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

pub use crate::engine::builtins::BUILTIN_STEP_CATEGORIES;

/// All builtin step names (from BUILTIN_STEP_CATEGORIES).
pub fn builtin_steps() -> HashSet<String> {
    BUILTIN_STEP_CATEGORIES
        .iter()
        .flat_map(|(_, steps)| steps.iter().map(|s| (*s).to_string()))
        .collect()
}

/// One-line description for a builtin step, derived from its file's frontmatter
/// or leading prose.
pub fn builtin_step_description(name: &str) -> String {
    crate::engine::builtins::builtin_step_description(name)
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
    /// Single file skill (e.g., rams)
    SingleFile,
    /// Agent Skills cache sourced by `npx skills`
    Npx,
}

/// Discover external skill sources (npx cache and rams).
pub fn discover_skill_sources(repo: Option<&Path>) -> Vec<SkillSource> {
    let mut sources = Vec::new();

    // npx skill cache in repo-local .agents/skills (fetched live on demand)
    if let Some(repo_root) = repo {
        let cache_dir = repo_root.join(".agents/skills");
        sources.push(SkillSource {
            name: "npx skills".to_string(),
            prefix: "npx".to_string(),
            path: Some(cache_dir.clone()),
            skills: discover_npx_cache_skills(&cache_dir),
            kind: SkillSourceKind::Npx,
        });
    }

    // rams single-file skill at ~/.claude/commands/rams.md
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
            let prefixed = format!("{}/{}", source.prefix, skill_name);
            skills.push((prefixed, source.name.clone()));
        }
    }
    skills.sort();
    skills
}

// =============================================================================
// Step discovery (user, global, builtin, external skills)
// =============================================================================

/// Discover a step by name. Tries the engine first (user paths → core builtins
/// → namespaced builtins → bare-name fallback), then falls back to `npx/<name>`
/// live fetch.
pub fn discover_step(repo: &Path, name: &str) -> Result<Step> {
    match crate::engine::load_step(name, repo) {
        Ok(step) => Ok(step),
        Err(LoadError::StepNotFound(_)) => {
            if let Some(step) = find_external_skill(name, Some(repo)) {
                Ok(step)
            } else {
                Err(LoadError::StepNotFound(name.to_string()).into())
            }
        }
        Err(err) => Err(err.into()),
    }
}

/// Resolve an external skill reference like `npx/vercel-labs/deep-research` or
/// `rams/rams` to a Step.
fn find_external_skill(name: &str, repo: Option<&Path>) -> Option<Step> {
    let (prefix, skill_name) = name.split_once('/')?;
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
    let Some((frontmatter, _body)) = crate::engine::flow::split_frontmatter(&content) else {
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

/// Find the prompt file for a skill within a source.
fn find_skill_prompt_path(source: &SkillSource, skill_name: &str) -> Option<PathBuf> {
    let source_path = source.path.as_ref()?;

    match source.kind {
        SkillSourceKind::SingleFile => {
            let candidate = source_path.join(format!("{skill_name}.md"));
            candidate.is_file().then_some(candidate)
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
/// Uses the same recursive extractor as user-defined flows, so builtin flows
/// show their interior (loop bodies, xor branches) in `lf -l` too.
fn format_flow_description(yaml_content: &str) -> String {
    let value: serde_yaml_ng::Value = match serde_yaml_ng::from_str(yaml_content) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };

    extract_step_names_from_value(&value).join(" → ")
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

    // 1. Repo-local flows (.lf/flows/*.yaml)
    let flows_dir = repo.join(".lf/flows");
    collect_flows_from_dir(&flows_dir, None, &mut flows);

    // 2. Namespaced flows (.lf/flows/<prefix>/*.yaml → "prefix-name")
    if let Ok(entries) = std::fs::read_dir(&flows_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(prefix) = path.file_name().and_then(|n| n.to_str()) {
                    collect_flows_from_dir(&path, Some(prefix), &mut flows);
                }
            }
        }
    }

    flows.sort_by(|a, b| a.name.cmp(&b.name));
    flows
}

fn collect_flows_from_dir(dir: &Path, prefix: Option<&str>, flows: &mut Vec<FlowInfo>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(stem) = path.file_stem() {
                let ext = path.extension().map(|e| e.to_string_lossy().to_string());
                if matches!(ext.as_deref(), Some("yaml") | Some("yml") | Some("json")) {
                    let stem = stem.to_string_lossy().to_string();
                    let name = match prefix {
                        Some(p) => format!("{p}-{stem}"),
                        None => stem,
                    };
                    let step_names = extract_flow_step_names(&path);
                    flows.push(FlowInfo { name, step_names });
                }
            }
        }
    }
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
            let initial_len = names.len();
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
            if let Some(serde_yaml_ng::Value::String(name)) =
                map.get(serde_yaml_ng::Value::String("flow".to_string()))
            {
                names.push(name.clone());
            }
            if let Some(serde_yaml_ng::Value::String(command)) =
                map.get(serde_yaml_ng::Value::String("op".to_string()))
            {
                names.push(format!("op: {command}"));
            }
            if let Some(and) = map.get(serde_yaml_ng::Value::String("and".to_string())) {
                names.push("[and]".to_string());
                names.extend(extract_and_preview(and));
            }
            if let Some(xor) = map.get(serde_yaml_ng::Value::String("xor".to_string())) {
                names.push("[xor]".to_string());
                names.extend(extract_branch_preview(xor));
            }
            if let Some(or) = map.get(serde_yaml_ng::Value::String("or".to_string())) {
                names.push("[or]".to_string());
                names.extend(extract_branch_preview(or));
            }
            if let Some(loop_value) = map.get(serde_yaml_ng::Value::String("loop".to_string())) {
                names.push("loop".to_string());
                names.extend(extract_branch_preview(loop_value));
            }
            if names.len() == initial_len {
                for (key, value) in map {
                    // Prose metadata, not steps.
                    if key.as_str() == Some("description") {
                        continue;
                    }
                    names.extend(extract_step_names_from_value(value));
                }
            }
        }
        _ => {}
    }

    names
}

fn extract_and_preview(value: &serde_yaml_ng::Value) -> Vec<String> {
    let mut names = extract_branch_preview(value);
    let serde_yaml_ng::Value::Mapping(map) = value else {
        return names;
    };

    if let Some(serde_yaml_ng::Value::String(step)) =
        map.get(serde_yaml_ng::Value::String("synthesize".to_string()))
    {
        names.push(step.clone());
    }

    names
}

fn extract_branch_preview(value: &serde_yaml_ng::Value) -> Vec<String> {
    let serde_yaml_ng::Value::Mapping(map) = value else {
        return Vec::new();
    };

    for key in ["branches", "steps", "paths", "exit"] {
        if let Some(nested) = map.get(serde_yaml_ng::Value::String(key.to_string())) {
            let extracted = extract_step_names_from_value(nested);
            if !extracted.is_empty() {
                return extracted;
            }
        }
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn discover_step_loads_user_namespaced_override() {
        let tmp = TempDir::new().expect("tempdir");
        let steps_dir = tmp.path().join(".lf/steps/gstack");
        fs::create_dir_all(&steps_dir).expect("create namespaced steps dir");
        fs::write(
            steps_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# user override\n",
        )
        .expect("write step");

        let step = discover_step(tmp.path(), "gstack/office-hours").expect("discover step");

        assert_eq!(step.interactive, Some(false));
        assert!(step
            .content
            .as_deref()
            .expect("content")
            .contains("# user override"));
    }

    #[test]
    fn discover_step_rejects_legacy_colon_form() {
        let tmp = TempDir::new().expect("tempdir");
        let err = discover_step(tmp.path(), "gstack:office-hours").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

}
