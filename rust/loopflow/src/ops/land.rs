use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    current_branch, get_default_branch, is_clean, land as git_land, LandStrategy,
};
use crate::engine::worktrees::{main_repo_root, worktree_path};

use crate::engine::command::run_command;
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pr::{generate_pr_copy, PrInfo};

use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct LandOptions {
    pub strict: bool,
    pub local: bool,
    pub create_pr: bool,
    pub complete: bool,
    pub next_slug: Option<String>,
    pub worktree: Option<String>,
    pub commit_message: Option<String>,
    pub pr_title: Option<String>,
    pub pr_body: Option<String>,
    pub agent: Option<String>,
}

/// How a prepared PR is handed off once it is rebased, scratch-cleared, and
/// marked ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finalize {
    /// Arm GitHub auto-merge so the PR merges itself once checks pass. Used by
    /// `land` — the walk-away path.
    AutoMerge,
    /// Assign the PR to the current user and leave it for a required, manual
    /// merge click. Used by `submit` — nothing merges without that one click.
    AssignForReview,
}

/// Run every land skill: commit, rebase onto main, clear scratch, mark the PR
/// ready, and finalize per `finalize`. `land` arms auto-merge; `submit` assigns
/// the PR for a human to merge. Neither rotates the worktree — the wave home is
/// permanent. Returns the resulting PR (`None` for a local merge) so callers can
/// record it against the run.
fn prepare_pr(
    repo: &Path,
    options: &LandOptions,
    finalize: Finalize,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    if options.complete && options.next_slug.is_some() {
        return Err(OpsError::Message(
            "--complete and --next cannot be used together".to_string(),
        ));
    }
    if options.local && (options.complete || options.next_slug.is_some()) {
        return Err(OpsError::Message(
            "Task disposition flags require a pull request merge and cannot be used with --local"
                .to_string(),
        ));
    }
    let (repo_root, main_repo) = resolve_repos(repo, options.worktree.as_deref())?;
    if finalize == Finalize::AssignForReview {
        crate::ops::task::reject_managed_task_shipping(&repo_root, "submit")?;
    } else if crate::ops::task::authorize_managed_task_land(
        &repo_root,
        if options.complete {
            crate::task::AfterMerge::CompleteTask
        } else {
            crate::task::AfterMerge::Review
        },
        options.next_slug.as_deref(),
    )? {
        if options.local || options.create_pr {
            return Err(OpsError::Message(
                "managed Task land uses its current published PR; --local and --create-pr are not valid"
                    .to_string(),
            ));
        }
        arm_approved_task_pr(&repo_root)?;
        crate::ops::task::record_managed_task_land_armed(&repo_root)?;
        return crate::ops::pr::current_pr(&repo_root);
    }
    crate::ops::pr::reject_control_plane_pr(&repo_root)?;
    let feature_branch = current_branch(&repo_root)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    // Prove the Task PR range is uncontaminated before the first GitHub side
    // effect (prepare_land pushes and can open a draft PR). No-op for non-Task
    // worktrees.
    crate::ops::task::verify_task_pr_range(&repo_root)?;
    prepare_land(&repo_root, options, progress)?;
    let main_branch = rebase_land(&repo_root, &main_repo, progress)?;
    // Rebase may advance the fork point. Re-run the authoritative proof to heal
    // the recorded base and refuse an empty range before any `gh pr` side effect.
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    let pr_exists = if options.local {
        false
    } else {
        crate::ops::pr::pr_exists_for_current_branch(&repo_root)?
    };
    if pr_exists {
        crate::ops::pr::retarget_open_pr(&repo_root, &main_branch)?;
    }
    if !options.local && !pr_exists && !options.create_pr {
        return Err(OpsError::Message(format!(
            "no open PR found for branch '{feature_branch}'; run lf pr open or use --create-pr"
        )));
    }
    let (pr_title, pr_body) = resolve_pr_copy(&repo_root, options, progress)?;
    clear_scratch(&repo_root, progress)?;

    if options.local {
        finalize_local(&repo_root, &main_branch, &feature_branch, progress)?;
        return Ok(None);
    }

    crate::ops::task::request_task_pr_publication(
        &repo_root,
        if options.complete {
            crate::task::AfterMerge::CompleteTask
        } else {
            crate::task::AfterMerge::Review
        },
        options.next_slug.as_deref(),
    )?;
    ensure_pr(
        &repo_root,
        pr_exists,
        &feature_branch,
        options.create_pr,
        pr_title.as_deref(),
        pr_body.as_deref(),
        options.agent.as_deref(),
        progress,
    )?;
    let pr = crate::ops::pr::current_pr(&repo_root)?;
    crate::ops::task::attach_task_github_pr(&repo_root, pr.as_ref())?;
    finalize_remote(
        &repo_root,
        pr_title.as_deref(),
        pr_body.as_deref(),
        finalize,
        progress,
    )?;
    Ok(pr)
}

/// Arm an already-published Task PR after the Task runner has validated its
/// durable lifecycle reviews and head fingerprint. This deliberately performs
/// no commit, rebase, copy generation, or scratch cleanup: any head mutation
/// after approval would invalidate the review evidence that authorized it.
pub(crate) fn arm_approved_task_pr(repo: &Path) -> OpsResult<()> {
    mark_ready(repo)?;
    if !auto_merge_enabled(repo)? {
        enable_auto_merge(repo, None, None)?;
    }
    Ok(())
}

/// Land a PR and walk away: commit, rebase, clear scratch, and arm auto-merge so
/// GitHub merges it once checks pass. Task worktrees self-prune once their PR
/// merges; permanent Wave homes are rejected before any mutation.
pub fn land(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    prepare_pr(repo, options, Finalize::AutoMerge, progress)
}

/// Prepare a PR to land without arming auto-merge: commit, rebase onto main,
/// clear scratch, mark the PR ready, and assign it to the current user. Nothing
/// merges until that user clicks merge on GitHub — that one click is the
/// required gate. Like `land`, this never rotates the worktree.
pub fn submit(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    prepare_pr(repo, options, Finalize::AssignForReview, progress)
}

fn resolve_pr_copy(
    repo_root: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<(Option<String>, Option<String>)> {
    if options.local {
        return Ok((options.pr_title.clone(), options.pr_body.clone()));
    }

    let mut pr_title = options.pr_title.clone();
    let mut pr_body = options.pr_body.clone();

    if pr_title.is_some() {
        return Ok((pr_title, pr_body));
    }

    if let Some((title, body)) = read_cached_pr_copy(repo_root, progress)? {
        progress.status("Using cached PR copy from scratch/");
        pr_title = Some(title);
        if pr_body.is_none() {
            pr_body = Some(body);
        }
        return Ok((pr_title, pr_body));
    }

    let generated = generate_pr_copy(repo_root, progress, options.agent.as_deref())?;
    pr_title = Some(generated.title);
    if pr_body.is_none() {
        pr_body = Some(generated.body);
    }
    Ok((pr_title, pr_body))
}

fn read_cached_pr_copy(
    repo_root: &Path,
    progress: &impl Progress,
) -> OpsResult<Option<(String, String)>> {
    let title_path = repo_root.join("scratch/pr-title.txt");
    let body_path = repo_root.join("scratch/pr-body.md");
    let ref_path = repo_root.join("scratch/.pr-copy-ref");

    if !title_path.exists() || !body_path.exists() {
        return Ok(None);
    }

    let title = std::fs::read_to_string(&title_path)?.trim().to_string();
    if title.is_empty() {
        return Ok(None);
    }

    let copied_for = match std::fs::read_to_string(&ref_path) {
        Ok(value) => value.trim().to_string(),
        Err(_) => {
            progress.status("Ignoring cached PR copy: scratch/.pr-copy-ref is missing");
            return Ok(None);
        }
    };
    if !is_recent_ancestor(repo_root, &copied_for, 1)? {
        progress.status("Ignoring cached PR copy: branch changed since gate output");
        return Ok(None);
    }

    let body = std::fs::read_to_string(body_path)?;
    Ok(Some((title, body)))
}

/// Check if HEAD is no more than `max_ahead` commits ahead of `commit`.
/// This tolerates one bookkeeping commit after gate output while still
/// forcing regeneration if substantive commits were added later.
fn is_recent_ancestor(repo_root: &Path, commit: &str, max_ahead: u32) -> OpsResult<bool> {
    let output = Command::new("git")
        .args(["rev-list", "--count", &format!("{commit}..HEAD")])
        .current_dir(repo_root)
        .output()?;
    if !output.status.success() {
        return Ok(false);
    }
    let ahead = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .unwrap_or(u32::MAX);
    Ok(ahead <= max_ahead)
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

    if !options.strict {
        let message = options
            .commit_message
            .clone()
            .unwrap_or_else(|| "lf land: stage uncommitted changes".to_string());
        let commit_options = CommitOptions {
            add: true,
            push: true,
            create_draft_pr: true,
            message: Some(message),
            agent: options.agent.clone(),
            ..CommitOptions::for_task("land")
        };
        let _ = commit_workflow(repo_root, &commit_options, progress)?;
    }

    Ok(())
}

fn rebase_land(repo_root: &Path, main_repo: &Path, progress: &impl Progress) -> OpsResult<String> {
    let main_branch = get_default_branch(main_repo)?;
    let onto = format!("origin/{main_branch}");
    // A stacked child collapses onto trunk deterministically: replay only
    // `base..HEAD`, dropping the (squash-)merged parent commits.
    let stacked = crate::ops::task::stacked_collapse(repo_root)?;
    crate::ops::rebase::rebase_with_recovery(
        repo_root,
        &crate::ops::rebase::RebaseOptions {
            onto: onto.clone(),
            push: true,
            fork_base: stacked.as_ref().map(|stacked| stacked.fork_base.clone()),
        },
        progress,
    )?;
    if let Some(stacked) = stacked {
        let new_base = crate::engine::git::rev_parse(repo_root, &onto)?;
        crate::ops::task::record_stack_rebase(&stacked, &new_base, true)?;
    }
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

#[allow(clippy::too_many_arguments)]
fn ensure_pr(
    repo_root: &Path,
    pr_exists: bool,
    feature_branch: &str,
    create_pr: bool,
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> OpsResult<()> {
    if !crate::ops::pr::gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }

    if !pr_exists {
        if create_pr {
            let title = pr_title.ok_or_else(|| {
                OpsError::Message(
                    "no PR title provided; run `lf gate` or pass --title/--body".to_string(),
                )
            })?;
            let body = pr_body.unwrap_or("");
            let _ = crate::ops::pr::create_or_update_pr(
                repo_root,
                &crate::ops::pr::PrOptions {
                    title: Some(title.to_string()),
                    body: Some(body.to_string()),
                    agent: agent_override.map(str::to_string),
                },
                progress,
            )?;
        } else {
            return Err(OpsError::Message(format!(
                "no open PR found for branch '{feature_branch}'; run lf pr open or use --create-pr"
            )));
        }
    }

    Ok(())
}

fn finalize_remote(
    repo_root: &Path,
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    finalize: Finalize,
    progress: &impl Progress,
) -> OpsResult<()> {
    if let Some(title) = pr_title {
        let body = pr_body.unwrap_or("");
        progress.status("Updating PR...");
        update_pr_message(repo_root, title, body)?;
    }
    mark_ready(repo_root)?;

    match finalize {
        Finalize::AutoMerge => {
            progress.status("Enabling auto-merge...");
            enable_auto_merge(repo_root, pr_title, pr_body)?;
        }
        Finalize::AssignForReview => {
            progress.status("Assigning PR for you to merge...");
            assign_to_me(repo_root)?;
        }
    }

    if let Some(url) = current_pr_url(repo_root)? {
        progress.status(&format!("\n{url}\n"));
    }

    Ok(())
}

/// Assign the open PR to the authenticated user so it lands in their review
/// queue. Nothing merges until they click merge — the required gate.
fn assign_to_me(repo: &Path) -> OpsResult<()> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("edit")
        .arg("--add-assignee")
        .arg("@me")
        .current_dir(repo);
    if let Err(err) = run_command(&mut cmd) {
        return Err(OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        });
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
    let gitkeep = scratch.join(".gitkeep");

    // Ensure scratch/ always exists.
    if !scratch.exists() {
        std::fs::create_dir_all(&scratch)?;
    }

    let mut removed = false;
    for entry in std::fs::read_dir(&scratch)? {
        let entry = entry?;
        let path = entry.path();
        // Preserve .gitkeep so the directory survives git operations.
        if path == gitkeep {
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
        removed = true;
    }

    // Ensure .gitkeep exists so scratch/ is tracked even when empty.
    if !gitkeep.exists() {
        std::fs::write(&gitkeep, "")?;
        removed = true; // needs staging
    }

    if !removed {
        return Ok(());
    }

    progress.status("Clearing scratch/...");
    crate::engine::git::stage_all(repo)?;
    if has_staged_changes(repo)? {
        crate::engine::git::commit(repo, "lf land: clear scratch/")?;
        crate::ops::commit::push_with_upstream_if_needed(repo)?;
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

fn enable_auto_merge(repo: &Path, title: Option<&str>, body: Option<&str>) -> OpsResult<()> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("merge").arg("--squash").arg("--auto");
    if let Some(title) = title {
        cmd.arg("--subject").arg(title);
    }
    if let Some(body) = body.filter(|b| !b.trim().is_empty()) {
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

fn auto_merge_enabled(repo: &Path) -> OpsResult<bool> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr")
        .arg("view")
        .arg("--json")
        .arg("autoMergeRequest")
        .arg("--jq")
        .arg(".autoMergeRequest != null")
        .current_dir(repo);
    let output = run_command(&mut cmd).map_err(|err| OpsError::CommandFailed {
        command: err.command_line(),
        stderr: err.stderr,
    })?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
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
