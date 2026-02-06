use std::path::{Path, PathBuf};
use std::process::Command;

use loopflow_engine::git::{get_default_branch, is_clean, land as git_land, LandStrategy};
use loopflow_engine::worktrees::{main_repo_root, worktree_path};

use crate::commit::{commit_workflow, CommitOptions};
use crate::error::{OpsError, OpsResult};
use crate::messages::generate_pr_message;
use crate::progress::Progress;
use crate::util::stderr_from_output;

#[derive(Debug, Clone)]
pub struct LandOptions {
    pub strict: bool,
    pub local: bool,
    pub create_pr: bool,
    pub worktree: Option<String>,
    pub lint: bool,
}

#[derive(Debug, Clone)]
pub struct LandResult {
    pub merged: bool,
}

pub fn land(repo: &Path, options: &LandOptions, progress: &impl Progress) -> OpsResult<LandResult> {
    let (repo_root, main_repo) = resolve_repos(repo, options.worktree.as_deref())?;
    if options.strict && !is_clean(&repo_root)? {
        return Err(OpsError::Message(
            "uncommitted changes; commit, stash, or rerun without --strict".to_string(),
        ));
    }

    if options.lint {
        crate::lint::ensure_lint_passes(&repo_root, progress)?;
    }

    if !options.strict {
        let commit_options = CommitOptions {
            add: true,
            lint: false,
            push: true,
            create_draft_pr: true,
            task: "commit".to_string(),
            flow_parents: Vec::new(),
            message: None,
        };
        let _ = commit_workflow(&repo_root, &commit_options, progress)?;
    }

    let main_branch = get_default_branch(&main_repo)?;
    crate::rebase::rebase_with_recovery(
        &repo_root,
        &crate::rebase::RebaseOptions {
            onto: format!("origin/{main_branch}"),
            push: true,
        },
        progress,
    )?;

    clear_scratch(&repo_root, progress)?;

    if options.local {
        progress.status("Merging locally...");
        let _ = git_land(&repo_root, LandStrategy::LocalMerge, &main_branch)?;
        return Ok(LandResult { merged: true });
    }

    if !crate::pr::gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    if !crate::pr::pr_exists_for_current_branch(&repo_root)? {
        if options.create_pr {
            let _ = crate::pr::create_or_update_pr(
                &repo_root,
                &crate::pr::PrOptions {
                    refresh: true,
                    lint: false,
                },
                progress,
            )?;
        } else {
            return Err(OpsError::Message(
                "no open PR found; run lf ops pr or use --create-pr".to_string(),
            ));
        }
    }

    progress.status("Updating PR...");
    let message = generate_pr_message(&repo_root)?;
    update_pr_message(&repo_root, &message.title, &message.body)?;
    mark_ready(&repo_root)?;
    enable_auto_merge(&repo_root, &message.title, &message.body)?;

    if let Some(url) = current_pr_url(&repo_root)? {
        open_url(&url);
    }

    Ok(LandResult { merged: true })
}

fn resolve_repos(repo: &Path, worktree: Option<&str>) -> OpsResult<(PathBuf, PathBuf)> {
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let repo_root = if let Some(worktree) = worktree {
        let candidate = Path::new(worktree);
        if candidate.exists() {
            candidate.to_path_buf()
        } else {
            let path = worktree_path(&main_repo, worktree);
            if path.exists() {
                path
            } else {
                return Err(OpsError::Message(format!(
                    "worktree not found: {}",
                    worktree
                )));
            }
        }
    } else {
        repo.to_path_buf()
    };

    Ok((repo_root, main_repo))
}

fn clear_scratch(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    let scratch = repo.join("scratch");
    if !scratch.exists() {
        return Ok(());
    }

    let mut removed = false;
    for entry in std::fs::read_dir(&scratch)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        removed = true;
    }

    if !removed {
        return Ok(());
    }

    progress.status("Clearing scratch/...");
    loopflow_engine::git::stage_all(repo)?;
    if has_staged_changes(repo)? {
        loopflow_engine::git::commit(repo, "lf land: clear scratch/")?;
        crate::commit::push_with_upstream_or_error(repo)?;
    }

    Ok(())
}

fn has_staged_changes(repo: &Path) -> OpsResult<bool> {
    let status = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .arg("--quiet")
        .current_dir(repo)
        .status()?;
    Ok(!status.success())
}

fn update_pr_message(repo: &Path, title: &str, body: &str) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr edit".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn mark_ready(repo: &Path) -> OpsResult<()> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("ready")
        .current_dir(repo)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Ok(())
}

fn enable_auto_merge(repo: &Path, title: &str, body: &str) -> OpsResult<()> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("merge")
        .arg("--squash")
        .arg("--auto")
        .arg("--subject")
        .arg(title);
    if !body.trim().is_empty() {
        cmd.arg("--body").arg(body);
    }
    let output = cmd.current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "gh pr merge --auto".to_string(),
            stderr: stderr_from_output(&output),
        });
    }
    Ok(())
}

fn current_pr_url(repo: &Path) -> OpsResult<Option<String>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg("--json")
        .arg("url")
        .arg("-q")
        .arg(".url")
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        Ok(None)
    } else {
        Ok(Some(url))
    }
}

fn open_url(url: &str) {
    let open = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = Command::new(open).arg(url).status();
}
