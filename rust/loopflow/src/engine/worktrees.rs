use crate::engine::error::GitError;
use crate::engine::git::{
    current_branch, get_default_branch, has_commits_beyond, is_ancestor, is_clean_ignoring_scratch,
    is_squash_merged, rev_parse, sync_main, worktree_add, WorktreeBranch,
};
use crate::engine::identity::{Timestamp, WaveId};
use crate::engine::naming::{generate_word_pair, git_user};
use crate::lfd::security::sanitize_fs_component;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeSegment(String);

impl WorktreeSegment {
    pub fn parse(raw: &str) -> Result<Self, PlacementError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(PlacementError::EmptySegment);
        }
        // Dots are the chain separator; a dot in a single segment is a user
        // mistake to surface, not something to silently join.
        if trimmed.contains('.') {
            return Err(PlacementError::DotsReserved(trimmed.to_string()));
        }
        Ok(Self(crate::engine::naming::sanitize_for_branch(trimmed)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementRequest {
    /// Root branch off the default branch. The no-flag default for
    /// `lf wt create` — ad-hoc worktrees never stack unless asked.
    Main { segment: WorktreeSegment },
    /// Child branch stacked under an explicit parent (`--child`).
    Stack {
        parent: String,
        segment: WorktreeSegment,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementStrategy {
    CreateRoot,
    CreateStackChild,
    CheckoutExisting,
    UseExistingWorktree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlacementPlan {
    pub base_ref: String,
    pub parent_branch: Option<String>,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub stack_depth: usize,
    pub strategy: PlacementStrategy,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PlacementError {
    #[error("worktree segment cannot be empty")]
    EmptySegment,
    #[error(
        "\"{0}\" is not a worktree segment. Dots are reserved for stack ancestry. Use a hyphen, or create ancestry with --child."
    )]
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

#[derive(Debug, Clone, Serialize)]
pub struct CreateWorktreeResult {
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: Option<String>,
    pub base_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeLease {
    pub path: PathBuf,
    pub branch: String,
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
pub fn worktree_dir(repo: &Path, id: &WaveId) -> PathBuf {
    dir_for_component(repo, &id.dir_component())
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

/// The worktree directory for a name in any surface form — wave name, branch, or
/// dir component. Liberal on input via [`WaveId::parse`], falling back to a
/// sanitized component when the name isn't a valid identity.
pub fn worktree_path(repo: &Path, name: &str) -> PathBuf {
    let user = git_user(repo).unwrap_or_else(|_| "user".to_string());
    let component = WaveId::parse(name, &user)
        .map(|id| id.dir_component())
        .unwrap_or_else(|| sanitize_fs_component(name));
    dir_for_component(repo, &component)
}

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<WorktreeLease> {
    let path = worktree_path(main_repo, wave_name);
    if path.exists() && path.join(".git").exists() {
        let branch = current_branch(&path)?.unwrap_or_default();
        return Ok(WorktreeLease { path, branch });
    }

    let result = create_wave_worktree(main_repo, wave_name, None, true)?;
    Ok(WorktreeLease {
        path: result.path,
        branch: result.branch,
    })
}

/// Short run id: the leading 8 hex chars of the run's UUID, tying the
/// worktree directory to its Run row.
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

/// The identity of a wave-dispatched worker: the wave, plus the short run id as
/// its chain segment and a fresh worker stamp. `bugs` + run `a1b2…` →
/// `bugs.a1b2c3d4.<ts>` on disk, `<user>/bugs.a1b2c3d4.<ts>` on the remote.
pub fn worker_id(repo: &Path, wave_name: &str, run_id: &str) -> Result<WaveId, GitError> {
    let user = git_user(repo)?;
    let wave = WaveId::for_wave(&user, wave_name).ok_or_else(|| GitError::CommandFailed {
        command: "worker id".to_string(),
        stderr: format!("invalid wave name: {wave_name}"),
    })?;
    let segment =
        WorktreeSegment::parse(&short_run_id(run_id)).map_err(|e| GitError::CommandFailed {
            command: "worker id".to_string(),
            stderr: e.to_string(),
        })?;
    Ok(wave.child(segment, Some(Timestamp::now())))
}

fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}

/// Extract the wave name from a worktree directory.
///
/// Given a path like `../loopflow.my-feature`, returns `Some("my-feature")`.
/// Returns `None` if not in a worktree (i.e., in the main repo).
pub fn wave_name_from_worktree(repo: &Path) -> Option<String> {
    let main_repo = main_repo_root(repo).ok()?;
    wave_name_from_worktree_and_main(repo, &main_repo)
}

/// Extract the wave name using an already-resolved main repo path.
///
/// Use this to avoid repeatedly shelling out to git when iterating many worktrees.
/// Only recognizes sibling worktrees (e.g., `../repo.feature`) — the worktree must
/// share the same parent directory as the main repo. Worktrees elsewhere (e.g.,
/// `.claude/worktrees/`) return `None`.
pub fn wave_name_from_worktree_and_main(repo: &Path, main_repo: &Path) -> Option<String> {
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
fn github_repo_nwo(repo: &Path) -> Option<(String, String)> {
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

/// Create (or reuse) the wave-home worktree: `<repo>.<wave>` on branch
/// `<user>/<wave>`, off the default branch (or `base` when forking). Waves are
/// persistent, so the branch carries no stamp.
pub fn create_wave_worktree(
    repo: &Path,
    wave_name: &str,
    base: Option<&str>,
    sync_default_base: bool,
) -> Result<CreateWorktreeResult, GitError> {
    if sync_default_base {
        if let Ok(default_branch) = get_default_branch(repo) {
            let _ = sync_main(repo, &default_branch);
        }
    }

    let user = git_user(repo)?;
    let id = WaveId::for_wave(&user, wave_name).ok_or_else(|| GitError::CommandFailed {
        command: "git worktree add".to_string(),
        stderr: format!("invalid wave name: {wave_name}"),
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

/// A fresh, stamped worker branch for `wave_name`, de-collided against existing
/// branches with a word-pair segment. Used by branch rotation.
pub fn fresh_stamped_branch(repo: &Path, wave_name: &str) -> Result<String, GitError> {
    let user = git_user(repo)?;
    let wave = WaveId::parse(wave_name, &user)
        .map(|id| id.wave_name().to_string())
        .unwrap_or_else(|| wave_name.to_string());
    let base = WaveId::for_wave(&user, &wave).ok_or_else(|| GitError::CommandFailed {
        command: "rotate branch".to_string(),
        stderr: format!("invalid wave name: {wave_name}"),
    })?;
    let mut id = base.clone().stamped(Timestamp::now());
    while branch_exists(repo, &id.branch())? {
        let pair =
            WorktreeSegment::parse(&generate_word_pair()).map_err(|e| GitError::CommandFailed {
                command: "rotate branch".to_string(),
                stderr: e.to_string(),
            })?;
        id = base.child(pair, Some(Timestamp::now()));
    }
    Ok(id.branch())
}

pub fn plan_placement(repo: &Path, request: PlacementRequest) -> Result<PlacementPlan, GitError> {
    let default_branch = get_default_branch(repo)?;
    let user = git_user(repo)?;

    let (id, base_ref, parent_branch) = match request {
        PlacementRequest::Main { segment } => {
            (WaveId::wave(&user, segment), default_branch.clone(), None)
        }
        PlacementRequest::Stack { parent, segment } => {
            let parent_id =
                WaveId::parse(&parent, &user).ok_or_else(|| GitError::CommandFailed {
                    command: "wt create --child".to_string(),
                    stderr: format!("cannot parse parent branch: {parent}"),
                })?;
            let parent_branch = parent_id.branch();
            let base_ref = if branch_exists(repo, &parent_branch)? {
                parent_branch.clone()
            } else {
                let remote_parent = format!("origin/{parent_branch}");
                if rev_parse(repo, &remote_parent).is_ok() {
                    remote_parent
                } else {
                    parent_branch.clone()
                }
            };
            // A human-created child is a persistent subwave — unstamped.
            (
                parent_id.child(segment, None),
                base_ref,
                Some(parent_branch),
            )
        }
    };

    let branch = id.branch();
    let planned_path = worktree_dir(repo, &id);
    let stack_depth = id.depth();

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
    } else if parent_branch.is_some() {
        PlacementStrategy::CreateStackChild
    } else {
        PlacementStrategy::CreateRoot
    };

    Ok(PlacementPlan {
        base_ref,
        parent_branch,
        branch,
        worktree_path: existing_worktree_path.unwrap_or(planned_path),
        stack_depth,
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
            base_branch: plan.parent_branch.clone(),
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
                base_branch: plan.parent_branch.clone(),
                base_commit: None,
            })
        }
        PlacementStrategy::CreateRoot | PlacementStrategy::CreateStackChild => {
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
            let base_commit = if plan.parent_branch.is_some() {
                rev_parse(repo, &plan.base_ref).ok()
            } else {
                None
            };
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
                base_branch: plan.parent_branch.clone(),
                base_commit,
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
        apply_network_enrichment, plan_placement, worktree_path, PlacementError, PlacementRequest,
        PlacementStrategy, WorktreeSegment, WorktreeState,
    };
    use std::collections::HashSet;
    use std::path::Path;
    use std::process::Command;

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
    fn worktree_path_uses_wave_fallback_for_empty_sanitized_component() {
        let path = worktree_path(Path::new("/tmp/repo"), "../..");
        assert_eq!(path, Path::new("/tmp/repo.wave"));
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
    fn worktree_segment_rejects_dots() {
        let err = WorktreeSegment::parse("api.v2").unwrap_err();
        assert_eq!(err, PlacementError::DotsReserved("api.v2".to_string()));
    }

    #[test]
    fn stack_placement_creates_child_branch() {
        let repo = init_repo();
        let segment = WorktreeSegment::parse("child").unwrap();
        let plan = plan_placement(
            repo.path(),
            PlacementRequest::Stack {
                parent: "a.b".to_string(),
                segment,
            },
        )
        .expect("plan placement");

        // Child of `a.b` under author `tester`: dir flat, branch author-scoped.
        assert_eq!(plan.branch, "tester/a.b.child");
        assert_eq!(plan.base_ref, "tester/a.b");
        assert_eq!(plan.parent_branch.as_deref(), Some("tester/a.b"));
        assert_eq!(plan.stack_depth, 3);
        assert_eq!(plan.strategy, PlacementStrategy::CreateStackChild);
        let file_name = plan
            .worktree_path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("worktree path file name");
        assert!(file_name.ends_with(".a.b.child"), "{file_name}");
    }

    #[test]
    fn main_placement_creates_root_branch() {
        let repo = init_repo();
        let segment = WorktreeSegment::parse("child").unwrap();
        let plan = plan_placement(repo.path(), PlacementRequest::Main { segment })
            .expect("plan placement");

        assert_eq!(plan.branch, "tester/child");
        assert_eq!(plan.base_ref, "main");
        assert_eq!(plan.parent_branch, None);
        assert_eq!(plan.stack_depth, 1);
        assert_eq!(plan.strategy, PlacementStrategy::CreateRoot);
    }
}
