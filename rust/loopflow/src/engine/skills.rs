//! Skill injection — materialize loopflow steps and directions as agent skills.
//!
//! When a wave spawns an agent, this module writes all known steps and
//! directions to `.agents/skills/<name>/SKILL.md`. Existing user files are
//! never overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use serde_yaml_ng::{Mapping, Value};
use tracing::debug;

use crate::engine::builtins;

const SKILL_FILE_NAME: &str = "SKILL.md";

/// Write all known steps and directions as `.agents/skills/<name>/SKILL.md`.
///
/// Returns paths of directories written (for cleanup). Skips directories that
/// already exist — user customizations always win.
pub fn inject_skills(repo: &Path, target_dir: &Path) -> Vec<PathBuf> {
    let skills_dir = target_dir.join(".agents/skills");
    if let Err(err) = fs::create_dir_all(&skills_dir) {
        tracing::warn!(
            path = %skills_dir.display(),
            error = %err,
            "failed to create .agents/skills directory for skill injection"
        );
        return Vec::new();
    }

    let mut injected = Vec::new();

    // Inject built-in steps.
    for name in builtins::builtin_step_names() {
        let Some(content) = builtins::get_builtin_step(name) else {
            continue;
        };
        let skill_name = flatten_step_name(name);
        let projected = project_to_skill_md(&skill_name, content, false);
        if let Some(path) = write_skill_if_absent(&skills_dir, &skill_name, &projected) {
            debug!(step = name, path = %path.display(), "injected step");
            injected.push(path);
        }
    }

    // Inject built-in directions.
    for name in builtins::builtin_direction_names() {
        let Some(content) = builtins::get_builtin_direction(name) else {
            continue;
        };
        let skill_name = format!("direction-{name}");
        let projected = project_to_skill_md(&skill_name, content, true);
        if let Some(path) = write_skill_if_absent(&skills_dir, &skill_name, &projected) {
            debug!(direction = name, path = %path.display(), "injected direction");
            injected.push(path);
        }
    }

    // Inject repo-local .lf/steps/ that aren't already in .agents/skills/.
    inject_md_dir(
        &repo.join(".lf/steps"),
        &skills_dir,
        &mut injected,
        false,
        |stem| stem.to_string(),
    );

    // Inject repo-local .lf/directions/ as direction-<name>/SKILL.md.
    inject_md_dir(
        &repo.join(".lf/directions"),
        &skills_dir,
        &mut injected,
        true,
        |stem| format!("direction-{stem}"),
    );

    injected
}

/// Project a loopflow step prompt to Agent Skills SKILL.md format.
fn project_to_skill_md(name: &str, content: &str, is_direction: bool) -> String {
    let (source_frontmatter, body) = parse_source_frontmatter(content);
    let mut frontmatter = Mapping::new();

    frontmatter.insert(key("name"), Value::String(name.to_string()));

    let description = source_field(&source_frontmatter, &["description"])
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| extract_description(&body))
        .unwrap_or_else(|| format!("Run {name}."));
    frontmatter.insert(key("description"), Value::String(description));
    frontmatter.insert(key("loopflow"), Value::Bool(true));

    if let Some(model) = source_field(&source_frontmatter, &["model"]) {
        frontmatter.insert(key("model"), model.clone());
    }

    if let Some(disable_model_invocation) = source_field(
        &source_frontmatter,
        &["disable-model-invocation", "disable_model_invocation"],
    ) {
        frontmatter.insert(
            key("disable-model-invocation"),
            disable_model_invocation.clone(),
        );
    } else if !is_direction
        && source_field(&source_frontmatter, &["interactive"]).and_then(Value::as_bool)
            != Some(true)
    {
        frontmatter.insert(key("disable-model-invocation"), Value::Bool(true));
    }

    if is_direction {
        frontmatter.insert(key("user-invocable"), Value::Bool(false));
    } else if let Some(user_invocable) =
        source_field(&source_frontmatter, &["user-invocable", "user_invocable"])
    {
        frontmatter.insert(key("user-invocable"), user_invocable.clone());
    }

    copy_field(
        &source_frontmatter,
        &mut frontmatter,
        "allowed-tools",
        &["allowed-tools", "allowed_tools"],
    );
    copy_field(
        &source_frontmatter,
        &mut frontmatter,
        "context",
        &["context"],
    );
    copy_field(&source_frontmatter, &mut frontmatter, "agent", &["agent"]);
    copy_field(
        &source_frontmatter,
        &mut frontmatter,
        "argument-hint",
        &["argument-hint", "argument_hint"],
    );

    let yaml_frontmatter = format_yaml_frontmatter(&frontmatter);
    format!("---\n{yaml_frontmatter}---\n{body}")
}

/// Extract description from first non-empty line in body.
fn extract_description(body: &str) -> Option<String> {
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let description = trimmed.trim_start_matches('#').trim();
        if description.is_empty() {
            continue;
        }
        return Some(description.to_string());
    }
    None
}

/// Write content to `.agents/skills/{name}/SKILL.md` if the directory doesn't exist.
fn write_skill_if_absent(skills_dir: &Path, name: &str, content: &str) -> Option<PathBuf> {
    let skill_dir = skills_dir.join(name);
    if skill_dir.exists() {
        debug!(name, "skipping injection — skill directory exists");
        return None;
    }

    if let Err(err) = fs::create_dir_all(&skill_dir) {
        tracing::warn!(name, error = %err, "failed to create skill directory");
        return None;
    }

    let skill_path = skill_dir.join(SKILL_FILE_NAME);
    if let Err(err) = fs::write(&skill_path, content) {
        tracing::warn!(name, error = %err, "failed to inject skill");
        let _ = fs::remove_dir_all(&skill_dir);
        return None;
    }
    Some(skill_dir)
}

/// Inject all top-level `.md` files from a directory into `.agents/skills`.
fn inject_md_dir<F>(
    src_dir: &Path,
    skills_dir: &Path,
    injected: &mut Vec<PathBuf>,
    is_direction: bool,
    name_for_stem: F,
) where
    F: Fn(&str) -> String,
{
    if !src_dir.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(src_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let src = entry.path();
        if src.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(stem) = src.file_stem() {
                if let Ok(content) = fs::read_to_string(&src) {
                    let skill_name = name_for_stem(stem.to_string_lossy().as_ref());
                    let projected = project_to_skill_md(&skill_name, &content, is_direction);
                    if let Some(path) = write_skill_if_absent(skills_dir, &skill_name, &projected) {
                        debug!(src = %src.display(), dest = %path.display(), "injected repo-local skill");
                        injected.push(path);
                    }
                }
            }
        }
    }
}

/// Remove previously injected skill files.
pub fn cleanup_injected_skills(paths: &[PathBuf]) {
    for path in paths {
        let result = if path.is_dir() {
            fs::remove_dir_all(path)
        } else {
            fs::remove_file(path)
        };
        if let Err(err) = result {
            // Path may already be gone (worktree cleaned up, etc.)
            debug!(path = %path.display(), error = %err, "failed to remove injected skill");
        }
    }
}

fn parse_source_frontmatter(content: &str) -> (Mapping, String) {
    let Some((frontmatter, body)) = split_frontmatter(content) else {
        return (Mapping::new(), content.to_string());
    };

    let Ok(value) = serde_yaml_ng::from_str::<Value>(&frontmatter) else {
        return (Mapping::new(), content.to_string());
    };
    let Some(map) = value.as_mapping() else {
        return (Mapping::new(), body);
    };
    (map.clone(), body)
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

fn format_yaml_frontmatter(map: &Mapping) -> String {
    let mut yaml = serde_yaml_ng::to_string(&Value::Mapping(map.clone())).unwrap_or_default();
    if let Some(stripped) = yaml.strip_prefix("---\n") {
        yaml = stripped.to_string();
    }
    if let Some(stripped) = yaml.strip_suffix("...\n") {
        yaml = stripped.to_string();
    }
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    yaml
}

fn copy_field(source: &Mapping, target: &mut Mapping, key_name: &str, aliases: &[&str]) {
    if let Some(value) = source_field(source, aliases) {
        target.insert(key(key_name), value.clone());
    }
}

fn source_field<'a>(map: &'a Mapping, aliases: &[&str]) -> Option<&'a Value> {
    aliases.iter().find_map(|alias| map.get(key(alias)))
}

fn key(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Flatten namespaced step names for `.agents/skills/` directory names.
/// `scan/scan-report` → `scan-scan-report`
fn flatten_step_name(name: &str) -> String {
    name.replace('/', "-")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn inject_skills_writes_builtin_steps_and_directions() {
        let tmp = TempDir::new().unwrap();
        let injected = inject_skills(tmp.path(), tmp.path());

        assert!(!injected.is_empty(), "should inject at least one skill");

        // All injected directories should exist.
        for path in &injected {
            assert!(
                path.exists(),
                "injected directory should exist: {}",
                path.display()
            );
            assert!(path.to_string_lossy().contains(".agents/skills/"));
            assert!(path.is_dir());
            assert!(path.join(SKILL_FILE_NAME).exists());
        }

        // Known builtin steps should be present.
        let skills_dir = tmp.path().join(".agents/skills");
        assert!(skills_dir.join("design").join(SKILL_FILE_NAME).exists());
        assert!(skills_dir.join("implement").join(SKILL_FILE_NAME).exists());
        assert!(skills_dir.join("debug").join(SKILL_FILE_NAME).exists());

        // Known builtin directions should be present with direction- prefix.
        assert!(skills_dir
            .join("direction-care")
            .join(SKILL_FILE_NAME)
            .exists());
        assert!(skills_dir
            .join("direction-clarity")
            .join(SKILL_FILE_NAME)
            .exists());
        assert!(skills_dir
            .join("direction-living")
            .join(SKILL_FILE_NAME)
            .exists());
    }

    #[test]
    fn inject_skills_skips_existing_files() {
        let tmp = TempDir::new().unwrap();
        let skills_dir = tmp.path().join(".agents/skills");
        let design_dir = skills_dir.join("design");
        fs::create_dir_all(&design_dir).unwrap();

        // Write a user-owned design/SKILL.md.
        let user_content = "# My custom design step";
        fs::write(design_dir.join(SKILL_FILE_NAME), user_content).unwrap();

        let injected = inject_skills(tmp.path(), tmp.path());

        // design should NOT be in the injected list.
        assert!(
            !injected.iter().any(|p| p.ends_with("design")),
            "should not inject over existing directory"
        );

        // User content should be preserved.
        let content = fs::read_to_string(design_dir.join(SKILL_FILE_NAME)).unwrap();
        assert_eq!(content, user_content);
    }

    #[test]
    fn inject_skills_includes_repo_local_steps() {
        let tmp = TempDir::new().unwrap();
        let lf_steps = tmp.path().join(".lf/steps");
        fs::create_dir_all(&lf_steps).unwrap();
        fs::write(lf_steps.join("my-custom.md"), "# Custom step").unwrap();

        let injected = inject_skills(tmp.path(), tmp.path());

        let skills_dir = tmp.path().join(".agents/skills");
        assert!(
            skills_dir.join("my-custom").join(SKILL_FILE_NAME).exists(),
            "repo-local step should be injected"
        );
        assert!(
            injected.iter().any(|p| p.ends_with("my-custom")),
            "repo-local step should be in injected list"
        );
    }

    #[test]
    fn inject_skills_includes_repo_local_directions() {
        let tmp = TempDir::new().unwrap();
        let directions = tmp.path().join(".lf/directions");
        fs::create_dir_all(&directions).unwrap();
        fs::write(directions.join("conductor.md"), "# Conductor direction").unwrap();

        let injected = inject_skills(tmp.path(), tmp.path());

        let skills_dir = tmp.path().join(".agents/skills");
        assert!(
            skills_dir
                .join("direction-conductor")
                .join(SKILL_FILE_NAME)
                .exists(),
            "repo-local direction should be injected with prefix"
        );
        assert!(
            injected.iter().any(|p| p.ends_with("direction-conductor")),
            "repo-local direction should be in injected list"
        );
    }

    #[test]
    fn cleanup_removes_injected_files() {
        let tmp = TempDir::new().unwrap();
        let injected = inject_skills(tmp.path(), tmp.path());
        assert!(!injected.is_empty());

        cleanup_injected_skills(&injected);

        for path in &injected {
            assert!(
                !path.exists(),
                "injected directory should be removed: {}",
                path.display()
            );
        }
    }

    #[test]
    fn project_to_skill_md_projects_frontmatter() {
        let content = r#"---
model: claude:sonnet
interactive: true
directions: [ux, craft]
requires: diff vs main
produces: verdict
allowed-tools: [read, edit]
---
Walk the human through the current diff and help them decide the next right move.
"#;
        let projected = project_to_skill_md("review", content, false);
        let (frontmatter, body) = split_frontmatter(&projected).expect("projected frontmatter");
        let value: Value = serde_yaml_ng::from_str(&frontmatter).expect("parse frontmatter");
        let map = value.as_mapping().expect("mapping");

        assert_eq!(map.get(key("name")).and_then(Value::as_str), Some("review"));
        assert_eq!(
            map.get(key("description")).and_then(Value::as_str),
            Some(
                "Walk the human through the current diff and help them decide the next right move."
            )
        );
        assert_eq!(
            map.get(key("model")).and_then(Value::as_str),
            Some("claude:sonnet")
        );
        assert_eq!(
            map.get(key("loopflow")).and_then(Value::as_bool),
            Some(true)
        );
        assert!(
            map.get(key("disable-model-invocation")).is_none(),
            "interactive: true should not force disable-model-invocation"
        );
        assert_eq!(
            map.get(key("allowed-tools"))
                .and_then(Value::as_sequence)
                .map(|sequence| sequence.len()),
            Some(2)
        );
        assert_eq!(
            body.trim(),
            "Walk the human through the current diff and help them decide the next right move."
        );
    }

    #[test]
    fn project_to_skill_md_marks_direction_as_not_user_invocable() {
        let projected =
            project_to_skill_md("direction-care", "Quality and attention to detail.", true);
        let (frontmatter, _body) = split_frontmatter(&projected).expect("projected frontmatter");
        let value: Value = serde_yaml_ng::from_str(&frontmatter).expect("parse frontmatter");
        let map = value.as_mapping().expect("mapping");

        assert_eq!(
            map.get(key("user-invocable")).and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            map.get(key("disable-model-invocation")).is_none(),
            "directions should not be forced into disable-model-invocation"
        );
    }

    #[test]
    fn flatten_step_name_handles_namespaced() {
        assert_eq!(flatten_step_name("scan/scan-report"), "scan-scan-report");
        assert_eq!(flatten_step_name("design"), "design");
    }
}
