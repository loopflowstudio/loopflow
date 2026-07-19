use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    abort_rebase as abort_git_rebase, continue_rebase as continue_git_rebase, current_branch,
    fetch, get_default_branch, intervention_state, rebase as start_git_rebase_for_resolution,
    rerere_remaining, rev_parse, squash_merge_fork_point,
};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::git_operation::{
    authorize_rebase_control, begin_rebase_operation, GitOperationOwner, RebaseOperation,
};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct RebaseOptions {
    pub onto: String,
    pub push: bool,
    /// The durable fork commit a stacked child was placed on. When set, the
    /// rebase replays exactly `fork_base..HEAD` onto `onto` via `git rebase
    /// --onto`, dropping the parent commits deterministically — squash-proof,
    /// because it never depends on patch-id matching. `None` keeps the
    /// runtime fork-point heuristic used for ordinary branches.
    pub fork_base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebaseClass {
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
    /// The durable stacked fork base, when this branch is a stacked child. The
    /// rebase replays `fork_base..HEAD` onto `base_ref`; `--plan`, execution,
    /// and the PR range all read this one value.
    pub fork_base: Option<String>,
    pub class: RebaseClass,
    pub strategy: RebaseStrategy,
    pub unique_commits: usize,
    pub changed_files: Vec<PathBuf>,
    pub scratch_stashed: bool,
}

#[derive(Debug)]
struct RebaseExpectation {
    strategy: RebaseStrategy,
    authored_commits: usize,
    expected_nonempty_range: bool,
    tracked_dirty: BTreeSet<String>,
    stacked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebaseVerification {
    pub branch: String,
    pub head: String,
    pub target_sha: String,
    pub unique_commits: usize,
}

#[derive(Debug)]
pub struct RebaseRecovery {
    operation: RebaseOperation,
    expected: RebaseExpectation,
    push: bool,
}

pub fn plan_rebase(
    repo: &Path,
    onto: Option<&str>,
    fork_base: Option<String>,
) -> OpsResult<RebasePlan> {
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
        fork_base,
        class,
        strategy,
        unique_commits,
        changed_files,
        scratch_stashed,
    })
}

pub fn rebase_with_recovery(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<RebaseVerification> {
    start_owned_rebase(repo, options, false, progress)
}

/// Submit/land's final replay: collapse disposable history inside the owned
/// integration operation. Kept crate-private so ordinary rebase has one stable,
/// history-preserving contract.
pub(crate) fn rebase_final_with_recovery(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<RebaseVerification> {
    start_owned_rebase(repo, options, true, progress)
}

/// Start a local rebase and preserve conflicts for inline resolution.
pub fn start_rebase_for_resolution(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<RebaseVerification> {
    let local = RebaseOptions {
        onto: options.onto.clone(),
        push: false,
        fork_base: options.fork_base.clone(),
    };
    start_owned_rebase(repo, &local, false, progress)
}

/// Continue a local rebase after its conflict paths have been resolved.
pub fn continue_rebase_for_resolution(repo: &Path, adopt: bool) -> OpsResult<()> {
    let authorization = authorize_rebase_control(repo, adopt)?;
    let result = continue_git_rebase(repo)?;
    if result.success {
        let owner = authorization.owner().clone();
        verify_control_completion(repo, &owner)?;
        record_control_task_state(repo, &owner)?;
        authorization.complete()?;
        return Ok(());
    }
    Err(OpsError::RebaseConflict {
        onto: authorization.owner().target_ref.clone(),
        detail: conflict_detail(result.conflicts),
        recovery: None,
    })
}

/// Abort a local rebase that was left open for inline resolution.
pub fn abort_rebase_for_resolution(repo: &Path, adopt: bool) -> OpsResult<()> {
    let authorization = authorize_rebase_control(repo, adopt)?;
    abort_git_rebase(repo)?;
    authorization.complete()?;
    Ok(())
}

pub fn recover_rebase(
    recovery: RebaseRecovery,
    launch: impl FnOnce(&BTreeMap<String, String>) -> OpsResult<()>,
) -> OpsResult<RebaseVerification> {
    let worktree = recovery.operation.owner().worktree.clone();
    launch(&recovery.operation.scoped_env())?;
    finish_rebase(
        &worktree,
        recovery.operation,
        &recovery.expected,
        recovery.push,
    )
}

fn start_owned_rebase(
    repo: &Path,
    options: &RebaseOptions,
    collapse: bool,
    progress: &impl Progress,
) -> OpsResult<RebaseVerification> {
    let mut operation = begin_rebase_operation(repo, &options.onto)?;
    let tracked_dirty = tracked_dirty_state(repo)?;
    fetch_target(repo, &options.onto)?;
    let target_sha = rev_parse(repo, &options.onto)?;
    let plan = plan_rebase(repo, Some(&options.onto), options.fork_base.clone())?;
    operation.pin_target(target_sha.clone())?;
    revalidate_start(repo, operation.owner())?;

    let collapsed_fork = if collapse {
        Some(collapse_authored_history(
            repo,
            operation.owner(),
            options.fork_base.as_deref(),
            &target_sha,
            progress,
        )?)
    } else {
        None
    };
    let prepared_head = rev_parse(repo, "HEAD")?;
    let authored_commits = cherry_authored_count(repo, &target_sha, "HEAD")?;
    let strategy = if collapse {
        RebaseStrategy::DirectRebase
    } else {
        plan.strategy.clone()
    };
    let expected = RebaseExpectation {
        strategy: strategy.clone(),
        authored_commits: if collapse { 1 } else { authored_commits },
        expected_nonempty_range: collapse
            || (matches!(strategy, RebaseStrategy::DirectRebase) && authored_commits > 0),
        tracked_dirty,
        stacked: options.fork_base.is_some(),
    };
    revalidate_owned_head(repo, &operation.owner().branch, &prepared_head)?;

    if matches!(strategy, RebaseStrategy::ResetToBase) {
        reset_to_base(repo, &plan, progress)?;
        return finish_rebase(repo, operation, &expected, options.push);
    }
    if matches!(strategy, RebaseStrategy::Noop) {
        return finish_rebase(repo, operation, &expected, options.push);
    }

    let fork_point = match collapsed_fork {
        Some(base) => Some(base),
        None => match resolve_fork_point(repo, options)? {
            Some(base) => Some(base),
            None => squash_merge_fork_point(repo, &target_sha).unwrap_or(None),
        },
    };
    revalidate_owned_head(repo, &operation.owner().branch, &prepared_head)?;
    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = start_git_rebase_for_resolution(repo, &target_sha, fork_point.as_deref())?;
    if result.success {
        if fork_point.is_some() {
            progress.status("Skipped merged parent commits");
        }
        return finish_rebase(repo, operation, &expected, options.push);
    }
    if intervention_state(repo)? != Some("rebase") {
        operation.complete()?;
        return Err(OpsError::Message(format!(
            "git rebase onto {} stopped before creating a recoverable sequencer",
            options.onto
        )));
    }

    if continue_reused_resolutions(repo, progress)? {
        progress.status("Reused recorded conflict resolution");
        return finish_rebase(repo, operation, &expected, options.push);
    }

    let detail = conflict_detail(result.conflicts);
    Err(OpsError::RebaseConflict {
        onto: options.onto.clone(),
        detail: detail.clone(),
        recovery: Some(Box::new(RebaseRecovery {
            operation,
            expected,
            push: options.push,
        })),
    })
}

/// Continue as long as every active conflict was populated by rerere. The Git
/// wrapper stages only the current unmerged paths, never unrelated work, and
/// keeps rerere's own auto-stage behavior disabled.
fn continue_reused_resolutions(repo: &Path, progress: &impl Progress) -> OpsResult<bool> {
    loop {
        let conflicts = git(repo, &["diff", "--diff-filter=U", "--name-only"])?;
        if conflicts.trim().is_empty() || !rerere_remaining(repo)?.is_empty() {
            return Ok(false);
        }
        progress.status("Applying recorded conflict resolution...");
        let result = continue_git_rebase(repo)?;
        if result.success {
            return Ok(true);
        }
        if intervention_state(repo)? != Some("rebase") {
            return Err(OpsError::Message(
                "rebase stopped after rerere without a recoverable sequencer".to_string(),
            ));
        }
    }
}

fn finish_rebase(
    repo: &Path,
    operation: RebaseOperation,
    expected: &RebaseExpectation,
    push: bool,
) -> OpsResult<RebaseVerification> {
    let verification = verify_rebase(repo, operation.owner(), expected)?;
    if !expected.stacked {
        crate::ops::task::validate_task_pr_range_for_integration(
            repo,
            &operation.owner().target_ref,
            &verification.target_sha,
        )?;
    }
    if push {
        // Rebase owns a force-push path rather than the ordinary commit helper,
        // but it must cross the same Task settlement fence first.
        let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
        crate::ops::task::clear_task_pr_merge_before_head_mutation(repo, false)?;
        push_rebased_branch(repo, &verification.branch)?;
        crate::ops::commit::verify_remote_branch_head(
            repo,
            &verification.branch,
            &verification.head,
        )?;
    }
    if !expected.stacked {
        crate::ops::task::record_task_pr_range_after_integration(
            repo,
            &operation.owner().target_ref,
            &verification.target_sha,
        )?;
    }
    operation.complete()?;
    Ok(verification)
}

fn verify_rebase(
    repo: &Path,
    owner: &GitOperationOwner,
    expected: &RebaseExpectation,
) -> OpsResult<RebaseVerification> {
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "rebase incomplete: Git still reports an active {state} operation"
        )));
    }
    let branch = current_branch(repo)?
        .ok_or_else(|| OpsError::Message("rebase incomplete: HEAD is detached".to_string()))?;
    if branch != owner.branch {
        return Err(OpsError::Message(format!(
            "rebase incomplete: expected branch {}, found {branch}",
            owner.branch
        )));
    }
    let head = rev_parse(repo, "HEAD")?;
    let target_sha = owner.target_sha.as_deref().ok_or_else(|| {
        OpsError::Message("rebase verification has no pinned target commit".to_string())
    })?;
    if !crate::engine::git::is_ancestor(repo, target_sha, &head)? {
        return Err(OpsError::Message(format!(
            "rebase incomplete: pinned target {target_sha} is not an ancestor of HEAD {head}"
        )));
    }
    let conflicts = git(repo, &["diff", "--diff-filter=U", "--name-only"])?;
    if !conflicts.trim().is_empty() {
        return Err(OpsError::Message(format!(
            "rebase incomplete: unresolved paths remain: {}",
            conflicts.lines().collect::<Vec<_>>().join(", ")
        )));
    }
    let tracked_dirty = tracked_dirty_state(repo)?;
    let introduced = tracked_dirty
        .difference(&expected.tracked_dirty)
        .cloned()
        .collect::<Vec<_>>();
    if !introduced.is_empty() {
        return Err(OpsError::Message(format!(
            "rebase incomplete: new tracked dirty state remains: {}",
            introduced.join(", ")
        )));
    }
    let unique_commits = count_unique_commits(repo, target_sha)?;
    if matches!(expected.strategy, RebaseStrategy::DirectRebase)
        && expected.expected_nonempty_range
        && unique_commits == 0
    {
        return Err(OpsError::Message(format!(
            "rebase incomplete: {} authored commit(s) were expected, but {target_sha}..HEAD is empty",
            expected.authored_commits
        )));
    }
    Ok(RebaseVerification {
        branch,
        head,
        target_sha: target_sha.to_string(),
        unique_commits,
    })
}

fn verify_control_completion(repo: &Path, owner: &GitOperationOwner) -> OpsResult<()> {
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "rebase incomplete: Git still reports an active {state} operation"
        )));
    }
    if owner.branch != "HEAD" {
        let branch = current_branch(repo)?
            .ok_or_else(|| OpsError::Message("rebase incomplete: HEAD is detached".to_string()))?;
        if branch != owner.branch {
            return Err(OpsError::Message(format!(
                "rebase incomplete: expected branch {}, found {branch}",
                owner.branch
            )));
        }
    }
    if let Some(target_sha) = owner.target_sha.as_deref() {
        if !crate::engine::git::is_ancestor(repo, target_sha, "HEAD")? {
            return Err(OpsError::Message(format!(
                "rebase incomplete: pinned target {target_sha} is not an ancestor of HEAD"
            )));
        }
        let introduced = tracked_dirty_state(repo)?;
        if !introduced.is_empty() {
            return Err(OpsError::Message(format!(
                "rebase incomplete: tracked dirty state remains: {}",
                introduced.into_iter().collect::<Vec<_>>().join(", ")
            )));
        }
        let authored_commits = cherry_authored_count(repo, target_sha, &owner.head)?;
        if authored_commits > 0 && count_unique_commits(repo, target_sha)? == 0 {
            return Err(OpsError::Message(format!(
                "rebase incomplete: {authored_commits} authored commit(s) were expected, but {target_sha}..HEAD is empty"
            )));
        }
    }
    Ok(())
}

fn record_control_task_state(repo: &Path, owner: &GitOperationOwner) -> OpsResult<()> {
    let Some(target_sha) = owner.target_sha.as_deref() else {
        return Ok(());
    };
    if let Some(stacked) = crate::ops::task::task_stack(repo)? {
        let clear_parent = stacked.parent_branch.is_none();
        return crate::ops::task::record_stack_rebase(&stacked, target_sha, clear_parent);
    }
    crate::ops::task::validate_task_pr_range_for_integration(repo, &owner.target_ref, target_sha)?;
    crate::ops::task::record_task_pr_range_after_integration(repo, &owner.target_ref, target_sha)
}

fn fetch_target(repo: &Path, target: &str) -> OpsResult<()> {
    if let Some(branch) = target.strip_prefix("origin/") {
        fetch(repo, "origin", branch)?;
    }
    Ok(())
}

fn revalidate_start(repo: &Path, owner: &GitOperationOwner) -> OpsResult<()> {
    revalidate_owned_head(repo, &owner.branch, &owner.head)
}

fn revalidate_owned_head(repo: &Path, expected_branch: &str, expected_head: &str) -> OpsResult<()> {
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "refusing to start the owned rebase: a {state} operation appeared during preparation"
        )));
    }
    let branch = current_branch(repo)?.ok_or_else(|| {
        OpsError::Message("refusing to start the owned rebase from detached HEAD".to_string())
    })?;
    let head = rev_parse(repo, "HEAD")?;
    if branch != expected_branch || head != expected_head {
        return Err(OpsError::Message(format!(
            "refusing to start the owned rebase: branch/HEAD changed during preparation (expected {} at {}, found {branch} at {head})",
            expected_branch, expected_head
        )));
    }
    Ok(())
}

/// Replace disposable checkpoint history with one commit whose tree exactly
/// matches the scratch-cleared source tree. The recovery ref preserves the
/// original head before any rewrite; the returned fork drives the one-commit
/// final replay onto the pinned target.
fn collapse_authored_history(
    repo: &Path,
    owner: &GitOperationOwner,
    recorded_fork: Option<&str>,
    target_sha: &str,
    progress: &impl Progress,
) -> OpsResult<String> {
    let fork = match recorded_fork {
        Some(fork) => {
            if !crate::engine::git::is_ancestor(repo, fork, "HEAD")? {
                return Err(OpsError::UnsafeRebaseBase {
                    base: fork.to_string(),
                    commits: git(repo, &["log", "--oneline", &format!("{fork}..HEAD")])?,
                });
            }
            fork.to_string()
        }
        None => crate::engine::git::merge_base(repo, target_sha, "HEAD")?,
    };
    let source_tree = rev_parse(repo, "HEAD^{tree}")?;
    let fork_tree = rev_parse(repo, &format!("{fork}^{{tree}}"))?;
    if source_tree == fork_tree {
        return Err(OpsError::Message(
            "no authored changes remain after clearing scratch; refusing final history collapse before rewriting or pushing"
                .to_string(),
        ));
    }
    let safe_branch = owner
        .branch
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    let recovery_ref = format!("refs/loopflow/recovery/{safe_branch}-{}", owner.id());
    git(repo, &["update-ref", &recovery_ref, &owner.head])?;

    progress.status("Collapsing authored history for final integration...");
    git(repo, &["reset", "--soft", &fork])?;
    git(
        repo,
        &["commit", "-m", "lf land: collapse authored history"],
    )?;
    let collapsed_tree = rev_parse(repo, "HEAD^{tree}")?;
    if collapsed_tree != source_tree {
        return Err(OpsError::Message(format!(
            "final history collapse changed the source tree: expected {source_tree}, found {collapsed_tree}; recover from {recovery_ref}"
        )));
    }
    Ok(fork)
}

fn tracked_dirty_state(repo: &Path) -> OpsResult<BTreeSet<String>> {
    Ok(
        git(repo, &["status", "--porcelain", "--untracked-files=no"])?
            .lines()
            .map(str::to_string)
            .collect(),
    )
}

fn cherry_authored_count(repo: &Path, target_sha: &str, head: &str) -> OpsResult<usize> {
    Ok(git(repo, &["cherry", target_sha, head])?
        .lines()
        .filter(|line| line.starts_with("+ "))
        .count())
}

fn push_rebased_branch(repo: &Path, branch: &str) -> OpsResult<()> {
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
    let args = if upstream.status.success() {
        vec!["push", "--force-with-lease"]
    } else {
        vec!["push", "--force-with-lease", "-u", "origin", branch]
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(&args)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

/// Validate a durable stacked fork base before it drives a `git rebase --onto`.
///
/// The base must be an ancestor of HEAD; only then does replaying
/// `fork_base..HEAD` preserve exactly the child-authored commits. If the base
/// diverged from HEAD (the child was itself rewritten, or the base is
/// unreachable), refuse rather than silently rewrite history, and name the
/// commits since the common ancestor so a human can reconcile.
fn resolve_fork_point(repo: &Path, options: &RebaseOptions) -> OpsResult<Option<String>> {
    let Some(base) = options.fork_base.as_deref() else {
        return Ok(None);
    };
    if crate::engine::git::is_ancestor(repo, base, "HEAD")? {
        return Ok(Some(base.to_string()));
    }
    let merge_base =
        crate::engine::git::merge_base(repo, base, "HEAD").unwrap_or_else(|_| base.to_string());
    let commits =
        git(repo, &["log", "--oneline", &format!("{merge_base}..HEAD")]).unwrap_or_default();
    Err(OpsError::UnsafeRebaseBase {
        base: base.to_string(),
        commits,
    })
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

fn reset_to_base(repo: &Path, plan: &RebasePlan, progress: &impl Progress) -> OpsResult<()> {
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
