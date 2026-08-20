use std::path::{Path, PathBuf};
use std::process::Command;

use crate::engine::git::{
    current_branch, get_default_branch, is_clean, land as git_land, LandStrategy,
};
use crate::engine::worktrees::{main_repo_root, worktree_path};

use crate::engine::command::run_command;
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pr::{generate_pr_copy, read_cached_pr_copy, PrInfo};

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
    UserMerge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Integration {
    Required,
    Completed,
}

/// Run every land skill: commit, rebase onto main, clear scratch, mark the PR
/// ready, and finalize per `finalize`. `land` arms auto-merge; `submit` assigns
/// the PR for a human to merge. Neither rotates the worktree — the wave home is
/// permanent. Returns the resulting PR, or `None` for a local merge or direct
/// Task completion over an already-merged PR, so callers can record it against
/// the run.
fn prepare_pr(
    repo: &Path,
    options: &LandOptions,
    finalize: Finalize,
    integration: Integration,
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
    crate::ops::pr::reject_control_plane_pr(&repo_root)?;
    if options.complete && (!options.strict || is_clean(&repo_root)?) {
        if let Some(issue) = crate::ops::task::find_discardable_final_task(&repo_root)? {
            // The final flow approved work that already landed, and rotation left
            // one unpublished branch at its recorded base. Consume that approval
            // instead of manufacturing either an empty GitHub PR or a new outcome.
            // Pre-final Tasks continue through the ordinary range refusal.
            clear_scratch(&repo_root, progress)?;
            crate::ops::task::task_complete_approved(&issue)?;
            progress.status("Completed Task over its merged pull request.");
            return Ok(None);
        }
    }
    if !options.local && !crate::ops::pr::gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    let feature_branch = current_branch(&repo_root)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    // Prove the Task PR range before preparing the exact local tree. Submit and
    // land make no remote mutation until the owned integration pushes once.
    crate::ops::task::verify_task_pr_range(&repo_root)?;
    {
        let _mutation = crate::ops::task::lock_task_pr_mutation(&repo_root)?;
        if matches!(finalize, Finalize::AutoMerge) && matches!(integration, Integration::Required) {
            let after_merge = if options.complete {
                crate::task::AfterMerge::CompleteTask
            } else {
                crate::task::AfterMerge::ContinueTask
            };
            if let Some((number, head)) = crate::ops::task::matching_task_pr_merge_request(
                &repo_root,
                crate::task::PrMergeMode::Auto,
                after_merge,
                options.next_slug.as_deref(),
            )? {
                if let Some(pr) = crate::ops::pr::current_pr(&repo_root)? {
                    if pr.number == u64::from(number)
                        && pr.head_sha.as_deref() == Some(head.as_str())
                        && crate::ops::pr::auto_merge_enabled(&repo_root, pr.number)?
                    {
                        progress.status("Pull request is already armed for this exact head");
                        return Ok(Some(pr));
                    }
                }
            }
        }
        let cleared_task_request =
            crate::ops::task::clear_task_pr_merge_before_head_mutation(&repo_root, true)?;
        if !options.local && !cleared_task_request {
            // Wave and other non-Task PRs have no durable Task request to
            // revoke, but GitHub may still have this branch in its merge queue.
            // Dequeue it before rebase/push; otherwise GitHub rejects the head
            // update and the repair cannot publish.
            if let Some(pr) = crate::ops::pr::current_pr(&repo_root)? {
                let number = u32::try_from(pr.number).map_err(|_| {
                    OpsError::Message(format!(
                        "pull request #{} exceeds supported range",
                        pr.number
                    ))
                })?;
                crate::ops::pr::disable_auto_merge(&repo_root, number)?;
            }
        }
    }
    match integration {
        Integration::Required => prepare_land(&repo_root, options, progress)?,
        Integration::Completed if !is_clean(&repo_root)? => {
            return Err(OpsError::Message(
                "recovered integration left uncommitted changes; refusing to mutate the verified head before PR finalization"
                    .to_string(),
            ));
        }
        Integration::Completed => {}
    }
    // Refuse an already-empty Task before scratch cleanup can manufacture a
    // bookkeeping-only `.gitkeep` commit or any GitHub command observes it.
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    let pr_exists = if options.local {
        false
    } else {
        crate::ops::pr::pr_exists_for_current_branch(&repo_root)?
    };
    if !options.local && !pr_exists && !options.create_pr {
        return Err(OpsError::Message(format!(
            "no open PR found for branch '{feature_branch}'; run lf pr open or use --create-pr"
        )));
    }
    let copy_head = crate::engine::git::rev_parse(&repo_root, "HEAD")?;
    let copy_state = read_worktree_state(&repo_root)?;
    let (pr_title, pr_body) = resolve_pr_copy(&repo_root, options, progress)?;
    let current_head = crate::engine::git::rev_parse(&repo_root, "HEAD")?;
    if current_head != copy_head || read_worktree_state(&repo_root)? != copy_state {
        return Err(OpsError::Message(
            "PR copy generation changed the worktree; refusing to integrate or finalize unreviewed provider mutations"
                .to_string(),
        ));
    }
    if matches!(integration, Integration::Required) {
        clear_scratch(&repo_root, progress)?;
    }
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    let main_branch = match integration {
        Integration::Required => rebase_land(&repo_root, &main_repo, progress)?,
        Integration::Completed => accept_completed_integration(&repo_root, &main_repo)?,
    };
    // Rebase may advance the fork point. Re-run the authoritative proof to heal
    // the recorded base and refuse an empty range before any `gh pr` side effect.
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    if pr_exists {
        crate::ops::pr::retarget_open_pr(&repo_root, &main_branch)?;
    }
    if options.local {
        finalize_local(&repo_root, &main_branch, &feature_branch, progress)?;
        return Ok(None);
    }

    // Keep the publication read, exact-head request, remote finalization, and
    // any failure rollback atomic with respect to other Loopflow PR commands
    // and pushes in this worktree.
    let _mutation = crate::ops::task::lock_task_pr_mutation(&repo_root)?;
    crate::ops::task::request_task_pr_publication(&repo_root)?;
    let created_pr = ensure_pr(
        &repo_root,
        pr_exists,
        &feature_branch,
        options.create_pr,
        &main_branch,
        pr_title.as_deref(),
        pr_body.as_deref(),
    )?;
    let pr = match created_pr {
        Some(pr) => Some(pr),
        None => crate::ops::pr::current_pr(&repo_root)?,
    };
    crate::ops::task::attach_task_github_pr(&repo_root, pr.as_ref())?;
    crate::ops::task::request_task_pr_merge(
        &repo_root,
        match finalize {
            Finalize::AutoMerge => crate::task::PrMergeMode::Auto,
            Finalize::UserMerge => crate::task::PrMergeMode::User,
        },
        pr.as_ref().and_then(|pr| pr.head_sha.as_deref()),
        if options.complete {
            crate::task::AfterMerge::CompleteTask
        } else {
            crate::task::AfterMerge::ContinueTask
        },
        options.next_slug.as_deref(),
    )?;
    if let Err(finalize_error) = finalize_remote(
        &repo_root,
        pr_title.as_deref(),
        pr_body.as_deref(),
        finalize,
        pr.as_ref().map(|pr| pr.number),
        pr.as_ref().and_then(|pr| pr.head_sha.as_deref()),
        progress,
    ) {
        // The durable request is written before its remote executor. If any
        // later step fails, revoke a possibly-armed Auto request and clear the
        // local request so status never claims GitHub owns settlement when this
        // command did not complete. A failed revocation leaves the durable
        // request intact and reports both failures rather than guessing.
        if let Err(clear_error) =
            crate::ops::task::clear_task_pr_merge_before_head_mutation(&repo_root, true)
        {
            return Err(OpsError::Message(format!(
                "{finalize_error}; failed to reconcile the durable merge request: {clear_error}"
            )));
        }
        return Err(finalize_error);
    }
    Ok(pr)
}

/// Land a PR and walk away: commit, rebase, clear scratch, and arm auto-merge so
/// GitHub merges it once checks pass. Task worktrees self-prune once their PR
/// merges; permanent Wave homes are rejected before any mutation.
pub fn land(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    prepare_pr(
        repo,
        options,
        Finalize::AutoMerge,
        Integration::Required,
        progress,
    )
}

/// Continue land after owned recovery already verified and pushed integration.
pub(crate) fn finish_land_after_rebase(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    prepare_pr(
        repo,
        options,
        Finalize::AutoMerge,
        Integration::Completed,
        progress,
    )
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
    prepare_pr(
        repo,
        options,
        Finalize::UserMerge,
        Integration::Required,
        progress,
    )
}

pub(crate) fn finish_submit_after_rebase(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    prepare_pr(
        repo,
        options,
        Finalize::UserMerge,
        Integration::Completed,
        progress,
    )
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

    if let Some(copy) = read_cached_pr_copy(repo_root, progress)? {
        progress.status("Using cached PR copy from scratch/");
        pr_title = Some(copy.title);
        if pr_body.is_none() {
            pr_body = Some(copy.body);
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
            push: false,
            create_draft_pr: false,
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
    let verification = crate::ops::rebase::rebase_final_with_recovery(
        repo_root,
        &crate::ops::rebase::RebaseOptions {
            onto: onto.clone(),
            push: true,
            fork_base: stacked.as_ref().map(|stacked| stacked.fork_base.clone()),
        },
        progress,
    )?;
    if let Some(stacked) = stacked {
        // Record the immutable target proven by the integration owner. Another
        // worktree may fetch and move origin/main after our pinned fetch.
        crate::ops::task::record_stack_rebase(&stacked, &verification.target_sha, true)?;
    }
    Ok(main_branch)
}

fn accept_completed_integration(repo_root: &Path, main_repo: &Path) -> OpsResult<String> {
    let main_branch = get_default_branch(main_repo)?;
    if let Some(stacked) = crate::ops::task::stacked_collapse(repo_root)? {
        // Final integration is exactly one collapsed authored commit. Its
        // parent is the immutable target the owner already verified; reading a
        // moving origin/main here could record a newer, unreachable base.
        let new_base = crate::engine::git::rev_parse(repo_root, "HEAD^")?;
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
    base_branch: &str,
    pr_title: Option<&str>,
    pr_body: Option<&str>,
) -> OpsResult<Option<PrInfo>> {
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
            return crate::ops::pr::create_pr_from_pushed_branch(
                repo_root,
                title,
                body,
                base_branch,
            )
            .map(Some);
        } else {
            return Err(OpsError::Message(format!(
                "no open PR found for branch '{feature_branch}'; run lf pr open or use --create-pr"
            )));
        }
    }

    Ok(None)
}

fn finalize_remote(
    repo_root: &Path,
    pr_title: Option<&str>,
    pr_body: Option<&str>,
    finalize: Finalize,
    number: Option<u64>,
    head_sha: Option<&str>,
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
            let number = number.ok_or_else(|| {
                OpsError::Message(
                    "GitHub did not report the current PR number; refusing to arm auto-merge"
                        .to_string(),
                )
            })?;
            let head_sha = head_sha.ok_or_else(|| {
                OpsError::Message(
                    "GitHub did not report the current PR head; refusing to arm unpinned auto-merge"
                        .to_string(),
                )
            })?;
            progress.status("Enabling auto-merge...");
            crate::ops::pr::enable_auto_merge(repo_root, number, pr_title, pr_body, head_sha)?;
        }
        Finalize::UserMerge => {
            progress.status("Assigning PR for you to merge...");
            assign_to_me(repo_root)?;
        }
    }

    if let Some(url) = current_pr_url(repo_root)? {
        progress.status(&format!("\n{url}\n"));
    }

    Ok(())
}

/// Assign the open PR to the authenticated user. Nothing merges until they
/// explicitly click merge.
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

/// Clear `scratch/` down to `.gitkeep` and commit it.
///
/// This step is what greens the `scratch-clear` required check, which is why
/// that check is a *land-time precondition* and no Task repair turn can act on
/// it — see [`crate::task::CiCheck::land_time_precondition`], which classifies it
/// so a ci-fix wake never arms against work only this function can do.
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

fn read_worktree_state(repo: &Path) -> OpsResult<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1"])
        .current_dir(repo)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "git status --porcelain=v1".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
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
