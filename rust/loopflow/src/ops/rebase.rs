use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    current_branch, delete_local_branch, fetch, get_default_branch, is_merged_into, rebase,
    rev_parse, squash_merge_fork_point, sync_main,
};
use crate::engine::identity::WaveId;
use crate::engine::naming::git_user;

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
    StackParentOpen,
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseStrategy {
    Noop,
    ResetToBase,
    DirectRebase,
    RebaseOntoParent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebasePlan {
    pub branch: String,
    pub base_ref: String,
    pub stack_parent: Option<String>,
    pub class: RebaseClass,
    pub strategy: RebaseStrategy,
    pub unique_commits: usize,
    pub changed_files: Vec<PathBuf>,
    pub protected: bool,
    pub scratch_stashed: bool,
    /// When the stack parent has already merged, the parent's tip commit. A
    /// `rebase --onto <base_ref> <fork_point>` replays only the child's own
    /// commits, dropping the parent's now-merged history.
    pub fork_point: Option<String>,
    /// A merged parent's local branch that is safe to prune once the child has
    /// re-parented (present only when the lingering local ref still exists).
    pub merged_parent: Option<String>,
}

/// Resolve a dotted stack parent to a usable ref: the local branch if it
/// survives (a squash-merge deletes `origin/P` but leaves the local `P`),
/// otherwise the remote-tracking ref.
fn resolve_parent_ref(repo: &Path, parent: &str) -> Option<String> {
    if ref_exists(repo, parent) {
        return Some(parent.to_string());
    }
    let remote_parent = format!("origin/{parent}");
    ref_exists(repo, &remote_parent).then_some(remote_parent)
}

/// The classic "GitHub deleted the head branch on merge" state: the parent's
/// remote branch is gone while its local ref lingers. In loopflow's model a
/// genuinely-open stacked parent keeps its `origin/P` (it was pushed with a
/// PR), so `origin/P`'s absence means the parent landed — in whatever form,
/// including a rework whose content no longer matches what the child stacked on.
fn parent_deleted_on_remote(repo: &Path, parent: &str) -> bool {
    ref_exists(repo, parent) && !ref_exists(repo, &format!("origin/{parent}"))
}

/// The fork point for re-parenting a stacked child onto the default branch when
/// its parent has landed, plus the parent's local branch name if it lingers.
///
/// The child owns only its own commits: once the parent lands in ANY form, we
/// replay the child's commits onto the default branch. "Landed" is detected
/// content-independently — a caller-supplied signal (the daemon's lfdb
/// `stack_status == Merged`), a fast-forward ancestor or squash-merge into the
/// default branch, or the parent's remote branch having been deleted. This is
/// deliberately broader than a content match: a *reworked* parent (its merged
/// content diverges from the child's base) still counts as landed, so the child
/// re-parents onto the default branch and never falls back to the stale local
/// `P`. If the rework diverges from lines the child also touched, the replay
/// conflicts — the correct outcome, surfaced to the caller, not auto-healed.
///
/// The fork point is the parent's tip, exact even for a multi-commit parent
/// that `squash_merge_fork_point` (patch-id based) cannot detect.
///
/// Returns `None` only when the branch has no stack parent, or the parent is
/// genuinely open (not landed) — the child keeps stacking on it, unchanged.
pub(crate) fn merged_parent_fork_point(
    repo: &Path,
    branch: &str,
    default_branch: &str,
    parent_landed: bool,
) -> Option<(String, Option<String>)> {
    let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
    let parent = WaveId::parse(branch, &user)?.parent()?;
    let parent_ref = resolve_parent_ref(repo, &parent)?;
    let default_ref = format!("origin/{default_branch}");
    let landed = parent_landed
        || is_merged_into(repo, &parent_ref, &default_ref).unwrap_or(false)
        || parent_deleted_on_remote(repo, &parent);
    if !landed {
        return None;
    }
    let fork_point = rev_parse(repo, &parent_ref).ok()?;
    let local_branch = ref_exists(repo, &parent).then_some(parent);
    Some((fork_point, local_branch))
}

pub fn plan_rebase(repo: &Path, onto: Option<&str>) -> OpsResult<RebasePlan> {
    let default_branch = get_default_branch(repo)?;
    let branch = current_branch(repo)?.unwrap_or_else(|| "HEAD".to_string());
    let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
    let stack_parent = WaveId::parse(&branch, &user).and_then(|id| id.parent());
    // A surviving parent ref is only a valid base while the parent is still
    // open. Once the parent merges into the default branch, its ref is a dead
    // tip: rebasing onto it drags the parent's already-merged commits back into
    // the child. Detect the merge and re-parent onto the default branch instead.
    // No daemon here: `parent_landed` is inferred from git alone (content merge
    // or a deleted remote branch) inside merged_parent_fork_point.
    let (fork_point, merged_parent) =
        merged_parent_fork_point(repo, &branch, &default_branch, false)
            .map(|(fork_point, local_branch)| (Some(fork_point), local_branch))
            .unwrap_or((None, None));
    let parent_base_ref = if fork_point.is_some() {
        None
    } else {
        stack_parent
            .as_deref()
            .and_then(|parent| resolve_parent_ref(repo, parent))
    };
    let base_ref = if let Some(onto) = onto {
        onto.to_string()
    } else if let Some(parent_base_ref) = parent_base_ref.as_ref() {
        parent_base_ref.clone()
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
    } else if parent_base_ref.is_some() && onto.is_none() {
        (
            RebaseClass::StackParentOpen,
            RebaseStrategy::RebaseOntoParent,
        )
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
        stack_parent: parent_base_ref.as_ref().and(stack_parent),
        class,
        strategy,
        unique_commits,
        changed_files,
        protected,
        scratch_stashed,
        fork_point,
        merged_parent,
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

    // When a stacked branch's parent has landed, the child owns only its own
    // commits: replay them onto the target via the parent's tip as the fork
    // point (exact, even for a multi-commit parent). Fall back to the patch-id
    // scan for non-stack squashes. A *reworked* parent (its merged content
    // diverges from the child's base) is still landed — the child re-parents
    // onto the target. If the rework diverges from lines the child also touched,
    // the replay conflicts; that conflict propagates to the caller to resolve,
    // rather than being swallowed or retried onto the stale parent.
    let fork_point = plan
        .fork_point
        .clone()
        .or_else(|| squash_merge_fork_point(repo, &options.onto).unwrap_or(None));

    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, fork_point.as_deref())?;
    if result.success {
        if fork_point.is_some() {
            progress.status("Skipped merged parent commits");
        }
        // The child has re-parented onto the default branch; the merged parent's
        // lingering local ref is now a dead base — prune it so future rebases
        // don't resolve it as an open parent. Best-effort: a ref still checked
        // out in the parent's worktree can't be deleted, which is fine.
        if let Some(parent) = plan.merged_parent.as_deref() {
            let _ = delete_local_branch(repo, parent);
        }
        if options.push {
            crate::ops::commit::push_with_upstream_if_needed(repo)?;
        }
        return Ok(());
    }
    let detail = result
        .conflicts
        .filter(|conflicts| !conflicts.is_empty())
        .map(|conflicts| {
            let conflict_paths = conflicts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("conflicts: {conflict_paths}")
        })
        .unwrap_or_else(|| "manual resolution required".to_string());
    Err(OpsError::RebaseConflict {
        onto: options.onto.clone(),
        detail,
    })
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

fn ref_exists(repo: &Path, ref_name: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet", ref_name])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
        RebaseClass::StackParentOpen => "stack_parent_open",
        RebaseClass::Protected => "protected",
    }
}

pub fn rebase_strategy_name(strategy: &RebaseStrategy) -> &'static str {
    match strategy {
        RebaseStrategy::Noop => "noop",
        RebaseStrategy::ResetToBase => "reset_to_base",
        RebaseStrategy::DirectRebase => "direct_rebase",
        RebaseStrategy::RebaseOntoParent => "rebase_onto_parent",
    }
}
