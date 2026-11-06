use crate::agent::{run_agent, ProcessConfig};
use crate::config::{load_config_or_default, Config};
use crate::git::worktree_name::WorktreeName;
use crate::git::worktrees::{
    diff_shortstats, list_worktrees, list_worktrees_timed, main_repo_root, prune_worktrees,
    sibling_worktree_name, sibling_worktree_name_with_main, PullRequestState, WorktreeOwnership,
    WorktreePruneReport,
};
use crate::git::{current_branch, get_default_branch};
use crate::graph::load_skill;
use crate::lf::commands::util::find_repo_root;
use crate::lf::output::{column_width, Colors};
use crate::lf::{PrCommand, ReleaseCommand, WtCommand};
use crate::naming::git_user;
use crate::ops::branch_prune::{
    prune_branches, BranchCopy, EntryStatus as BranchPruneStatus, TaskBranches,
};
use crate::ops::OpsError;
use crate::ops::{
    abort_sync_after_authorization, abort_sync_for_resolution, continue_sync_after_authorization,
    continue_sync_for_resolution, current_pr, plan_sync, preview_release_notes, recover_sync,
    release_bump, release_check, release_issues, release_notes, release_promote, release_publish,
    release_regress, release_status, release_tag, sync_class_name, sync_strategy_name,
    sync_with_plan, Progress, SyncOptions,
};
use crate::prompt::process::prepare_process_prompt;
use crate::prompt::process::ContextSourceOverrides;
use crate::prompt::process::ProcessPromptInput;
use crate::prompt::Surface;
use crate::skills::sync_skills;
use crate::skills::SkillSyncOptions;
use anyhow::{anyhow, Result};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

mod review;

pub fn run_pr(cmd: Option<&PrCommand>, cli_agent: Option<&str>) -> Result<()> {
    use crate::engine::pr as engine;
    let progress = CliProgress;
    let repo_root = find_repo_root()?;
    let land_input = |strict: &bool,
                      local: bool,
                      wait_and_fix: bool,
                      worktree: &Option<String>,
                      message: &Option<String>,
                      title: &Option<String>,
                      body: &Option<String>| engine::LandInput {
        strict: *strict,
        local,
        wait_and_fix,
        worktree: worktree.clone(),
        message: message.clone(),
        title: title.clone(),
        body: body.clone(),
        agent: cli_agent.map(str::to_string),
    };
    match cmd {
        None => pr_status(&repo_root),
        Some(PrCommand::Reconcile { json }) => {
            let report = engine::reconcile(&repo_root)?;
            if *json {
                print_json(&report)?;
            }
            if !report.errors.is_empty() {
                anyhow::bail!(report.errors.join("\n"));
            }
            progress.status("delivery check complete");
            Ok(())
        }
        Some(PrCommand::Checks {
            watch,
            logs,
            required,
            json,
        }) => pr_checks(&repo_root, *watch, *logs, *required, *json),
        Some(PrCommand::RequestReview { pr, reviewer, json }) => {
            let requested = engine::request_review(&repo_root, pr.as_deref(), reviewer)
                .map_err(engine::exit_of)?;
            if *json {
                print_json(&requested)?;
            }
            Ok(())
        }
        Some(PrCommand::Reviews { pr, json }) => {
            let reviews = engine::reviews(&repo_root, pr.as_deref()).map_err(engine::exit_of)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&reviews)?);
            } else {
                print!("{}", review::render(&reviews));
            }
            Ok(())
        }
        Some(PrCommand::Publish { title, body, json }) => {
            let published = engine::publish(
                &repo_root,
                title.as_deref(),
                body.as_deref(),
                cli_agent,
                &progress,
            )?;
            print_published(&published, *json)
        }
        Some(PrCommand::Open { title, body, json }) => {
            let published = engine::open(
                &repo_root,
                title.as_deref(),
                body.as_deref(),
                cli_agent,
                &progress,
            )?;
            // The PR exists — print it before presenting so a failed
            // review-surface launch fails only `pr open` and never hides the PR.
            print_published(&published, *json)?;
            crate::ops::present_pr_review(&published.url).map_err(|err| {
                anyhow!(
                    "PR available at {} but opening it for review failed: {err}",
                    published.url
                )
            })?;
            Ok(())
        }
        Some(PrCommand::Submit {
            strict,
            worktree,
            message,
            title,
            body,
            json,
        }) => {
            let prepared = engine::submit(
                &repo_root,
                &land_input(strict, false, false, worktree, message, title, body),
                &progress,
            )?;
            if *json {
                print_json(&prepared)?;
            }
            progress.status("Ready to land — click merge on the PR once checks pass.");
            Ok(())
        }
        Some(PrCommand::Arm {
            strict,
            local,
            worktree,
            message,
            title,
            body,
            json,
        }) => {
            let landed = engine::arm(
                &repo_root,
                &land_input(strict, *local, false, worktree, message, title, body),
                &progress,
            )?;
            if *json {
                print_json(&landed)?;
            }
            Ok(())
        }
        Some(PrCommand::Land {
            wait_and_fix,
            strict,
            local,
            worktree,
            message,
            title,
            body,
            json,
        }) => {
            let landed = engine::carry_land(
                &repo_root,
                &land_input(
                    strict,
                    *local,
                    *wait_and_fix,
                    worktree,
                    message,
                    title,
                    body,
                ),
                &progress,
            )?;
            if *json {
                print_json(&landed)?;
            }
            Ok(())
        }
        Some(PrCommand::Abandon {
            force,
            branch,
            json,
        }) => {
            let abandoned = engine::abandon(&repo_root, branch.as_deref(), *force, &progress)?;
            if *json {
                print_json(&abandoned)?;
            }
            Ok(())
        }
    }
}

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

/// Print the PR's state and URL; the raw URL when GitHub state wasn't re-read.
fn print_published(published: &crate::engine::pr::Published, json: bool) -> Result<()> {
    if json {
        return print_json(published);
    }
    let verb = if published.created {
        "created"
    } else {
        "updated"
    };
    match &published.pr {
        Some(pr) => println!("{verb} #{} {} {}", pr.number, pr.state, published.url),
        None => println!("{verb} {}", published.url),
    }
    Ok(())
}

pub fn run_release(cmd: &ReleaseCommand) -> Result<()> {
    let progress = CliProgress;
    match cmd {
        ReleaseCommand::Run {
            version,
            target,
            json,
        } => release_run_cmd(version.as_deref(), target.as_deref(), *json, &progress),
        ReleaseCommand::Check { target } => release_check_cmd(target.as_deref()),
        ReleaseCommand::Notes {
            version,
            prev_tag,
            preview,
            target,
        } => release_notes_cmd(
            version,
            prev_tag.as_deref(),
            target.as_deref(),
            *preview,
            &progress,
        ),
        ReleaseCommand::Bump { version, target } => {
            release_bump_cmd(version, target.as_deref(), &progress)
        }
        ReleaseCommand::Tag { version, target } => release_tag_cmd(version, target.as_deref()),
        ReleaseCommand::Publish {
            tag,
            notes,
            assets,
            finalize,
        } => {
            let repo_root = find_repo_root()?;
            release_publish(&repo_root, tag, notes.as_deref(), assets, *finalize)?;
            println!(
                "GitHub Release {tag}: {}",
                if *finalize {
                    "published"
                } else {
                    "draft staged"
                }
            );
            Ok(())
        }
        ReleaseCommand::Status { target } => release_status_cmd(target.as_deref()),
        ReleaseCommand::Issues {
            version,
            json,
            target,
        } => release_issues_cmd(version.as_deref(), *json, target.as_deref()),
        ReleaseCommand::Regress { version, title } => {
            println!("{}", release_regress(&find_repo_root()?, version, title)?);
            Ok(())
        }
        ReleaseCommand::Promote { dry_run, target } => {
            release_promote_cmd(*dry_run, target.as_deref(), &progress)
        }
    }
}

#[derive(Debug)]
pub(crate) struct CliProgress;

impl Progress for CliProgress {
    fn status(&self, msg: &str) {
        println!("{}", msg);
    }

    fn error(&self, msg: &str) {
        eprintln!("{}", msg);
    }

    fn warning(&self, msg: &str) {
        eprintln!("{}", msg);
    }

    fn confirm(&self, msg: &str) -> bool {
        print!("{} [y/N]: ", msg);
        let _ = io::stdout().flush();
        let mut input = String::new();
        if io::stdin().read_line(&mut input).is_err() {
            return false;
        }
        matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
    }
}

pub fn run_sync(input: &crate::engine::checkout::SyncInput<'_>, json: bool) -> Result<()> {
    let repo_root = crate::repo::require_repo_root(&std::env::current_dir()?, "lf sync")?;
    let outcome = crate::engine::checkout::carry_sync(&repo_root, input, &CliProgress)?;
    if json {
        println!("{}", serde_json::to_string(&outcome)?);
    } else {
        println!("{}", outcome.render());
    }
    Ok(())
}

/// `lf sync` in `repo_root`. A sync that moves the branch first announces
/// itself to the launcher that started it detached, if one did.
pub(crate) fn run_sync_in(
    repo_root: &Path,
    input: &crate::engine::checkout::SyncInput<'_>,
    progress: &impl Progress,
) -> Result<crate::engine::checkout::Sync> {
    use crate::engine::checkout::Sync;
    let crate::engine::checkout::SyncInput {
        onto,
        preview: plan_only,
        manual,
        continue_sync,
        abort,
        adopt,
    } = *input;
    if onto.is_some() && (continue_sync || abort) {
        return Err(anyhow!(
            "a sync target cannot be combined with --continue or --abort"
        ));
    }
    if adopt && !(continue_sync || abort) {
        return Err(anyhow!(
            "--adopt is only valid with `lf sync --continue` or `lf sync --abort`"
        ));
    }
    if continue_sync {
        if adopt {
            continue_sync_after_authorization(repo_root, true, || {
                crate::ops::task::record_task_pr_repair(
                    repo_root,
                    crate::work::task::TaskPrRepairKind::ManualGitRepair,
                )
                .map(|_| ())
            })?;
        } else {
            continue_sync_for_resolution(repo_root, false)?;
        }
        return Ok(Sync::Finished {
            message: "Sync complete; branch remains local.".into(),
        });
    }
    if abort {
        if adopt {
            abort_sync_after_authorization(repo_root, true, || {
                crate::ops::task::record_task_pr_repair(
                    repo_root,
                    crate::work::task::TaskPrRepairKind::ManualGitRepair,
                )
                .map(|_| ())
            })?;
        } else {
            abort_sync_for_resolution(repo_root, false)?;
        }
        return Ok(Sync::Finished {
            message: "Sync aborted.".into(),
        });
    }
    if !plan_only {
        crate::engine::launch::carry(
            "checkout.sync",
            format!("sync onto {}", onto.unwrap_or("the branch it follows")),
        )?;
    }
    let started = Instant::now();
    let default = get_default_branch(repo_root)?;
    let upstream = crate::git::integration_base(repo_root)?;
    let on_main = current_branch(repo_root)?.as_deref() == Some(&default);
    if !plan_only && (onto.is_none() || (on_main && onto == Some(upstream.as_str()))) {
        crate::ops::checkout::refresh_main(repo_root, progress)?;
        if on_main && onto.is_none_or(|target| target == upstream) {
            return Ok(Sync::Finished {
                message: "Main is current; unpublished commits and edits remain local.".into(),
            });
        }
    }
    // An explicit default branch detaches the child using its recorded fork;
    // ordinary sync continues to follow the live parent.
    let stacked = if on_main {
        None
    } else {
        crate::ops::task::task_stack(repo_root)?
    };
    let detach =
        stacked.is_some() && onto.is_some_and(|target| target == default || target == upstream);
    if stacked.is_some() && onto.is_some() && !detach {
        return Err(anyhow!(
            "use the default branch to detach this Task, or omit the target to sync its parent"
        ));
    }
    let stacked_onto = stacked
        .as_ref()
        .and_then(|stacked| stacked.parent_branch.as_ref())
        .map(|branch| format!("origin/{branch}"));
    let fork_base = stacked.as_ref().map(|stacked| stacked.fork_base.clone());
    let default_target = if on_main { &upstream } else { &default };
    let target = if detach {
        upstream.as_str()
    } else {
        stacked_onto.as_deref().or(onto).unwrap_or(default_target)
    };
    if !plan_only {
        crate::git::fetch_tracking_ref(repo_root, target)?;
    }
    // Classify against the tree holding the caller's edits. The plan that
    // runs is the one the metric records.
    let mut plan = plan_sync(repo_root, Some(target), fork_base.clone())?;
    if plan_only {
        return Ok(Sync::Planned { plan });
    }
    // Checkpoint ordinary edits so their conflicts reach the merge resolver,
    // rather than surfacing during stash restoration after the resolver is gone.
    // The checkpoint moves HEAD, so plan again against it.
    if !manual
        && !on_main
        && matches!(plan.strategy, crate::ops::SyncStrategy::MergeTarget)
        && crate::git::intervention_state(repo_root)?.is_none()
        && checkpoint_uncommitted_work(repo_root, &plan.base_ref, progress)?
    {
        plan = plan_sync(repo_root, Some(target), fork_base.clone())?;
    }
    let options = SyncOptions {
        onto: plan.base_ref.clone(),
        push: !manual && !on_main,
        fork_base,
    };
    let avoidable = is_avoidable_sync_class(&plan.class);
    let (verification, agent_launched) = if manual {
        (sync_with_plan(repo_root, &options, &plan, progress)?, false)
    } else {
        let recovery_config = load_config_or_default(Some(repo_root));
        crate::ops::checkout::with_preserved_edits(repo_root, || {
            match sync_with_plan(repo_root, &options, &plan, progress) {
                Ok(verification) => Ok((verification, false)),
                Err(OpsError::SyncConflict {
                    onto,
                    detail,
                    recovery,
                }) => Ok((
                    resolve_sync_conflict(
                        repo_root,
                        &onto,
                        &detail,
                        recovery,
                        avoidable,
                        progress,
                        &recovery_config,
                    )
                    .map_err(|error| OpsError::Message(error.to_string()))?,
                    true,
                )),
                Err(err) => Err(err),
            }
        })?
    };
    if manual {
        return Ok(Sync::Finished {
            message: format!(
                "Synced onto {}; the sync and edits remain local.",
                plan.base_ref
            ),
        });
    }
    record_ops_metric(
        repo_root,
        serde_json::json!({
            "op": "sync",
            "branch": plan.branch,
            "base_ref": plan.base_ref,
            "class": sync_class_name(&plan.class),
            "strategy": sync_strategy_name(&plan.strategy),
            "unique_commits": verification.unique_commits,
            "changed_files": plan.changed_files.len(),
            "protected": matches!(plan.class, crate::ops::SyncClass::Protected),
            "scratch_stashed": plan.scratch_stashed,
            "agent_launched": agent_launched,
            "duration_ms": started.elapsed().as_millis(),
            "exit_status": "ok",
        }),
    );
    Ok(Sync::Finished {
        message: format!("Synced onto {}.", plan.base_ref),
    })
}

/// Checkpoint ordinary edits while retaining telemetry's staged and working contents.
fn checkpoint_uncommitted_work(repo: &Path, base: &str, progress: &impl Progress) -> Result<bool> {
    // A literal `.lf/tmp` prefix makes Git reject the pathspec when ignored.
    // Use the same rooted glob when staging and refreshing the real index.
    const EXCLUDE_TELEMETRY: &str = ":(exclude,top,glob)[.]lf/tmp/**";
    crate::git::git_stdout(repo, &["add", "-A", "--", ":/", EXCLUDE_TELEMETRY])?;
    // Start from the staged tree so explicitly staged ignored user files survive.
    // Excluding telemetry from `add` alone cannot exclude already-staged entries.
    let staged_tree = crate::git::git_stdout(repo, &["write-tree"])?;
    let directory = tempfile::tempdir()?;
    let index = directory.path().join("index");
    let inherit_index = |command: &mut Command| {
        command.env("GIT_INDEX_FILE", &index);
    };
    let run = |args: &[&str]| crate::git::git_stdout_inheriting(repo, args, &inherit_index);
    run(&["read-tree", staged_tree.trim()])?;
    run(&["reset", "-q", "HEAD", "--", ":(top,glob)[.]lf/tmp/**"])?;
    // Filenames can consist entirely of whitespace.
    if run(&["diff", "--cached", "--name-only"])?.is_empty() {
        return Ok(false);
    }
    progress.status("Committing uncommitted edits before merging...");
    crate::git::commit(
        repo,
        &format!("Checkpoint edits before syncing onto {base}"),
        &inherit_index,
    )?;
    // Refresh only committed paths; the merge's existing preservation wrapper
    // carries the remaining telemetry through the merge with its index intact.
    crate::git::git_stdout(
        repo,
        &["reset", "-q", "HEAD", "--", ":/", EXCLUDE_TELEMETRY],
    )
    .map_err(|error| anyhow!("checkpoint saved; index refresh failed: {error}"))?;
    Ok(true)
}

/// Hand a conflicted sync to exactly one recovery agent under the owning
/// operation's scoped ids. The agent continues the existing sequencer; the
/// caller keeps ownership and performs verification and the single push.
fn resolve_sync_conflict(
    repo_root: &Path,
    onto: &str,
    detail: &str,
    recovery: Option<Box<crate::ops::SyncRecovery>>,
    avoidable: bool,
    progress: &impl Progress,
    config: &Config,
) -> Result<crate::ops::SyncVerification> {
    let recovery =
        recovery.ok_or_else(|| anyhow!("sync conflict has no owned recovery operation"))?;
    let context = format!(
        "<lf:sync-conflict>\nSync onto: {onto}\n{detail}\nContinue the existing owned sequencer; do not start another sync or push.\n</lf:sync-conflict>"
    );
    if avoidable {
        crate::ops::task::record_task_pr_repair(
            repo_root,
            crate::work::task::TaskPrRepairKind::AvoidableRebaseAgent,
        )?;
    }
    progress.status("Launching sync agent to resolve conflicts...");
    Ok(recover_sync(*recovery, || {
        process_skill_agent(repo_root, "sync-conflicts", Some(&context), config)
            .map_err(|error| OpsError::Message(error.to_string()))
    })?)
}

fn is_avoidable_sync_class(class: &crate::ops::SyncClass) -> bool {
    matches!(
        class,
        crate::ops::SyncClass::StaleEmpty | crate::ops::SyncClass::ScratchOnly
    )
}

#[cfg(test)]
mod sync_performance_tests {
    use super::is_avoidable_sync_class;
    use crate::ops::SyncClass;

    #[test]
    fn disposable_sync_classes_make_an_agent_launch_a_repair_incident() {
        assert!(is_avoidable_sync_class(&SyncClass::StaleEmpty));
        assert!(is_avoidable_sync_class(&SyncClass::ScratchOnly));
        assert!(!is_avoidable_sync_class(&SyncClass::CleanAuthored));
        assert!(!is_avoidable_sync_class(&SyncClass::Protected));
    }
}

/// Run a PR-mutating op; on a sync conflict, launch the sync agent to
/// resolve it and retry once. A second conflict is a real error.
pub(crate) fn with_sync_retry<T>(
    repo_root: &Path,
    label: &str,
    progress: &impl Progress,
    op: impl Fn(&Path, bool) -> Result<T, OpsError>,
) -> Result<T> {
    match op(repo_root, false) {
        Ok(value) => Ok(value),
        Err(OpsError::SyncConflict {
            onto,
            detail,
            recovery,
        }) => {
            resolve_sync_conflict(
                repo_root,
                &onto,
                &detail,
                recovery,
                false,
                progress,
                &load_config_or_default(Some(repo_root)),
            )?;
            progress.status(&format!("Retrying {label} after sync..."));
            op(repo_root, true).map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

fn pr_status(repo_root: &Path) -> Result<()> {
    match current_pr(repo_root)? {
        Some(pr) => {
            println!("#{} {} {} {}", pr.number, pr.state, pr.branch, pr.url);
        }
        None => println!("No open PR for the current branch."),
    }
    Ok(())
}

pub fn run_sync_skills(yes: bool, no_prune: bool, repo: bool) -> Result<()> {
    if !yes && !repo {
        if !std::io::stdin().is_terminal() {
            return Err(anyhow!(
                "skill sync writes under ~/.claude and ~/.agents; rerun with --yes to confirm"
            ));
        }
        let progress = CliProgress;
        if !progress
            .confirm("Write loopflow-generated skills under ~/.claude/skills and ~/.agents/skills?")
        {
            return Err(anyhow!("skill sync cancelled"));
        }
    }

    let report = sync_skills(&SkillSyncOptions {
        prune: !no_prune,
        global_home: None,
        repo: repo.then(find_repo_root).transpose()?,
    })?;
    println!(
        "synced skills ({} written, {} pruned, {} skipped)",
        report.written.len(),
        report.pruned.len(),
        report.skipped.len()
    );
    Ok(())
}

pub fn run_commit(
    message: Option<&str>,
    push: bool,
    no_add: bool,
    paths: &[String],
    agent_override: Option<&str>,
    json: bool,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let committed = crate::engine::checkout::commit(
        &repo_root,
        message,
        push,
        no_add,
        paths,
        agent_override,
        &CliProgress,
    )?;
    if json {
        println!("{}", serde_json::to_string(&committed)?);
    }
    Ok(())
}

pub(crate) fn planning_wave(
    repo: &std::path::Path,
    explicit: Option<&str>,
) -> Result<Option<String>> {
    use crate::work::wave::context::WaveResolveError;
    match crate::work::wave::context::resolve_managed_wave_sync(Some(repo), explicit) {
        Ok(wave) => Ok(Some(wave.slug().to_string())),
        Err(WaveResolveError::NoContext) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn connect_wave(
    repo_root: &std::path::Path,
    wave: Option<&str>,
    all: bool,
    team_key: Option<&str>,
    team_name: Option<&str>,
    json: bool,
) -> Result<()> {
    let results = crate::engine::wave::planning_connect(
        repo_root,
        wave,
        all,
        team_key,
        team_name,
        &CliProgress,
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }
    for result in &results {
        let initiative_state = if result.created { "created" } else { "linked" };
        let team_state = if result.team_created {
            format!(
                ", repository Team {} created ({}-*)",
                result.team_id, result.team_key
            )
        } else {
            format!(
                ", repository Team {} adopted ({}-*)",
                result.team_id, result.team_key
            )
        };
        println!(
            "{}: Linear Initiative {} ({initiative_state}){team_state}",
            result.wave, result.initiative_id
        );
        if let crate::ops::wave_lifecycle::AuthoredFiles::Uncommitted { reason } = &result.files {
            println!(
                ".lf/wave/{} is not committed on the main checkout: {reason}",
                result.wave
            );
        }
    }
    Ok(())
}

pub fn sync_planning(
    repo: &std::path::Path,
    wave: Option<&str>,
    all: bool,
    plan: bool,
    json: bool,
) -> Result<()> {
    let wave = if all {
        None
    } else {
        planning_wave(repo, wave)?
    };
    let result = crate::ops::linear::refresh(
        repo,
        &crate::ops::linear::LinearSyncOptions { wave, plan },
        &CliProgress,
    )?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &serde_json::json!({"actions": result.actions, "diagnostics": result.diagnostics})
            )?
        );
    } else {
        print_pm_sync_result(&result);
    }
    Ok(())
}

/// Refresh shared planning before `wave status --sync` reads it: the Wave from
/// Linear, or the repository's Git planning ref, whose exchange is per repository.
/// Returns the Wave a Linear refresh resolved.
pub fn refresh_status(directory: &std::path::Path, wave: Option<&str>) -> Result<Option<String>> {
    let repo = crate::repo::working_directory_in(directory)?;
    if crate::lf::commands::planning::plans_through_git(&repo)? {
        crate::lf::commands::planning::refresh(&repo)?;
        return Ok(None);
    }
    let wave =
        planning_wave(&repo, wave)?.ok_or_else(|| anyhow::anyhow!("Select a Wave to refresh"))?;
    // The Team delta brings changes and comments in and pending saves go out,
    // so a save left by an outage is delivered on the next refresh.
    if let Err(error) = crate::ops::planning_sync::refresh_directory(&repo) {
        tracing::debug!(%error, "planning delivery pending before refresh");
    }
    tokio::runtime::Runtime::new()?.block_on(crate::ops::linear::refresh_wave(&repo, &wave))?;
    Ok(Some(wave))
}

pub fn reteam(repo: &std::path::Path, apply: bool, json: bool) -> Result<()> {
    let result = crate::engine::wave::planning_reteam(repo, apply, &CliProgress)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_pm_reteam_result(&result);
    }
    Ok(())
}

fn print_pm_reteam_result(result: &crate::ops::linear::LinearReteamResult) {
    let verb = if result.applied { "moved" } else { "will move" };
    println!(
        "repository {} (waves: {}) → team {} ({}-*){}",
        result.repository,
        result.waves.join(", "),
        result.team_id,
        result.team_key,
        if result.applied {
            ""
        } else {
            "  [dry run — pass --apply to execute]"
        }
    );

    if !result.project_moves.is_empty() {
        println!("  {verb} Project(s) ({}):", result.project_moves.len());
        for pm in &result.project_moves {
            let from = if pm.from_teams.is_empty() {
                "no team".to_string()
            } else {
                format!("team(s) [{}]", pm.from_teams.join(", "))
            };
            println!("    wave/{}: {}  (from {from})", pm.wave, pm.name);
        }
    }

    if result.moves.is_empty() {
        println!("  {verb}: none");
    } else {
        println!("  {verb} ({}):", result.moves.len());
        for mv in &result.moves {
            match &mv.new_identifier {
                Some(new_id) => println!(
                    "    wave/{}: {} → {new_id}  {}",
                    mv.wave, mv.old_identifier, mv.title
                ),
                None => println!(
                    "    wave/{}: {}  {}  (Linear assigns the new number at move time)",
                    mv.wave, mv.old_identifier, mv.title
                ),
            }
        }
    }

    if result.task_updates > 0 {
        println!("  reconciled Task identifiers: {}", result.task_updates);
    }
    if result.already > 0 {
        println!("  already in repository Team: {} (skipped)", result.already);
    }
}

fn print_pm_sync_result(result: &crate::ops::linear::LinearSyncResult) {
    if result.actions.is_empty() && result.diagnostics.is_empty() {
        println!("PM state matches local waves and projects");
        return;
    }
    for action in &result.actions {
        println!("action: {action}");
    }
    for diagnostic in &result.diagnostics {
        println!("diagnostic: {diagnostic}");
    }
}

fn release_check_cmd(target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let changes = release_check(&repo_root, target_name)?;

    if changes.commits.is_empty() {
        eprintln!("No commits in the target area since the last tag.");
        return Err(crate::process::CommandExit(1).into());
    }

    let is_tty = std::io::stdout().is_terminal();
    if is_tty {
        for commit in &changes.commits {
            let short_sha = commit.sha.get(..7).unwrap_or(&commit.sha);
            println!("{short_sha} {}", commit.title);
        }
        for pr in &changes.merged_prs {
            println!(
                "#{:<6} {} (+{} -{}, {} files)",
                pr.number, pr.title, pr.additions, pr.deletions, pr.changed_files
            );
        }
        println!(
            "\n{} commit(s), {} merged PR(s) in the release range.",
            changes.commits.len(),
            changes.merged_prs.len()
        );
    } else {
        let json = serde_json::to_string_pretty(&changes)?;
        println!("{}", json);
    }

    Ok(())
}

fn release_run_cmd(
    version_input: Option<&str>,
    target_name: Option<&str>,
    json: bool,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let outcome =
        crate::engine::repository::carry_release(&repo_root, version_input, target_name, progress)?;
    if json {
        println!("{}", serde_json::to_string(&outcome)?);
        return Ok(());
    }
    match outcome {
        crate::ops::ReleaseRunOutcome::NoChanges {
            target, latest_tag, ..
        } => {
            let latest = latest_tag.as_deref().unwrap_or("(none)");
            println!("No release ({target}): no merged PRs since {latest}");
        }
        crate::ops::ReleaseRunOutcome::Released(receipt) => {
            print_release_receipt("Released", &receipt);
        }
        crate::ops::ReleaseRunOutcome::Resumed(receipt) => {
            print_release_receipt("Resumed", &receipt);
        }
    }
    Ok(())
}

fn print_release_receipt(action: &str, receipt: &crate::ops::ReleaseReceipt) {
    println!("{action} {} ({})", receipt.tag, receipt.target);
    println!("Commit: {}", receipt.commit);
    if let Some(url) = receipt.workflow_url.as_deref() {
        println!("Workflow URL: {url}");
    }
    println!(
        "GitHub Release: {}",
        if receipt.release_exists { "yes" } else { "no" }
    );
}

fn release_notes_cmd(
    version: &str,
    prev_tag: Option<&str>,
    target_name: Option<&str>,
    preview: bool,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    if preview {
        print!(
            "{}",
            preview_release_notes(
                &repo_root,
                version,
                prev_tag,
                target_name,
                &NotesPreviewProgress
            )?
        );
        return Ok(());
    }
    release_notes(&repo_root, version, prev_tag, target_name, progress)?;
    println!(
        "RELEASE_NOTES.md updated for v{}",
        version.trim_start_matches('v')
    );
    Ok(())
}

struct NotesPreviewProgress;

impl Progress for NotesPreviewProgress {
    fn status(&self, message: &str) {
        eprintln!("{message}");
    }
    fn warning(&self, message: &str) {
        eprintln!("{message}");
    }
    fn error(&self, message: &str) {
        eprintln!("{message}");
    }
    fn confirm(&self, _message: &str) -> bool {
        false
    }
}

fn release_bump_cmd(
    version: &str,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    release_bump(&repo_root, version, target_name, progress)?;
    println!("Manifests bumped to v{}", version.trim_start_matches('v'));
    Ok(())
}

fn release_tag_cmd(version: &str, target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let tag = release_tag(&repo_root, version, target_name)?;
    println!("{}", tag);
    Ok(())
}

fn release_issues_cmd(version: Option<&str>, json: bool, target_name: Option<&str>) -> Result<()> {
    let issues = release_issues(&find_repo_root()?, version, target_name)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&issues)?);
        return Ok(());
    }
    if issues.is_empty() {
        match version {
            Some(version) => println!("No regressions reported against {version}."),
            None => println!("No open regressions."),
        }
    }
    for issue in &issues {
        println!(
            "{} #{} {}: {}",
            issue.regressed_in,
            issue.number,
            issue.state.to_lowercase(),
            issue.title
        );
        match issue.holds.as_slice() {
            [] => println!("  holds nothing"),
            tags => println!("  holds {}", tags.join(", ")),
        }
        println!("  {}", issue.url);
    }
    Ok(())
}

fn release_promote_cmd(
    dry_run: bool,
    target_name: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let stable = release_promote(&find_repo_root()?, target_name, dry_run, progress)?;
    let current = stable
        .current
        .as_ref()
        .map_or("(none)", |release| release.tag.as_str());
    match &stable.next {
        Some(next) if dry_run => println!("Stable: {current} (would move to {})", next.tag),
        Some(next) => println!("Stable: {} (was {current})", next.tag),
        None => println!("Stable: {current} (stays)"),
    }
    for hold in &stable.holds {
        println!("  {hold}");
    }
    Ok(())
}

fn release_status_cmd(target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let status = release_status(&repo_root, target_name)?;
    println!("Target: {}", status.target);
    let preview = status.preview_tag.as_deref().unwrap_or("(none)");
    match status.preview_published_at {
        Some(at) => println!("Preview: {preview} (published {})", at.format("%Y-%m-%d")),
        None => println!("Preview: {preview}"),
    }
    if let Some(stable) = &status.stable {
        let current = stable
            .current
            .as_ref()
            .map_or("(none)", |release| release.tag.as_str());
        match &stable.next {
            Some(next) => println!(
                "Stable: {current} (moves to {} on the next promote or release run)",
                next.tag
            ),
            None => println!("Stable: {current}"),
        }
        for hold in &stable.holds {
            println!("  {hold}");
        }
    }

    match status.workflow_status.as_deref() {
        Some(workflow_status) => {
            let conclusion = status.workflow_conclusion.as_deref().unwrap_or("(pending)");
            println!("Workflow: {workflow_status} / {conclusion}");
        }
        None => println!("Workflow: (not found)"),
    }

    if let Some(url) = status.workflow_url.as_deref() {
        println!("Workflow URL: {url}");
    }

    match status.notes_status {
        Some(crate::ops::ReleaseNotesStatus::Narrative) => {
            println!("Release notes: narrative / gate safe");
        }
        Some(crate::ops::ReleaseNotesStatus::Degraded(reason)) => {
            println!("Release notes: degraded ({reason}) / gate safe");
        }
        Some(crate::ops::ReleaseNotesStatus::Missing) => {
            println!("Release notes: missing / gate unsafe");
        }
        Some(crate::ops::ReleaseNotesStatus::Legacy) => {
            println!("Release notes: legacy / gate status unknown");
        }
        None => println!("Release notes: (no release tag)"),
    }

    println!(
        "GitHub Release: {}",
        if status.release_exists { "yes" } else { "no" }
    );
    Ok(())
}

pub fn run_wt(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create {
            name,
            plan,
            persistent,
            json,
        } => {
            let repo_root = find_repo_root()?;
            let checkout = crate::engine::checkout::create(
                &repo_root,
                name,
                *plan,
                *persistent,
                &CliProgress,
            )?;
            if *json {
                println!("{}", serde_json::to_string(&checkout)?);
                return Ok(());
            }
            print_checkout(name, *persistent, &checkout)?;
            Ok(())
        }
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { json, sync } => wt_list(*json, *sync),
        WtCommand::Timing { json } => wt_timing(*json),
        WtCommand::Delete { name, force } => wt_delete(name, *force),
        WtCommand::Prune {
            all,
            dry_run,
            builds,
            branches,
            json,
            retention_hours,
            maximum_roots,
            maximum_bytes,
        } => {
            if *builds {
                wt_prune_builds(
                    *dry_run,
                    *json,
                    *retention_hours,
                    *maximum_roots,
                    *maximum_bytes,
                )
            } else if *branches {
                wt_prune_branches(*dry_run)
            } else {
                wt_prune(*all, *dry_run, *json)
            }
        }
    }
}

fn print_checkout(
    name: &str,
    persistent: bool,
    checkout: &crate::engine::checkout::Checkout,
) -> Result<()> {
    use crate::git::worktrees::PlacementStrategy;
    if !checkout.created {
        println!("branch: {}", checkout.branch);
        if let Some(base) = &checkout.base {
            println!("base: {base}");
        }
        println!("worktree: {}", checkout.path.display());
        if let Some(strategy) = &checkout.strategy {
            println!(
                "strategy: {}",
                serde_json::to_value(strategy)?.as_str().unwrap_or_default()
            );
        }
        return Ok(());
    }
    if persistent {
        println!("Persistent workspace: {}", checkout.path.display());
        return Ok(());
    }
    if checkout.strategy == Some(PlacementStrategy::UseExistingWorktree) {
        println!("Using existing worktree: {}", checkout.path.display());
    } else {
        println!("Created worktree: {}", checkout.path.display());
    }
    if checkout.branch != name {
        println!("Branch: {}", checkout.branch);
    }
    if let Some(base) = &checkout.base {
        println!("Base: {base}");
    }
    if !write_shell_directive(&format!("cd {}", checkout.path.display()))? {
        println!("cd {}", checkout.path.display());
        println!("Tip: source scripts/dev-lf to apply auto-cd in this shell");
    }
    Ok(())
}

/// Local ops telemetry lives under the git-ignored `.lf/tmp/` tree so read-only
/// operations (`sync --plan`, status, dispatch) never dirty a tracked
/// worktree. Single source of truth: `crate::ops::telemetry`.
use crate::ops::telemetry::record_ops_metric;

fn wt_switch(name: &str) -> Result<()> {
    cd_directive(&resolve_worktree(name)?)
}

pub fn resolve_worktree(name: &str) -> Result<PathBuf> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let worktrees = list_worktrees(&main_repo)?;

    let path = if let Some(exact_branch_match) = worktrees
        .iter()
        .find(|wt| wt.branch.as_deref() == Some(name))
        .map(|wt| wt.path.clone())
    {
        exact_branch_match
    } else {
        let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());
        let mut matches = worktrees
            .into_iter()
            .filter(|wt| {
                let wt_name = sibling_worktree_name_with_main(&wt.path, &main_repo);
                let parsed = wt
                    .branch
                    .as_deref()
                    .and_then(|branch| WorktreeName::parse(branch, &user));
                wt_name.as_deref() == Some(name)
                    || wt
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy() == name)
                        .unwrap_or(false)
                    || parsed.as_ref().map(|id| id.name() == name).unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            matches.remove(0).path
        } else if matches.is_empty() {
            return Err(anyhow!("no worktree found for '{}'", name));
        } else {
            return Err(anyhow!("multiple worktrees match '{}'", name));
        }
    };

    Ok(path)
}

fn cd_directive(path: &Path) -> Result<()> {
    if !write_shell_directive(&format!("cd {}", path.display()))? {
        println!("cd {}", path.display());
    }
    Ok(())
}

fn wt_timing(json: bool) -> Result<()> {
    let report = crate::ops::wt_timing::report(&crate::store::lf_home_dir())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.groups.is_empty() {
        println!("No `lf wt list` invocations recorded in {}", report.path);
        return Ok(());
    }
    let seconds = |ms: u64| format!("{:.2}s", ms as f64 / 1000.0);
    let phase = |spread: Option<crate::ops::wt_timing::Spread>| {
        spread.map_or_else(
            || "-".to_string(),
            |spread| format!("{} / {}", seconds(spread.median), seconds(spread.p95)),
        )
    };
    for group in &report.groups {
        let mode = match (group.json, group.sync) {
            (false, false) => "text",
            (true, false) => "json",
            (false, true) => "text --sync",
            (true, true) => "json --sync",
        };
        println!("{}  lf {}  {mode}", group.repo, group.version);
        println!(
            "  samples {}   median {}   p95 {}   max {}",
            group.samples,
            seconds(group.total_ms.median),
            seconds(group.total_ms.p95),
            seconds(group.total_ms.max)
        );
        println!(
            "  median / p95:  startup {}   local git {}   remote {}   receipts {}",
            phase(Some(group.startup_ms)),
            phase(group.local_git_ms),
            phase(group.remote_ms),
            phase(group.receipt_ms)
        );
        println!(
            "  failed {}   interrupted {}   remote timed out {}   remote unavailable {}   receipts unrecorded {}",
            group.failed,
            group.interrupted,
            group.remote_timed_out,
            group.remote_unavailable,
            group.receipts_unrecorded
        );
    }
    println!(
        "{} samples in {} (at most {} kept)",
        report.samples, report.path, report.retained_limit
    );
    if report.unreadable > 0 {
        println!("{} lines unreadable by this lf", report.unreadable);
    }
    Ok(())
}

fn wt_list(json: bool, sync: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    crate::ops::wt_timing::begin(&main_repo, json, sync);
    // `wt list` is an inspection surface and stays side-effect free by default:
    // merge/fresh flags reflect the last-synced main. `--sync` is the explicit,
    // self-owned mutation that fetches origin and integrates main first — a
    // read never fetches, resets, or stashes the canonical checkout behind the
    // user's back.
    if sync {
        crate::ops::checkout::refresh_main(&main_repo, &crate::ops::NullProgress)?;
    }
    let listing = list_worktrees_timed(&main_repo)?;
    crate::ops::wt_timing::listed(&listing);
    let (default_branch, worktrees) = (listing.default_branch, listing.worktrees);

    if json {
        let json = serde_json::to_string_pretty(&worktrees)?;
        println!("{}", json);
        return Ok(());
    }

    let c = Colors::new();
    let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());

    // Collect one flat display row per worktree.
    struct Row {
        label: String,
        sort_key: String,
        is_current: bool,
        is_main: bool,
        merged: bool,
        squash_merged: bool,
        fresh: bool,
        dirty: bool,
        remote_gone: bool,
        pull_request: Option<PullRequestState>,
        diff_stat: String,
    }

    let branches: Vec<&str> = worktrees
        .iter()
        .filter_map(|wt| wt.branch.as_deref())
        .filter(|branch| *branch != default_branch)
        .collect();
    let diff_stats = diff_shortstats(&main_repo, &default_branch, &branches);

    let mut rows: Vec<Row> = worktrees
        .iter()
        .map(|wt| {
            let is_main = wt.branch.as_deref() == Some(&default_branch);
            let parsed = wt
                .branch
                .as_deref()
                .and_then(|branch| WorktreeName::parse(branch, &user));
            let (label, sort_key) = if is_main {
                (default_branch.clone(), String::new())
            } else if let Some(name) = &parsed {
                (name.name().to_string(), name.name().to_string())
            } else {
                let name = sibling_worktree_name(&wt.path).unwrap_or_else(|| {
                    wt.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "?".to_string())
                });
                (name.clone(), name)
            };
            let is_current = wt.path == repo_root;
            // "3 files changed, 10 insertions(+), 5 deletions(-)" → "+10 -5 (3 files)"
            let diff_stat = wt
                .branch
                .as_deref()
                .and_then(|branch| diff_stats.get(branch))
                .map(|raw| parse_shortstat(raw))
                .unwrap_or_default();
            Row {
                label,
                sort_key,
                is_current,
                is_main,
                merged: wt.merged,
                squash_merged: wt.squash_merged,
                fresh: wt.fresh,
                dirty: wt.dirty,
                remote_gone: wt.remote_gone,
                pull_request: wt.pull_request,
                diff_stat,
            }
        })
        .collect();

    // Main first (empty key), then alphabetical flat names.
    rows.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));

    let display_name = |row: &Row| row.label.clone();
    let max_name = column_width("", rows.iter().map(display_name));

    for row in &rows {
        let marker = if row.is_current { "*" } else { " " };

        let any_merged = row.merged || (row.squash_merged && !row.fresh);
        let landed_dirty = any_merged && row.dirty;
        let name_color = if row.is_main || any_merged || row.fresh {
            c.dim
        } else {
            c.bold
        };

        let (status_label, status_color) = if landed_dirty {
            ("landed-dirty", c.red)
        } else if row.merged {
            ("merged", c.green)
        } else if row.fresh {
            ("fresh", c.dim)
        } else if row.squash_merged {
            ("squash-merged", c.green)
        } else if row.remote_gone {
            ("remote-gone", c.yellow)
        } else if row.pull_request == Some(PullRequestState::Closed) {
            ("closed-pr", c.yellow)
        } else if row.pull_request == Some(PullRequestState::Open) {
            ("open-pr", c.cyan)
        } else {
            ("active", c.cyan)
        };
        let status = format!("{status_color}{status_label}{}", c.reset);

        let dirty_flag = if row.dirty && !landed_dirty {
            format!(" {}dirty{}", c.yellow, c.reset)
        } else {
            String::new()
        };

        let diff = if row.diff_stat.is_empty() {
            String::new()
        } else {
            format!("  {}{}{}", c.dim, row.diff_stat, c.reset)
        };

        println!(
            "{marker} {name_color}{:<width$}{reset}  {status}{dirty_flag}{diff}",
            display_name(row),
            width = max_name,
            marker = marker,
            name_color = name_color,
            reset = c.reset,
            status = status,
            dirty_flag = dirty_flag,
            diff = diff,
        );
    }
    Ok(())
}

fn wt_delete(name: &str, force: bool) -> Result<()> {
    crate::ops::wt::delete_worktree(&find_repo_root()?, name, force, &CliProgress)?;
    println!("Deleted {name}");
    Ok(())
}

fn parse_shortstat(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let mut files = "";
    let mut insertions = "";
    let mut deletions = "";
    for part in raw.split(", ") {
        let part = part.trim();
        if part.contains("file") {
            files = part.split_whitespace().next().unwrap_or("0");
        } else if part.contains("insertion") {
            insertions = part.split_whitespace().next().unwrap_or("0");
        } else if part.contains("deletion") {
            deletions = part.split_whitespace().next().unwrap_or("0");
        }
    }
    let ins = if insertions.is_empty() {
        "0"
    } else {
        insertions
    };
    let del = if deletions.is_empty() { "0" } else { deletions };
    format!("+{ins} -{del} ({files} files)")
}

fn wt_prune_builds(
    dry_run: bool,
    json: bool,
    retention_hours: u64,
    maximum_roots: usize,
    maximum_bytes: u64,
) -> Result<()> {
    let repo = find_repo_root()?;
    let main = main_repo_root(&repo)?;
    let registries = build_cleanup_registries();
    let retention = std::time::Duration::from_secs(
        retention_hours
            .checked_mul(3600)
            .ok_or_else(|| anyhow!("retention hours overflow"))?,
    );
    let report = crate::git::build_cleanup::prune_builds(
        &main,
        &repo,
        &registries,
        crate::git::build_cleanup::BuildCleanupPolicy {
            retention,
            maximum_roots,
            maximum_bytes,
        },
        dry_run,
    )?;
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        for entry in &report.entries {
            println!(
                "{} {} ({} bytes): {}",
                entry.status,
                entry.path.display(),
                entry
                    .allocated_bytes
                    .map(|bytes| bytes.to_string())
                    .unwrap_or_else(|| "unknown".into()),
                entry.reason
            );
        }
        println!(
            "Reclaimed {} allocated bytes; checkouts retained.",
            report.removed_bytes
        );
    }
    Ok(())
}

/// Registries whose completed Task ownership authorizes build-output cleanup.
/// This path never opens a registry for writes or advances its frontier.
pub(crate) fn build_cleanup_registries() -> Vec<PathBuf> {
    let selected = crate::store::default_db_path();
    let production = crate::store::production_database_path();
    // A missing or unreadable selected registry is unknown, not permission
    // to rely exclusively on another Home's completed ownership.
    let mut registries = vec![selected.clone()];
    if production != selected
        && !matches!(std::fs::metadata(&production), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        registries.push(production);
    }
    registries
}

/// Remove worktrees no unfinished Task, running process or open work explains.
pub(crate) fn prune_repo_worktrees(
    main_repo: &Path,
    current: &Path,
    all: bool,
    dry_run: bool,
) -> Result<WorktreePruneReport> {
    if !dry_run {
        crate::ops::checkout::refresh_main(main_repo, &CliProgress)?;
    }
    Ok(prune_worktrees(
        main_repo,
        current,
        &task_ownership()?,
        all,
        dry_run,
    )?)
}

fn wt_prune(all: bool, dry_run: bool, json: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let report = prune_repo_worktrees(&main_repo, &repo_root, all, dry_run)?;

    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_prune_report(&report, all, dry_run);
    }
    if !report.failed.is_empty() {
        return Err(anyhow!(
            "failed to remove {} prunable worktree(s)",
            report.failed.len()
        ));
    }
    Ok(())
}

fn print_prune_report(report: &WorktreePruneReport, all: bool, dry_run: bool) {
    let label = |path: &Path, branch: &Option<String>| match branch {
        Some(branch) => format!("{branch}  {}", path.display()),
        None => path.display().to_string(),
    };
    let with_saved = |target: &crate::git::worktrees::WorktreePruneTarget| {
        let mut detail = target.reason.to_string();
        if target.source_changes > 0 {
            detail.push_str(&format!("; {} source changes", target.source_changes));
        }
        if let Some(saved) = &target.saved_ref {
            detail.push_str(&format!("; saved to {saved}"));
        }
        detail
    };
    for retained in &report.retained {
        println!(
            "keep    {}  ({})",
            label(&retained.path, &retained.branch),
            retained.reason
        );
    }
    let (verb, summary, removing) = if dry_run {
        ("remove", "to remove", &report.candidates)
    } else {
        ("removed", "removed", &report.removed)
    };
    for target in removing {
        println!(
            "{verb:<7} {}  ({})",
            label(&target.path, &target.branch),
            with_saved(target)
        );
    }
    for failure in &report.failed {
        eprintln!(
            "failed  {}: {}",
            label(&failure.target.path, &failure.target.branch),
            failure.error
        );
    }
    println!(
        "{} {summary}, {} kept.",
        removing.len(),
        report.retained.len()
    );
    let releasable = report
        .retained
        .iter()
        .filter(|retained| retained.reason.releasable())
        .count();
    if !all && releasable > 0 {
        println!(
            "{releasable} kept only for local changes, open PRs or recent activity; \
             `lf wt prune --all` removes them and saves unpushed work under refs/lf/pruned/."
        );
    }
}

fn wt_prune_branches(dry_run: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let ownership = task_ownership()?;
    let tasks = TaskBranches {
        live: ownership.live_branches,
        finished: ownership.finished_branches,
    };
    let report = prune_branches(&repo_root, tasks, dry_run).map_err(|stop| {
        record_ops_metric(
            &repo_root,
            serde_json::json!({"op": "wt.prune.branches", "exit_status": "stopped", "reason": stop.to_string()}),
        );
        anyhow!("{stop}")
    })?;
    for entry in &report.entries {
        if matches!(entry.status, BranchPruneStatus::Failed(_)) {
            eprintln!("{entry}");
        } else {
            println!("{entry}");
        }
    }
    println!("{report}");
    record_ops_metric(
        &repo_root,
        serde_json::json!({
            "op": "wt.prune.branches",
            "dry_run": dry_run,
            "deleted_remote": report.deleted(BranchCopy::Remote),
            "deleted_local": report.deleted(BranchCopy::Local),
            "retained": report.retained(),
            "failed": report.failed(),
            "graphql_remaining": report.budget.remaining,
            "exit_status": if report.failed() == 0 { "ok" } else { "failed" },
        }),
    );
    if report.failed() > 0 {
        return Err(anyhow!(
            "failed to delete {} branch copies",
            report.failed()
        ));
    }
    Ok(())
}

/// Read Task ownership from the selected registry and, without migrations,
/// the release registry, plus the paths running processes hold. Pruning
/// mutates machine-wide state, so an unreadable registry stops it.
fn task_ownership() -> Result<WorktreeOwnership> {
    let mut ownership = WorktreeOwnership {
        process_paths: crate::git::build_cleanup::running_paths()?,
        ..WorktreeOwnership::default()
    };
    let mut named = Vec::new();
    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(crate::store::open_existing_store()) {
        Ok(Some(store)) => {
            let tasks = runtime
                .block_on(store.list_tasks(None))
                .map_err(|error| anyhow!("cannot verify Task ownership before pruning: {error}"))?;
            for task in tasks {
                let work = runtime
                    .block_on(store.work_for_child(&crate::child::ChildRef::Task(task.id.clone())))
                    .map_err(|error| anyhow!("cannot resolve Task Work: {error}"))?;
                let status = runtime
                    .block_on(store.work_status(&work))
                    .map_err(|error| anyhow!("cannot read Task Work status: {error}"))?;
                let open = !matches!(
                    status,
                    crate::durable::WorkStatus::Done | crate::durable::WorkStatus::Abandoned
                );
                let prs = runtime
                    .block_on(store.task_prs(&task.id))
                    .map_err(|error| anyhow!("cannot read Task PRs: {error}"))?;
                named.push((task.branch, open));
                named.extend(prs.into_iter().map(|pr| (pr.branch, open)));
                if let Some(worktree) = task.worktree {
                    if open {
                        ownership
                            .unfinished_checkouts
                            .insert(worktree, Some(task.identifier));
                    } else {
                        ownership
                            .finished_checkouts
                            .insert(worktree, task.identifier);
                    }
                }
            }
        }
        Ok(None) => {}
        Err(error) => {
            return Err(anyhow!(
                "cannot verify Task ownership before pruning: {error}"
            ));
        }
    }

    // An explicit experiment owns its own registry, but pruning is
    // machine-wide mutation. Read the release registry without migrations so
    // `cargo run -- lf wt prune` cannot erase release-owned Tasks' work.
    let production = crate::store::production_database_path();
    if production.exists() {
        let unreadable = |error: crate::store::StoreError| {
            anyhow!(
                "cannot verify Task ownership from {} before pruning: {error}",
                production.display()
            )
        };
        for path in
            crate::store::read_nonterminal_task_worktrees(&production).map_err(unreadable)?
        {
            ownership.finished_checkouts.remove(&path);
            ownership.unfinished_checkouts.entry(path).or_insert(None);
        }
        named.extend(crate::store::read_task_branches(&production).map_err(unreadable)?);
    }
    let TaskBranches { live, finished } = named.into_iter().collect();
    ownership.live_branches = live;
    ownership.finished_branches = finished;
    Ok(ownership)
}

fn pr_checks(repo_root: &Path, watch: bool, logs: bool, required: bool, json: bool) -> Result<()> {
    use crate::engine::pr::ChecksState;
    if watch {
        // gh streams the watch to the terminal; the served read has no watch.
        let branch = current_branch(repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
        let mut args = vec!["pr", "checks", &branch, "--watch"];
        if required {
            args.push("--required");
        }
        let status = Command::new("gh")
            .args(&args)
            .current_dir(repo_root)
            .status()?;
        return if status.success() {
            Ok(())
        } else {
            Err(crate::process::CommandExit(status.code().unwrap_or(1) as u8).into())
        };
    }
    let checks =
        crate::engine::pr::checks(repo_root, required).map_err(crate::engine::pr::exit_of)?;
    if json {
        print_json(&checks)?;
    } else {
        print!("{}", checks.report);
    }
    match checks.state {
        ChecksState::Passing => Ok(()),
        ChecksState::Pending => {
            eprintln!("CI checks are pending.");
            Err(crate::process::CommandExit(checks.exit_code()).into())
        }
        ChecksState::Failing => {
            if logs {
                print_failed_check_logs(repo_root, &checks.branch)?;
            }
            Err(crate::process::CommandExit(checks.exit_code()).into())
        }
    }
}

/// A GitHub Actions run reference extracted from a `detailsUrl` or bare id.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RunRef {
    run_id: String,
    job_id: Option<String>,
}

/// Parse a GitHub Actions `detailsUrl`, run/job URL, or bare numeric run id
/// into a [`RunRef`]. Returns `None` for non-Actions URLs (external CI
/// services) or unparseable input.
fn parse_run_ref(value: &str) -> Option<RunRef> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.chars().all(|c| c.is_ascii_digit()) {
        return Some(RunRef {
            run_id: trimmed.to_string(),
            job_id: None,
        });
    }
    let marker = "/actions/runs/";
    let idx = trimmed.find(marker)?;
    let after = &trimmed[idx + marker.len()..];
    let mut parts = after.split('/');
    let run_id = parts.next()?;
    if !is_numeric(run_id) {
        return None;
    }
    let job_id = match parts.next() {
        Some("jobs") => parts.next().filter(|j| is_numeric(j)),
        _ => None,
    };
    Some(RunRef {
        run_id: run_id.to_string(),
        job_id: job_id.map(str::to_string),
    })
}

fn is_numeric(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

/// Fetch and print logs for every failed check on `branch`, with attribution.
/// Non-Actions checks and missing/expired/private logs are reported actionably
/// rather than silently dropped.
fn print_failed_check_logs(repo_root: &Path, branch: &str) -> Result<()> {
    println!("\n--- Failed check logs ---\n");

    let output = Command::new("gh")
        .args([
            "pr",
            "view",
            branch,
            "--json",
            "statusCheckRollup",
            "-q",
            r#".statusCheckRollup[] | select(.conclusion == "FAILURE" or .conclusion == "failure") | [.name, (.detailsUrl // "")] | @tsv"#,
        ])
        .current_dir(repo_root)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        eprintln!("Couldn't list failed checks: {stderr}");
        return Ok(());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let (name, url) = line.split_once('\t').unwrap_or((line, ""));
        print_single_check_logs(repo_root, name, url);
    }

    Ok(())
}

fn print_single_check_logs(repo_root: &Path, name: &str, url: &str) {
    let Some(run_ref) = parse_run_ref(url) else {
        if url.is_empty() {
            eprintln!("### {name}\nNo details URL for this check; open the PR checks tab.\n");
        } else {
            eprintln!("### {name}\nLogs not available via gh for this check. Open: {url}\n");
        }
        return;
    };

    let mut args: Vec<&str> = vec!["run", "view", &run_ref.run_id, "--log"];
    if let Some(job_id) = &run_ref.job_id {
        args.extend(["--job", job_id]);
    }

    let output = Command::new("gh")
        .args(&args)
        .current_dir(repo_root)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let logs = String::from_utf8_lossy(&out.stdout);
            print!("### {name}\n\n{logs}");
            if !logs.ends_with('\n') {
                println!();
            }
            println!();
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            eprintln!(
                "### {name}\nCouldn't fetch logs (run {}): {stderr}\n\
                 The run may be missing, expired (>90 days), or private. Open: {url}\n",
                run_ref.run_id
            );
        }
        Err(err) => {
            eprintln!("### {name}\nFailed to invoke gh: {err}\n");
        }
    }
}

fn write_shell_directive(command: &str) -> Result<bool> {
    let directive = std::env::var("LOOPFLOW_DIRECTIVE_FILE").ok();
    let Some(path) = directive else {
        return Ok(false);
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    use std::io::Write;
    writeln!(file, "{}", command)?;
    Ok(true)
}

// ==========================================================================
// Skill agent fallback
// ==========================================================================

/// Launch an agent with a named skill when an ops command needs judgment.
///
/// Used when mechanical operations hit a situation that requires agent
/// reasoning — e.g., sync conflicts that need conflict resolution.
fn process_skill_agent(
    repo_root: &Path,
    skill_name: &str,
    context: Option<&str>,
    config: &Config,
) -> Result<()> {
    let skill = load_skill(skill_name, repo_root)?;

    let message = context.map(|value| value.to_string());
    let prepared = prepare_process_prompt(
        config,
        ProcessPromptInput {
            repo_root: repo_root.to_path_buf(),
            skill: Some(skill_name.to_string()),
            resolved_skill: Some(skill),
            surface: Surface::Headless,
            message,
            source_overrides: ContextSourceOverrides {
                // Sync conflicts already name the affected paths in `context`.
                // Embedding every authored file here makes the task prompt grow
                // with the branch and can exceed the OS argument limit before
                // the resolver starts. The agent has the repository as its cwd
                // and can inspect the conflict in place.
                diff_files: Some(false),
                diff: Some(false),
                ..Default::default()
            },
            ..ProcessPromptInput::default()
        },
    )?;

    let agent = prepared.config.agent()?;
    let (provider, model) = crate::config::parse_agent(agent);
    let cwd = prepared
        .config
        .cwd
        .clone()
        .unwrap_or_else(|| repo_root.to_path_buf());
    let context = crate::trace::PreparedTurnContext::from_prompts(
        &crate::agent::system_prompt_with_structured_replies(&prepared.config),
        &prepared.config.task_prompt,
    );
    let capture = crate::session_record::CaptureHandle::begin_with_context(
        crate::session_record::SessionCaptureSpec {
            harness: provider,
            model,
            surface: "headless".to_string(),
            cwd,
            repo: Some(repo_root.to_path_buf()),
            worktree: Some(repo_root.to_path_buf()),
            skill: Some(skill_name.to_string()),
            subjects: Vec::new(),
            graph: crate::session_record::SessionGraphMembership::Independent,
            work: None,
        },
        &context,
        Some(crate::session_record::AgentProcessRequest::from_prepared(
            &prepared.config,
        )?),
    )?;

    let launch = prepared.config;
    let process = ProcessConfig {
        auto: true,
        stream: true,
        capture: Some(capture.clone().into()),
        ..Default::default()
    };

    let result = run_agent(&launch, &process);
    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture.finish(outcome)?;
    let result = result?;
    if result.exit_code != 0 {
        return Err(anyhow!(
            "agent exited with code {} while resolving {}",
            result.exit_code,
            skill_name,
        ));
    }
    Ok(())
}

// ==========================================================================
// System dependency manifest
// ==========================================================================

/// How a dependency is installed via Homebrew (macOS).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Brew {
    /// `brew install <name>` — plain formula (or tap-qualified, e.g. doppler).
    Formula(&'static str),
    /// `brew install --cask <name>` — GUI app.
    Cask(&'static str),
}

/// A single declared system dependency. This array is the source of truth for
/// the repo-root `Brewfile`.
#[derive(Debug, Clone, Copy)]
struct SystemDep {
    /// Display name. Also the binary probed via `which`, unless `command` differs.
    name: &'static str,
    /// Binary probed with `which`; differs from `name` when the tool ships under
    /// another command (e.g. rust ships `cargo`).
    command: &'static str,
    /// Build/run essentials are required; agent CLIs and editors are optional.
    required: bool,
    /// GUI apps only distributed for macOS here — skipped on other hosts.
    macos_only: bool,
    /// Homebrew package, when installable via brew (feeds the Brewfile).
    brew: Option<Brew>,
    /// Install hint for non-macOS hosts (or when there is no brew package).
    fallback: &'static str,
}

impl SystemDep {
    fn is_present(&self) -> bool {
        which(self.command)
    }

    /// The install hint shown by the doctor when the dep is missing.
    fn install_hint(&self, is_macos: bool) -> String {
        if is_macos {
            if let Some(brew) = self.brew {
                return match brew {
                    Brew::Formula(f) => format!("brew install {f}"),
                    Brew::Cask(c) => format!("brew install --cask {c}"),
                };
            }
        }
        self.fallback.to_string()
    }
}

/// The declared system dependencies loopflow expects on a working host.
///
/// Required deps are the build/run essentials; optional deps are the agent CLIs
/// and editors. The repo-root Brewfile is generated from it — do not
/// hand-maintain a second list.
const SYSTEM_DEPS: &[SystemDep] = &[
    // Required: build/run essentials.
    SystemDep {
        name: "git",
        command: "git",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("git")),
        fallback: "https://git-scm.com/downloads",
    },
    SystemDep {
        name: "rust",
        command: "cargo",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("rust")),
        fallback: "https://rustup.rs/",
    },
    SystemDep {
        name: "uv",
        command: "uv",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("uv")),
        fallback: "https://docs.astral.sh/uv/getting-started/installation/",
    },
    SystemDep {
        name: "gh",
        command: "gh",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("gh")),
        fallback: "https://cli.github.com/",
    },
    SystemDep {
        name: "doppler",
        command: "doppler",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("doppler")),
        fallback: "https://docs.doppler.com/docs/install-cli",
    },
    // Optional: agent CLIs and editors.
    SystemDep {
        name: "npm",
        command: "npm",
        required: false,
        macos_only: false,
        brew: Some(Brew::Formula("node")),
        fallback: "https://nodejs.org/",
    },
    SystemDep {
        name: "claude",
        command: "claude",
        required: false,
        macos_only: false,
        brew: None,
        fallback: "npm install -g @anthropic-ai/claude-code",
    },
    SystemDep {
        name: "codex",
        command: "codex",
        required: false,
        macos_only: false,
        brew: None,
        fallback: "npm install -g @openai/codex",
    },
    SystemDep {
        name: "warp",
        command: "warp",
        required: false,
        macos_only: true,
        brew: Some(Brew::Cask("warp")),
        fallback: "",
    },
    SystemDep {
        name: "cursor",
        command: "cursor",
        required: false,
        macos_only: true,
        brew: Some(Brew::Cask("cursor")),
        fallback: "",
    },
];

/// Render the repo-root Brewfile from the declared dependency list.
fn brewfile_contents() -> String {
    let mut out = String::new();
    out.push_str("# Generated from the declared SYSTEM_DEPS list in\n");
    out.push_str("# rust/loopflow/src/lf/commands/ops/mod.rs — do not edit by hand.\n");
    out.push_str("# Keep this file in sync with SYSTEM_DEPS.\n");
    out.push_str("# Install everything with: brew bundle\n\n");
    for dep in SYSTEM_DEPS {
        let Some(brew) = dep.brew else { continue };
        let tag = if dep.required { "required" } else { "optional" };
        match brew {
            Brew::Formula(f) => out.push_str(&format!("brew \"{f}\"  # {} ({tag})\n", dep.name)),
            Brew::Cask(c) => out.push_str(&format!("cask \"{c}\"  # {} ({tag})\n", dep.name)),
        }
    }
    out
}

pub fn run_doctor(brewfile: bool) -> Result<()> {
    if brewfile {
        print!("{}", brewfile_contents());
        return Ok(());
    }

    let repo_root = find_repo_root().ok();

    // Repo status
    if let Some(ref root) = repo_root {
        let lf_dir = root.join(".lf");
        if lf_dir.join("skills").is_dir() || lf_dir.join("graphs").is_dir() {
            println!("✓ task files found");
        } else {
            println!("- no task files (run: lf init)");
        }
    } else {
        println!("- not in a git repo");
    }

    let is_macos = cfg!(target_os = "macos");
    let mut missing_required = 0;

    for dep in SYSTEM_DEPS {
        if dep.macos_only && !is_macos {
            continue;
        }
        if dep.is_present() {
            println!("✓ {}", dep.name);
        } else {
            let tag = if dep.required { " (required)" } else { "" };
            println!("- {}: {}{}", dep.name, dep.install_hint(is_macos), tag);
            if dep.required {
                missing_required += 1;
            }
        }
    }

    if missing_required > 0 {
        println!("\n{missing_required} required dep(s) missing");
    } else {
        println!("\nall required deps present");
    }

    Ok(())
}

fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod doctor_tests {
    use super::{brewfile_contents, SYSTEM_DEPS};
    use std::fs;
    use std::path::Path;

    #[test]
    fn declared_deps_non_empty_and_well_formed() {
        assert!(!SYSTEM_DEPS.is_empty());
        for dep in SYSTEM_DEPS {
            assert!(!dep.name.is_empty());
            // Every dep has a check: the `which` target.
            assert!(
                !dep.command.is_empty(),
                "{} needs a check command",
                dep.name
            );
            if dep.required {
                // Required deps need an install hint on every host: a brew package
                // (macOS) and a non-empty fallback (elsewhere).
                assert!(
                    dep.brew.is_some(),
                    "{} (required) needs a brew package",
                    dep.name
                );
                assert!(
                    !dep.fallback.is_empty(),
                    "{} (required) needs a fallback install hint",
                    dep.name
                );
            }
        }
    }

    #[test]
    fn brewfile_matches_declared_list() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let committed =
            fs::read_to_string(root.join("Brewfile")).expect("Brewfile exists at repo root");
        assert_eq!(
            committed,
            brewfile_contents(),
            "Brewfile is stale; update it alongside SYSTEM_DEPS"
        );
    }
}

#[cfg(test)]
mod wt_ci_tests {
    use super::{parse_run_ref, RunRef};

    #[test]
    fn parse_run_ref_handles_job_url() {
        let url =
            "https://github.com/loopflowstudio/loopflow/actions/runs/978123456/jobs/111222333";
        let r = parse_run_ref(url).expect("job URL parses");
        assert_eq!(r.run_id, "978123456");
        assert_eq!(r.job_id, Some("111222333".to_string()));
    }

    #[test]
    fn parse_run_ref_handles_run_url() {
        let url = "https://github.com/loopflowstudio/loopflow/actions/runs/983654321";
        let r = parse_run_ref(url).expect("run URL parses");
        assert_eq!(r.run_id, "983654321");
        assert_eq!(r.job_id, None);
    }

    #[test]
    fn parse_run_ref_handles_bare_numeric_id() {
        let r = parse_run_ref("978123456").expect("numeric id parses");
        assert_eq!(
            r,
            RunRef {
                run_id: "978123456".to_string(),
                job_id: None
            }
        );
    }

    #[test]
    fn parse_run_ref_trims_whitespace() {
        let r = parse_run_ref("  978123456  ").expect("trimmed numeric id parses");
        assert_eq!(r.run_id, "978123456");
    }

    #[test]
    fn parse_run_ref_rejects_non_actions_url() {
        assert_eq!(
            parse_run_ref("https://example.com/build/123"),
            None,
            "external CI URLs are not Actions runs"
        );
    }

    #[test]
    fn parse_run_ref_rejects_empty_and_garbage() {
        assert_eq!(parse_run_ref(""), None);
        assert_eq!(parse_run_ref("   "), None);
        assert_eq!(parse_run_ref("not-a-url"), None);
    }

    #[test]
    fn parse_run_ref_rejects_non_numeric_run_id() {
        assert_eq!(
            parse_run_ref("https://github.com/o/r/actions/runs/abc"),
            None,
            "non-numeric run id is not a valid ref"
        );
    }
}
