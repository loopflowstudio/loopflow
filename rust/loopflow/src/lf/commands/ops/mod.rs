use crate::engine::agent::{launch_agent, AgentCapabilities, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::{current_branch, delete_local_branch, get_default_branch, sync_main};
use crate::engine::identity::WorktreeName;
use crate::engine::naming::git_user;
use crate::engine::worktrees::{
    create_from_placement_plan, list_worktrees, main_repo_root, plan_placement, prune_worktrees,
    sibling_worktree_name, sibling_worktree_name_with_main, PlacementStrategy, PullRequestState,
    WorktreePrunePolicy, WorktreeSegment,
};
use crate::engine::{
    prepare_launch_prompt, sync_skills, ContextSourceOverrides, LaunchPromptInput,
    SkillSyncOptions, Surface,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::discover_skill;
use crate::lf::output::{column_width, Colors};
use crate::lf::{
    CronCommand, PmCommand, PmProjectCommand, PmTaskCommand, PrCommand, ReleaseCommand, WtCommand,
};
use crate::ops::OpsError;
use crate::ops::{
    abandon_branch, abort_rebase_for_resolution, commit_workflow, continue_rebase_for_resolution,
    create_or_update_pr, current_pr, finish_land_after_rebase, finish_submit_after_rebase, land,
    plan_rebase, rebase_class_name, rebase_strategy_name, rebase_with_recovery, recover_rebase,
    release_bump, release_check, release_notes, release_publish, release_run, release_status,
    release_tag, start_rebase_for_resolution, submit, AbandonOptions, CommitOptions, CronSpec,
    LandOptions, PrOptions, Progress, RebaseOptions, SystemLaunchctl,
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
        None | Some(PrCommand::Status) => pr_status(),
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
        Some(PrCommand::Land {
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
        ReleaseCommand::Run { version, target } => {
            release_run_cmd(version.as_deref(), target.as_deref(), &progress)
        }
        ReleaseCommand::Check { target } => release_check_cmd(target.as_deref()),
        ReleaseCommand::Notes {
            version,
            prev_tag,
            target,
        } => release_notes_cmd(version, prev_tag.as_deref(), target.as_deref(), &progress),
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

struct CliProgress;

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

pub fn run_rebase(
    onto: Option<&str>,
    plan_only: bool,
    manual: bool,
    continue_rebase: bool,
    abort: bool,
    adopt: bool,
) -> Result<()> {
    let progress = &CliProgress;
    let repo_root = find_repo_root()?;
    if onto.is_some() && (continue_rebase || abort) {
        return Err(anyhow!(
            "a rebase target cannot be combined with --continue or --abort"
        ));
    }
    if adopt && !(continue_rebase || abort) {
        return Err(anyhow!(
            "--adopt is only valid with `lf rebase --continue` or `lf rebase --abort`"
        ));
    }
    if continue_rebase {
        continue_rebase_for_resolution(&repo_root, adopt)?;
        progress.status("Rebase complete; branch remains local.");
        return Ok(());
    }
    if abort {
        abort_rebase_for_resolution(&repo_root, adopt)?;
        progress.status("Rebase aborted.");
        return Ok(());
    }
    let started = Instant::now();
    // A Task stack owns its rebase target: the live parent branch until merge,
    // then the default branch. An explicit override could silently drop work.
    let stacked = crate::ops::task::task_stack(&repo_root)?;
    if stacked.is_some() && onto.is_some() {
        return Err(anyhow!(
            "stacked Task rebases choose their parent automatically; omit --onto"
        ));
    }
    let stacked_onto = stacked
        .as_ref()
        .and_then(|stacked| stacked.parent_branch.as_ref())
        .map(|branch| format!("origin/{branch}"));
    let fork_base = stacked.as_ref().map(|stacked| stacked.fork_base.clone());
    let plan = plan_rebase(
        &repo_root,
        stacked_onto.as_deref().or(onto),
        fork_base.clone(),
    )?;
    let onto_ref = plan.base_ref.clone();
    if plan_only {
        print_rebase_plan(&plan);
        return Ok(());
    }
    if manual {
        return start_rebase_for_resolution(
            &repo_root,
            &RebaseOptions {
                onto: onto_ref,
                push: false,
                fork_base,
            },
            progress,
        )
        .map(|_| ())
        .map_err(Into::into);
    }
    let (verification, agent_launched) = match rebase_with_recovery(
        &repo_root,
        &RebaseOptions {
            onto: onto_ref.clone(),
            push: true,
            fork_base,
        },
        progress,
    ) {
        Ok(verification) => (verification, false),
        Err(OpsError::RebaseConflict {
            onto,
            detail,
            recovery,
        }) => (
            resolve_rebase_conflict(&repo_root, &onto, &detail, recovery, progress)?,
            true,
        ),
        Err(err) => return Err(err.into()),
    };
    if let Some(stacked) = stacked.as_ref() {
        crate::ops::task::record_stack_rebase(
            stacked,
            &verification.target_sha,
            stacked.parent_branch.is_none(),
        )?;
    }
    record_ops_metric(
        &repo_root,
        serde_json::json!({
            "op": "rebase",
            "branch": plan.branch,
            "base_ref": plan.base_ref,
            "class": rebase_class_name(&plan.class),
            "strategy": rebase_strategy_name(&plan.strategy),
            "unique_commits": verification.unique_commits,
            "changed_files": plan.changed_files.len(),
            "protected": matches!(plan.class, crate::ops::RebaseClass::Protected),
            "scratch_stashed": plan.scratch_stashed,
            "agent_launched": agent_launched,
            "duration_ms": started.elapsed().as_millis(),
            "exit_status": "ok",
        }),
    );
    Ok(())
}

fn print_rebase_plan(plan: &crate::ops::RebasePlan) {
    println!("branch: {}", plan.branch);
    println!("base: {}", plan.base_ref);
    if let Some(fork_base) = &plan.fork_base {
        println!("fork_base: {fork_base}");
    }
    println!("class: {}", rebase_class_name(&plan.class));
    println!("strategy: {}", rebase_strategy_name(&plan.strategy));
    println!("unique_commits: {}", plan.unique_commits);
    println!("changed_files: {}", plan.changed_files.len());
    println!(
        "protected: {}",
        matches!(plan.class, crate::ops::RebaseClass::Protected)
    );
}

/// Hand a conflicted rebase to exactly one recovery agent under the owning
/// operation's scoped ids. The agent continues the existing sequencer; the
/// caller keeps ownership and performs verification and the single push.
fn resolve_rebase_conflict(
    repo_root: &Path,
    onto: &str,
    detail: &str,
    recovery: Option<Box<crate::ops::RebaseRecovery>>,
    progress: &impl Progress,
) -> Result<crate::ops::RebaseVerification> {
    let recovery =
        recovery.ok_or_else(|| anyhow!("rebase conflict has no owned recovery operation"))?;
    let context = format!(
        "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\nContinue the existing owned sequencer; do not start another rebase or push.\n</lf:rebase-conflict>"
    );
    progress.status("Launching rebase agent to resolve conflicts...");
    Ok(recover_rebase(*recovery, |env| {
        launch_skill_agent(repo_root, "rebase-conflicts", Some(&context), Some(env))
            .map_err(|error| OpsError::Message(error.to_string()))
    })?)
}

/// Run a PR-mutating op; on a rebase conflict, launch the rebase agent to
/// resolve it and retry once. A second conflict is a real error.
fn with_rebase_retry<T>(
    repo_root: &Path,
    label: &str,
    progress: &impl Progress,
    op: impl Fn(&Path, bool) -> Result<T, OpsError>,
) -> Result<T> {
    match op(repo_root, false) {
        Ok(value) => Ok(value),
        Err(OpsError::RebaseConflict {
            onto,
            detail,
            recovery,
        }) => {
            resolve_rebase_conflict(repo_root, &onto, &detail, recovery, progress)?;
            progress.status(&format!("Retrying {label} after rebase..."));
            op(repo_root, true).map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

fn land_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    // The wave home stays put on land — no rotation, no cd.
    with_rebase_retry(&repo_root, "land", progress, |repo, integrated| {
        if integrated {
            finish_land_after_rebase(repo, options, progress)
        } else {
            land(repo, options, progress)
        }
    })?;
    Ok(())
}

fn submit_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    with_rebase_retry(&repo_root, "submit", progress, |repo, integrated| {
        if integrated {
            finish_submit_after_rebase(repo, options, progress)
        } else {
            submit(repo, options, progress)
        }
    })?;
    progress.status("Ready to land — click merge on the PR once checks pass.");
    Ok(())
}

/// Shared publication: push + create/update the PR. Opens no review surface.
/// Both `lf pr publish` and `lf pr open` publish through here.
fn publish_current(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> Result<crate::ops::PrResult> {
    let repo_root = find_repo_root()?;
    let result = create_or_update_pr(
        &repo_root,
        &PrOptions {
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
    let result = publish_current(title, body, agent_override, progress)?;
    print_published_pr(&result);
    Ok(())
}

fn open_pr(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let result = publish_current(title, body, agent_override, progress)?;
    // Publication succeeded — print the URL before presenting so a failed
    // review-surface launch fails only `pr open` and never hides the PR.
    print_published_pr(&result);
    crate::ops::present_pr_review(&result.url).map_err(|err| {
        anyhow!(
            "PR published at {} but opening it for review failed: {err}",
            result.url
        )
    })?;
    Ok(())
}

/// Print a freshly published PR's state and URL. Falls back to the raw URL when
/// GitHub state can't be re-read.
fn print_published_pr(result: &crate::ops::PrResult) {
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
    push: bool,
    no_add: bool,
    agent_override: Option<&str>,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let _ = commit_workflow(
        &repo_root,
        &CommitOptions {
            add: !no_add,
            push,
            create_draft_pr: true,
            message: message.map(str::to_string),
            agent: agent_override.map(str::to_string),
            ..CommitOptions::for_task("commit")
        },
        &CliProgress,
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

pub fn run_pm(cmd: &PmCommand) -> Result<()> {
    let progress = &CliProgress;
    let repo_root = find_repo_root()?;
    // The one ambient-Wave rule for every PM arm: `--wave` wins, else
    // `LF_WAVE_ID` (durable UUID → registry name, hand-set name as fallback).
    // `NoContext` stays `None` so a bare command keeps its "all waves" / "pass
    // --wave" behavior outside a managed process; a stale id is a loud error.
    let ambient_wave = |explicit: Option<&str>| -> Result<Option<String>> {
        use crate::engine::wave_context::WaveResolveError;
        match crate::engine::wave_context::resolve_managed_wave_name_sync(explicit) {
            Ok(name) => Ok(Some(name)),
            Err(WaveResolveError::NoContext) => Ok(None),
            Err(other) => Err(other.into()),
        }
    };

    match cmd {
        PmCommand::Init {
            wave,
            wave_flag,
            all,
            team_key,
            team_name,
        } => {
            let targets = if *all {
                crate::ops::pm::list_local_waves(&repo_root)?
            } else {
                let explicit = wave.as_deref().or(wave_flag.as_deref());
                // pm init is a creation flow: an explicit --wave may name a
                // wave not yet registered (it links a wave directory to
                // Linear, not a registry row). Normalize-only for explicit;
                // ambient still uses the shared validating resolver.
                let name = if let Some(raw) = explicit {
                    crate::ops::normalize_wave_name(raw)
                        .ok_or_else(|| anyhow!("--wave requires a non-empty wave name"))?
                } else {
                    ambient_wave(None)?
                        .ok_or_else(|| anyhow!("cannot determine wave; pass --wave <name>"))?
                };
                vec![name]
            };
            for wave in targets {
                let result = crate::ops::pm::pm_init(
                    &repo_root,
                    &crate::ops::pm::PmInitOptions {
                        wave: Some(wave),
                        team_key: team_key.clone(),
                        team_name: team_name.clone(),
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
        }
        PmCommand::Show {
            wave,
            project,
            json,
            sync,
            no_sync,
        } => {
            let refresh = if *sync {
                crate::ops::pm::PmRefresh::Force
            } else if *no_sync {
                crate::ops::pm::PmRefresh::Never
            } else {
                crate::ops::pm::PmRefresh::Auto
            };
            let options = crate::ops::pm::PmShowOptions {
                wave: ambient_wave(wave.as_deref())?,
                project: project.clone(),
                refresh,
            };
            let result = if *json {
                crate::ops::pm::pm_show(&repo_root, &options, &crate::ops::NullProgress)?
            } else {
                crate::ops::pm::pm_show(&repo_root, &options, progress)?
            };
            if *json {
                println!("{}", serde_json::to_string(&result)?);
            } else {
                print_pm_show_result(&result);
            }
        }
        PmCommand::Status { wave } => {
            let result = crate::ops::pm::pm_status(
                &repo_root,
                &crate::ops::pm::PmStatusOptions {
                    wave: ambient_wave(wave.as_deref())?,
                },
                progress,
            )?;
            if result.waves.is_empty() {
                println!("no PM-linked waves");
            } else {
                for wave in result.waves {
                    println!(
                        "{}: Linear Initiative `{}` ({}) — {} open / {} total",
                        wave.wave, wave.initiative_name, wave.initiative, wave.open, wave.total
                    );
                    for (project, open) in wave.open_by_project {
                        println!("  {project:<28} {open} open");
                    }
                }
            }
        }
        PmCommand::Rename { wave, title } => {
            let result = crate::ops::pm::pm_rename(
                &repo_root,
                &crate::ops::pm::PmRenameOptions {
                    wave: ambient_wave(wave.as_deref())?,
                    title: title.clone(),
                },
                progress,
            )?;
            println!(
                "{}: renamed Linear Initiative {} to `{}`",
                result.wave, result.initiative, result.title
            );
        }
        PmCommand::Task { cmd } => match cmd {
            PmTaskCommand::Create {
                wave,
                project,
                title,
                notes,
            } => {
                let result = crate::ops::pm::pm_update(
                    &repo_root,
                    &crate::ops::pm::PmUpdateOptions {
                        wave: ambient_wave(wave.as_deref())?,
                        project: Some(project.clone()),
                        id: None,
                        title: Some(title.clone()),
                        notes: notes.clone(),
                        status: None,
                        pr: None,
                    },
                    progress,
                )?;
                println!(
                    "{}: created task {} in project:{}",
                    result.wave, result.id, project
                );
            }
            PmTaskCommand::Update {
                id,
                wave,
                project,
                title,
                notes,
            } => {
                let result = crate::ops::pm::pm_update(
                    &repo_root,
                    &crate::ops::pm::PmUpdateOptions {
                        wave: ambient_wave(wave.as_deref())?,
                        project: project.clone(),
                        id: Some(id.clone()),
                        title: title.clone(),
                        notes: notes.clone(),
                        status: None,
                        pr: None,
                    },
                    progress,
                )?;
                println!("{}: updated task {}", result.wave, result.id);
            }
            PmTaskCommand::Done { id, wave, pr } => {
                let result = crate::ops::pm::pm_update(
                    &repo_root,
                    &crate::ops::pm::PmUpdateOptions {
                        wave: ambient_wave(wave.as_deref())?,
                        project: None,
                        id: Some(id.clone()),
                        title: None,
                        notes: None,
                        status: Some("done".to_string()),
                        pr: pr.clone(),
                    },
                    progress,
                )?;
                let linked = match result.linked_pr {
                    Some(pr) => format!(", linked {pr}"),
                    None => String::new(),
                };
                println!("{}: closed task {}{linked}", result.wave, result.id);
            }
            PmTaskCommand::Move { id, wave, project } => {
                let result = crate::ops::pm::pm_task_move(
                    &repo_root,
                    &crate::ops::pm::PmTaskMoveOptions {
                        id: id.clone(),
                        wave: ambient_wave(wave.as_deref())?,
                        project: project.clone(),
                    },
                    progress,
                )?;
                println!(
                    "{}: moved task {} to project:{}",
                    result.wave, result.id, result.project
                );
            }
        },
        PmCommand::Project { cmd } => {
            let (wave, project, title, definition, krs, first, loop_, finally) = match cmd {
                PmProjectCommand::Create {
                    wave,
                    title,
                    definition,
                    krs,
                    first,
                    loop_,
                    finally,
                } => (
                    wave.clone(),
                    None,
                    Some(title.clone()),
                    Some(definition.clone()),
                    krs.clone(),
                    first.clone(),
                    loop_.clone(),
                    finally.clone(),
                ),
                PmProjectCommand::Update {
                    wave,
                    project,
                    title,
                    definition,
                    krs,
                    first,
                    loop_,
                    finally,
                } => (
                    wave.clone(),
                    Some(project.clone()),
                    title.clone(),
                    definition.clone(),
                    krs.clone(),
                    first.clone(),
                    loop_.clone(),
                    finally.clone(),
                ),
                PmProjectCommand::Archive { wave, project } => {
                    let result = crate::ops::pm::pm_project_archive(
                        &repo_root,
                        &crate::ops::pm::PmProjectArchiveOptions {
                            wave: ambient_wave(wave.as_deref())?,
                            project: project.clone(),
                        },
                        progress,
                    )?;
                    println!(
                        "{}: archived project:{} ({})",
                        result.wave, result.slug, result.id
                    );
                    return Ok(());
                }
            };
            let result = crate::ops::pm::pm_project_write(
                &repo_root,
                &crate::ops::pm::PmProjectWriteOptions {
                    wave: ambient_wave(wave.as_deref())?,
                    project,
                    title,
                    definition,
                    krs,
                    first,
                    loop_,
                    finally,
                },
                progress,
            )?;
            let verb = if result.created { "created" } else { "updated" };
            println!(
                "{}: {verb} project:{} ({})",
                result.wave, result.slug, result.id
            );
        }
        PmCommand::Doctor => {
            let result = crate::ops::pm::pm_sync(
                &repo_root,
                &crate::ops::pm::PmSyncOptions {
                    wave: None,
                    plan: true,
                },
                progress,
            )?;
            print_pm_sync_result(&result);
        }
        PmCommand::Sync { wave, plan } => {
            let result = crate::ops::pm::pm_sync(
                &repo_root,
                &crate::ops::pm::PmSyncOptions {
                    wave: ambient_wave(wave.as_deref())?,
                    plan: *plan,
                },
                progress,
            )?;
            print_pm_sync_result(&result);
        }
        PmCommand::Reteam { apply } => {
            let result = crate::ops::pm::pm_reteam(
                &repo_root,
                &crate::ops::pm::PmReteamOptions { apply: *apply },
                progress,
            )?;
            print_pm_reteam_result(&result);
        }
        PmCommand::Webhook { cmd } => run_pm_webhook(&repo_root, cmd)?,
    }
    Ok(())
}

/// The Linear webhook receiver and its one-time registration. The signing secret
/// is read from the environment (sourced from Doppler), never a flag or the
/// store, so a raw value never lands in shell history or a process listing.
fn run_pm_webhook(repo_root: &std::path::Path, cmd: &crate::lf::PmWebhookCommand) -> Result<()> {
    use crate::lf::PmWebhookCommand;

    let secret = std::env::var("LF_LINEAR_WEBHOOK_SECRET").unwrap_or_default();
    if secret.is_empty() {
        return Err(anyhow!(
            "set LF_LINEAR_WEBHOOK_SECRET to a non-empty value (source it from Doppler: `doppler run -- lf pm webhook ...`)"
        ));
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let client = crate::ops::pm::linear_client(repo_root).await?;
        match cmd {
            PmWebhookCommand::Register { url, .. } => {
                let id = client.create_webhook(url, &secret).await?;
                println!("registered Linear webhook {id} → {url}");
                Ok(())
            }
            PmWebhookCommand::Serve { addr, .. } => {
                let viewer = client.viewer_id().await?;
                let store = std::sync::Arc::new(
                    crate::store::open_existing_store()
                        .await
                        .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
                );
                let socket: std::net::SocketAddr = addr
                    .parse()
                    .map_err(|error| anyhow!("invalid --addr {addr:?}: {error}"))?;
                println!(
                    "lf pm webhook · serving Linear deliveries on http://{socket}/linear/webhook"
                );
                crate::webhook::serve(store, secret.into_bytes(), viewer, socket).await
            }
        }
    })
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

    if !result.deferrals.is_empty() {
        println!(
            "  deferred — protected active Task Run ({}):",
            result.deferrals.len()
        );
        for deferral in &result.deferrals {
            println!(
                "    wave/{}: {}  {}  ({})",
                deferral.wave, deferral.identifier, deferral.title, deferral.reason
            );
        }
    }
    if result.task_updates > 0 {
        println!("  reconciled Task identifiers: {}", result.task_updates);
    }
    if result.already > 0 {
        println!("  already in repository Team: {} (skipped)", result.already);
    }
}

fn print_pm_show_result(result: &crate::ops::pm::PmShowResult) {
    if result.items.is_empty() {
        let suffix = result
            .project
            .as_deref()
            .map(|project| format!(" project:{project}"))
            .unwrap_or_default();
        println!("{}{}: no Linear tasks", result.wave, suffix);
        print_pm_snapshot_age(result);
        return;
    }

    let colors = Colors::default();
    for (index, line) in format_pm_task_table(&result.items).iter().enumerate() {
        if index == 0 {
            println!("{}{}{}", colors.bold, line, colors.reset);
        } else {
            println!("{line}");
        }
    }
    print_pm_snapshot_age(result);
}

fn print_pm_snapshot_age(result: &crate::ops::pm::PmShowResult) {
    let age = time::OffsetDateTime::now_utc().unix_timestamp() - result.synced_at;
    let colors = Colors::default();
    let phrase = if age < 60 {
        "just now".to_string()
    } else {
        format!("{} ago", crate::ops::pm::format_age(age))
    };
    println!("{}snapshot synced {}{}", colors.dim, phrase, colors.reset);
}

#[derive(Debug)]
struct PmTaskRow {
    status: &'static str,
    title: String,
    project: String,
    assignee: String,
    id: String,
    completed: bool,
    rank: u32,
}

fn format_pm_task_table(items: &[crate::pm::PmItem]) -> Vec<String> {
    let mut rows: Vec<_> = items
        .iter()
        .map(|item| PmTaskRow {
            status: if item.completed { "done" } else { "open" },
            title: item.name.split_whitespace().collect::<Vec<_>>().join(" "),
            project: item.project.clone(),
            assignee: item.assignee.clone().unwrap_or_else(|| "-".to_string()),
            id: item.id.clone(),
            completed: item.completed,
            rank: item.rank,
        })
        .collect();
    rows.sort_by_key(|row| (row.completed, row.rank));

    let status_width = column_width("STATUS", rows.iter().map(|row| row.status));
    let title_width = column_width("TITLE", rows.iter().map(|row| row.title.as_str()));
    let project_width = column_width("PROJECT", rows.iter().map(|row| row.project.as_str()));
    let assignee_width = column_width("ASSIGNEE", rows.iter().map(|row| row.assignee.as_str()));

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push(format!(
        "{:<status_width$}  {:<title_width$}  {:<project_width$}  {:<assignee_width$}  ID",
        "STATUS", "TITLE", "PROJECT", "ASSIGNEE"
    ));
    lines.extend(rows.into_iter().map(|row| {
        format!(
            "{:<status_width$}  {:<title_width$}  {:<project_width$}  {:<assignee_width$}  {}",
            row.status, row.title, row.project, row.assignee, row.id
        )
    }));
    lines
}

#[cfg(test)]
mod pm_output_tests {
    use super::format_pm_task_table;
    use crate::pm::PmItem;

    #[test]
    fn task_table_is_aligned_complete_and_open_first() {
        let lines = format_pm_task_table(&[
            PmItem {
                id: "done-1".to_string(),
                identifier: "INF-1".to_string(),
                url: None,
                name: "Done task".to_string(),
                description: String::new(),
                rank: 0,
                completed: true,
                project_id: "project-done".to_string(),
                project: "-".to_string(),
                team_id: "team-loo".to_string(),
                assignee: None,
            },
            PmItem {
                id: "open-1".to_string(),
                identifier: "INF-2".to_string(),
                url: None,
                name: "Longer\ntitle".to_string(),
                description: String::new(),
                rank: 1,
                completed: false,
                project_id: "project-chat".to_string(),
                project: "wave-chat".to_string(),
                team_id: "team-loo".to_string(),
                assignee: Some("me".to_string()),
            },
        ]);

        assert_eq!(
            lines,
            vec![
                "STATUS  TITLE         PROJECT    ASSIGNEE  ID",
                "open    Longer title  wave-chat  me        open-1",
                "done    Done task     -          -         done-1",
            ]
        );
        assert!(lines.iter().all(|line| line.lines().count() == 1));
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

pub fn cron_cmd(cmd: &CronCommand) -> Result<()> {
    let launch_agents_dir = crate::ops::default_launch_agents_dir()?;
    match cmd {
        CronCommand::Add {
            wave,
            flow,
            schedule,
        } => {
            let repo_root = find_repo_root()?;
            // The one ambient-Wave rule, like every PM arm: `--wave` wins, else
            // `LF_WAVE_ID` (UUID → registry name, hand-set name as fallback). A
            // scheduled invocation needs a concrete wave, so `NoContext` is the
            // familiar "pass --wave" error.
            let wave = crate::engine::wave_context::resolve_managed_wave_name_sync(wave.as_deref())
                .map_err(|err| match err {
                    crate::engine::wave_context::WaveResolveError::NoContext => {
                        anyhow!("cannot determine wave; pass --wave <name>")
                    }
                    other => other.into(),
                })?;
            let spec = CronSpec {
                wave,
                flow: flow.clone(),
                schedule: crate::ops::parse_schedule(schedule)?,
                working_directory: repo_root,
                lf_path: crate::ops::resolve_lf_path()?,
            };
            let cron = crate::ops::add_cron(&launch_agents_dir, &spec, &SystemLaunchctl)?;
            println!("installed {} at {}", cron.label, cron.path.display());
        }
        CronCommand::List => {
            let crons = crate::ops::list_crons(&launch_agents_dir)?;
            if crons.is_empty() {
                println!("no loopflow crons installed");
            } else {
                for cron in crons {
                    println!("{} {} {}", cron.label, cron.wave, cron.flow);
                }
            }
        }
        CronCommand::Sync { wave } => {
            let repo_root = find_repo_root()?;
            let declared = crate::engine::wave_config::read_wave_config(&repo_root, wave)
                .and_then(|config| config.crons)
                .unwrap_or_default();
            let lf_path = crate::ops::resolve_lf_path()?;
            let result = crate::ops::sync_crons(
                &launch_agents_dir,
                wave,
                &declared,
                &repo_root,
                &lf_path,
                &SystemLaunchctl,
            )?;
            if result.installed.is_empty() && result.removed.is_empty() && result.skipped.is_empty()
            {
                println!("no crons declared for wave {wave}; nothing to sync");
            }
            for cron in &result.installed {
                println!("installed {} ({})", cron.label, cron.flow);
            }
            for cron in &result.removed {
                println!("pruned {} ({})", cron.label, cron.flow);
            }
            for skip in &result.skipped {
                eprintln!("skipped {} ({}): {}", skip.flow, skip.schedule, skip.reason);
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

fn release_check_cmd(target_name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let changes = release_check(&repo_root, target_name)?;

    if changes.commits.is_empty() {
        eprintln!("No commits in the target area since the last tag.");
        std::process::exit(1);
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
        crate::ops::ReleaseRunOutcome::NoChanges { target, latest_tag } => {
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
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    release_notes(&repo_root, version, prev_tag, target_name, progress)?;
    println!(
        "RELEASE_NOTES.md updated for v{}",
        version.trim_start_matches('v')
    );
    Ok(())
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

    println!(
        "GitHub Release: {}",
        if status.release_exists { "yes" } else { "no" }
    );
    Ok(())
}

pub fn run_wt(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create { name, plan } => wt_create(name, *plan),
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { format, sync, .. } => wt_list(format.as_deref(), *sync),
        WtCommand::Remove { name, force } => wt_remove(name, *force),
        WtCommand::Prune { dry_run } => wt_prune(*dry_run),
        WtCommand::Ci { watch, logs } => wt_ci(*watch, *logs),
    }
}

fn wt_create(name: &str, dry_run: bool) -> Result<()> {
    let started = Instant::now();
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let segment = WorktreeSegment::parse(name)?;

    let default_branch = get_default_branch(&main_repo)?;
    let _ = sync_main(&main_repo, &default_branch);

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
/// operations (`rebase --plan`, status, dispatch) never dirty a tracked
/// worktree. Single source of truth: `crate::ops::telemetry`.
use crate::ops::telemetry::record_ops_metric;

fn wt_switch(name: &str) -> Result<()> {
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

    cd_directive(&path)
}

fn cd_directive(path: &Path) -> Result<()> {
    if !write_shell_directive(&format!("cd {}", path.display()))? {
        println!("cd {}", path.display());
    }
    Ok(())
}

fn wt_list(format: Option<&str>, sync: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let default_branch = get_default_branch(&main_repo)?;
    // `wt list` is an inspection surface and stays side-effect free by default:
    // merge/fresh flags reflect the last-synced main. `--sync` is the explicit,
    // self-owned mutation that fetches origin and fast-forwards main first — a
    // read never fetches, resets, or stashes the canonical checkout behind the
    // user's back.
    if sync {
        let _ = sync_main(&main_repo, &default_branch);
    }
    let worktrees = list_worktrees(&main_repo)?;

    if matches!(format, Some("json")) {
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
            let diff_stat = if is_main {
                String::new()
            } else {
                wt_diff_stat(&main_repo, wt.branch.as_deref(), &default_branch)
            };
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

fn wt_remove(name: &str, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;

    // Find the worktree by short name or directory name
    let worktrees = list_worktrees(&main_repo)?;
    let target = worktrees.iter().find(|wt| {
        sibling_worktree_name(&wt.path).as_deref() == Some(name)
            || wt
                .path
                .file_name()
                .map(|n| n.to_string_lossy() == name)
                .unwrap_or(false)
    });

    let wt = match target {
        Some(wt) => wt,
        None => return Err(anyhow!("no worktree found for '{}'", name)),
    };

    if wt.path == repo_root {
        return Err(anyhow!("cannot remove the current worktree"));
    }

    let default_branch = get_default_branch(&main_repo)?;
    if wt.branch.as_deref() == Some(&default_branch) {
        return Err(anyhow!("cannot remove the main worktree"));
    }

    if !force && wt.dirty {
        return Err(anyhow!(
            "worktree has uncommitted changes (use --force to override)"
        ));
    }

    let branch = wt.branch.clone();
    crate::engine::git::worktree_remove(&main_repo, &wt.path)?;
    if let Some(branch) = branch {
        let _ = delete_local_branch(&main_repo, &branch);
    }
    println!("Removed {}", name);
    Ok(())
}

/// Get a compact diff stat for a branch vs default branch.
fn wt_diff_stat(repo: &std::path::Path, branch: Option<&str>, default_branch: &str) -> String {
    let branch = match branch {
        Some(b) => b,
        None => return String::new(),
    };
    let target = format!("origin/{default_branch}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["diff", "--shortstat", &format!("{target}...{branch}")])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // "3 files changed, 10 insertions(+), 5 deletions(-)" → "+10 -5 (3 files)"
            parse_shortstat(&raw)
        }
        _ => String::new(),
    }
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
    let default_branch = get_default_branch(&main_repo)?;
    let _ = sync_main(&main_repo, &default_branch);
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

    // A development binary owns an isolated `.lf-dev` registry, but pruning is
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

fn wt_ci(watch: bool, logs: bool) -> Result<()> {
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
/// reasoning — e.g., rebase conflicts that need conflict resolution.
fn launch_skill_agent(
    repo_root: &Path,
    skill_name: &str,
    context: Option<&str>,
    env: Option<&std::collections::BTreeMap<String, String>>,
) -> Result<()> {
    let skill = discover_skill(repo_root, skill_name)?;
    let config = load_config_or_default(Some(repo_root));

    let message = context.map(|value| value.to_string());
    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.to_path_buf(),
            skill: Some(skill_name.to_string()),
            resolved_skill: Some(skill),
            surface: Surface::Headless,
            message,
            cwd: Some(repo_root.to_path_buf()),
            yolo_mode: config.yolo,
            source_overrides: ContextSourceOverrides {
                // Rebase conflicts already name the affected paths in `context`.
                // Embedding every authored file here makes the task prompt grow
                // with the branch and can exceed the OS argument limit before
                // the resolver starts. The agent has the repository as its cwd
                // and can inspect the conflict in place.
                diff_files: Some(false),
                diff: Some(false),
                ..Default::default()
            },
            ..LaunchPromptInput::default()
        },
    )?;

    let effective_system =
        crate::engine::agent::system_prompt_with_structured_replies(&prepared.config);
    let context = crate::lf::commands::run::attributed_context(
        &prepared.components,
        &effective_system,
        &prepared.config.task_prompt,
        &prepared.deduplicated_docs,
    );
    let agent = prepared.config.agent();
    let (provider, model) = crate::engine::parse_agent(agent);
    let capture_context =
        crate::journal::trace_capture_context(repo_root, None, Some(skill_name.to_string()))
            .ok_or_else(|| anyhow!("trace capture identity is unavailable before agent launch"))?;
    let capture = crate::trace::CaptureHandle::begin(
        capture_context,
        context,
        crate::trace::CaptureStart {
            provider,
            model,
            surface: "headless".to_string(),
            input_op: "initial".to_string(),
            gather_ms: 0,
            render_ms: 0,
            raw_provider: true,
            basis: None,
            supervision: None,
        },
    )?;

    let process = ProcessConfig {
        auto: true,
        stream: true,
        capture: Some(capture.clone()),
        env: env.cloned().unwrap_or_default(),
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = launch_agent(&prepared.config, &process, &capabilities);
    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture.finish(outcome, false)?;
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
    /// `brew install <name>` — plain formula (or tap-qualified, e.g. dopplerhq/cli/doppler).
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
        brew: Some(Brew::Formula("dopplerhq/cli/doppler")),
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
        fallback: "lf init",
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
        name: "gemini",
        command: "gemini",
        required: false,
        macos_only: false,
        brew: None,
        fallback: "npm install -g @google/gemini-cli",
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
