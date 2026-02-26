use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    current_branch, get_default_branch, is_clean, land as git_land, LandStrategy,
};
use crate::engine::worktrees::{main_repo_root, worktree_path};

use crate::engine::command::run_command;
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::messages::{generate_pr_message, resolve_wave_name};
use crate::ops::progress::Progress;

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
    let feature_branch = current_branch(&repo_root)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    prepare_land(&repo_root, options, progress)?;
    let main_branch = rebase_land(&repo_root, &main_repo, progress)?;
    clear_scratch(&repo_root, progress)?;

    if options.local {
        finalize_local(&repo_root, &main_branch, &feature_branch, progress)?;
        return Ok(LandResult { merged: true });
    }

    ensure_pr(&repo_root, options, progress)?;
    finalize_remote(&repo_root, progress)?;
    Ok(LandResult { merged: true })
}

fn prepare_land(
    repo_root: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    if options.strict && !is_clean(repo_root)? {
        return Err(OpsError::Message(
            "uncommitted changes; commit, stash, or rerun without --strict".to_string(),
        ));
    }

    if options.lint {
        crate::ops::lint::ensure_lint_passes(repo_root, progress)?;
    }

    if !options.strict {
        let commit_options = CommitOptions {
            add: true,
            push: true,
            create_draft_pr: true,
            ..CommitOptions::for_task("commit")
        };
        let _ = commit_workflow(repo_root, &commit_options, progress)?;
    }

    Ok(())
}

fn rebase_land(repo_root: &Path, main_repo: &Path, progress: &impl Progress) -> OpsResult<String> {
    let main_branch = get_default_branch(main_repo)?;
    crate::ops::rebase::rebase_with_recovery(
        repo_root,
        &crate::ops::rebase::RebaseOptions {
            onto: format!("origin/{main_branch}"),
            push: true,
        },
        progress,
    )?;
    Ok(main_branch)
}

fn finalize_local(
    repo_root: &Path,
    main_branch: &str,
    feature_branch: &str,
    progress: &impl Progress,
) -> OpsResult<()> {
    progress.status("Merging locally...");
    let _ = git_land(repo_root, LandStrategy::LocalMerge, main_branch)?;
    delete_remote_branch(repo_root, feature_branch)?;
    Ok(())
}

fn ensure_pr(repo_root: &Path, options: &LandOptions, progress: &impl Progress) -> OpsResult<()> {
    if !crate::ops::pr::gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    if !crate::ops::pr::pr_exists_for_current_branch(repo_root)? {
        if options.create_pr {
            let _ = crate::ops::pr::create_or_update_pr(
                repo_root,
                &crate::ops::pr::PrOptions {
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

    Ok(())
}

fn finalize_remote(repo_root: &Path, progress: &impl Progress) -> OpsResult<()> {
    progress.status("Updating PR...");
    let wave = resolve_wave_name(repo_root, None);
    let message = generate_pr_message(repo_root, wave.as_deref())?;
    update_pr_message(repo_root, &message.title, &message.body)?;
    mark_ready(repo_root)?;
    enable_auto_merge(repo_root, &message.title, &message.body)?;

    if let Some(url) = current_pr_url(repo_root)? {
        progress.status(&format!("\n{url}\n"));
        open_url(&url);
    }

    Ok(())
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
    crate::engine::git::stage_all(repo)?;
    if has_staged_changes(repo)? {
        crate::engine::git::commit(repo, "lf land: clear scratch/")?;
        crate::ops::commit::push_with_upstream_or_error(repo)?;
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
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("edit")
        .arg("--title")
        .arg(title)
        .arg("--body")
        .arg(body)
        .current_dir(repo);
    if let Err(err) = run_command(&mut cmd) {
        return Err(OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        });
    }
    Ok(())
}

pub fn mark_ready(repo: &Path) -> OpsResult<()> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("ready").current_dir(repo);
    let _ = run_command(&mut cmd);
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
    cmd.current_dir(repo);
    if let Err(err) = run_command(&mut cmd) {
        return Err(OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        });
    }
    Ok(())
}

fn delete_remote_branch(repo: &Path, branch: &str) -> OpsResult<()> {
    let mut cmd = Command::new("git");
    cmd.args(["push", "origin", "--delete", branch])
        .current_dir(repo);
    if let Err(err) = run_command(&mut cmd) {
        return Err(OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        });
    }
    Ok(())
}

fn current_pr_url(repo: &Path) -> OpsResult<Option<String>> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("view")
        .arg("--json")
        .arg("url")
        .arg("-q")
        .arg(".url")
        .current_dir(repo);
    let output = match run_command(&mut cmd) {
        Ok(output) => output,
        Err(_) => return Ok(None),
    };
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        Ok(None)
    } else {
        Ok(Some(url))
    }
}

fn open_url(url: &str) {
    crate::engine::platform::open_url(url);
}
