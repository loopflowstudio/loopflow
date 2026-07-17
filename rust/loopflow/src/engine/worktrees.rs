use crate::engine::error::GitError;
use crate::engine::git::{
    delete_local_branch, get_default_branch, has_commits_beyond, is_ancestor,
    is_clean_ignoring_scratch, is_squash_merged, rev_parse, sync_main, worktree_add,
    worktree_remove, WorktreeBranch,
};
use crate::engine::identity::WorktreeName;
use crate::engine::naming::git_user;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSegment(String);

impl WorktreeSegment {
    pub fn parse(raw: &str) -> Result<Self, PlacementError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PlacementError::EmptySegment);
        }
        // Keep the sibling suffix and branch leaf unambiguous and shell-safe.
        if trimmed.contains('.') {
            return Err(PlacementError::DotsReserved(trimmed.to_string()));
        }
        Ok(Self(crate::engine::naming::sanitize_for_branch(trimmed)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementStrategy {
    Create,
    CheckoutExisting,
    UseExistingWorktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlacementPlan {
    pub base_ref: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub strategy: PlacementStrategy,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlacementError {
    #[error("worktree segment cannot be empty")]
    EmptySegment,
    #[error("\"{0}\" is not a flat worktree name. Use a hyphen instead of a dot.")]
    DotsReserved(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub squash_merged: bool,
    pub prunable: bool,
    /// No commits beyond main and not merged — a brand new worktree.
    pub fresh: bool,
    pub dirty: bool,
    pub remote_gone: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorktreePruneReason {
    Merged,
    SquashMerged,
    RemoteGone,
    Fresh,
    Unprotected,
    Terminal,
}

impl WorktreePruneReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Merged => "merged",
            Self::SquashMerged => "squash-merged",
            Self::RemoteGone => "remote-gone",
            Self::Fresh => "fresh",
            Self::Unprotected => "unprotected",
            Self::Terminal => "terminal",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreePruneTarget {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub reason: WorktreePruneReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreePruneFailure {
    pub target: WorktreePruneTarget,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorktreePruneReport {
    pub candidates: Vec<WorktreePruneTarget>,
    pub removed: Vec<WorktreePruneTarget>,
    pub retained_dirty: Vec<PathBuf>,
    pub failed: Vec<WorktreePruneFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorktreePrunePolicy {
    pub remove_unprotected: bool,
}

impl WorktreePrunePolicy {
    pub fn manual() -> Self {
        Self {
            remove_unprotected: true,
        }
    }

    pub fn automatic() -> Self {
        Self {
            remove_unprotected: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetedPruneOutcome {
    Removed(WorktreePruneTarget),
    RetainedDirty(PathBuf),
    Protected,
    NotFound,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateWorktreeResult {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
}

pub fn main_repo_root(repo: &Path) -> Result<PathBuf, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git rev-parse --git-common-dir".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let common_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let common_path = PathBuf::from(common_dir);
    let repo_root =
        common_path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| GitError::CommandFailed {
                command: "git rev-parse --git-common-dir".to_string(),
                stderr: "unable to resolve common dir parent".to_string(),
            })?;
    Ok(repo_root)
}

/// The worktree directory for an identity: `<parent>/<repo>.<dir_component>`.
/// The `/`-scoped branch never reaches disk — only the flat dir component does.
pub fn worktree_dir(repo: &Path, id: &WorktreeName) -> PathBuf {
    dir_for_component(repo, id.dir_component())
}

fn dir_for_component(repo: &Path, component: &str) -> PathBuf {
    let repo_root = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    repo_root
        .parent()
        .unwrap_or(repo_root.as_path())
        .join(format!("{repo_name}.{component}"))
}

/// The worktree directory for a branch or local name. Invalid flat names use a
/// neutral fallback; callers that create worktrees validate the segment first.
pub fn worktree_path(repo: &Path, name: &str) -> PathBuf {
    let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
    let component = WorktreeName::parse(name, &user)
        .map(|id| id.dir_component().to_string())
        .unwrap_or_else(|| "worktree".to_string());
    dir_for_component(repo, &component)
}

/// Short execution id: the leading 8 hex chars of a trace UUID.
pub fn short_run_id(run_id: &str) -> String {
    let hex: String = run_id
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(8)
        .collect();
    if hex.len() == 8 {
        hex
    } else {
        short_hash(run_id, 8)
    }
}

fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}

/// Extract the suffix from a named sibling worktree directory.
///
/// Given a path like `../loopflow.my-feature`, returns `Some("my-feature")`.
/// Returns `None` if not in a worktree (i.e., in the main repo).
pub fn sibling_worktree_name(repo: &Path) -> Option<String> {
    let main_repo = main_repo_root(repo).ok()?;
    sibling_worktree_name_with_main(repo, &main_repo)
}

/// Extract the sibling suffix using an already-resolved main repo path.
///
/// Use this to avoid repeatedly shelling out to git when iterating many worktrees.
/// Only recognizes sibling worktrees (e.g., `../repo.feature`) — the worktree must
/// share the same parent directory as the main repo. Worktrees elsewhere (e.g.,
/// `.claude/worktrees/`) return `None`.
pub fn sibling_worktree_name_with_main(repo: &Path, main_repo: &Path) -> Option<String> {
    if repo == main_repo {
        return None;
    }

    // Only recognize sibling worktrees: same parent directory as main repo.
    // Canonicalize to resolve symlinks (macOS: /tmp → /private/tmp).
    let repo_parent = repo.parent()?.canonicalize().ok()?;
    let main_parent = main_repo.parent()?.canonicalize().ok()?;
    if repo_parent != main_parent {
        return None;
    }

    let main_name = main_repo.file_name()?.to_str()?;
    let dir_name = repo.file_name()?.to_str()?;
    let prefix = format!("{main_name}.");
    let short_name = dir_name.strip_prefix(&prefix)?;

    (!short_name.is_empty()).then(|| short_name.to_string())
}

pub fn branch_exists(repo: &Path, branch: &str) -> Result<bool, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", &format!("refs/heads/{branch}")])
        .output()?;
    Ok(output.status.success())
}

pub(crate) fn list_porcelain(repo: &Path) -> Result<Vec<(PathBuf, Option<String>)>, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "list", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: "git worktree list --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut items = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(path) = current_path.take() {
                items.push((path, current_branch.take()));
            }
            current_path = Some(PathBuf::from(path.trim()));
            current_branch = None;
        } else if let Some(branch) = line.strip_prefix("branch ") {
            let branch = branch.trim().strip_prefix("refs/heads/").unwrap_or(branch);
            current_branch = Some(branch.to_string());
        } else if line.trim() == "detached" {
            current_branch = None;
        }
    }

    if let Some(path) = current_path.take() {
        items.push((path, current_branch.take()));
    }

    Ok(items)
}

fn upstream_branch(worktree: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args([
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw.is_empty() {
        return None;
    }
    let branch = raw
        .strip_prefix("origin/")
        .unwrap_or(raw.as_str())
        .to_string();
    Some(branch)
}

/// Parse GitHub owner/repo from the origin remote URL.
pub(crate) fn github_repo_nwo(repo: &Path) -> Option<(String, String)> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // Handle SSH (git@github.com:owner/repo.git) and HTTPS (https://github.com/owner/repo.git)
    let path = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("https://github.com/"))?;
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    Some((owner.to_string(), name.to_string()))
}

/// Check which branches have merged PRs using a single GitHub GraphQL call.
fn merged_pr_branches(repo: &Path, branches: &[String]) -> HashSet<String> {
    if branches.is_empty() {
        return HashSet::new();
    }
    let (owner, name) = match github_repo_nwo(repo) {
        Some(nwo) => nwo,
        None => return HashSet::new(),
    };

    // Build aliased GraphQL query: one field per branch
    let mut fields = String::new();
    for (i, branch) in branches.iter().enumerate() {
        let escaped = branch.replace('\\', "\\\\").replace('"', "\\\"");
        fields.push_str(&format!(
            "b{i}: pullRequests(first: 1, headRefName: \"{escaped}\", states: MERGED) {{ nodes {{ headRefName }} }}\n"
        ));
    }
    let query =
        format!("query {{ repository(owner: \"{owner}\", name: \"{name}\") {{ {fields} }} }}");

    let output = Command::new("gh")
        .current_dir(repo)
        .args(["api", "graphql", "-f", &format!("query={query}")])
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return HashSet::new(),
    };

    // Parse: any headRefName in a non-empty nodes array is a merged branch
    // Simple string scan — avoids adding a JSON parsing dep to this module
    let mut result = HashSet::new();
    for branch in branches {
        if stdout.contains(&format!("\"headRefName\":\"{branch}\"")) {
            result.insert(branch.clone());
        }
    }
    result
}

/// List all remote branch names via a single `git ls-remote --heads origin` call.
/// Returns an empty set on failure (offline, no remote, etc.).
fn list_remote_branches(repo: &Path) -> HashSet<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-remote", "--heads", "origin"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter_map(|line| {
                line.split('\t')
                    .nth(1)?
                    .strip_prefix("refs/heads/")
                    .map(|b| b.to_string())
            })
            .collect(),
        _ => HashSet::new(),
    }
}

/// List worktrees using only local git operations. No network calls.
///
/// Returns `(default_branch, states)` so callers can forward the resolved
/// default branch to `enrich_worktrees_network` without a redundant git call.
///
/// `merged` reflects local detection only (fast-forward ancestor check and
/// squash-merge patch-id comparison). `remote_gone` is always `false`.
/// `prunable` is conservative — network enrichment may mark additional
/// worktrees as prunable.
pub fn list_worktrees_local(repo: &Path) -> Result<(String, Vec<WorktreeState>), GitError> {
    let default_branch = get_default_branch(repo)?;
    let merge_target = format!("origin/{default_branch}");
    let items = list_porcelain(repo)?;

    let branches_to_check: Vec<String> = items
        .iter()
        .filter_map(|(_, branch)| branch.as_ref())
        .filter(|b| *b != &default_branch)
        .cloned()
        .collect();

    // Squash-merge checks: local git operations, one thread per branch
    let repo_for_squash = repo.to_path_buf();
    let target_for_squash = merge_target.clone();
    let squash_handle = thread::spawn(move || {
        let handles: Vec<_> = branches_to_check
            .into_iter()
            .map(|branch| {
                let r = repo_for_squash.clone();
                let t = target_for_squash.clone();
                thread::spawn(move || {
                    if is_squash_merged(&r, &branch, &t).unwrap_or(false) {
                        Some(branch)
                    } else {
                        None
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect::<HashSet<String>>()
    });

    let squash_merged = squash_handle.join().unwrap_or_default();

    let mut results = Vec::new();
    for (path, branch) in items {
        let base = upstream_branch(&path);
        let base_branch = base.filter(|b| b != &default_branch);
        let is_default = branch.as_deref() == Some(&default_branch);
        let has_commits = if is_default {
            true
        } else {
            branch
                .as_deref()
                .map(|b| has_commits_beyond(repo, b, &merge_target).unwrap_or(true))
                .unwrap_or(false)
        };
        let merged = branch.as_deref().is_some_and(|b| {
            !is_default && has_commits && is_ancestor(repo, b, &merge_target).unwrap_or(false)
        });
        let squash_merged_flag = branch
            .as_deref()
            .is_some_and(|b| !is_default && has_commits && squash_merged.contains(b));
        let dirty = !is_clean_ignoring_scratch(&path).unwrap_or(true);
        // "Fresh" means no net content delta against main yet.
        // This includes newly-rotated branches that were forked from a landed
        // branch (commit graph differs, but tree is identical to main).
        let fresh = !is_default && !merged && (!has_commits || squash_merged_flag);
        let prunable = !is_default && (merged || (squash_merged_flag && !fresh));
        results.push(WorktreeState {
            branch,
            path,
            base_branch,
            merged,
            squash_merged: squash_merged_flag,
            prunable,
            fresh,
            dirty,
            remote_gone: false,
        });
    }

    Ok((default_branch, results))
}

/// Enrich worktree states with network-dependent checks.
///
/// Queries GitHub for merged PRs and `git ls-remote` for remote branch
/// existence. Updates `merged`, `remote_gone`, and `prunable` fields.
/// Silently skips enrichment on network failure.
///
/// `default_branch` should match what `list_worktrees_local` used
/// (typically from `get_default_branch`).
pub fn enrich_worktrees_network(repo: &Path, default_branch: &str, states: &mut [WorktreeState]) {
    let branches: Vec<String> = states
        .iter()
        .filter_map(|wt| wt.branch.as_ref())
        .filter(|b| b.as_str() != default_branch)
        .cloned()
        .collect();

    if branches.is_empty() {
        return;
    }

    let repo_for_pr = repo.to_path_buf();
    let pr_branches = branches;
    let pr_handle = thread::spawn(move || merged_pr_branches(&repo_for_pr, &pr_branches));

    let repo_for_remote = repo.to_path_buf();
    let remote_handle = thread::spawn(move || list_remote_branches(&repo_for_remote));

    let pr_merged = pr_handle.join().unwrap_or_default();
    let remote_branches = remote_handle.join().unwrap_or_default();

    apply_network_enrichment(states, default_branch, &pr_merged, &remote_branches);
}

fn apply_network_enrichment(
    states: &mut [WorktreeState],
    default_branch: &str,
    pr_merged: &HashSet<String>,
    remote_branches: &HashSet<String>,
) {
    for state in states.iter_mut() {
        let is_default = state.branch.as_deref() == Some(default_branch);
        if is_default {
            continue;
        }

        if !state.merged
            && state
                .branch
                .as_deref()
                .is_some_and(|b| pr_merged.contains(b))
        {
            state.merged = true;
        }

        if !remote_branches.is_empty() {
            state.remote_gone = state
                .branch
                .as_deref()
                .is_some_and(|b| !remote_branches.contains(b));
        }

        if !state.prunable && state.merged {
            state.prunable = true;
            state.fresh = false;
        }
        if !state.prunable && state.squash_merged && !state.fresh {
            state.prunable = true;
        }
        // remote_gone on a non-fresh worktree means the PR was likely merged
        // or the branch was deleted upstream — mark prunable.
        if !state.prunable && !state.fresh && state.remote_gone && !state.dirty {
            state.prunable = true;
        }
    }
}

/// Full worktree listing with all checks (local + network).
pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeState>, GitError> {
    let (default_branch, mut states) = list_worktrees_local(repo)?;
    enrich_worktrees_network(repo, &default_branch, &mut states);
    Ok(states)
}

fn worktree_prune_reason(
    state: &WorktreeState,
    policy: WorktreePrunePolicy,
) -> Option<WorktreePruneReason> {
    if policy.remove_unprotected {
        return Some(if state.merged {
            WorktreePruneReason::Merged
        } else if state.squash_merged {
            WorktreePruneReason::SquashMerged
        } else if state.remote_gone {
            WorktreePruneReason::RemoteGone
        } else if state.fresh {
            WorktreePruneReason::Fresh
        } else {
            WorktreePruneReason::Unprotected
        });
    }
    if state.fresh {
        return None;
    }
    if !state.prunable || state.dirty {
        return None;
    }
    if state.merged {
        Some(WorktreePruneReason::Merged)
    } else if state.squash_merged {
        Some(WorktreePruneReason::SquashMerged)
    } else if state.remote_gone {
        Some(WorktreePruneReason::RemoteGone)
    } else {
        None
    }
}

fn prune_stale_worktree_metadata(repo: &Path) -> Result<(), GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "prune"])
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(GitError::CommandFailed {
        command: "git worktree prune".to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

fn remove_worktree_target(
    repo: &Path,
    default_branch: &str,
    target: &WorktreePruneTarget,
) -> Result<(), GitError> {
    worktree_remove(repo, &target.path)?;
    if let Some(branch) = target.branch.as_deref() {
        if branch != default_branch {
            let _ = delete_local_branch(repo, branch);
        }
    }
    Ok(())
}

fn path_is_protected(path: &Path, protected_paths: &HashSet<PathBuf>) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    protected_paths
        .iter()
        .map(|protected| {
            protected
                .canonicalize()
                .unwrap_or_else(|_| protected.clone())
        })
        .any(|protected| protected == path || protected.starts_with(&path))
}

/// Remove worktrees selected from local Git state plus GitHub/remote evidence.
///
/// Automatic callers retain dirty worktrees. The manual CLI may explicitly use
/// the more aggressive policy it historically exposed.
pub fn prune_worktrees(
    repo: &Path,
    current_path: &Path,
    protected_paths: &HashSet<PathBuf>,
    policy: WorktreePrunePolicy,
    dry_run: bool,
) -> Result<WorktreePruneReport, GitError> {
    prune_stale_worktree_metadata(repo)?;
    let default_branch = get_default_branch(repo)?;
    let states = if policy.remove_unprotected {
        list_worktrees_local(repo)?.1
    } else {
        list_worktrees(repo)?
    };
    let mut report = WorktreePruneReport::default();

    for state in states {
        if state.path == current_path
            || state.branch.as_deref() == Some(&default_branch)
            || path_is_protected(&state.path, protected_paths)
        {
            continue;
        }
        if state.dirty && state.prunable && !policy.remove_unprotected {
            report.retained_dirty.push(state.path.clone());
            continue;
        }
        let Some(reason) = worktree_prune_reason(&state, policy) else {
            continue;
        };
        report.candidates.push(WorktreePruneTarget {
            branch: state.branch,
            path: state.path,
            reason,
        });
    }

    if dry_run {
        return Ok(report);
    }

    for target in report.candidates.clone() {
        match remove_worktree_target(repo, &default_branch, &target) {
            Ok(()) => report.removed.push(target),
            Err(error) => report.failed.push(WorktreePruneFailure {
                target,
                error: error.to_string(),
            }),
        }
    }
    Ok(report)
}

fn targeted_prune(
    repo: &Path,
    current_path: &Path,
    path: &Path,
    branch: Option<String>,
    reason: WorktreePruneReason,
    protected_paths: &HashSet<PathBuf>,
) -> Result<TargetedPruneOutcome, GitError> {
    let default_branch = get_default_branch(repo)?;
    if path == current_path
        || branch.as_deref() == Some(&default_branch)
        || path_is_protected(path, protected_paths)
    {
        return Ok(TargetedPruneOutcome::Protected);
    }
    if !is_clean_ignoring_scratch(path)? {
        return Ok(TargetedPruneOutcome::RetainedDirty(path.to_path_buf()));
    }
    let target = WorktreePruneTarget {
        branch,
        path: path.to_path_buf(),
        reason,
    };
    remove_worktree_target(repo, &default_branch, &target)?;
    Ok(TargetedPruneOutcome::Removed(target))
}

/// Remove one clean worktree named by a trusted remote branch event.
pub fn prune_branch_worktree(
    repo: &Path,
    current_path: &Path,
    branch: &str,
    reason: WorktreePruneReason,
    protected_paths: &HashSet<PathBuf>,
) -> Result<TargetedPruneOutcome, GitError> {
    let Some((path, branch)) = list_porcelain(repo)?
        .into_iter()
        .find(|(_, candidate)| candidate.as_deref() == Some(branch))
    else {
        return Ok(TargetedPruneOutcome::NotFound);
    };
    targeted_prune(repo, current_path, &path, branch, reason, protected_paths)
}

/// Remove one clean worktree whose durable owner is terminal.
pub fn prune_terminal_worktree(
    repo: &Path,
    current_path: &Path,
    path: &Path,
    protected_paths: &HashSet<PathBuf>,
) -> Result<TargetedPruneOutcome, GitError> {
    let Some((path, branch)) = list_porcelain(repo)?
        .into_iter()
        .find(|(candidate, _)| candidate == path)
    else {
        return Ok(TargetedPruneOutcome::NotFound);
    };
    targeted_prune(
        repo,
        current_path,
        &path,
        branch,
        WorktreePruneReason::Terminal,
        protected_paths,
    )
}

/// Delete abandoned atomic-write directories without touching durable logs.
pub fn prune_abandoned_prompt_logs(
    lf_home: &Path,
    older_than: Duration,
) -> std::io::Result<Vec<PathBuf>> {
    let logs = lf_home.join("logs");
    if !logs.exists() {
        return Ok(Vec::new());
    }
    let cutoff = SystemTime::now()
        .checked_sub(older_than)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut removed = Vec::new();
    for entry in fs::read_dir(logs)? {
        let entry = entry?;
        let path = entry.path();
        let is_abandoned_temp = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(".tmp"));
        if !is_abandoned_temp || !entry.file_type()?.is_dir() {
            continue;
        }
        let modified = entry.metadata()?.modified()?;
        if modified > cutoff {
            continue;
        }
        fs::remove_dir_all(&path)?;
        removed.push(path);
    }
    removed.sort();
    Ok(removed)
}

/// Create a named sibling worktree on an author-scoped branch.
///
/// This is a low-level compatibility helper for release and diagnostic
/// worktree operations. Wave and Project runtimes always use the canonical
/// main checkout; Task placement uses [`plan_placement`].
pub fn create_named_worktree(
    repo: &Path,
    name: &str,
    base: Option<&str>,
    sync_default_base: bool,
) -> Result<CreateWorktreeResult, GitError> {
    if sync_default_base {
        if let Ok(default_branch) = get_default_branch(repo) {
            let _ = sync_main(repo, &default_branch);
        }
    }

    let user = git_user(repo)?;
    let segment = WorktreeSegment::parse(name).map_err(|error| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: error.to_string(),
    })?;
    let id = WorktreeName::new(&user, segment).ok_or_else(|| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: format!("invalid worktree author: {user}"),
    })?;
    let branch = id.branch();
    let worktree_path = worktree_dir(repo, &id);

    if worktree_path.exists() {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("worktree path already exists: {worktree_path:?}"),
        });
    }
    if list_porcelain(repo)?
        .into_iter()
        .filter_map(|(_, existing)| existing)
        .any(|existing| existing == branch)
    {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch already checked out: {branch}"),
        });
    }

    let remote_branch = format!("origin/{branch}");
    if rev_parse(repo, &remote_branch).is_ok() {
        let mode = if branch_exists(repo, &branch)? {
            WorktreeBranch::Existing
        } else {
            WorktreeBranch::Track {
                remote: &remote_branch,
            }
        };
        worktree_add(repo, &worktree_path, &branch, mode)?;
        return Ok(CreateWorktreeResult {
            path: worktree_path,
            branch,
            base_branch: None,
            base_commit: None,
        });
    }

    if branch_exists(repo, &branch)? {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch exists without worktree: {branch}"),
        });
    }

    let default_branch = get_default_branch(repo)?;
    let base_ref = base.unwrap_or(default_branch.as_str());
    let base_branch = base.and_then(|value| (value != default_branch).then(|| value.to_string()));
    let base_commit = if base_branch.is_some() {
        rev_parse(repo, base_ref).ok()
    } else {
        None
    };

    worktree_add(
        repo,
        &worktree_path,
        &branch,
        WorktreeBranch::New {
            start_point: base_ref,
        },
    )?;
    schedule_upstream_sync(worktree_path.clone(), branch.clone());
    Ok(CreateWorktreeResult {
        path: worktree_path,
        branch,
        base_branch,
        base_commit,
    })
}

pub fn plan_placement(repo: &Path, segment: WorktreeSegment) -> Result<PlacementPlan, GitError> {
    let default_branch = get_default_branch(repo)?;
    let user = git_user(repo)?;

    let id = WorktreeName::new(&user, segment).ok_or_else(|| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: format!("invalid worktree author: {user}"),
    })?;
    let base_ref = default_branch;
    let branch = id.branch();
    let planned_path = worktree_dir(repo, &id);

    let existing_worktree_path =
        list_porcelain(repo)?
            .into_iter()
            .find_map(|(path, existing_branch)| {
                (existing_branch.as_deref() == Some(&branch)).then_some(path)
            });
    let strategy = if existing_worktree_path.is_some() {
        PlacementStrategy::UseExistingWorktree
    } else if branch_exists(repo, &branch)? || rev_parse(repo, &format!("origin/{branch}")).is_ok()
    {
        PlacementStrategy::CheckoutExisting
    } else {
        PlacementStrategy::Create
    };

    Ok(PlacementPlan {
        base_ref,
        branch,
        worktree_path: existing_worktree_path.unwrap_or(planned_path),
        strategy,
    })
}

pub fn create_from_placement_plan(
    repo: &Path,
    plan: &PlacementPlan,
) -> Result<CreateWorktreeResult, GitError> {
    match plan.strategy {
        PlacementStrategy::UseExistingWorktree => Ok(CreateWorktreeResult {
            path: plan.worktree_path.clone(),
            branch: plan.branch.clone(),
            base_branch: None,
            base_commit: None,
        }),
        PlacementStrategy::CheckoutExisting => {
            if plan.worktree_path.exists() {
                return Err(GitError::CommandFailed {
                    command: "git worktree add".to_string(),
                    stderr: format!("worktree path already exists: {:?}", plan.worktree_path),
                });
            }
            let remote_branch = format!("origin/{}", plan.branch);
            let mode = if branch_exists(repo, &plan.branch)? {
                WorktreeBranch::Existing
            } else {
                WorktreeBranch::Track {
                    remote: &remote_branch,
                }
            };
            worktree_add(repo, &plan.worktree_path, &plan.branch, mode)?;
            Ok(CreateWorktreeResult {
                path: plan.worktree_path.clone(),
                branch: plan.branch.clone(),
                base_branch: None,
                base_commit: None,
            })
        }
        PlacementStrategy::Create => {
            if plan.worktree_path.exists() {
                return Err(GitError::CommandFailed {
                    command: "git worktree add".to_string(),
                    stderr: format!("worktree path already exists: {:?}", plan.worktree_path),
                });
            }
            if branch_exists(repo, &plan.branch)? {
                return Err(GitError::CommandFailed {
                    command: "git worktree add".to_string(),
                    stderr: format!("branch exists without worktree: {}", plan.branch),
                });
            }
            worktree_add(
                repo,
                &plan.worktree_path,
                &plan.branch,
                WorktreeBranch::New {
                    start_point: &plan.base_ref,
                },
            )?;
            schedule_upstream_sync(plan.worktree_path.clone(), plan.branch.clone());
            Ok(CreateWorktreeResult {
                path: plan.worktree_path.clone(),
                branch: plan.branch.clone(),
                base_branch: None,
                base_commit: None,
            })
        }
    }
}

pub fn schedule_upstream_sync(worktree: PathBuf, branch: String) {
    thread::spawn(move || {
        // Don't block the caller on network/auth issues. Retry in the background.
        for backoff_secs in [0_u64, 2, 5, 15, 30, 60] {
            if backoff_secs > 0 {
                thread::sleep(Duration::from_secs(backoff_secs));
            }
            if upstream_branch(&worktree).is_some() {
                return;
            }
            if push_branch_with_upstream(&worktree, &branch).is_ok() {
                return;
            }
        }
    });
}

pub fn push_branch_with_upstream(worktree: &Path, branch: &str) -> Result<(), GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .args(["push", "-u", "origin", branch])
        .output()?;
    if !output.status.success() {
        return Err(GitError::CommandFailed {
            command: format!("git push -u origin {branch}"),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_network_enrichment, plan_placement, prune_abandoned_prompt_logs,
        prune_branch_worktree, worktree_path, worktree_prune_reason, PlacementError,
        PlacementStrategy, TargetedPruneOutcome, WorktreePrunePolicy, WorktreePruneReason,
        WorktreeSegment, WorktreeState,
    };
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        let output = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["init", "-b", "main"])
            .output()
            .expect("git init");
        assert!(output.status.success());
        // Deterministic author so branch projections don't depend on the host.
        for (key, value) in [("user.name", "tester"), ("user.email", "t@example.com")] {
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["config", key, value])
                .output()
                .expect("git config");
        }
        dir
    }

    #[test]
    fn worktree_path_sanitizes_segment_for_filesystem() {
        let path = worktree_path(Path::new("/tmp/repo"), "new*wave");
        assert_eq!(path, Path::new("/tmp/repo.new-wave"));
    }

    #[test]
    fn worktree_path_uses_neutral_fallback_for_invalid_flat_name() {
        let path = worktree_path(Path::new("/tmp/repo"), "../..");
        assert_eq!(path, Path::new("/tmp/repo.worktree"));
    }

    #[test]
    fn network_enrichment_keeps_squash_fresh_branch_unprunable() {
        let mut states = vec![
            WorktreeState {
                branch: Some("old".to_string()),
                path: Path::new("/tmp/repo.old").to_path_buf(),
                base_branch: None,
                merged: false,
                squash_merged: true,
                prunable: false,
                fresh: true,
                dirty: false,
                remote_gone: false,
            },
            WorktreeState {
                branch: Some("new".to_string()),
                path: Path::new("/tmp/repo.new").to_path_buf(),
                base_branch: None,
                merged: false,
                squash_merged: true,
                prunable: false,
                fresh: true,
                dirty: false,
                remote_gone: false,
            },
        ];

        let pr_merged = HashSet::from(["old".to_string()]);
        let remote_branches = HashSet::from(["old".to_string(), "new".to_string()]);
        apply_network_enrichment(&mut states, "main", &pr_merged, &remote_branches);

        let old = states
            .iter()
            .find(|state| state.branch.as_deref() == Some("old"))
            .unwrap();
        assert!(old.merged, "merged PR branch should be marked merged");
        assert!(old.prunable, "merged PR branch should be prunable");
        assert!(!old.fresh, "merged PR branch should not stay fresh");

        let new = states
            .iter()
            .find(|state| state.branch.as_deref() == Some("new"))
            .unwrap();
        assert!(
            !new.prunable,
            "new squashed-equivalent branch should stay unprunable while fresh"
        );
        assert!(new.fresh, "new branch should remain fresh");
    }

    #[test]
    fn automatic_prune_retains_dirty_landed_worktrees() {
        let state = WorktreeState {
            branch: Some("landed".to_string()),
            path: Path::new("/tmp/repo.landed").to_path_buf(),
            base_branch: None,
            merged: true,
            squash_merged: false,
            prunable: true,
            fresh: false,
            dirty: true,
            remote_gone: true,
        };

        assert_eq!(
            worktree_prune_reason(&state, WorktreePrunePolicy::automatic()),
            None
        );
        assert_eq!(
            worktree_prune_reason(&state, WorktreePrunePolicy::manual()),
            Some(WorktreePruneReason::Merged)
        );
    }

    #[test]
    fn manual_prune_selects_unmerged_dirty_worktrees() {
        let state = WorktreeState {
            branch: Some("abandoned".to_string()),
            path: Path::new("/tmp/repo.abandoned").to_path_buf(),
            base_branch: None,
            merged: false,
            squash_merged: false,
            prunable: false,
            fresh: false,
            dirty: true,
            remote_gone: false,
        };

        assert_eq!(
            worktree_prune_reason(&state, WorktreePrunePolicy::automatic()),
            None
        );
        assert_eq!(
            worktree_prune_reason(&state, WorktreePrunePolicy::manual()),
            Some(WorktreePruneReason::Unprotected)
        );
    }

    #[test]
    fn prompt_log_prune_only_removes_abandoned_directories() {
        let home = tempfile::tempdir().expect("create home");
        let logs = home.path().join("logs");
        fs::create_dir_all(logs.join(".tmp-abandoned")).unwrap();
        fs::write(logs.join(".tmp-abandoned/prompt.md"), "partial").unwrap();
        fs::create_dir_all(logs.join("durable")).unwrap();
        fs::write(logs.join(".tmp-file"), "not a directory").unwrap();

        let removed = prune_abandoned_prompt_logs(home.path(), Duration::ZERO).expect("prune logs");

        assert_eq!(removed, vec![logs.join(".tmp-abandoned")]);
        assert!(!logs.join(".tmp-abandoned").exists());
        assert!(logs.join("durable").exists());
        assert!(logs.join(".tmp-file").exists());
    }

    #[test]
    fn targeted_prune_retains_dirty_work_and_removes_clean_worktree() {
        let repo = init_repo();
        fs::write(repo.path().join("README.md"), "base").unwrap();
        for args in [
            ["add", "README.md"].as_slice(),
            ["commit", "-m", "base"].as_slice(),
        ] {
            let output = Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(args)
                .output()
                .expect("prepare repository");
            assert!(output.status.success());
        }
        let worktrees = tempfile::tempdir().expect("worktree parent");
        let path = worktrees.path().join("landed");
        let output = Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "landed", path.to_str().unwrap()])
            .output()
            .expect("add worktree");
        assert!(output.status.success());
        fs::write(path.join("notes.txt"), "unsaved").unwrap();

        let dirty = prune_branch_worktree(
            repo.path(),
            repo.path(),
            "landed",
            WorktreePruneReason::Merged,
            &HashSet::new(),
        )
        .expect("inspect dirty worktree");
        let TargetedPruneOutcome::RetainedDirty(retained) = dirty else {
            panic!("dirty worktree was not retained: {dirty:?}");
        };
        assert_eq!(
            retained.canonicalize().unwrap(),
            path.canonicalize().unwrap()
        );
        assert!(path.exists());

        fs::remove_file(path.join("notes.txt")).unwrap();
        let protected = HashSet::from([path.clone()]);
        let owned = prune_branch_worktree(
            repo.path(),
            repo.path(),
            "landed",
            WorktreePruneReason::Merged,
            &protected,
        )
        .expect("inspect owned worktree");
        assert_eq!(owned, TargetedPruneOutcome::Protected);
        assert!(path.exists());

        let clean = prune_branch_worktree(
            repo.path(),
            repo.path(),
            "landed",
            WorktreePruneReason::Merged,
            &HashSet::new(),
        )
        .expect("prune clean worktree");
        assert!(matches!(clean, TargetedPruneOutcome::Removed(_)));
        assert!(!path.exists());
    }

    #[test]
    fn worktree_segment_rejects_dots() {
        let err = WorktreeSegment::parse("api.v2").unwrap_err();
        assert_eq!(err, PlacementError::DotsReserved("api.v2".to_string()));
    }

    #[test]
    fn main_placement_creates_flat_branch() {
        let repo = init_repo();
        let segment = WorktreeSegment::parse("child").unwrap();
        let plan = plan_placement(repo.path(), segment).expect("plan task worktree");

        assert_eq!(plan.branch, "tester/child");
        assert_eq!(plan.base_ref, "main");
        assert_eq!(plan.strategy, PlacementStrategy::Create);
    }
}
