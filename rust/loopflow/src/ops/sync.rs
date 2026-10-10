use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::git::{
    abort_merge, continue_merge, current_branch, fetch, get_default_branch, intervention_state,
    merge, rerere_remaining, rev_parse,
};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::git_operation::{
    authorize_sync_control, begin_sync_operation, GitOperationOwner, SyncOperation,
};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub onto: String,
    pub push: bool,
    /// The recorded parent tip used as the merge base after squash landing.
    pub fork_base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncClass {
    StaleEmpty,
    ScratchOnly,
    GeneratedOnly,
    CleanAuthored,
    Protected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStrategy {
    Noop,
    ResetToBase,
    MergeTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncPlan {
    pub branch: String,
    pub base_ref: String,
    /// The recorded stack base shared by planning, integration and the PR range.
    pub fork_base: Option<String>,
    pub class: SyncClass,
    pub strategy: SyncStrategy,
    pub unique_commits: usize,
    pub changed_files: Vec<PathBuf>,
    pub scratch_stashed: bool,
}

#[derive(Debug)]
struct SyncExpectation {
    strategy: SyncStrategy,
    tracked_dirty: BTreeSet<String>,
    stacked: bool,
    require_changes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncVerification {
    pub branch: String,
    pub head: String,
    pub target_sha: String,
    pub unique_commits: usize,
}

#[derive(Debug)]
pub struct SyncRecovery {
    operation: SyncOperation,
    expected: SyncExpectation,
    push: bool,
}

pub fn plan_sync(
    repo: &Path,
    onto: Option<&str>,
    fork_base: Option<String>,
) -> OpsResult<SyncPlan> {
    let default_branch = get_default_branch(repo)?;
    let branch = current_branch(repo)?.unwrap_or_else(|| "HEAD".to_string());
    let base_ref = if let Some(onto) = onto {
        onto.to_string()
    } else {
        format!("origin/{default_branch}")
    };

    let comparison_base = fork_base.as_deref().unwrap_or(&base_ref);
    let unique_commits = count_unique_commits(repo, comparison_base)?;
    let mut changed_files = diff_names(repo, comparison_base)?;
    for path in dirty_paths(repo)? {
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
        let strategy = if crate::git::is_ancestor(repo, &base_ref, "HEAD")? {
            SyncStrategy::Noop
        } else {
            SyncStrategy::MergeTarget
        };
        (SyncClass::Protected, strategy)
    } else if protected {
        (SyncClass::Protected, SyncStrategy::MergeTarget)
    } else if unique_commits == 0 && scratch_only {
        (SyncClass::ScratchOnly, SyncStrategy::ResetToBase)
    } else if unique_commits == 0 && changed_files.is_empty() {
        (SyncClass::StaleEmpty, SyncStrategy::ResetToBase)
    } else if has_non_scratch_changes {
        (SyncClass::CleanAuthored, SyncStrategy::MergeTarget)
    } else if scratch_only || generated_only(repo, comparison_base)? {
        (SyncClass::GeneratedOnly, SyncStrategy::ResetToBase)
    } else {
        (SyncClass::CleanAuthored, SyncStrategy::MergeTarget)
    };
    // A stacked child's scratch deletion is intentional history. Resetting to
    // the parent would discard it and copy the parent's notes back into the child.
    let persistent = crate::git::worktrees::is_persistent_worktree(repo)?;
    let strategy = if fork_base.is_some() || persistent {
        if crate::git::is_ancestor(repo, &base_ref, "HEAD")? {
            SyncStrategy::Noop
        } else if fork_base.is_none() && landed(repo, &base_ref)? {
            // A persistent branch outlives its squash-merged PRs. Once the base
            // holds everything it committed, restart it from the base instead
            // of carrying the merged commits into every later PR.
            SyncStrategy::ResetToBase
        } else {
            SyncStrategy::MergeTarget
        }
    } else {
        strategy
    };

    let scratch_stashed = !persistent
        && matches!(strategy, SyncStrategy::ResetToBase)
        && repo.join("scratch").exists();

    Ok(SyncPlan {
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

pub fn sync_with_recovery(
    repo: &Path,
    options: &SyncOptions,
    progress: &impl Progress,
) -> OpsResult<SyncVerification> {
    let default = get_default_branch(repo)?;
    if current_branch(repo)?.as_deref() == Some(&default)
        && options.onto == format!("origin/{default}")
    {
        crate::ops::checkout::refresh_main(repo, progress)?;
        let target_sha = rev_parse(repo, &options.onto)?;
        return Ok(SyncVerification {
            branch: default,
            head: rev_parse(repo, "HEAD")?,
            unique_commits: count_unique_commits(repo, &target_sha)?,
            target_sha,
        });
    }
    start_owned_sync(repo, options, false, progress)
}

/// Delivery must not publish an empty tree, even for an unmanaged branch.
pub(crate) fn sync_for_delivery(
    repo: &Path,
    options: &SyncOptions,
    progress: &impl Progress,
) -> OpsResult<SyncVerification> {
    start_owned_sync(repo, options, true, progress)
}

/// Continue a local sync after its conflict paths have been resolved.
pub fn continue_sync_for_resolution(repo: &Path, adopt: bool) -> OpsResult<()> {
    continue_sync_after_authorization(repo, adopt, || Ok(()))
}

pub(crate) fn continue_sync_after_authorization(
    repo: &Path,
    adopt: bool,
    before_mutation: impl FnOnce() -> OpsResult<()>,
) -> OpsResult<()> {
    let authorization = authorize_sync_control(repo, adopt)?;
    before_mutation()?;
    let result = continue_merge(repo, authorization.owner().target_sha.as_deref())?;
    if result.success {
        verify_control_completion(repo, authorization.owner())?;
        record_control_task_state(repo, authorization.owner())?;
        authorization.complete()?;
        return Ok(());
    }
    Err(OpsError::SyncConflict {
        onto: authorization.owner().target_ref.clone(),
        detail: conflict_detail(result.conflicts),
        recovery: None,
    })
}

/// Abort a local sync that was left open for inline resolution.
pub fn abort_sync_for_resolution(repo: &Path, adopt: bool) -> OpsResult<()> {
    abort_sync_after_authorization(repo, adopt, || Ok(()))
}

pub(crate) fn abort_sync_after_authorization(
    repo: &Path,
    adopt: bool,
    before_mutation: impl FnOnce() -> OpsResult<()>,
) -> OpsResult<()> {
    let authorization = authorize_sync_control(repo, adopt)?;
    before_mutation()?;
    abort_merge(repo)?;
    authorization.complete()?;
    Ok(())
}

pub fn recover_sync(
    recovery: SyncRecovery,
    launch: impl FnOnce(&BTreeMap<String, String>) -> OpsResult<()>,
) -> OpsResult<SyncVerification> {
    let worktree = recovery.operation.owner().worktree.clone();
    launch(&recovery.operation.scoped_env())?;
    finish_sync(
        &worktree,
        recovery.operation,
        &recovery.expected,
        recovery.push,
    )
}

fn start_owned_sync(
    repo: &Path,
    options: &SyncOptions,
    require_changes: bool,
    progress: &impl Progress,
) -> OpsResult<SyncVerification> {
    let mut operation = begin_sync_operation(repo, &options.onto)?;
    let tracked_dirty = tracked_dirty_state(repo)?;
    fetch_target(repo, &options.onto)?;
    let target_sha = rev_parse(repo, &options.onto)?;
    let plan = plan_sync(repo, Some(&options.onto), options.fork_base.clone())?;
    operation.pin_target(target_sha.clone())?;
    revalidate_start(repo, operation.owner())?;

    let expected = SyncExpectation {
        strategy: plan.strategy.clone(),
        tracked_dirty,
        stacked: options.fork_base.is_some(),
        require_changes,
    };
    validate_fork_base(repo, options.fork_base.as_deref())?;

    if matches!(plan.strategy, SyncStrategy::ResetToBase) {
        reset_to_base(repo, &plan, progress)?;
        return finish_sync(repo, operation, &expected, options.push);
    }
    if matches!(plan.strategy, SyncStrategy::Noop) {
        return finish_sync(repo, operation, &expected, false);
    }

    progress.status(&format!("Merging {}...", options.onto));
    let result = merge(repo, &target_sha, options.fork_base.as_deref())?;
    if result.success {
        return finish_sync(repo, operation, &expected, options.push);
    }
    if intervention_state(repo)? != Some("merge") {
        operation.complete()?;
        return Err(OpsError::Message(format!(
            "git merge of {} stopped before creating a recoverable sequencer",
            options.onto
        )));
    }

    if continue_reused_resolutions(repo, progress)? {
        progress.status("Reused recorded conflict resolution");
        return finish_sync(repo, operation, &expected, options.push);
    }

    Err(OpsError::SyncConflict {
        onto: options.onto.clone(),
        detail: conflict_detail(result.conflicts),
        recovery: Some(Box::new(SyncRecovery {
            operation,
            expected,
            push: options.push,
        })),
    })
}

/// Complete a merge when rerere populated every unresolved path.
fn continue_reused_resolutions(repo: &Path, progress: &impl Progress) -> OpsResult<bool> {
    let conflicts = git(repo, &["diff", "--diff-filter=U", "--name-only"])?;
    if conflicts.trim().is_empty() || !rerere_remaining(repo)?.is_empty() {
        return Ok(false);
    }
    progress.status("Applying recorded conflict resolution...");
    Ok(continue_merge(repo, None)?.success)
}

fn finish_sync(
    repo: &Path,
    operation: SyncOperation,
    expected: &SyncExpectation,
    push: bool,
) -> OpsResult<SyncVerification> {
    let verification = verify_sync(repo, operation.owner(), expected)?;
    if !expected.stacked {
        crate::ops::task::validate_task_pr_range_for_integration(
            repo,
            &operation.owner().target_ref,
            &verification.target_sha,
        )?;
    }
    if push {
        // Sync owns a force-push path rather than the ordinary commit helper,
        // but it must cross the same Task settlement fence first.
        let _mutation = crate::ops::task::lock_task_pr_mutation(repo)?;
        crate::ops::task::clear_task_pr_merge_before_head_mutation(repo, false, &|_| {})?;
        push_synced_branch(repo, &verification.branch)?;
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

fn verify_sync(
    repo: &Path,
    owner: &GitOperationOwner,
    expected: &SyncExpectation,
) -> OpsResult<SyncVerification> {
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "sync incomplete: Git still reports an active {state} operation"
        )));
    }
    let branch = current_branch(repo)?
        .ok_or_else(|| OpsError::Message("sync incomplete: HEAD is detached".to_string()))?;
    if branch != owner.branch {
        return Err(OpsError::Message(format!(
            "sync incomplete: expected branch {}, found {branch}",
            owner.branch
        )));
    }
    let head = rev_parse(repo, "HEAD")?;
    let target_sha = owner.target_sha.as_deref().ok_or_else(|| {
        OpsError::Message("sync verification has no pinned target commit".to_string())
    })?;
    if !crate::git::is_ancestor(repo, target_sha, &head)? {
        return Err(OpsError::Message(format!(
            "sync incomplete: pinned target {target_sha} is not an ancestor of HEAD {head}"
        )));
    }
    let conflicts = git(repo, &["diff", "--diff-filter=U", "--name-only"])?;
    if !conflicts.trim().is_empty() {
        return Err(OpsError::Message(format!(
            "sync incomplete: unresolved paths remain: {}",
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
            "sync incomplete: new tracked dirty state remains: {}",
            introduced.join(", ")
        )));
    }
    let unique_commits = count_unique_commits(repo, target_sha)?;
    if matches!(expected.strategy, SyncStrategy::MergeTarget)
        && !crate::git::is_ancestor(repo, &owner.head, &head)?
    {
        return Err(OpsError::Message(
            "sync lost the original branch history".to_string(),
        ));
    }
    if expected.require_changes
        && rev_parse(repo, "HEAD^{tree}")? == rev_parse(repo, &format!("{target_sha}^{{tree}}"))?
    {
        return Err(OpsError::Message(
            "no authored changes remain after integration; refusing to publish an empty PR"
                .to_string(),
        ));
    }
    Ok(SyncVerification {
        branch,
        head,
        target_sha: target_sha.to_string(),
        unique_commits,
    })
}

fn verify_control_completion(repo: &Path, owner: &GitOperationOwner) -> OpsResult<()> {
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "sync incomplete: Git still reports an active {state} operation"
        )));
    }
    if owner.branch != "HEAD" {
        let branch = current_branch(repo)?
            .ok_or_else(|| OpsError::Message("sync incomplete: HEAD is detached".to_string()))?;
        if branch != owner.branch {
            return Err(OpsError::Message(format!(
                "sync incomplete: expected branch {}, found {branch}",
                owner.branch
            )));
        }
    }
    if let Some(target_sha) = owner.target_sha.as_deref() {
        if !crate::git::is_ancestor(repo, target_sha, "HEAD")? {
            return Err(OpsError::Message(format!(
                "sync incomplete: pinned target {target_sha} is not an ancestor of HEAD"
            )));
        }
        let introduced = tracked_dirty_state(repo)?;
        if !introduced.is_empty() {
            return Err(OpsError::Message(format!(
                "sync incomplete: tracked dirty state remains: {}",
                introduced.into_iter().collect::<Vec<_>>().join(", ")
            )));
        }
        if !crate::git::is_ancestor(repo, &owner.head, "HEAD")? {
            return Err(OpsError::Message(
                "sync lost the original branch history".to_string(),
            ));
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
        return crate::ops::task::record_stack_sync(&stacked, target_sha, clear_parent);
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
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "refusing to start the owned sync: a {state} operation appeared during preparation"
        )));
    }
    let branch = current_branch(repo)?.ok_or_else(|| {
        OpsError::Message("refusing to start the owned sync from detached HEAD".to_string())
    })?;
    let head = rev_parse(repo, "HEAD")?;
    if branch != owner.branch || head != owner.head {
        return Err(OpsError::Message(format!(
            "refusing to start the owned sync: branch/HEAD changed during preparation (expected {} at {}, found {branch} at {head})",
            owner.branch, owner.head
        )));
    }
    Ok(())
}

fn tracked_dirty_state(repo: &Path) -> OpsResult<BTreeSet<String>> {
    Ok(
        git(repo, &["status", "--porcelain", "--untracked-files=no"])?
            .lines()
            .map(str::to_string)
            .collect(),
    )
}

fn push_synced_branch(repo: &Path, branch: &str) -> OpsResult<()> {
    let reference = format!("refs/heads/{branch}");
    let remote = git(repo, &["ls-remote", "--heads", "origin", &reference])?;
    let lease = if remote.trim().is_empty() {
        // Deletion leaves a stale tracking ref. Require absence atomically so a
        // concurrently recreated branch cannot be overwritten.
        format!("--force-with-lease={reference}:")
    } else {
        "--force-with-lease".to_string()
    };
    git(repo, &["push", &lease, "-u", "origin", &reference]).map(|_| ())
}

/// Validate the recorded comparison base before merging a squash-landed parent.
fn validate_fork_base(repo: &Path, fork_base: Option<&str>) -> OpsResult<()> {
    let Some(base) = fork_base else {
        return Ok(());
    };
    if crate::git::is_ancestor(repo, base, "HEAD")? {
        return Ok(());
    }
    let merge_base =
        crate::git::merge_base(repo, base, "HEAD").unwrap_or_else(|_| base.to_string());
    let commits =
        git(repo, &["log", "--oneline", &format!("{merge_base}..HEAD")]).unwrap_or_default();
    Err(OpsError::UnsafeSyncBase {
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

/// True when merging the base into `rev` would add nothing the base lacks.
fn landed_at(repo: &Path, base_ref: &str, rev: &str) -> OpsResult<bool> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["merge-tree", "--write-tree", base_ref, rev])
        .output()?;
    // A conflicted merge exits 1: the branch still differs from the base.
    Ok(output.status.success()
        && String::from_utf8_lossy(&output.stdout).trim()
            == rev_parse(repo, &format!("{base_ref}^{{tree}}"))?)
}

fn landed(repo: &Path, base_ref: &str) -> OpsResult<bool> {
    landed_at(repo, base_ref, "HEAD")
}

/// Recreate `commits` on top of `onto` without touching the working tree.
/// Returns None when one of them conflicts with the new base.
fn replay(repo: &Path, onto: &str, commits: &str) -> OpsResult<Option<String>> {
    let mut parent = rev_parse(repo, onto)?;
    // A merge's first-parent delta includes both the incoming changes and any
    // resolution authored in the merge itself. Replaying its side commits
    // separately would lose those resolutions or reintroduce discarded edits.
    let range = git(repo, &["rev-list", "--reverse", "--first-parent", commits])?;
    for commit in range.lines() {
        let merged = Command::new("git")
            .current_dir(repo)
            .args(["merge-tree", "--write-tree", "--merge-base"])
            .args([&format!("{commit}^"), &parent, commit])
            .output()?;
        if !merged.status.success() {
            return Ok(None);
        }
        let tree = String::from_utf8_lossy(&merged.stdout).trim().to_string();
        if tree == rev_parse(repo, &format!("{parent}^{{tree}}"))? {
            continue;
        }
        let author = git(repo, &["log", "-1", "--format=%an%n%ae%n%aI", commit])?;
        let mut author = author.lines();
        let message = git(repo, &["log", "-1", "--format=%B", commit])?;
        let created = Command::new("git")
            .current_dir(repo)
            .env("GIT_AUTHOR_NAME", author.next().unwrap_or_default())
            .env("GIT_AUTHOR_EMAIL", author.next().unwrap_or_default())
            .env("GIT_AUTHOR_DATE", author.next().unwrap_or_default())
            .args([
                "commit-tree",
                &tree,
                "-p",
                &parent,
                "-m",
                message.trim_end(),
            ])
            .output()?;
        if !created.status.success() {
            return Err(OpsError::Message(
                String::from_utf8_lossy(&created.stderr).into_owned(),
            ));
        }
        parent = String::from_utf8_lossy(&created.stdout).trim().to_string();
    }
    Ok(Some(parent))
}

/// Restart a persistent branch from the default branch once its PR has merged.
/// Commits made after the last push are carried over. Returns false and
/// leaves the checkout alone when nothing has landed or Git is mid-operation.
pub(crate) fn restart_landed_persistent(repo: &Path) -> OpsResult<bool> {
    let base = format!("origin/{}", get_default_branch(repo)?);
    // Offline, the last fetched base still answers for what it has seen.
    let _ = fetch_target(repo, &base);
    if rev_parse(repo, &base).is_err() {
        return Ok(false);
    }
    if intervention_state(repo)?.is_some() || crate::git::is_ancestor(repo, "HEAD", &base)? {
        return Ok(false);
    }
    let target = if landed(repo, &base)? {
        Some(base.clone())
    } else {
        // After a squash merge the pushed head is what landed; anything
        // committed since then is still unpublished work.
        let branch = current_branch(repo)?.unwrap_or_default();
        match rev_parse(repo, &format!("refs/remotes/origin/{branch}")) {
            Ok(pushed)
                if crate::git::is_ancestor(repo, &pushed, "HEAD")?
                    && landed_at(repo, &base, &pushed)? =>
            {
                replay(repo, &base, &format!("{pushed}..HEAD"))?
            }
            _ => None,
        }
    };
    let Some(target) = target else {
        return Ok(false);
    };
    // Reset clears staging even with --keep. Preserve the caller's index and
    // working edits through the same recovery boundary as explicit sync.
    crate::ops::checkout::with_preserved_edits(repo, || {
        git(repo, &["reset", "--keep", &target]).map(|_| ())
    })?;
    Ok(true)
}

fn reset_to_base(repo: &Path, plan: &SyncPlan, progress: &impl Progress) -> OpsResult<()> {
    if !plan.scratch_stashed && crate::git::worktrees::is_persistent_worktree(repo)? {
        progress.status(&format!(
            "{} has landed; restarting it from {}...",
            plan.branch, plan.base_ref
        ));
        // --keep moves the branch without touching uncommitted edits, and
        // refuses rather than overwriting one the base also changed.
        git(repo, &["reset", "--keep", &plan.base_ref])?;
        return Ok(());
    }
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
    Ok(crate::git::git_stdout(repo, args)?.trim().to_string())
}

fn count_unique_commits(repo: &Path, base_ref: &str) -> OpsResult<usize> {
    let stdout = git(repo, &["rev-list", "--count", &format!("{base_ref}..HEAD")])?;
    Ok(stdout.trim().parse().unwrap_or(0))
}

fn diff_names(repo: &Path, base_ref: &str) -> OpsResult<Vec<PathBuf>> {
    // Three-dot: diff from the merge-base, so we see only what this branch
    // authored — not files the base advanced past us. A stale branch (the whole
    // reason to sync) would otherwise report the base's new files as its own,
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
        return Err(OpsError::Git(crate::error::GitError::CommandFailed {
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
        || path == Path::new(".lf/config.yaml")
}

pub fn sync_class_name(class: &SyncClass) -> &'static str {
    match class {
        SyncClass::StaleEmpty => "stale_empty",
        SyncClass::ScratchOnly => "scratch_only",
        SyncClass::GeneratedOnly => "generated_only",
        SyncClass::CleanAuthored => "clean_authored",
        SyncClass::Protected => "protected",
    }
}

pub fn sync_strategy_name(strategy: &SyncStrategy) -> &'static str {
    match strategy {
        SyncStrategy::Noop => "noop",
        SyncStrategy::ResetToBase => "reset_to_base",
        SyncStrategy::MergeTarget => "merge_target",
    }
}

#[cfg(test)]
mod tests {
    use super::{restart_landed_persistent, scratch_stash_path};
    use std::path::Path;

    fn git(repo: &Path, args: &[&str]) -> String {
        crate::git::git_stdout(repo, args).unwrap()
    }

    #[test]
    fn merged_persistent_branch_restarts_from_main_and_keeps_local_files() {
        let repo = loopflow_test_support::TestRepo::new();
        let persistent = crate::git::worktrees::ensure_agent_worktree(
            repo.path(),
            crate::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
        )
        .unwrap();
        std::fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
        crate::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
            .unwrap();
        std::fs::create_dir_all(persistent.path.join("scratch")).unwrap();
        std::fs::write(persistent.path.join("scratch/plan.md"), "private\n").unwrap();

        // Unmerged commits: the checkout is left alone.
        let before = git(&persistent.path, &["rev-parse", "HEAD"]);
        assert!(!restart_landed_persistent(&persistent.path).unwrap());
        assert_eq!(git(&persistent.path, &["rev-parse", "HEAD"]), before);

        git(repo.path(), &["merge", "--squash", &persistent.branch]);
        repo.commit("Merged document PR");
        repo.push();
        std::fs::write(persistent.path.join("memory.md"), "followup\n").unwrap();

        assert!(restart_landed_persistent(&persistent.path).unwrap());
        assert_eq!(
            git(&persistent.path, &["rev-parse", "HEAD"]),
            git(&persistent.path, &["rev-parse", "origin/main"])
        );
        assert_eq!(
            std::fs::read_to_string(persistent.path.join("memory.md")).unwrap(),
            "followup\n"
        );
        assert!(persistent.path.join("scratch/plan.md").exists());
    }

    #[test]
    fn commits_made_after_the_merged_push_move_onto_main() {
        let repo = loopflow_test_support::TestRepo::new();
        let persistent = crate::git::worktrees::ensure_agent_worktree(
            repo.path(),
            crate::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
        )
        .unwrap();
        std::fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
        crate::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
            .unwrap();
        git(&persistent.path, &["push", "origin", &persistent.branch]);
        std::fs::write(persistent.path.join("later.md"), "unpublished\n").unwrap();
        crate::ops::commit_selected(&persistent.path, &["later.md".into()], Some("Later")).unwrap();
        git(
            repo.path(),
            &["merge", "--squash", &format!("{}~1", persistent.branch)],
        );
        repo.commit("Merged document PR");
        repo.push();
        std::fs::write(persistent.path.join("memory.md"), "local edit\n").unwrap();
        git(&persistent.path, &["add", "memory.md"]);
        std::fs::write(persistent.path.join("memory.md"), "later local edit\n").unwrap();
        let staged = git(&persistent.path, &["diff", "--cached"]);
        let unstaged = git(&persistent.path, &["diff"]);

        assert!(restart_landed_persistent(&persistent.path).unwrap());
        assert_eq!(git(&persistent.path, &["diff", "--cached"]), staged);
        assert_eq!(git(&persistent.path, &["diff"]), unstaged);
        assert_eq!(
            git(
                &persistent.path,
                &["log", "--format=%s", "origin/main..HEAD"]
            ),
            "Later\n"
        );
        assert_eq!(
            git(&persistent.path, &["show", "HEAD:later.md"]),
            "unpublished\n"
        );
        assert_eq!(
            std::fs::read_to_string(persistent.path.join("memory.md")).unwrap(),
            "later local edit\n"
        );
    }

    #[test]
    fn unpublished_merge_results_survive_persistent_restart() {
        let repo = loopflow_test_support::TestRepo::new();
        let persistent = crate::git::worktrees::ensure_agent_worktree(
            repo.path(),
            crate::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
        )
        .unwrap();
        std::fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
        crate::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
            .unwrap();
        git(&persistent.path, &["push", "origin", &persistent.branch]);
        let pushed = git(&persistent.path, &["rev-parse", "HEAD"]);

        git(&persistent.path, &["checkout", "-b", "side"]);
        std::fs::write(persistent.path.join("side.md"), "incoming\n").unwrap();
        git(&persistent.path, &["add", "side.md"]);
        git(&persistent.path, &["commit", "-m", "Side document"]);
        git(&persistent.path, &["checkout", &persistent.branch]);
        git(
            &persistent.path,
            &["merge", "--no-ff", "--no-commit", "side"],
        );
        std::fs::write(persistent.path.join("side.md"), "resolved in merge\n").unwrap();
        std::fs::write(
            persistent.path.join("resolution.md"),
            "merge-only decision\n",
        )
        .unwrap();
        git(&persistent.path, &["add", "."]);
        git(&persistent.path, &["commit", "-m", "Resolve documents"]);
        let tree = git(&persistent.path, &["rev-parse", "HEAD^{tree}"]);

        git(repo.path(), &["merge", "--squash", pushed.trim()]);
        repo.commit("Merged document PR");
        std::fs::write(repo.path().join("upstream.md"), "new upstream\n").unwrap();
        repo.stage_all();
        repo.commit("Upstream document");
        repo.push();
        std::fs::create_dir_all(persistent.path.join("scratch")).unwrap();
        std::fs::write(persistent.path.join("scratch/plan.md"), "local plan\n").unwrap();

        assert!(restart_landed_persistent(&persistent.path).unwrap());
        assert_eq!(
            git(&persistent.path, &["show", "HEAD:side.md"]),
            "resolved in merge\n"
        );
        assert_eq!(
            git(&persistent.path, &["show", "HEAD:resolution.md"]),
            "merge-only decision\n"
        );
        assert_eq!(
            git(&persistent.path, &["show", "HEAD:upstream.md"]),
            "new upstream\n"
        );
        assert_eq!(
            git(
                &persistent.path,
                &["diff", "--name-only", tree.trim(), "HEAD"]
            ),
            "upstream.md\n"
        );
        assert_eq!(
            std::fs::read_to_string(persistent.path.join("scratch/plan.md")).unwrap(),
            "local plan\n"
        );
    }

    #[test]
    fn persistent_commit_after_merge_starts_from_main() {
        for selected in [true, false] {
            let repo = loopflow_test_support::TestRepo::new();
            let persistent = crate::git::worktrees::ensure_agent_worktree(
                repo.path(),
                crate::git::worktrees::WorktreeSegment::parse("repo").unwrap(),
            )
            .unwrap();
            std::fs::write(persistent.path.join("memory.md"), "accepted\n").unwrap();
            crate::ops::commit_selected(&persistent.path, &["memory.md".into()], Some("Memory"))
                .unwrap();
            git(repo.path(), &["merge", "--squash", &persistent.branch]);
            repo.commit("Merged document PR");
            repo.push();

            // No sync and no settled landing: the next commit notices by itself.
            std::fs::write(persistent.path.join("memory.md"), "followup\n").unwrap();
            if selected {
                crate::ops::commit_selected(
                    &persistent.path,
                    &["memory.md".into()],
                    Some("Followup"),
                )
                .unwrap();
            } else {
                git(&persistent.path, &["add", "memory.md"]);
                crate::ops::commit_workflow(
                    &persistent.path,
                    &crate::ops::CommitOptions {
                        message: Some("Followup".into()),
                        ..crate::ops::CommitOptions::for_task("commit")
                    },
                    &crate::ops::NullProgress,
                    &|_| {},
                )
                .unwrap();
            }
            assert_eq!(
                git(
                    &persistent.path,
                    &["rev-list", "--count", "origin/main..HEAD"]
                )
                .trim(),
                "1"
            );
            assert_eq!(
                git(&persistent.path, &["show", "HEAD:memory.md"]),
                "followup\n"
            );
        }
    }

    #[test]
    fn scratch_stash_lands_under_the_ignored_tmp_prefix() {
        let path = scratch_stash_path(Path::new("/repo"), "jack/reconcile-out-of-band-merges");
        // .lf/tmp/ is gitignored; a sibling like .lf/scratch-stash/ would dirty
        // the worktree and block the Task from reaching `end`.
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
