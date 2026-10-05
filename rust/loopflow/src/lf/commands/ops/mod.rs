use crate::engine::agent::{exec_agent, AgentCapabilities, ProcessConfig};
use crate::engine::config::{load_config_or_default, Config};
use crate::engine::git::{current_branch, get_default_branch};
use crate::engine::identity::WorktreeName;
use crate::engine::naming::git_user;
use crate::engine::worktrees::{
    create_from_placement_plan, diff_shortstats, list_worktrees, list_worktrees_timed,
    main_repo_root, plan_placement, prune_worktrees, sibling_worktree_name,
    sibling_worktree_name_with_main, PlacementStrategy, PullRequestState, WorktreePrunePolicy,
    WorktreeSegment,
};
use crate::engine::{
    prepare_exec_prompt, sync_skills, ContextSourceOverrides, ExecPromptInput, SkillSyncOptions,
    Surface,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::{discover_skill, resolve_definition, Target};
use crate::lf::output::{column_width, Colors};
use crate::lf::{CronCommand, PrCommand, ReleaseCommand, RepoCommand, WtCommand};
use crate::ops::OpsError;
use crate::ops::{
    abandon_branch, abort_sync_after_authorization, abort_sync_for_resolution, arm,
    commit_workflow, continue_sync_after_authorization, continue_sync_for_resolution,
    create_or_update_pr, current_pr, finish_arm_after_sync, finish_submit_after_sync, plan_sync,
    preview_release_notes, recover_sync, release_bump, release_check, release_notes,
    release_publish, release_run, release_status, release_tag, submit, sync_class_name,
    sync_strategy_name, sync_with_recovery, AbandonOptions, CommitOptions, CronHost, CronOutcome,
    CronSource, CronSpec, CronTargetKind, LandOptions, PrOptions, Progress, SyncOptions,
    SystemLaunchctl,
};
use crate::store::RegistryUnavailable;
use anyhow::{anyhow, Result};
use std::collections::HashSet;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub fn run_pr(cmd: Option<&PrCommand>, cli_model: Option<&str>) -> Result<()> {
    let progress = CliProgress;
    match cmd {
        None => pr_status(),
        Some(PrCommand::Reconcile) => {
            let repo = find_repo_root()?;
            crate::ops::task::reconcile_checkout_pr(&repo)?;
            let report = crate::ops::pr_landing::reconcile_repository(&repo)?;
            if !report.errors.is_empty() {
                anyhow::bail!(report.errors.join("\n"));
            }
            progress.status("delivery check complete");
            Ok(())
        }
        Some(PrCommand::Checks { watch, logs }) => pr_checks(*watch, *logs),
        Some(PrCommand::Publish { model, title, body }) => publish_pr(
            title.clone(),
            body.clone(),
            model.as_deref().or(cli_model),
            &progress,
        ),
        Some(PrCommand::Open { model, title, body }) => open_pr(
            title.clone(),
            body.clone(),
            model.as_deref().or(cli_model),
            &progress,
        ),
        Some(PrCommand::Submit {
            strict,
            create_pr,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        }) => submit_current(
            &LandOptions {
                strict: *strict,
                local: false,
                create_pr: *create_pr,
                complete: *complete,
                next_slug: next.clone(),
                worktree: worktree.clone(),
                commit_message: message.clone(),
                pr_title: title.clone(),
                pr_body: body.clone(),
                agent: cli_model.map(str::to_string),
            },
            &progress,
        ),
        Some(PrCommand::Arm {
            strict,
            local,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        })
        | Some(PrCommand::Land {
            strict,
            local,
            complete,
            next,
            worktree,
            message,
            title,
            body,
        }) => land_current(
            &LandOptions {
                strict: *strict,
                local: *local,
                create_pr: true,
                complete: *complete,
                next_slug: next.clone(),
                worktree: worktree.clone(),
                commit_message: message.clone(),
                pr_title: title.clone(),
                pr_body: body.clone(),
                agent: cli_model.map(str::to_string),
            },
            &progress,
        ),
        Some(PrCommand::Abandon { force, branch }) => {
            abandon_current(branch.as_deref(), *force, &progress)
        }
        Some(PrCommand::Next { slug }) => pr_next(slug.as_deref()),
    }
}

fn pr_next(slug: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let pr = crate::ops::task::pr_next(&repo_root, slug)?;
    println!(
        "Rotated to PR {} on {} (base {}).",
        pr.sequence,
        pr.branch,
        &pr.base_commit[..pr.base_commit.len().min(12)]
    );
    println!("Push your follow-up edits, then `lf pr open` when ready.");
    Ok(())
}

pub fn run_release(cmd: &ReleaseCommand) -> Result<()> {
    let progress = CliProgress;
    match cmd {
        ReleaseCommand::History { wave, days, json } => {
            let repo = crate::engine::worktrees::main_repo_root(&find_repo_root()?)?;
            let now = chrono::Utc::now().timestamp();
            let history = crate::ops::cron::history::release_history(
                &crate::store::lf_home_dir(),
                &repo,
                wave,
                *days,
                now,
            )?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&history)?);
            } else {
                println!("{} due, {} accounted, {} collapsed, {} executions; {} published, {} no-change, {} unresolved",
                    history.summary.due, history.summary.accounted, history.summary.collapsed,
                    history.summary.executions, history.summary.published, history.summary.no_change,
                    history.summary.unresolved);
                println!("{} failed telemetry targets, {} undispositioned failures, {} late dispositions; {} qualifying consecutive pairs",
                    history.summary.failed_verifications, history.summary.undispositioned_failures.len(),
                    history.summary.late_dispositions.len(), history.summary.qualifying_pairs.len());
                if history.observation_frontier.is_none() {
                    println!("Opportunity coverage unknown: no retained release obligation; sync the configured cron to begin observation. Historical receipts below remain evidence.");
                }
                for obligation in &history.obligations {
                    println!(
                        "{}/{} on {} ({})",
                        obligation.wave, obligation.flow, obligation.home_id, obligation.timezone
                    );
                    for owner in obligation.closed_unsettled() {
                        println!(
                            "blocked {}: obligation {} closed at {}; no future firing on original Home {}. Retained candidate: {}",
                            owner.id,
                            obligation.id,
                            obligation.closed_at.expect("closed owner has a closure timestamp"),
                            obligation.home_id,
                            owner.attempts.iter().rev().find_map(|a| a.selection.as_ref())
                                .map(|s| format!("{} at {}", s.tag, s.commit))
                                .unwrap_or_else(|| "none recorded".into())
                        );
                        if let Some(disposition) = history
                            .dispositions
                            .iter()
                            .rev()
                            .find(|d| d.subject == owner.id)
                        {
                            println!(
                                "  repair owner {} at {}: {}",
                                disposition.owner, disposition.recorded_at, disposition.reason
                            );
                        }
                        println!("  Record repair on that Home: lf cron disposition {} --wave {} --owner <task-work-id> --reason <repair-plan>", owner.id, obligation.wave);
                    }
                    for opportunity in obligation
                        .opportunities
                        .iter()
                        .filter(|o| o.due_at >= history.window_start)
                    {
                        let owner = opportunity
                            .coalesced_into
                            .as_ref()
                            .and_then(|id| {
                                history
                                    .obligations
                                    .iter()
                                    .flat_map(|r| &r.opportunities)
                                    .find(|o| &o.id == id)
                            })
                            .unwrap_or(opportunity);
                        println!(
                            "{} {} {}{}",
                            opportunity.id,
                            opportunity.due_local,
                            owner
                                .attempts
                                .last()
                                .map(|a| format!("last attempt: {:?}", a.outcome))
                                .or_else(|| owner.wait.as_ref().map(|wait| {
                                    if obligation.closed_at.is_some() {
                                        format!(
                                            "last wait: {}; previously expected firing {}",
                                            wait.reason, wait.retry_at
                                        )
                                    } else {
                                        format!(
                                            "deferred: {}; next firing {}",
                                            wait.reason, wait.retry_at
                                        )
                                    }
                                }))
                                .unwrap_or_else(|| "pending; no execution recorded".into()),
                            opportunity
                                .coalesced_into
                                .as_ref()
                                .map(|id| format!(" (collapsed into {id})"))
                                .unwrap_or_default()
                        );
                    }
                }
                for receipt in history
                    .receipts
                    .iter()
                    .filter(|r| r.outcome == CronOutcome::Failed)
                {
                    println!(
                        "failed {} {}: {}",
                        receipt.id,
                        receipt.flow,
                        receipt.error.as_deref().unwrap_or("inspect cron log")
                    );
                }
            }
            Ok(())
        }

        ReleaseCommand::Run { version, target } => {
            release_run_cmd(version.as_deref(), target.as_deref(), &progress)
        }
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

pub fn run_sync(
    onto: Option<&str>,
    plan_only: bool,
    manual: bool,
    continue_sync: bool,
    abort: bool,
    adopt: bool,
) -> Result<()> {
    let repo_root = crate::repo::require_repo_root(&std::env::current_dir()?, "lf sync")?;
    run_sync_in(
        &repo_root,
        onto,
        plan_only,
        manual,
        continue_sync,
        abort,
        adopt,
    )
}

pub(crate) fn run_sync_in(
    repo_root: &Path,
    onto: Option<&str>,
    plan_only: bool,
    manual: bool,
    continue_sync: bool,
    abort: bool,
    adopt: bool,
) -> Result<()> {
    let progress = &CliProgress;
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
        progress.status("Sync complete; branch remains local.");
        return Ok(());
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
        progress.status("Sync aborted.");
        return Ok(());
    }
    let started = Instant::now();
    let default = get_default_branch(repo_root)?;
    let upstream = format!("origin/{default}");
    let on_main = current_branch(repo_root)?.as_deref() == Some(&default);
    if !plan_only {
        crate::ops::checkout::refresh_main(repo_root, progress)?;
        if on_main && onto.is_none_or(|target| target == upstream) {
            progress.status("Main is current; unpublished commits and edits remain local.");
            return Ok(());
        }
    }
    // A Task stack owns its sync target: the live parent branch until merge,
    // then the default branch. An explicit override could silently drop work.
    let stacked = if on_main {
        None
    } else {
        crate::ops::task::task_stack(repo_root)?
    };
    if stacked.is_some() && onto.is_some() {
        return Err(anyhow!(
            "stacked Task syncs choose their parent automatically; omit the target"
        ));
    }
    let stacked_onto = stacked
        .as_ref()
        .and_then(|stacked| stacked.parent_branch.as_ref())
        .map(|branch| format!("origin/{branch}"));
    let fork_base = stacked.as_ref().map(|stacked| stacked.fork_base.clone());
    let default_target = if on_main { upstream } else { default.clone() };
    let plan = plan_sync(
        repo_root,
        stacked_onto.as_deref().or(onto).or(Some(&default_target)),
        fork_base.clone(),
    )?;
    if plan_only {
        print_sync_plan(&plan);
        return Ok(());
    }
    let options = SyncOptions {
        onto: plan.base_ref.clone(),
        push: !manual && !on_main,
        fork_base,
    };
    let (verification, agent_launched) = if manual {
        (sync_with_recovery(repo_root, &options, progress)?, false)
    } else {
        let recovery_config = load_config_or_default(Some(repo_root));
        crate::ops::checkout::with_preserved_edits(repo_root, || {
            match sync_with_recovery(repo_root, &options, progress) {
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
                        is_avoidable_sync_class(&plan.class),
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
    if let Some(stacked) = stacked.as_ref() {
        crate::ops::task::record_stack_sync(
            stacked,
            &verification.target_sha,
            stacked.parent_branch.is_none(),
        )?;
    }
    if manual {
        return Ok(());
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
    Ok(())
}

fn print_sync_plan(plan: &crate::ops::SyncPlan) {
    println!("branch: {}", plan.branch);
    println!("base: {}", plan.base_ref);
    if let Some(fork_base) = &plan.fork_base {
        println!("fork_base: {fork_base}");
    }
    println!("class: {}", sync_class_name(&plan.class));
    println!("strategy: {}", sync_strategy_name(&plan.strategy));
    println!("unique_commits: {}", plan.unique_commits);
    println!("changed_files: {}", plan.changed_files.len());
    println!(
        "protected: {}",
        matches!(plan.class, crate::ops::SyncClass::Protected)
    );
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
    Ok(recover_sync(*recovery, |env| {
        exec_skill_agent(
            repo_root,
            "sync-conflicts",
            Some(&context),
            Some(env),
            config,
        )
        .map_err(|error| OpsError::Message(error.to_string()))
    })?)
}

fn is_avoidable_sync_class(class: &crate::ops::SyncClass) -> bool {
    matches!(
        class,
        crate::ops::SyncClass::StaleEmpty
            | crate::ops::SyncClass::ScratchOnly
            | crate::ops::SyncClass::GeneratedOnly
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
        assert!(is_avoidable_sync_class(&SyncClass::GeneratedOnly));
        assert!(!is_avoidable_sync_class(&SyncClass::CleanAuthored));
        assert!(!is_avoidable_sync_class(&SyncClass::Protected));
    }
}

/// Run a PR-mutating op; on a sync conflict, launch the sync agent to
/// resolve it and retry once. A second conflict is a real error.
fn with_sync_retry<T>(
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

fn land_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    land_repo(&repo_root, options, progress)
}

pub(crate) fn land_repo(
    repo_root: &Path,
    options: &LandOptions,
    progress: &impl Progress,
) -> Result<()> {
    // The wave home stays put on land — no rotation, no cd.
    let pr = with_sync_retry(repo_root, "land", progress, |repo, integrated| {
        if integrated {
            finish_arm_after_sync(repo, options, progress, &|_| {})
        } else {
            arm(repo, options, progress)
        }
    })?;
    if let Some(pr) = pr {
        progress.status(&format!(
            "PR #{} handed off; lf pr reconcile checks delivery.",
            pr.number
        ));
    }
    Ok(())
}

fn submit_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    with_sync_retry(&repo_root, "submit", progress, |repo, integrated| {
        if integrated {
            finish_submit_after_sync(repo, options, progress)
        } else {
            submit(repo, options, progress)
        }
    })?;
    progress.status("Ready to land — click merge on the PR once checks pass.");
    Ok(())
}

/// Push and create/update the PR with the requested readiness. Opens no browser.
fn prepare_current_pr(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    draft: bool,
    progress: &impl Progress,
) -> Result<crate::ops::PrResult> {
    let repo_root = find_repo_root()?;
    let result = create_or_update_pr(
        &repo_root,
        &PrOptions {
            draft,
            title,
            body,
            agent: agent_override.map(str::to_string),
        },
        progress,
    )?;
    Ok(result)
}

fn publish_pr(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let result = prepare_current_pr(title, body, agent_override, false, progress)?;
    print_pr_result(&result);
    Ok(())
}

fn open_pr(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let result = prepare_current_pr(title, body, agent_override, true, progress)?;
    // The PR exists — print the URL before presenting so a failed
    // review-surface launch fails only `pr open` and never hides the PR.
    print_pr_result(&result);
    crate::ops::present_pr_review(&result.url).map_err(|err| {
        anyhow!(
            "PR available at {} but opening it for review failed: {err}",
            result.url
        )
    })?;
    Ok(())
}

/// Print the PR's state and URL. Falls back to the raw URL when
/// GitHub state can't be re-read.
fn print_pr_result(result: &crate::ops::PrResult) {
    let verb = if result.created { "created" } else { "updated" };
    match find_repo_root()
        .ok()
        .and_then(|repo| current_pr(&repo).ok()?)
    {
        Some(pr) => println!("{verb} #{} {} {}", pr.number, pr.state, result.url),
        None => println!("{verb} {}", result.url),
    }
}

fn pr_status() -> Result<()> {
    let repo_root = find_repo_root()?;
    match current_pr(&repo_root)? {
        Some(pr) => {
            println!("#{} {} {} {}", pr.number, pr.state, pr.branch, pr.url);
        }
        None => println!("No open PR for the current branch."),
    }
    Ok(())
}

pub fn run_sync_skills(yes: bool, no_prune: bool) -> Result<()> {
    if !yes {
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
    })?;
    println!(
        "synced skills ({} written, {} pruned)",
        report.written.len(),
        report.pruned.len()
    );
    Ok(())
}

pub fn run_commit(
    message: Option<&str>,
    no_add: bool,
    paths: &[String],
    agent_override: Option<&str>,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    if !paths.is_empty() {
        return Ok(crate::ops::commit_selected(&repo_root, paths, message)?);
    }
    let _ = commit_workflow(
        &repo_root,
        &CommitOptions {
            add: !no_add,
            message: message.map(str::to_string),
            agent: agent_override.map(str::to_string),
            ..CommitOptions::for_task("commit")
        },
        &CliProgress,
        &|_| {},
    )?;
    Ok(())
}

fn abandon_current(branch: Option<&str>, force: bool, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    abandon_branch(
        &repo_root,
        &AbandonOptions {
            branch: branch.map(str::to_string),
            force,
        },
        progress,
    )?;
    Ok(())
}

fn planning_wave(repo: &std::path::Path, explicit: Option<&str>) -> Result<Option<String>> {
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
) -> Result<()> {
    let progress = &CliProgress;
    let ambient_wave = |explicit| planning_wave(repo_root, explicit);
    let targets = if all {
        crate::ops::pm::list_local_waves(repo_root)?
    } else {
        let explicit = wave;
        // Wave connection is a creation flow: its positional name may select a
        // wave not yet registered (it links a wave directory to
        // Linear, not a registry row). Normalize-only for explicit;
        // ambient still uses the shared validating resolver.
        let name = if let Some(raw) = explicit {
            crate::ops::normalize_wave_name(raw)
                .ok_or_else(|| anyhow!("repo connect requires a non-empty wave name"))?
        } else {
            ambient_wave(None)?
                .ok_or_else(|| anyhow!("cannot determine wave; run `lf repo connect <name>`"))?
        };
        vec![name]
    };
    for wave in targets {
        let result = crate::ops::pm::pm_init(
            repo_root,
            &crate::ops::pm::PmInitOptions {
                wave: Some(wave),
                team_key: team_key.map(str::to_string),
                team_name: team_name.map(str::to_string),
            },
            progress,
        )?;
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
    let result = crate::ops::pm::pm_sync(
        repo,
        &crate::ops::pm::PmSyncOptions { wave, plan },
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

pub fn refresh_status(wave: Option<&str>) -> Result<String> {
    let repo = crate::repo::working_directory()?;
    let result = crate::ops::pm::pm_show(
        &repo,
        &crate::ops::pm::PmShowOptions {
            wave: planning_wave(&repo, wave)?,
            refresh: crate::ops::pm::PmRefresh::Force,
        },
        &crate::ops::NullProgress,
    )?;
    Ok(result.wave)
}

pub fn run_repo(cmd: &RepoCommand) -> Result<()> {
    match cmd {
        RepoCommand::Connect {
            wave,
            all,
            team_key,
            team_name,
        } => {
            let repo = crate::repo::working_directory()?;
            connect_wave(
                &repo,
                wave.as_deref(),
                *all,
                team_key.as_deref(),
                team_name.as_deref(),
            )
        }
        RepoCommand::Refresh { wave, all } => {
            let repo = crate::repo::working_directory()?;
            sync_planning(&repo, wave.as_deref(), *all, false, false)
        }
        RepoCommand::NewChapter {
            name,
            dry_run,
            json,
        } => {
            let repo = crate::repo::working_directory()?;
            let rotation = crate::ops::chapter::new_chapter(&repo, name, *dry_run)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&rotation)?);
            } else {
                println!("Chapter {}", rotation.name);
                for wave in rotation.waves {
                    println!(
                        "  {} → {} ({})",
                        wave.wave, rotation.name, wave.successor_id
                    );
                    for task in wave.tasks {
                        println!(
                            "    {}  {:?}  {}",
                            task.task.identifier, task.disposition, task.reason
                        );
                    }
                }
            }
            Ok(())
        }
        RepoCommand::Release { cmd } => run_release(cmd),
        RepoCommand::Tokens { json, days } => crate::lf::commands::tokens::run(*json, *days),
        RepoCommand::Ci { cmd: Some(cmd), .. } => ci_watch_cmd(cmd),
        RepoCommand::Ci {
            cmd: None,
            since,
            wave,
            repo,
            json,
        } => crate::lf::commands::ci::run(since, wave.as_deref(), repo.as_deref(), *json),
        RepoCommand::Reteam { apply } => {
            let repo = crate::repo::working_directory()?;
            let result = crate::ops::pm::pm_reteam(
                &repo,
                &crate::ops::pm::PmReteamOptions { apply: *apply },
                &CliProgress,
            )?;
            print_pm_reteam_result(&result);
            Ok(())
        }
    }
}

fn print_pm_reteam_result(result: &crate::ops::pm::PmReteamResult) {
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
            println!(
                "    wave/{}: {} → {}  (from {from})",
                pm.wave, pm.name, pm.target_name
            );
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

fn print_pm_sync_result(result: &crate::ops::pm::PmSyncResult) {
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

fn ci_watch_cmd(cmd: &crate::lf::CiCommand) -> Result<()> {
    use crate::ops::ci_watch;
    let crate::lf::CiCommand::Watch {
        once,
        install,
        uninstall,
        status,
        json,
        parent_pid,
    } = *cmd;
    let repo = find_repo_root()?;
    match (install, uninstall, status) {
        (false, false, false) => Ok(ci_watch::watch(
            &repo,
            ci_watch::WatchOptions { once, parent_pid },
        )?),
        (true, _, _) => {
            require_release_cron_binary()?;
            let authority = cron_authority("")?;
            let path = ci_watch::install_service(
                &crate::ops::default_launch_agents_dir()?,
                &ci_watch::ServiceSpec {
                    repo: authority.repo,
                    lf_path: crate::ops::resolve_lf_path()?,
                    lf_home: authority.host.lf_home,
                    path_env: authority.host.path_env,
                },
                &SystemLaunchctl,
            )?;
            println!("installed {}", path.display());
            Ok(())
        }
        (_, true, _) => {
            let removed = ci_watch::uninstall_service(
                &crate::ops::default_launch_agents_dir()?,
                &main_repo_root(&repo)?,
                &SystemLaunchctl,
            )?;
            println!(
                "{}",
                if removed {
                    "removed the CI watch service"
                } else {
                    "no CI watch service is installed"
                }
            );
            Ok(())
        }
        (_, _, true) => {
            let status = ci_watch::status(&repo)?;
            if json {
                println!("{}", serde_json::to_string(&status)?);
                return Ok(());
            }
            println!(
                "{}{}",
                if status.running {
                    "watching"
                } else {
                    "not running"
                },
                if status.installed {
                    " · service installed"
                } else {
                    ""
                }
            );
            let Some(state) = status.state else {
                return Ok(());
            };
            if let Some(at) = state.last_poll_at {
                println!(
                    "last poll {}s ago · rate limit {}",
                    chrono::Utc::now().timestamp() - at,
                    state
                        .rate_remaining
                        .map_or("unknown".to_string(), |left| left.to_string())
                );
            }
            if let Some(degraded) = &state.degraded {
                println!("degraded: {degraded}");
            }
            for pr in &state.prs {
                println!("{pr}");
            }
            for repair in &state.repairs {
                println!(
                    "started ci-fix for PR #{} {} {}s ago: {}",
                    repair.pr_number,
                    repair.task.as_deref().unwrap_or("(no Task)"),
                    chrono::Utc::now().timestamp() - repair.at,
                    repair.reason
                );
            }
            Ok(())
        }
    }
}

pub fn cron_cmd(cmd: &CronCommand) -> Result<()> {
    let launch_agents_dir = crate::ops::default_launch_agents_dir()?;
    match cmd {
        CronCommand::Disposition {
            subject,
            wave,
            owner,
            reason,
        } => {
            let owner = crate::durable::TaskId::parse(owner)?;
            tokio::runtime::Runtime::new()?.block_on(async {
                let store = crate::store::open_registry_for_authority()
                    .await
                    .map_err(cron_registry_error)?;
                if store.get_task(&owner).await?.is_none() {
                    return Err(anyhow!("repair owner {owner} is not a registered Task"));
                }
                Ok(())
            })?;
            crate::ops::cron::history::disposition(
                &crate::store::lf_home_dir(),
                wave,
                subject,
                owner,
                reason,
                chrono::Utc::now().timestamp(),
            )?;
            println!("recorded repair disposition for {subject}; original evidence retained");
        }
        CronCommand::Add {
            wave,
            flow,
            schedule,
        } => {
            let repo_root = find_repo_root()?;
            // The one ambient-Wave rule, like every PM arm: `--wave` wins, else
            // `LF_WAVE_ID` (UUID or repository-scoped registered name). A
            // scheduled invocation needs a concrete wave, so `NoContext` is the
            // familiar "pass --wave" error.
            let wave = crate::work::wave::context::resolve_managed_wave_sync(
                Some(&repo_root),
                wave.as_deref(),
            )
            .map(|wave| wave.slug().to_string())
            .map_err(|err| match err {
                crate::work::wave::context::WaveResolveError::NoContext => {
                    anyhow!("cannot determine wave; pass --wave <name>")
                }
                other => other.into(),
            })?;
            require_release_cron_binary()?;
            let authority = cron_authority(&wave)?;
            ensure_cron_placement(&wave, &authority)?;
            let target_kind = cron_target_kind(&authority.repo, flow)?;
            let spec = CronSpec {
                wave,
                flow: flow.clone(),
                target_kind,
                schedule: crate::ops::parse_schedule(schedule)?,
                working_directory: authority.repo,
                lf_path: crate::ops::resolve_lf_path()?,
                host: authority.host,
            };
            let cron = crate::ops::add_cron(&launch_agents_dir, &spec, &SystemLaunchctl)?;
            println!("installed {} at {}", cron.label, cron.path.display());
        }
        CronCommand::List { wave, json } => {
            let mut crons = crate::ops::list_crons(&launch_agents_dir, &SystemLaunchctl)?;
            if let Some(wave) = wave {
                crons.retain(|cron| cron.wave == *wave);
            }
            if *json {
                println!("{}", serde_json::to_string(&crons)?);
                return Ok(());
            }
            if crons.is_empty() {
                println!("no loopflow crons installed");
            } else {
                for cron in crons {
                    let latest = cron
                        .latest_receipt
                        .as_ref()
                        .map(cron_receipt_outcome)
                        .unwrap_or("never");
                    println!(
                        "{}  {}  {}  {}  {}",
                        cron.flow,
                        cron.schedule,
                        if cron.loaded { "loaded" } else { "not-loaded" },
                        cron.home_id,
                        latest,
                    );
                }
            }
        }
        CronCommand::Preflight { wave } => {
            require_release_cron_binary()?;
            let authority = cron_authority(wave)?;
            ensure_cron_placement(wave, &authority)?;
            let specs = cron_specs(&authority, wave)?;
            crate::ops::validate_cron_specs(wave, &specs)?;
            println!(
                "cron preflight passed: {} jobs for Wave {wave} on Home {}",
                specs.len(),
                authority.local_home
            );
        }
        CronCommand::Sync {
            wave,
            repo,
            disable,
        } => {
            if *repo {
                require_release_cron_binary()?;
                let authority = cron_authority("")?;
                let key =
                    crate::ops::cron::repository_cron_key(&authority.repo, &authority.local_home);
                if *disable {
                    crate::ops::remove_cron(&launch_agents_dir, "", &key, &SystemLaunchctl)?;
                } else {
                    let spec = CronSpec {
                        wave: String::new(),
                        flow: key,
                        target_kind: CronTargetKind::Repository,
                        schedule: crate::ops::parse_schedule("every-minute")?,
                        working_directory: authority.repo,
                        lf_path: crate::ops::resolve_lf_path()?,
                        host: authority.host,
                    };
                    crate::ops::add_cron(&launch_agents_dir, &spec, &SystemLaunchctl)?;
                }
                return Ok(());
            }
            let wave = wave.as_deref().expect("clap requires Wave or repository");
            require_release_cron_binary()?;
            let authority = cron_authority(wave)?;
            ensure_cron_placement(wave, &authority)?;
            let specs = cron_specs(&authority, wave)?;
            let result =
                crate::ops::sync_crons(&launch_agents_dir, wave, &specs, &SystemLaunchctl)?;
            if result.installed.is_empty() && result.removed.is_empty() {
                println!("no crons declared for wave {wave}; nothing to sync");
            }
            for cron in &result.installed {
                println!("installed {} ({})", cron.label, cron.flow);
            }
            for cron in &result.removed {
                println!("pruned {} ({})", cron.label, cron.flow);
            }
        }
        CronCommand::Run {
            wave,
            flow,
            scheduled,
        } => {
            let source = if *scheduled {
                CronSource::Scheduled
            } else {
                CronSource::Manual
            };
            let authority = match cron_authority(wave) {
                Ok(authority) => authority,
                Err(error) => {
                    let detail = error.to_string();
                    return match crate::ops::record_cron_preflight_failure(
                        &launch_agents_dir,
                        wave,
                        flow,
                        source,
                        &detail,
                    ) {
                        Ok(receipt) => Err(anyhow!(
                            "cron {wave}/{flow} failed before launch: {}; log {}",
                            receipt.error.as_deref().unwrap_or(&detail),
                            receipt.log_path.display()
                        )),
                        Err(receipt_error) => Err(anyhow!(
                            "cron {wave}/{flow} failed before launch: {detail}; receipt persistence also failed: {receipt_error}"
                        )),
                    };
                }
            };
            let receipt = crate::ops::run_cron(
                &launch_agents_dir,
                wave,
                flow,
                &authority.local_home,
                &authority.placed_home,
                source,
            )?;
            println!(
                "{} {} {}",
                receipt.id,
                receipt.flow,
                cron_receipt_outcome(&receipt)
            );
        }
        CronCommand::History {
            wave,
            flow,
            days,
            json,
        } => {
            let root = crate::ops::receipt_root(&crate::store::lf_home_dir());
            let receipts = crate::ops::list_cron_receipts(&root, wave, flow.as_deref(), *days)?;
            if *json {
                println!("{}", serde_json::to_string(&receipts)?);
                return Ok(());
            }
            if receipts.is_empty() {
                println!("no cron receipts for wave {wave} in the last {days} days");
            }
            for receipt in receipts {
                let started = chrono::DateTime::from_timestamp(receipt.started_at, 0)
                    .map(|time| time.to_rfc3339())
                    .unwrap_or_else(|| receipt.started_at.to_string());
                let duration = receipt
                    .finished_at
                    .map(|finished| finished.saturating_sub(receipt.started_at))
                    .map(|seconds| format!("{seconds}s"))
                    .unwrap_or_else(|| "open".to_string());
                let exit = receipt
                    .exit_code
                    .map(|code| format!("exit={code}"))
                    .unwrap_or_else(|| "exit=-".to_string());
                println!(
                    "{}  {}  {}  {}  {}  {}  {}",
                    started,
                    receipt.flow,
                    cron_source_name(receipt.source),
                    cron_receipt_outcome(&receipt),
                    duration,
                    exit,
                    receipt.log_path.display(),
                );
            }
        }
        CronCommand::Trigger {
            wave,
            flow,
            wait,
            timeout,
        } => {
            let authority = cron_authority(wave)?;
            ensure_cron_placement(wave, &authority)?;
            let root = crate::ops::receipt_root(&authority.host.lf_home);
            let prior = crate::ops::cron_receipt_ids(&root, wave, flow)?;
            let triggered_at = chrono::Utc::now().timestamp();
            let request = crate::ops::cron::record_cron_trigger(&launch_agents_dir, wave, flow)?;
            if let Err(error) = crate::ops::trigger_cron(&SystemLaunchctl, wave, flow) {
                crate::ops::cron::record_cron_trigger_failure(
                    &launch_agents_dir,
                    wave,
                    flow,
                    &request,
                    &error.to_string(),
                )?;
                return Err(error.into());
            }
            println!("triggered {wave}/{flow} through launchd");
            if *wait {
                let receipt = crate::ops::wait_for_cron_receipt(
                    &root,
                    wave,
                    flow,
                    &prior,
                    triggered_at,
                    crate::ops::parse_wait_duration(timeout)?,
                )?;
                println!(
                    "{} {} {}",
                    receipt.id,
                    receipt.flow,
                    cron_receipt_outcome(&receipt)
                );
                if receipt.outcome == CronOutcome::Failed
                    || crate::ops::receipt_is_stale(&receipt, chrono::Utc::now().timestamp())
                {
                    return Err(anyhow!(
                        "cron {wave}/{flow} {}: {}; log {}",
                        cron_receipt_outcome(&receipt),
                        receipt
                            .error
                            .as_deref()
                            .unwrap_or("no terminal error recorded"),
                        receipt.log_path.display()
                    ));
                }
            }
        }
        CronCommand::Remove { wave, flow } => {
            match crate::ops::remove_cron(&launch_agents_dir, wave, flow, &SystemLaunchctl)? {
                Some(cron) => println!("removed {}", cron.label),
                None => println!("not installed"),
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct CronAuthority {
    host: CronHost,
    local_home: crate::durable::HomeId,
    placed_home: crate::durable::HomeId,
    repo: PathBuf,
}

fn cron_authority(wave_name: &str) -> Result<CronAuthority> {
    let repo_root = find_repo_root()?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = crate::store::open_registry_for_authority()
            .await
            .map_err(cron_registry_error)?;
        let local = store.local_home().await?;
        let (repo, placed_home) = if wave_name.is_empty() {
            (main_repo_root(&repo_root)?, local.id.clone())
        } else {
            let wave = crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(&repo_root),
                Some(wave_name),
                None,
            )
            .await?;
            let placement = store
                .placement(&crate::durable::WorkRef::Wave(wave.id().clone()))
                .await?;
            (main_repo_root(Path::new(wave.repo()))?, placement.home_id)
        };
        let path_env = std::env::var("PATH").map_err(|_| {
            anyhow!("PATH is absent; cannot install an unattended cron environment")
        })?;
        if path_env.is_empty() {
            return Err(anyhow!(
                "PATH is empty; cannot install an unattended cron environment"
            ));
        }
        Ok(CronAuthority {
            host: CronHost {
                home_id: local.id.clone(),
                lf_home: crate::store::lf_home_dir(),
                path_env,
            },
            local_home: local.id,
            placed_home,
            repo,
        })
    })
}

fn cron_registry_error(error: RegistryUnavailable) -> anyhow::Error {
    match error {
        RegistryUnavailable::MissingFile { path } => anyhow!(
            "Home registry is missing at {}; initialize or restore it before running cron",
            path.display()
        ),
        RegistryUnavailable::Unresolved { error } => {
            anyhow!("Home registry path cannot be resolved: {error}")
        }
        RegistryUnavailable::Incompatible { path, error } => anyhow!(
            "Home registry at {} is incompatible: {error}; run `lf home doctor`",
            path.display()
        ),
    }
}

fn ensure_cron_placement(wave: &str, authority: &CronAuthority) -> Result<()> {
    if authority.local_home == authority.placed_home {
        return Ok(());
    }
    Err(anyhow!(
        "Wave {wave} is placed on Home {}, not local Home {}; run `lf home ssh {} cron sync --wave {wave}`",
        authority.placed_home,
        authority.local_home,
        authority.placed_home,
    ))
}

fn cron_target_kind(repo: &Path, name: &str) -> Result<CronTargetKind> {
    match resolve_definition(repo, name, None)? {
        Target::Command(_) | Target::Flow(_) | Target::Xor(_) => Ok(CronTargetKind::Flow),
        Target::Skill(_) => Ok(CronTargetKind::Skill),
    }
}

#[test]
fn scheduled_release_prefers_its_operation_flow_over_the_builtin_skill() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
    std::fs::write(
        repo.path().join(".lf/flows/release-run.yaml"),
        "- cmd: repo release run patch\n",
    )
    .unwrap();
    assert_eq!(
        cron_target_kind(repo.path(), "release-run").unwrap(),
        CronTargetKind::Flow
    );
    assert_eq!(
        cron_target_kind(repo.path(), "debug").unwrap(),
        CronTargetKind::Skill
    );
    std::fs::write(repo.path().join(".lf/flows/release-run.yaml"), "invalid: [").unwrap();
    assert!(cron_target_kind(repo.path(), "release-run")
        .unwrap_err()
        .to_string()
        .contains("invalid flow"));
}

fn cron_specs(authority: &CronAuthority, wave: &str) -> Result<Vec<CronSpec>> {
    let lf_path = crate::ops::resolve_lf_path()?;
    crate::work::wave::config::try_read_wave_config(&authority.repo, wave)?
        .ok_or_else(|| {
            anyhow!(
                "Wave {wave} has no GOAL.md in {}; refusing to prune installed cron jobs",
                authority.repo.display()
            )
        })?
        .crons
        .unwrap_or_default()
        .into_iter()
        .map(|cron| {
            let schedule = crate::ops::parse_schedule(&cron.schedule).map_err(|error| {
                anyhow!(
                    "{} ({}) cannot be installed: {error}",
                    cron.flow,
                    cron.schedule
                )
            })?;
            let target_kind = cron_target_kind(&authority.repo, &cron.flow)?;
            if target_kind != CronTargetKind::Flow {
                return Err(anyhow!(
                    "configured cron flow {} is missing; refusing to replace it with a skill",
                    cron.flow
                ));
            }
            Ok(CronSpec {
                wave: wave.to_string(),
                flow: cron.flow,
                target_kind,
                schedule,
                working_directory: authority.repo.clone(),
                lf_path: lf_path.clone(),
                host: authority.host.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod cron_catalog_tests {
    use super::{cron_specs, CronAuthority};
    use crate::durable::HomeId;
    use crate::ops::{CronHost, CronTargetKind};
    use std::fs;

    #[test]
    fn declared_cron_flow_cannot_fall_back_to_builtin_skill() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join("wave/infrastructure")).unwrap();
        fs::create_dir_all(repo.path().join(".lf/flows")).unwrap();
        fs::write(
            repo.path().join("wave/infrastructure/GOAL.md"),
            "---\ncrons:\n- flow: release-run\n  schedule: '0 0 10 * * *'\n---\n",
        )
        .unwrap();
        let flow = repo.path().join(".lf/flows/release-run.yaml");
        fs::write(&flow, "- cmd: lf release run patch\n").unwrap();
        let home = HomeId::new();
        let authority = CronAuthority {
            host: CronHost {
                home_id: home.clone(),
                lf_home: repo.path().join("home"),
                path_env: "/usr/bin:/bin".into(),
            },
            local_home: home.clone(),
            placed_home: home,
            repo: repo.path().to_path_buf(),
        };
        let specs = cron_specs(&authority, "infrastructure").unwrap();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].target_kind, CronTargetKind::Flow);
        fs::remove_file(&flow).unwrap();
        assert!(cron_specs(&authority, "infrastructure")
            .unwrap_err()
            .to_string()
            .contains("refusing to replace it with a skill"));
        fs::write(flow, "[").unwrap();
        assert!(cron_specs(&authority, "infrastructure").is_err());
    }
}

fn require_release_cron_binary() -> Result<()> {
    if crate::build_info::provenance().is_release() {
        return Ok(());
    }
    Err(anyhow!(
        "lf wave cron installation requires an installed release binary; promote this build before configuring launchd"
    ))
}

fn cron_receipt_outcome(receipt: &crate::ops::CronReceipt) -> &'static str {
    if crate::ops::receipt_is_stale(receipt, chrono::Utc::now().timestamp()) {
        return "stale";
    }
    match receipt.outcome {
        CronOutcome::Running => "running",
        CronOutcome::Succeeded => "succeeded",
        CronOutcome::Failed => "failed",
    }
}

fn cron_source_name(source: CronSource) -> &'static str {
    match source {
        CronSource::Scheduled => "scheduled",
        CronSource::Triggered => "triggered",
        CronSource::Recovery => "recovery",
        CronSource::Manual => "manual",
    }
}

fn release_check_cmd(target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let changes = release_check(&repo_root, target_name)?;

    if changes.commits.is_empty() {
        eprintln!("No commits in the target area since the last tag.");
        return Err(crate::exec::CommandExit(1).into());
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
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let input = version_input.unwrap_or("patch");
    match release_run(&repo_root, input, target_name, progress)? {
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

fn release_status_cmd(target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let status = release_status(&repo_root, target_name)?;
    println!("Target: {}", status.target);
    match status.latest_tag.as_deref() {
        Some(tag) => println!("Latest tag: {tag}"),
        None => println!("Latest tag: (none)"),
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
        } => {
            if *persistent && !*plan {
                let repo = find_repo_root()?;
                let workspace = crate::engine::worktrees::ensure_agent_worktree(
                    &repo,
                    WorktreeSegment::parse(name)?,
                )?;
                println!("Persistent workspace: {}", workspace.path.display());
                Ok(())
            } else {
                wt_create(name, *plan)
            }
        }
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { json, sync } => wt_list(*json, *sync),
        WtCommand::Timing { json } => wt_timing(*json),
        WtCommand::Delete { name, force } => wt_delete(name, *force),
        WtCommand::Prune { dry_run } => wt_prune(*dry_run),
    }
}

fn wt_create(name: &str, dry_run: bool) -> Result<()> {
    let started = Instant::now();
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let segment = WorktreeSegment::parse(name)?;

    if !dry_run {
        crate::ops::checkout::refresh_main(&main_repo, &CliProgress)?;
    }

    let placement = plan_placement(&main_repo, segment)?;

    if dry_run {
        print_placement_plan(&placement);
        return Ok(());
    }

    let result = create_from_placement_plan(&main_repo, &placement)?;
    record_ops_metric(
        &repo_root,
        serde_json::json!({
            "op": "wt.create",
            "branch": placement.branch,
            "base_ref": placement.base_ref,
            "strategy": placement_strategy_name(&placement.strategy),
            "duration_ms": started.elapsed().as_millis(),
            "exit_status": "ok",
        }),
    );

    if placement.strategy == PlacementStrategy::UseExistingWorktree {
        println!("Using existing worktree: {}", result.path.display());
    } else {
        println!("Created worktree: {}", result.path.display());
    }
    if result.branch != name {
        println!("Branch: {}", result.branch);
    }
    if let Some(base_branch) = result.base_branch {
        println!("Base: {base_branch}");
    }

    if !write_shell_directive(&format!("cd {}", result.path.display()))? {
        println!("cd {}", result.path.display());
        println!("Tip: source scripts/dev-lf to apply auto-cd in this shell");
    }

    Ok(())
}

fn print_placement_plan(plan: &crate::engine::worktrees::PlacementPlan) {
    println!("branch: {}", plan.branch);
    println!("base: {}", plan.base_ref);
    println!("worktree: {}", plan.worktree_path.display());
    println!("strategy: {}", placement_strategy_name(&plan.strategy));
}

fn placement_strategy_name(strategy: &PlacementStrategy) -> &'static str {
    match strategy {
        PlacementStrategy::Create => "create",
        PlacementStrategy::CheckoutExisting => "checkout_existing",
        PlacementStrategy::UseExistingWorktree => "use_existing_worktree",
    }
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

fn wt_prune(dry_run: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    if !dry_run {
        crate::ops::checkout::refresh_main(&main_repo, &CliProgress)?;
    }
    let protected_paths = protected_worktree_paths()?;
    let report = prune_worktrees(
        &main_repo,
        &repo_root,
        &protected_paths,
        WorktreePrunePolicy::manual(),
        dry_run,
    )?;

    if report.candidates.is_empty() {
        println!("No prunable worktrees.");
        return Ok(());
    }

    if dry_run {
        for target in &report.candidates {
            println!(
                "  {} ({reason})  {}",
                target.branch.as_deref().unwrap_or("detached"),
                target.path.display(),
                reason = target.reason.as_str(),
            );
        }
        return Ok(());
    }

    for target in &report.removed {
        println!("Removed {}", target.path.display());
    }
    for failure in &report.failed {
        eprintln!(
            "Failed to remove {}: {}",
            failure.target.path.display(),
            failure.error
        );
    }
    if !report.failed.is_empty() {
        return Err(anyhow!(
            "failed to remove {} prunable worktree(s)",
            report.failed.len()
        ));
    }
    Ok(())
}

fn protected_worktree_paths() -> Result<HashSet<PathBuf>> {
    let mut protected = crate::lf::commands::top::running_workspace_paths();
    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(crate::store::open_registry_for_authority()) {
        Ok(store) => {
            let tasks = runtime.block_on(store.list_tasks(None)).map_err(|error| {
                anyhow!("cannot verify Task worktree ownership before pruning: {error}")
            })?;
            for task in tasks {
                let work = runtime
                    .block_on(store.work_for_child(&crate::child::ChildRef::Task(task.id.clone())))
                    .map_err(|error| anyhow!("cannot resolve Task Work: {error}"))?;
                let status = runtime
                    .block_on(store.work_status(&work))
                    .map_err(|error| anyhow!("cannot read Task Work status: {error}"))?;
                if !matches!(
                    status,
                    crate::durable::WorkStatus::Done | crate::durable::WorkStatus::Abandoned
                ) {
                    protected.insert(task.worktree);
                }
            }
        }
        Err(RegistryUnavailable::MissingFile { .. }) => {}
        Err(RegistryUnavailable::Unresolved { error }) => {
            return Err(anyhow!(
                "cannot verify Task worktree ownership before pruning: {error}"
            ));
        }
        Err(RegistryUnavailable::Incompatible { path, error }) => {
            return Err(anyhow!(
                "cannot verify Task worktree ownership from {} before pruning: {error}",
                path.display()
            ));
        }
    }

    // An explicit experiment owns its own registry, but pruning is
    // machine-wide filesystem mutation. Read the release registry without
    // migrations so `cargo run -- lf wt prune` cannot erase release-owned Tasks.
    let production = crate::store::production_database_path();
    if production.exists() {
        protected.extend(
            crate::store::read_nonterminal_task_worktrees(&production).map_err(|error| {
                anyhow!(
                    "cannot verify Task worktree ownership from {} before pruning: {error}",
                    production.display()
                )
            })?,
        );
    }
    Ok(protected)
}

fn pr_checks(watch: bool, logs: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let branch = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;

    let mut args = vec!["pr", "checks", &branch];
    if watch {
        args.push("--watch");
    }

    let status = Command::new("gh")
        .args(&args)
        .current_dir(&repo_root)
        .status()?;

    if !status.success() && logs {
        print_failed_check_logs(&repo_root, &branch)?;
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("ci checks failed"))
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
fn exec_skill_agent(
    repo_root: &Path,
    skill_name: &str,
    context: Option<&str>,
    env: Option<&std::collections::BTreeMap<String, String>>,
    config: &Config,
) -> Result<()> {
    let skill = discover_skill(repo_root, skill_name)?;

    let message = context.map(|value| value.to_string());
    let prepared = prepare_exec_prompt(
        config,
        ExecPromptInput {
            repo_root: repo_root.to_path_buf(),
            skill: Some(skill_name.to_string()),
            resolved_skill: Some(skill),
            surface: Surface::Headless,
            message,
            cwd: Some(repo_root.to_path_buf()),
            yolo_mode: config.yolo,
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
            ..ExecPromptInput::default()
        },
    )?;

    let agent = prepared.config.agent();
    let (provider, model) = crate::engine::parse_agent(agent);
    let cwd = prepared
        .config
        .cwd
        .clone()
        .unwrap_or_else(|| repo_root.to_path_buf());
    let context = crate::trace::PreparedTurnContext::from_prompts(
        &crate::engine::agent::system_prompt_with_structured_replies(&prepared.config),
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
            flow: crate::session_record::SessionFlowMembership::Independent,
            work: None,
        },
        &context,
        Some(crate::session_record::AgentExecRequest::from_prepared(
            &prepared.config,
            &AgentCapabilities {
                chrome: config.chrome,
            },
        )),
    )?;
    capture.record_input("initial", &prepared.config.task_prompt);

    let mut launch = prepared.config;
    launch.env.extend(env.cloned().unwrap_or_default());
    let process = ProcessConfig {
        auto: true,
        stream: true,
        capture: Some(capture.clone().into()),
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = exec_agent(&launch, &process, &capabilities);
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
        name: "tmux",
        command: "tmux",
        required: true,
        macos_only: false,
        brew: Some(Brew::Formula("tmux")),
        fallback: "https://github.com/tmux/tmux/wiki/Installing",
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
        if lf_dir.join("skills").is_dir() || lf_dir.join("flows").is_dir() {
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
