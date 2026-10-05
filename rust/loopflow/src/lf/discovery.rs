use crate::engine::{LoadError, Skill, Step, XorPath};
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_yaml_ng::Value;
use tracing::debug;

const SKILL_FILE_NAME: &str = "SKILL.md";

// =============================================================================
// Auto-dispatch: skill or flow
// =============================================================================

pub use crate::engine::target::{DefinitionKind, Target};

/// Execution may fetch external skills; read-only callers use the local resolver.
pub fn resolve_definition(repo: &Path, name: &str, kind: Option<DefinitionKind>) -> Result<Target> {
    match resolve_local_definition(repo, name, kind) {
        Ok(target) => Ok(target),
        Err(error)
            if matches!(
                error.downcast_ref::<LoadError>(),
                Some(LoadError::SkillNotFound(_) | LoadError::TargetNotFound(_))
            ) =>
        {
            let Some(path) = fetch_npx_skill(repo, name) else {
                return Err(error);
            };
            crate::engine::flow::load_skill_from_path(name, &path)
                .map(Target::Skill)
                .map_err(Into::into)
        }
        Err(error) => Err(error),
    }
}

pub fn resolve_local_definition(
    repo: &Path,
    name: &str,
    kind: Option<DefinitionKind>,
) -> Result<Target> {
    match crate::engine::target::resolve_definition(repo, name, kind) {
        Ok(target) => Ok(target),
        Err(error @ (LoadError::SkillNotFound(_) | LoadError::TargetNotFound(_))) => {
            match cached_external_skill_path(repo, name) {
                Some(path) => crate::engine::flow::load_skill_from_path(name, &path)
                    .map(Target::Skill)
                    .map_err(Into::into),
                None => Err(error.into()),
            }
        }
        Err(error) => Err(error.into()),
    }
}

/// architecture-shim: rams-alias
pub fn cached_external_skill_path(repo: &Path, name: &str) -> Option<PathBuf> {
    let (prefix, skill_name) = name.split_once('/')?;
    if prefix == "npx" {
        return find_cached_npx_skill(&repo.join(".agents/skills"), skill_name);
    }
    discover_skill_sources(Some(repo))
        .iter()
        .find(|source| source.prefix == prefix)
        .and_then(|source| find_skill_prompt_path(source, skill_name))
}

pub(crate) fn definition_source(repo: &Path, name: &str, kind: DefinitionKind) -> String {
    let path = match kind {
        DefinitionKind::Flow => crate::engine::flow::find_flow_source_path(name, repo),
        DefinitionKind::Skill => crate::engine::find_skill_source_path(name, repo)
            .or_else(|| cached_external_skill_path(repo, name)),
    };
    path.map(|path| {
        path.strip_prefix(repo)
            .unwrap_or(&path)
            .display()
            .to_string()
    })
    .unwrap_or_else(|| "builtin".to_string())
}

// =============================================================================
// Built-in skill metadata for formatted listing
// =============================================================================

pub use crate::engine::builtins::BUILTIN_SKILL_CATEGORIES;

/// All builtin skill names (from BUILTIN_SKILL_CATEGORIES).
pub fn builtin_skills() -> HashSet<String> {
    BUILTIN_SKILL_CATEGORIES
        .iter()
        .flat_map(|(_, skills)| skills.iter().map(|s| (*s).to_string()))
        .collect()
}

/// One-line description for a builtin skill, derived from its file's frontmatter
/// or leading prose.
pub fn builtin_skill_description(name: &str) -> String {
    crate::engine::builtins::builtin_skill_description(name)
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
pub fn list_external_skills(sources: &[SkillSource]) -> Vec<(String, String)> {
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
// Skill discovery (user, global, builtin, external skills)
// =============================================================================

/// Resolve a skill, fetching an external definition only when it is absent locally.
pub fn discover_skill(repo: &Path, name: &str) -> Result<Skill> {
    match resolve_definition(repo, name, Some(DefinitionKind::Skill))? {
        Target::Skill(skill) => Ok(skill),
        Target::Command(_) | Target::Flow(_) | Target::Xor(_) => {
            unreachable!("skill-only resolution cannot select another kind")
        }
    }
}

fn fetch_npx_skill(repo: &Path, name: &str) -> Option<PathBuf> {
    let skill_name = name.strip_prefix("npx/")?;
    let cache_dir = repo.join(".agents/skills");

    if run_npx_add(skill_name, repo) {
        if let Some(path) = find_cached_npx_skill(&cache_dir, skill_name) {
            return Some(path);
        }
    }

    let found = run_npx_find(skill_name, repo)?;
    if !run_npx_add(&found, repo) {
        return None;
    }

    // After `npx skills add owner/repo@skill`, the skill lands at .agents/skills/<skill>/
    let skill_from_qualified = found.split_once('@').map(|(_, skill)| skill);

    find_cached_npx_skill(&cache_dir, skill_name)
        .or_else(|| skill_from_qualified.and_then(|s| find_cached_npx_skill(&cache_dir, s)))
        .or_else(|| find_cached_npx_skill(&cache_dir, &found))
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

/// List repo-local skills (.lf/skills/, .claude/commands/).
pub fn list_user_skills(repo: &Path) -> Vec<String> {
    list_markdown_names(&[repo.join(".lf/skills"), repo.join(".claude/commands")])
}

/// List global skills (~/.lf/skills/, ~/.claude/commands/).
pub fn list_global_skills() -> Vec<String> {
    let Some(home) = dirs::home_dir() else {
        return Vec::new();
    };
    list_markdown_names(&[home.join(".lf/skills"), home.join(".claude/commands")])
}

fn list_markdown_names(dirs: &[PathBuf]) -> Vec<String> {
    let mut names = HashSet::new();
    for dir in dirs {
        collect_markdown_names(dir, dir, &mut names);
    }
    let mut sorted: Vec<_> = names.into_iter().collect();
    sorted.sort();
    sorted
}

fn collect_markdown_names(root: &Path, dir: &Path, names: &mut HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_names(root, &path, names);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "md") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let mut name = relative.to_path_buf();
        name.set_extension("");
        names.insert(
            name.components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        );
    }
}

/// Structured result from list_all_skills.
pub type SkillListResult = (Vec<String>, Vec<String>, Vec<String>, Vec<(String, String)>);

/// Return (user_skills, global_skills, builtin_only_skills, external_skills).
///
/// User skills include any that override builtins or globals.
/// Global skills are from ~/.claude/commands/ not overridden by repo-local.
/// Builtin-only skills are builtins not overridden by user or global skills.
/// External skills are (prefixed_name, source_name) tuples from skill sources.
pub fn list_all_skills(repo: Option<&Path>) -> SkillListResult {
    let builtins = builtin_skills();
    let user: HashSet<String> = repo
        .map(|r| list_user_skills(r).into_iter().collect())
        .unwrap_or_default();
    let global: HashSet<String> = list_global_skills().into_iter().collect();

    let sources = discover_skill_sources(repo);
    let external_skills = list_external_skills(&sources);

    // Collect skill names that are handled by external sources (to exclude from global)
    let mut external_skill_names = HashSet::new();
    for source in &sources {
        // Single-file skills like rams are named after the file
        if source.skills.len() == 1 && source.skills[0] == source.prefix {
            external_skill_names.insert(source.skills[0].clone());
        }
    }

    // Global skills not overridden by repo-local or handled by external sources
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
// Built-in flow metadata for formatted listing
// =============================================================================

// Generated by build.rs from the flows/ directory structure.
pub use crate::engine::builtins::BUILTIN_FLOW_CATEGORIES;

pub fn format_written_steps(steps: &[Step]) -> String {
    if steps.is_empty() {
        return "∅".to_string();
    }
    steps
        .iter()
        .map(format_written_step)
        .collect::<Vec<_>>()
        .join(" → ")
}

fn format_written_step(step: &Step) -> String {
    match &step.target {
        Target::Flow(flow) => flow.name.clone(),
        target => format_target(target),
    }
}

pub(crate) fn format_target(target: &Target) -> String {
    match target {
        Target::Skill(skill) => skill.name.clone(),
        Target::Command(command) => command.to_string(),
        Target::Flow(flow) => format_written_steps(&flow.items),
        Target::Xor(xor) => format_xor(xor.router.as_deref(), &xor.paths),
    }
}

fn format_xor(router: Option<&str>, paths: &HashMap<String, XorPath>) -> String {
    let label = router.map_or_else(|| "xor".to_string(), |name| format!("xor[{name}]"));
    let mut names: Vec<_> = paths.keys().collect();
    names.sort();
    let rendered = names
        .into_iter()
        .map(|name| format!("{name}: {}", format_written_steps(&paths[name].steps)))
        .collect::<Vec<_>>()
        .join(" | ");
    format!("{label}{{{rendered}}}")
}

#[cfg(test)]
mod tests {
    use super::{discover_skill, resolve_definition, Target};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn ordinary_skill_discovery_ignores_legacy_review_instructions() {
        let _lock = crate::journal::test_env_lock();
        let _environment = crate::test_ambient::EnvGuard::clear(&["LF_HUMAN_SESSION"]);
        let repo = TempDir::new().unwrap();
        fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
        fs::write(
            repo.path().join(".lf/skills/review.md"),
            "Current instructions",
        )
        .unwrap();
        let mut captured = crate::engine::Skill::named("review");
        captured.content = Some("Retired captured instructions".into());
        std::env::set_var(
            "LF_HUMAN_SESSION",
            serde_json::json!({
                "kind": "flow",
                "token": {
                    "task_id": crate::work::task::TaskId::new(),
                    "invocation_id": "legacy",
                    "flow": "legacy",
                    "node_id": "review",
                    "skill": captured,
                    "iteration": 0
                }
            })
            .to_string(),
        );

        let Target::Skill(skill) =
            resolve_definition(repo.path(), "review", Some(super::DefinitionKind::Skill)).unwrap()
        else {
            panic!("explicit skill must resolve to a skill");
        };
        assert_eq!(skill.content.as_deref(), Some("Current instructions"));
    }

    #[test]
    fn operate_resolves_to_repo_and_wave_operate_stays_explicit() {
        let tmp = TempDir::new().unwrap();
        for (name, expected) in [
            ("operate", "repo/operate"),
            ("repo/operate", "repo/operate"),
            ("wave/operate", "wave/operate"),
        ] {
            let Target::Skill(skill) = resolve_definition(tmp.path(), name, None).unwrap() else {
                panic!("expected a skill for {name}");
            };
            assert_eq!(skill.name, expected);
        }

        let skills = tmp.path().join(".lf/skills/repo");
        fs::create_dir_all(&skills).unwrap();
        fs::write(skills.join("operate.md"), "Repository operation override").unwrap();
        for name in ["operate", "repo/operate"] {
            let Target::Skill(skill) = resolve_definition(tmp.path(), name, None).unwrap() else {
                panic!("expected a skill for {name}");
            };
            assert_eq!(skill.name, "repo/operate");
            assert_eq!(
                skill.content.as_deref(),
                Some("Repository operation override")
            );
        }
    }

    #[test]
    fn discover_skill_loads_user_namespaced_override() {
        let tmp = TempDir::new().expect("tempdir");
        let skills_dir = tmp.path().join(".lf/skills/gstack");
        fs::create_dir_all(&skills_dir).expect("create namespaced skills dir");
        fs::write(
            skills_dir.join("office-hours.md"),
            "---\ninteractive: false\n---\n# user override\n",
        )
        .expect("write skill");

        let skill = discover_skill(tmp.path(), "gstack/office-hours").expect("discover skill");

        assert!(skill
            .content
            .as_deref()
            .expect("content")
            .contains("# user override"));
    }

    #[test]
    fn discover_skill_rejects_legacy_colon_form() {
        let tmp = TempDir::new().expect("tempdir");
        let err = discover_skill(tmp.path(), "gstack:office-hours").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn untyped_names_resolve_available_definitions() {
        let tmp = TempDir::new().expect("tempdir");
        for name in ["design", "launch-plan", "debug", "unbreak", "implement"] {
            assert!(
                matches!(
                    resolve_definition(tmp.path(), name, None).unwrap(),
                    Target::Skill(_)
                ),
                "{name}"
            );
        }
        for name in ["code", "incident", "feature", "vsm-operate"] {
            assert!(
                matches!(
                    resolve_definition(tmp.path(), name, None).unwrap(),
                    Target::Flow(_)
                ),
                "{name}"
            );
        }
    }
}
