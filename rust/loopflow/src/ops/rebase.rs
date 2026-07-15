use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    abort_rebase as abort_git_rebase, continue_rebase as continue_git_rebase, current_branch,
    fetch, get_default_branch, rebase, rebase_for_resolution as start_git_rebase_for_resolution,
    squash_merge_fork_point, sync_main,
};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct RebaseOptions {
    pub onto: String,
    pub push: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseClass {
    Current,
    StaleEmpty,
    ScratchOnly,
    GeneratedOnly,
    CleanAuthored,
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseStrategy {
    Noop,
    ResetToBase,
    DirectRebase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlan {
    pub branch: String,
    pub base_ref: String,
    pub class: RebaseClass,
    pub strategy: RebaseStrategy,
    pub unique_commits: usize,
    pub changed_files: Vec<PathBuf>,
    pub protected: bool,
    pub scratch_stashed: bool,
}

pub fn plan_rebase(repo: &Path, onto: Option<&str>) -> OpsResult<RebasePlan> {
    let default_branch = get_default_branch(repo)?;
    let branch = current_branch(repo)?.unwrap_or_else(|| "HEAD".to_string());
    let base_ref = if let Some(onto) = onto {
        onto.to_string()
    } else {
        format!("origin/{default_branch}")
    };

    let unique_commits = count_unique_commits(repo, &base_ref).unwrap_or(0);
    let mut changed_files = diff_names(repo, &base_ref).unwrap_or_default();
    for path in dirty_paths(repo).unwrap_or_default() {
        if !changed_files.contains(&path) {
            changed_files.push(path);
        }
    }
    changed_files.sort();

    let protected = changed_files.iter().any(|path| is_protected_path(path));
    let scratch_only = !changed_files.is_empty()
        && changed_files
            .iter()
            .all(|path| path.starts_with(Path::new("scratch")));
    let has_non_scratch_changes = changed_files
        .iter()
        .any(|path| !path.starts_with(Path::new("scratch")));

    let (class, strategy) = if branch == default_branch {
        (RebaseClass::Protected, RebaseStrategy::Noop)
    } else if protected {
        (RebaseClass::Protected, RebaseStrategy::DirectRebase)
    } else if unique_commits == 0 && scratch_only {
        (RebaseClass::ScratchOnly, RebaseStrategy::ResetToBase)
    } else if unique_commits == 0 && changed_files.is_empty() {
        (RebaseClass::StaleEmpty, RebaseStrategy::ResetToBase)
    } else if has_non_scratch_changes {
        (RebaseClass::CleanAuthored, RebaseStrategy::DirectRebase)
    } else if unique_commits == 0 {
        (RebaseClass::Current, RebaseStrategy::Noop)
    } else if scratch_only || generated_only(repo, &base_ref).unwrap_or(false) {
        (RebaseClass::GeneratedOnly, RebaseStrategy::ResetToBase)
    } else {
        (RebaseClass::CleanAuthored, RebaseStrategy::DirectRebase)
    };

    let scratch_stashed =
        matches!(strategy, RebaseStrategy::ResetToBase) && repo.join("scratch").exists();

    Ok(RebasePlan {
        branch,
        base_ref,
        class,
        strategy,
        unique_commits,
        changed_files,
        protected,
        scratch_stashed,
    })
}

pub fn rebase_with_recovery(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let plan = plan_rebase(repo, Some(&options.onto))?;
    if matches!(plan.strategy, RebaseStrategy::ResetToBase) && !plan.protected {
        return reset_to_base(repo, &plan, options.push, progress);
    }

    if let Some(branch) = options.onto.strip_prefix("origin/") {
        let _ = fetch(repo, "origin", branch);
        // Keep local main in sync with origin so downstream reads see current state.
        let _ = sync_main(repo, branch);
    }

    let fork_point = squash_merge_fork_point(repo, &options.onto).unwrap_or(None);

    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, fork_point.as_deref())?;
    if result.success {
        if fork_point.is_some() {
            progress.status("Skipped merged parent commits");
        }
        if options.push {
            crate::ops::commit::push_with_upstream_if_needed(repo)?;
        }
        return Ok(());
    }
    let detail = conflict_detail(result.conflicts);
    Err(OpsError::RebaseConflict {
        onto: options.onto.clone(),
        detail,
    })
}

/// Start a local rebase and preserve conflicts for inline resolution.
pub fn start_rebase_for_resolution(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let plan = plan_rebase(repo, Some(&options.onto))?;
    if matches!(plan.strategy, RebaseStrategy::ResetToBase) && !plan.protected {
        return reset_to_base(repo, &plan, false, progress);
    }

    if let Some(branch) = options.onto.strip_prefix("origin/") {
        let _ = fetch(repo, "origin", branch);
        let _ = sync_main(repo, branch);
    }

    let fork_point = squash_merge_fork_point(repo, &options.onto).unwrap_or(None);
    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = start_git_rebase_for_resolution(repo, &options.onto, fork_point.as_deref())?;
    if result.success {
        return Ok(());
    }

    let detail = conflict_detail(result.conflicts);
    Err(OpsError::RebaseConflict {
        onto: options.onto.clone(),
        detail,
    })
}

/// Continue a local rebase after its conflict paths have been resolved.
pub fn continue_rebase_for_resolution(repo: &Path) -> OpsResult<()> {
    let result = continue_git_rebase(repo)?;
    if result.success {
        return Ok(());
    }
    Err(OpsError::RebaseConflict {
        onto: "the selected base".to_string(),
        detail: conflict_detail(result.conflicts),
    })
}

/// Abort a local rebase that was left open for inline resolution.
pub fn abort_rebase_for_resolution(repo: &Path) -> OpsResult<()> {
    abort_git_rebase(repo)?;
    Ok(())
}

fn conflict_detail(conflicts: Option<Vec<PathBuf>>) -> String {
    conflicts
        .filter(|conflicts| !conflicts.is_empty())
        .map(|conflicts| {
            let conflict_paths = conflicts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("conflicts: {conflict_paths}")
        })
        .unwrap_or_else(|| "manual resolution required".to_string())
}

fn reset_to_base(
    repo: &Path,
    plan: &RebasePlan,
    push: bool,
    progress: &impl Progress,
) -> OpsResult<()> {
    let stash_path = if repo.join("scratch").exists() {
        let path = scratch_stash_path(repo, &plan.branch);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            std::fs::remove_dir_all(&path)?;
        }
        copy_dir(&repo.join("scratch"), &path)?;
        Some(path)
    } else {
        None
    };

    progress.status(&format!(
        "Resetting {} to {}...",
        plan.branch, plan.base_ref
    ));
    git(repo, &["reset", "--hard", &plan.base_ref])?;

    if let Some(path) = stash_path {
        let scratch = repo.join("scratch");
        if scratch.exists() {
            std::fs::remove_dir_all(&scratch)?;
        }
        copy_dir(&path, &scratch)?;
        progress.status(&format!("Restored scratch from {}", path.display()));
    }

    if push {
        push_reset(repo)?;
    }
    Ok(())
}

fn push_reset(repo: &Path) -> OpsResult<()> {
    let upstream = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
        .output()?;
    if upstream.status.success() {
        crate::engine::git::push(repo, true)?;
        return Ok(());
    }

    let branch =
        current_branch(repo)?.ok_or_else(|| OpsError::Message("not on a branch".into()))?;
    crate::engine::git::push_with_upstream(repo, "origin", &branch)?;
    Ok(())
}

fn scratch_stash_path(repo: &Path, branch: &str) -> PathBuf {
    let ts = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
    let safe_branch = branch.replace(['/', '.'], "-");
    repo.join(".lf")
        .join("tmp")
        .join("scratch-stash")
        .join(format!("{safe_branch}-{ts}"))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let source = entry.path();
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            std::fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> OpsResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::Git(crate::engine::GitError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn count_unique_commits(repo: &Path, base_ref: &str) -> OpsResult<usize> {
    let stdout = git(repo, &["rev-list", "--count", &format!("{base_ref}..HEAD")])?;
    Ok(stdout.trim().parse().unwrap_or(0))
}

fn diff_names(repo: &Path, base_ref: &str) -> OpsResult<Vec<PathBuf>> {
    // Three-dot: diff from the merge-base, so we see only what this branch
    // authored — not files the base advanced past us. A stale branch (the whole
    // reason to rebase) would otherwise report the base's new files as its own,
    // misclassifying a scratch-only branch as clean_authored.
    let stdout = git(
        repo,
        &["diff", "--name-only", &format!("{base_ref}...HEAD")],
    )?;
    Ok(stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

fn dirty_paths(repo: &Path) -> OpsResult<Vec<PathBuf>> {
    // Read raw stdout: porcelain lines carry a leading status column (e.g.
    // " M path" for a working-tree-only change), and the shared git() helper
    // would trim the first line's leading space, shifting the fixed 3-char
    // path offset and dropping the first character of that path.
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(OpsError::Git(crate::engine::GitError::CommandFailed {
            command: "git status --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        }));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let path = &line[3..];
            let path = path
                .split_once(" -> ")
                .map(|(_, right)| right)
                .unwrap_or(path);
            Some(PathBuf::from(path))
        })
        .collect())
}

fn generated_only(repo: &Path, base_ref: &str) -> OpsResult<bool> {
    let stdout = git(repo, &["log", "--format=%s", &format!("{base_ref}..HEAD")])?;
    let subjects = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    Ok(!subjects.is_empty()
        && subjects.iter().all(|subject| {
            subject.starts_with("checkpoint:")
                || subject.starts_with("wip:")
                || subject.contains("generated")
        }))
}

fn is_protected_path(path: &Path) -> bool {
    path.starts_with(Path::new("wave"))
        || path.starts_with(Path::new(".lf/skills"))
        || path.starts_with(Path::new(".lf/flows"))
        || path.starts_with(Path::new(".lf/directions"))
        || path == Path::new(".lf/config.yaml")
}

pub fn rebase_class_name(class: &RebaseClass) -> &'static str {
    match class {
        RebaseClass::Current => "current",
        RebaseClass::StaleEmpty => "stale_empty",
        RebaseClass::ScratchOnly => "scratch_only",
        RebaseClass::GeneratedOnly => "generated_only",
        RebaseClass::CleanAuthored => "clean_authored",
        RebaseClass::Protected => "protected",
    }
}

pub fn rebase_strategy_name(strategy: &RebaseStrategy) -> &'static str {
    match strategy {
        RebaseStrategy::Noop => "noop",
        RebaseStrategy::ResetToBase => "reset_to_base",
        RebaseStrategy::DirectRebase => "direct_rebase",
    }
}

#[cfg(test)]
mod tests {
    use super::scratch_stash_path;
    use std::path::Path;

    #[test]
    fn scratch_stash_lands_under_the_ignored_tmp_prefix() {
        let path = scratch_stash_path(Path::new("/repo"), "jack/reconcile-out-of-band-merges");
        // .lf/tmp/ is gitignored; a sibling like .lf/scratch-stash/ would dirty
        // the worktree and block `lf task complete`.
        assert!(
            path.starts_with("/repo/.lf/tmp/scratch-stash"),
            "stash must sit under the ignored .lf/tmp prefix, got {}",
            path.display()
        );
        // Slashes and dots in the branch are flattened so the dir name is safe.
        let leaf = path.file_name().unwrap().to_string_lossy();
        assert!(leaf.starts_with("jack-reconcile-out-of-band-merges-"));
    }
}
