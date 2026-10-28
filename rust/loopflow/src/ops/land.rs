use std::path::{Path, PathBuf};
use std::process::Command;

use crate::git::worktrees::{main_repo_root, worktree_path};
use crate::git::{current_branch, get_default_branch, is_clean, land as git_land, LandStrategy};

use crate::command::run_command;
use crate::ops::commit::{commit_workflow, CommitOptions};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::pr::{
    generate_pr_copy, normalize_task_pr_copy, read_cached_pr_copy, PrCopy, PrInfo,
    TaskPrCopyLifecycle,
};

use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct LandOptions {
    pub strict: bool,
    pub local: bool,
    pub wait_and_fix: bool,
    pub worktree: Option<String>,
    pub commit_message: Option<String>,
    pub pr_title: Option<String>,
    pub pr_body: Option<String>,
    pub agent: Option<String>,
}

/// How a prepared PR is handed off once it is synced, cleaned up if configured, and
/// marked ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Finalize {
    /// Request auto-merge and retain delivery for later finite checks.
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

/// Prepare one PR: commit, merge main when GitHub needs it, apply configured
/// cleanup, mark ready, and finalize per `finalize`. `arm` requests auto-merge;
/// `submit` leaves the PR for manual merge. Neither rotates the worktree.
/// Returns the resulting PR, or `None` for a local merge.
fn prepare_pr(
    repo: &Path,
    options: &LandOptions,
    finalize: Finalize,
    integration: Integration,
    progress: &impl Progress,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<Option<PrInfo>> {
    let (repo_root, main_repo) = resolve_repos(repo, options.worktree.as_deref())?;
    if !options.local {
        crate::ops::task::refresh_task_pr_for_publication(&repo_root)?;
    }
    // Establish the Task's commit range before delivery availability checks.
    crate::ops::task::refresh_task_pr_base(&repo_root)?;
    if !options.local && !crate::git::has_origin(&repo_root)? {
        return Err(OpsError::Message(
            "hosted delivery requires an origin remote".to_string(),
        ));
    }
    crate::ops::pr::reject_control_plane_pr(&repo_root)?;
    crate::ops::sync::restart_landed_branch(&repo_root)?;
    if !options.local && !crate::ops::pr::gh_available() {
        return Err(OpsError::Message("gh CLI not found".to_string()));
    }
    let task_context = if options.local {
        None
    } else {
        crate::ops::task::task_pr_context(&repo_root)?
    };
    let feature_branch = current_branch(&repo_root)?
        .ok_or_else(|| OpsError::Message("not on a branch".to_string()))?;
    let resync = {
        let _mutation = crate::ops::task::lock_task_pr_mutation(&repo_root)?;
        if matches!(finalize, Finalize::AutoMerge) && matches!(integration, Integration::Required) {
            if let Some((number, head)) = crate::ops::task::matching_task_pr_merge_request(
                &repo_root,
                crate::work::task::PrMergeMode::Auto,
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
            } else if task_context.is_none() && !options.local && is_clean(&repo_root)? {
                let head = crate::git::rev_parse(&repo_root, "HEAD")?;
                if let Some(pr) = crate::ops::pr::current_pr(&repo_root)? {
                    if pr.head_sha.as_deref() == Some(head.as_str())
                        && crate::ops::pr::auto_merge_enabled(&repo_root, pr.number)?
                    {
                        update_pr_message(
                            &repo_root,
                            options.pr_title.as_deref(),
                            options.pr_body.as_deref(),
                            &|_| {},
                        )?;
                        progress.status("Pull request is already armed for this exact head");
                        return Ok(Some(pr));
                    }
                }
            }
        }
        // Read GitHub's merge state before disarming: a dequeue replaces the
        // eviction that may be the only record of a conflict.
        let resync =
            matches!(integration, Integration::Required) && needs_resync(&repo_root, options)?;
        let cleared_task_request = crate::ops::task::clear_task_pr_merge_before_head_mutation(
            &repo_root, true, inherit_pr,
        )?;
        if !options.local && !cleared_task_request {
            // Wave and other non-Task PRs have no durable Task request to
            // revoke, but GitHub may still have this branch in its merge queue.
            // Dequeue it before sync/push; otherwise GitHub rejects the head
            // update and the repair cannot publish.
            if let Some(pr) = crate::ops::pr::current_pr(&repo_root)? {
                let number = u32::try_from(pr.number).map_err(|_| {
                    OpsError::Message(format!(
                        "pull request #{} exceeds supported range",
                        pr.number
                    ))
                })?;
                crate::ops::pr::disable_auto_merge(&repo_root, number, inherit_pr)?;
            }
        }
        resync
    };
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
    // Refuse an already-empty Task before cleanup or any GitHub command observes it.
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    let pr_exists = if options.local {
        false
    } else {
        crate::ops::pr::pr_exists_for_current_branch(&repo_root)?
    };
    let copy_head = crate::git::rev_parse(&repo_root, "HEAD")?;
    let copy_state = read_worktree_state(&repo_root)?;
    let copy = normalize_task_pr_copy(
        resolve_pr_copy(&repo_root, &copy_head, options, progress)?,
        task_context.as_ref(),
        &TaskPrCopyLifecycle::Completes,
    )?;
    let current_head = crate::git::rev_parse(&repo_root, "HEAD")?;
    if current_head != copy_head || read_worktree_state(&repo_root)? != copy_state {
        return Err(OpsError::Message(
            "PR copy generation changed the worktree; refusing to integrate or finalize unreviewed provider mutations"
                .to_string(),
        ));
    }
    if matches!(integration, Integration::Required) {
        crate::ops::scratch::clear(&repo_root)?;
    }
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    let main_branch = match integration {
        Integration::Required if resync => sync_land(&repo_root, &main_repo, progress)?,
        Integration::Required => {
            crate::ops::commit::push_with_upstream_if_needed(&repo_root, &|_| {})?;
            get_default_branch(&main_repo)?
        }
        Integration::Completed => accept_completed_integration(&repo_root, &main_repo)?,
    };
    // Sync may advance the fork point. Re-run the authoritative proof to heal
    // the recorded base and refuse an empty range before any `gh pr` side effect.
    crate::ops::task::require_task_pr_range_nonempty(&repo_root)?;
    if pr_exists {
        crate::ops::pr::retarget_open_pr(&repo_root, &main_branch, inherit_pr)?;
    }
    if options.local {
        finalize_local(&repo_root, &main_branch, &feature_branch, progress)?;
        return Ok(None);
    }

    // Keep the publication read, exact-head request, remote finalization, and
    // any failure rollback atomic with respect to other Loopflow PR commands
    // and pushes in this worktree.
    let _mutation = crate::ops::task::lock_task_pr_mutation(&repo_root)?;
    crate::ops::task::request_task_pr_publication(&repo_root, &copy.title, &copy.body)?;
    let pr = if pr_exists {
        crate::ops::pr::current_pr(&repo_root)?
    } else {
        Some(crate::ops::pr::create_pr_from_pushed_branch(
            &repo_root,
            &copy.title,
            &copy.body,
            &main_branch,
            inherit_pr,
        )?)
    };
    crate::ops::task::attach_task_github_pr(&repo_root, pr.as_ref(), inherit_pr)?;
    crate::ops::task::record_task_pr_presentation(&repo_root, &copy.title, &copy.body)?;
    crate::ops::task::request_task_pr_merge(
        &repo_root,
        match finalize {
            Finalize::AutoMerge => crate::work::task::PrMergeMode::Auto,
            Finalize::UserMerge => crate::work::task::PrMergeMode::User,
        },
        pr.as_ref().and_then(|pr| pr.head_sha.as_deref()),
        inherit_pr,
    )?;
    if let Err(finalize_error) = finalize_remote(
        &repo_root,
        &copy,
        finalize,
        pr.as_ref().map(|pr| pr.number),
        pr.as_ref().and_then(|pr| pr.head_sha.as_deref()),
        progress,
        inherit_pr,
    ) {
        // The durable request is written before its remote executor. If any
        // later step fails, revoke a possibly-armed Auto request and clear the
        // local request so status never claims GitHub owns settlement when this
        // command did not complete. A failed revocation leaves the durable
        // request intact and reports both failures rather than guessing.
        if let Err(clear_error) =
            crate::ops::task::clear_task_pr_merge_before_head_mutation(&repo_root, true, inherit_pr)
        {
            return Err(OpsError::Message(format!(
                "{finalize_error}; failed to reconcile the durable merge request: {clear_error}"
            )));
        }
        return Err(finalize_error);
    }
    Ok(pr)
}

/// Prepare a PR and request exact-head auto-merge, then return without watching.
pub fn arm(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<Option<PrInfo>> {
    let pr = prepare_pr(
        repo,
        options,
        Finalize::AutoMerge,
        Integration::Required,
        progress,
        &|_| {},
    )?;
    if let Some(pr) = &pr {
        crate::ops::pr_landing::record_armed_pr(repo, options, pr)?;
    }
    Ok(pr)
}

/// Continue land after owned recovery already verified and pushed integration.
pub(crate) fn finish_arm_after_sync(
    repo: &Path,
    options: &LandOptions,
    progress: &impl Progress,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<Option<PrInfo>> {
    let pr = prepare_pr(
        repo,
        options,
        Finalize::AutoMerge,
        Integration::Completed,
        progress,
        inherit_pr,
    )?;
    if let Some(pr) = &pr {
        crate::ops::pr_landing::record_armed_pr(repo, options, pr)?;
    }
    Ok(pr)
}

/// Prepare a PR to land without arming auto-merge: commit, sync onto main,
/// apply configured scratch cleanup and mark ready. Reviewer requests and
/// assignments remain unchanged. A teammate merges on GitHub.
/// Like `arm`, this never rotates the worktree.
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
        &|_| {},
    )
}

pub(crate) fn finish_submit_after_sync(
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
        &|_| {},
    )
}

fn resolve_pr_copy(
    repo_root: &Path,
    head: &str,
    options: &LandOptions,
    progress: &impl Progress,
) -> OpsResult<PrCopy> {
    if options.local || options.pr_title.is_some() {
        return Ok(PrCopy {
            title: options.pr_title.clone().unwrap_or_default(),
            body: options.pr_body.clone().unwrap_or_default(),
        });
    }

    let mut copy = match read_cached_pr_copy(repo_root, progress)? {
        Some(copy) => {
            progress.status("Using cached PR copy from scratch/");
            copy
        }
        None => match crate::ops::pr::published_pr_copy(repo_root, head)? {
            Some(copy) => {
                progress.status("Keeping published PR copy for this head");
                copy
            }
            None => generate_pr_copy(repo_root, progress, options.agent.as_deref())?,
        },
    };
    if let Some(body) = &options.pr_body {
        copy.body = body.clone();
    }
    Ok(copy)
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

    if !options.strict && !crate::git::worktrees::is_persistent_worktree(repo_root)? {
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
        let _ = commit_workflow(repo_root, &commit_options, progress, &|_| {})?;
    }

    Ok(())
}

/// Merge main only when the land needs it: a local merge, a first publication,
/// a stacked child whose parent landed, or a PR GitHub cannot merge as it is.
/// A CI repair on a mergeable PR goes on top and keeps its tested base.
fn needs_resync(repo_root: &Path, options: &LandOptions) -> OpsResult<bool> {
    if options.local || crate::ops::task::stack_for_landing(repo_root)?.is_some() {
        return Ok(true);
    }
    match crate::ops::pr::current_pr(repo_root)? {
        Some(pr) => crate::ops::pr::pr_requires_resync(
            &crate::ops::pr::observe_pr_merge(repo_root, pr.number)?,
            || crate::ops::pr::merge_request_removal(repo_root, pr.number, &[]),
        ),
        None => Ok(true),
    }
}

fn sync_land(repo_root: &Path, main_repo: &Path, progress: &impl Progress) -> OpsResult<String> {
    let main_branch = get_default_branch(main_repo)?;
    let onto = crate::git::integration_base(main_repo)?;
    // Use the recorded parent base when integrating a squash-landed stack.
    let stacked = crate::ops::task::stack_for_landing(repo_root)?;
    let verification = crate::ops::sync::sync_for_delivery(
        repo_root,
        &crate::ops::sync::SyncOptions {
            onto: onto.clone(),
            push: true,
            fork_base: stacked.as_ref().map(|stacked| stacked.fork_base.clone()),
        },
        progress,
    )?;
    if let Some(stacked) = stacked {
        // Record the immutable target proven by the integration owner. Another
        // worktree may fetch and move origin/main after our pinned fetch.
        crate::ops::task::record_stack_sync(&stacked, &verification.target_sha, true)?;
    }
    Ok(main_branch)
}

fn accept_completed_integration(repo_root: &Path, main_repo: &Path) -> OpsResult<String> {
    let main_branch = get_default_branch(main_repo)?;
    if let Some(stacked) = crate::ops::task::stack_for_landing(repo_root)? {
        // The merge's second parent is the immutable integration target.
        let new_base = crate::git::rev_parse(repo_root, "HEAD^2")?;
        crate::ops::task::record_stack_sync(&stacked, &new_base, true)?;
    }
    Ok(main_branch)
}

fn finalize_local(
    repo_root: &Path,
    main_branch: &str,
    feature_branch: &str,
    progress: &impl Progress,
) -> OpsResult<()> {
    let merged_head = crate::git::rev_parse(repo_root, "HEAD")?;
    progress.status("Merging locally...");
    let _ = git_land(repo_root, LandStrategy::LocalMerge, main_branch)?;
    // Only the head just merged; a remote that moved since carries new work.
    let remote = crate::ops::wt::branch_copies_at(
        &main_repo_root(repo_root)?,
        feature_branch,
        Some(merged_head),
        None,
    );
    crate::ops::wt::apply_delete(remote, progress)
}

#[allow(clippy::too_many_arguments)] // Remote PR facts and explicit child capability inheritance.
fn finalize_remote(
    repo_root: &Path,
    copy: &PrCopy,
    finalize: Finalize,
    number: Option<u64>,
    head_sha: Option<&str>,
    progress: &impl Progress,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<()> {
    progress.status("Updating PR...");
    update_pr_message(repo_root, Some(&copy.title), Some(&copy.body), inherit_pr)?;
    mark_ready(repo_root, inherit_pr)?;

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
            crate::ops::pr::enable_auto_merge(repo_root, number, Some(copy), head_sha, inherit_pr)?;
        }
        Finalize::UserMerge => {
            progress.status(
                "PR prepared for manual merge. Reviewer requests and assignments are unchanged.",
            );
        }
    }

    if let Some(url) = current_pr_url(repo_root)? {
        progress.status(&format!("\n{url}\n"));
    }

    Ok(())
}

pub(crate) fn resolve_repos(repo: &Path, worktree: Option<&str>) -> OpsResult<(PathBuf, PathBuf)> {
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

fn update_pr_message(
    repo: &Path,
    title: Option<&str>,
    body: Option<&str>,
    inherit_pr: &impl Fn(&mut Command),
) -> OpsResult<()> {
    if title.is_none() && body.is_none() {
        return Ok(());
    }
    let mut cmd = Command::new("gh");
    cmd.args(["pr", "edit"]).current_dir(repo);
    if let Some(title) = title {
        cmd.args(["--title", title]);
    }
    if let Some(body) = body {
        cmd.args(["--body", body]);
    }
    inherit_pr(&mut cmd);
    if let Err(err) = run_command(&mut cmd) {
        return Err(OpsError::CommandFailed {
            command: err.command_line(),
            stderr: err.stderr,
        });
    }
    Ok(())
}

pub fn mark_ready(repo: &Path, inherit_pr: &impl Fn(&mut Command)) -> OpsResult<()> {
    let mut cmd = Command::new("gh");
    cmd.arg("pr").arg("ready").current_dir(repo);
    inherit_pr(&mut cmd);
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
