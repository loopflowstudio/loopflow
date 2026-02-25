//! Skill injection — materialize loopflow steps and directions as agent-native commands.
//!
//! When a wave spawns an agent, this module writes all known steps and
//! directions to `.claude/commands/*.md` so they appear as slash commands
//! in Claude Code. Existing user files are never overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use tracing::debug;

use crate::engine::builtins;

/// Write all known steps and directions as `.claude/commands/*.md` files.
///
/// Returns paths of files written (for cleanup). Skips files that already
/// exist — user customizations always win.
pub fn inject_skills(repo: &Path, target_dir: &Path) -> Vec<PathBuf> {
    let commands_dir = target_dir.join(".claude/commands");
    if let Err(err) = fs::create_dir_all(&commands_dir) {
        tracing::warn!(
            path = %commands_dir.display(),
            error = %err,
            "failed to create .claude/commands directory for skill injection"
        );
        return Vec::new();
    }

    let mut injected = Vec::new();

    // Inject built-in steps.
    for name in builtins::builtin_step_names() {
        let Some(content) = builtins::get_builtin_step(name) else {
            continue;
        };
        let filename = flatten_step_name(name);
        if let Some(path) = write_if_absent(&commands_dir, &filename, content) {
            debug!(step = name, path = %path.display(), "injected step");
            injected.push(path);
        }
    }

    // Inject built-in directions.
    for name in builtins::builtin_direction_names() {
        let Some(content) = builtins::get_builtin_direction(name) else {
            continue;
        };
        let filename = format!("direction-{name}");
        if let Some(path) = write_if_absent(&commands_dir, &filename, content) {
            debug!(direction = name, path = %path.display(), "injected direction");
            injected.push(path);
        }
    }

    // Inject repo-local .lf/steps/ that aren't already in .claude/commands/.
    inject_md_dir(&repo.join(".lf/steps"), &commands_dir, &mut injected);

    // Inject repo-local .lf/directions/ as direction-<name>.md.
    let directions_dir = repo.join(".lf/directions");
    if directions_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&directions_dir) {
            for entry in entries.flatten() {
                let src = entry.path();
                if src.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Some(stem) = src.file_stem() {
                        let filename = format!("direction-{}", stem.to_string_lossy());
                        if let Ok(content) = fs::read_to_string(&src) {
                            if let Some(path) = write_if_absent(&commands_dir, &filename, &content)
                            {
                                debug!(src = %src.display(), dest = %path.display(), "injected repo-local direction");
                                injected.push(path);
                            }
                        }
                    }
                }
            }
        }
    }

    injected
}

/// Write content to `dir/{name}.md` if the file doesn't already exist.
fn write_if_absent(dir: &Path, name: &str, content: &str) -> Option<PathBuf> {
    let path = dir.join(format!("{name}.md"));
    if path.exists() {
        debug!(name, "skipping injection — file exists");
        return None;
    }
    if let Err(err) = fs::write(&path, content) {
        tracing::warn!(name, error = %err, "failed to inject skill");
        return None;
    }
    Some(path)
}

/// Inject all `.md` files from a directory into commands_dir.
fn inject_md_dir(src_dir: &Path, commands_dir: &Path, injected: &mut Vec<PathBuf>) {
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
                    let name = stem.to_string_lossy();
                    if let Some(path) = write_if_absent(commands_dir, &name, &content) {
                        debug!(src = %src.display(), dest = %path.display(), "injected repo-local step");
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
        if let Err(err) = fs::remove_file(path) {
            // File may already be gone (worktree cleaned up, etc.)
            debug!(path = %path.display(), error = %err, "failed to remove injected skill");
        }
    }
}

/// Flatten namespaced step names for `.claude/commands/` (which is flat).
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

        // All injected files should exist.
        for path in &injected {
            assert!(
                path.exists(),
                "injected file should exist: {}",
                path.display()
            );
            assert!(path.to_string_lossy().contains(".claude/commands/"));
        }

        // Known builtin steps should be present.
        let commands_dir = tmp.path().join(".claude/commands");
        assert!(commands_dir.join("design.md").exists());
        assert!(commands_dir.join("implement.md").exists());
        assert!(commands_dir.join("debug.md").exists());

        // Known builtin directions should be present with direction- prefix.
        assert!(commands_dir.join("direction-care.md").exists());
        assert!(commands_dir.join("direction-clarity.md").exists());
        assert!(commands_dir.join("direction-living.md").exists());
    }

    #[test]
    fn inject_skills_skips_existing_files() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join(".claude/commands");
        fs::create_dir_all(&commands_dir).unwrap();

        // Write a user-owned design.md.
        let user_content = "# My custom design step";
        fs::write(commands_dir.join("design.md"), user_content).unwrap();

        let injected = inject_skills(tmp.path(), tmp.path());

        // design.md should NOT be in the injected list.
        assert!(
            !injected.iter().any(|p| p.ends_with("design.md")),
            "should not inject over existing file"
        );

        // User content should be preserved.
        let content = fs::read_to_string(commands_dir.join("design.md")).unwrap();
        assert_eq!(content, user_content);
    }

    #[test]
    fn inject_skills_includes_repo_local_steps() {
        let tmp = TempDir::new().unwrap();
        let lf_steps = tmp.path().join(".lf/steps");
        fs::create_dir_all(&lf_steps).unwrap();
        fs::write(lf_steps.join("my-custom.md"), "# Custom step").unwrap();

        let injected = inject_skills(tmp.path(), tmp.path());

        let commands_dir = tmp.path().join(".claude/commands");
        assert!(
            commands_dir.join("my-custom.md").exists(),
            "repo-local step should be injected"
        );
        assert!(
            injected.iter().any(|p| p.ends_with("my-custom.md")),
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

        let commands_dir = tmp.path().join(".claude/commands");
        assert!(
            commands_dir.join("direction-conductor.md").exists(),
            "repo-local direction should be injected with prefix"
        );
        assert!(
            injected
                .iter()
                .any(|p| p.ends_with("direction-conductor.md")),
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
                "injected file should be removed: {}",
                path.display()
            );
        }
    }

    #[test]
    fn flatten_step_name_handles_namespaced() {
        assert_eq!(flatten_step_name("scan/scan-report"), "scan-scan-report");
        assert_eq!(flatten_step_name("design"), "design");
    }
}
