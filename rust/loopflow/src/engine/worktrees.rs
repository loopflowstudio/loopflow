use crate::engine::config::BranchNameConfig;
use crate::engine::error::GitError;
use crate::engine::git::{
    get_default_branch, has_commits_beyond, is_ancestor, is_clean_ignoring_scratch,
    is_squash_merged, rev_parse, worktree_add, worktree_move, WorktreeBranch,
};
use crate::engine::naming::{format_branch_name, wave_name_from_branch};
use crate::lfd::security::sanitize_fs_component;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub squash_merged: bool,
    pub prunable: bool,
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

pub fn worktree_path(repo: &Path, name: &str) -> PathBuf {
    worktree_path_with_config(repo, name, None)
}

pub fn worktree_path_with_config(
    repo: &Path,
    name: &str,
    branch_config: Option<&BranchNameConfig>,
) -> PathBuf {
    let repo_root = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let dir_name =
        wave_name_from_branch(name, branch_config).unwrap_or_else(|| sanitize_fs_component(name));
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    repo_root
        .parent()
        .unwrap_or(repo_root.as_path())
        .join(format!("{repo_name}.{dir_name}"))
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
pub fn wave_name_from_worktree_and_main(repo: &Path, main_repo: &Path) -> Option<String> {
    if repo == main_repo {
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
    let branch = raw.split('/').next_back().unwrap_or(&raw).to_string();
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
            .is_some_and(|b| !is_default && squash_merged.contains(b));
        let dirty = !is_clean_ignoring_scratch(&path).unwrap_or(true);
        let mut prunable = !is_default && (merged || squash_merged_flag || !has_commits);

        // Protect shortname worktrees from pruning unless content is in main.
        if prunable && !merged && !squash_merged_flag {
            let is_shortname = wave_name_from_worktree_and_main(&path, repo)
                .map(|name| !name.contains('.'))
                .unwrap_or(false);
            if is_shortname {
                prunable = false;
            }
        }
        results.push(WorktreeState {
            branch,
            path,
            base_branch,
            merged,
            squash_merged: squash_merged_flag,
            prunable,
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

        if !state.prunable {
            if state.merged || state.squash_merged {
                state.prunable = true;
            } else if state.remote_gone && !state.dirty {
                let is_shortname = wave_name_from_worktree_and_main(&state.path, repo)
                    .map(|name| !name.contains('.'))
                    .unwrap_or(false);
                if !is_shortname {
                    state.prunable = true;
                }
            }
        }
    }
}

/// Full worktree listing with all checks (local + network).
pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeState>, GitError> {
    let (default_branch, mut states) = list_worktrees_local(repo)?;
    enrich_worktrees_network(repo, &default_branch, &mut states);
    Ok(states)
}

pub fn create_with_schema(
    repo: &Path,
    short_name: &str,
    base: Option<&str>,
    branch_config: Option<&BranchNameConfig>,
) -> Result<CreateWorktreeResult, GitError> {
    let remote_branch = format!("origin/{short_name}");
    let has_remote_branch = rev_parse(repo, &remote_branch).is_ok();
    let branch_name = if has_remote_branch {
        short_name.to_string()
    } else {
        format_branch_name(short_name, branch_config, repo)?
    };

    let worktree_path = worktree_path_with_config(repo, short_name, branch_config);
    if worktree_path.exists() {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("worktree path already exists: {worktree_path:?}"),
        });
    }

    if list_porcelain(repo)?
        .into_iter()
        .filter_map(|(_, branch)| branch)
        .any(|branch| branch == branch_name)
    {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch already exists: {branch_name}"),
        });
    }
    if has_remote_branch {
        let mode = if branch_exists(repo, &branch_name)? {
            WorktreeBranch::Existing
        } else {
            WorktreeBranch::Track {
                remote: &remote_branch,
            }
        };
        worktree_add(repo, &worktree_path, &branch_name, mode)?;
        return Ok(CreateWorktreeResult {
            path: worktree_path,
            branch: branch_name,
            base_branch: None,
            base_commit: None,
        });
    }

    if branch_exists(repo, &branch_name)? {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch exists without worktree: {branch_name}"),
        });
    }

    let default_branch = get_default_branch(repo)?;
    let base_ref = base.unwrap_or(default_branch.as_str());
    let base_branch = base.and_then(|value| {
        if value != default_branch {
            Some(value.to_string())
        } else {
            None
        }
    });
    let base_commit = if base_branch.is_some() {
        rev_parse(repo, base_ref).ok()
    } else {
        None
    };

    worktree_add(
        repo,
        &worktree_path,
        &branch_name,
        WorktreeBranch::New {
            start_point: base_ref,
        },
    )?;
    schedule_upstream_sync(worktree_path.clone(), branch_name.clone());

    Ok(CreateWorktreeResult {
        path: worktree_path,
        branch: branch_name,
        base_branch,
        base_commit,
    })
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

fn push_branch_with_upstream(worktree: &Path, branch: &str) -> Result<(), GitError> {
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

pub fn preserve_worktree(repo: &Path, worktree: &Path) -> Result<PathBuf, GitError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = worktree
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("worktree");
    let new_path = worktree
        .parent()
        .unwrap_or(worktree)
        .join(format!("{name}.{ts}"));
    worktree_move(repo, worktree, &new_path)?;
    Ok(new_path)
}

#[cfg(test)]
mod tests {
    use super::{worktree_path, worktree_path_with_config};
    use std::path::Path;

    #[test]
    fn worktree_path_sanitizes_wave_name_for_filesystem_component() {
        let path = worktree_path(Path::new("/tmp/repo"), "feature/new*wave");
        assert_eq!(path, Path::new("/tmp/repo.feature-new-wave"));
    }

    #[test]
    fn worktree_path_uses_wave_fallback_for_empty_sanitized_component() {
        let path = worktree_path(Path::new("/tmp/repo"), "../..");
        assert_eq!(path, Path::new("/tmp/repo.wave"));
    }

    #[test]
    fn worktree_path_extracts_wave_name_from_branch_style_input() {
        let path = worktree_path_with_config(
            Path::new("/tmp/repo"),
            "jack-heart.mobile.20260225_1122",
            None,
        );
        assert_eq!(path, Path::new("/tmp/repo.mobile"));
    }
}
