use crate::config::BranchNameConfig;
use crate::error::GitError;
use crate::git::{get_default_branch, is_ancestor, rev_parse, worktree_add, worktree_move};
use crate::naming::format_branch_name;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeState {
    pub branch: Option<String>,
    pub path: PathBuf,
    pub base_branch: Option<String>,
    pub merged: bool,
    pub prunable: bool,
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
    let sanitized = name.replace('/', "-").replace('\\', "-");
    repo.parent().unwrap_or(repo).join(sanitized)
}

fn branch_exists(repo: &Path, branch: &str) -> Result<bool, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["show-ref", "--verify", &format!("refs/heads/{branch}")])
        .output()?;
    Ok(output.status.success())
}

fn list_porcelain(repo: &Path) -> Result<Vec<(PathBuf, Option<String>)>, GitError> {
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
    let branch = raw.split('/').last().unwrap_or(&raw).to_string();
    Some(branch)
}

pub fn list_worktrees(repo: &Path) -> Result<Vec<WorktreeState>, GitError> {
    let default_branch = get_default_branch(repo)?;
    let merge_target = format!("origin/{default_branch}");
    let items = list_porcelain(repo)?;
    let mut results = Vec::new();

    for (path, branch) in items {
        let base = upstream_branch(&path);
        let base_branch = base.filter(|b| b != &default_branch);
        let merged = if let Some(branch) = &branch {
            if branch == &default_branch {
                false
            } else {
                is_ancestor(repo, branch, &merge_target).unwrap_or(false)
            }
        } else {
            false
        };
        let prunable = merged && branch.as_deref() != Some(&default_branch);
        results.push(WorktreeState {
            branch,
            path,
            base_branch,
            merged,
            prunable,
        });
    }

    Ok(results)
}

pub fn create_with_schema(
    repo: &Path,
    short_name: &str,
    base: Option<&str>,
    branch_config: Option<&BranchNameConfig>,
) -> Result<CreateWorktreeResult, GitError> {
    let branch_name = format_branch_name(short_name, branch_config, repo)?;
    let worktree_path = worktree_path(repo, short_name);
    if worktree_path.exists() {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("worktree path already exists: {worktree_path:?}"),
        });
    }

    let existing_branches = list_worktrees(repo)?
        .into_iter()
        .filter_map(|wt| wt.branch)
        .collect::<Vec<_>>();
    if existing_branches.iter().any(|b| b == &branch_name) {
        return Err(GitError::CommandFailed {
            command: "git worktree add".to_string(),
            stderr: format!("branch already exists: {branch_name}"),
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

    worktree_add(repo, &worktree_path, &branch_name, base_ref)?;

    let _ = Command::new("git")
        .arg("-C")
        .arg(&worktree_path)
        .args(["push", "-u", "origin", &branch_name])
        .status();

    Ok(CreateWorktreeResult {
        path: worktree_path,
        branch: branch_name,
        base_branch,
        base_commit,
    })
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
