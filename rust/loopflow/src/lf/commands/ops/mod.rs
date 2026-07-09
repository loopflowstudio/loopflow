use crate::engine::agent::{launch_agent, AgentCapabilities, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::{current_branch, delete_local_branch, get_default_branch, sync_main};
use crate::engine::identity::WaveId;
use crate::engine::naming::git_user;
use crate::engine::worktrees::{
    create_from_placement_plan, list_worktrees, main_repo_root, plan_placement,
    wave_name_from_worktree, wave_name_from_worktree_and_main, worktree_path, PlacementRequest,
    PlacementStrategy, WorktreeSegment,
};
use crate::engine::{
    prepare_launch_prompt, sync_skills, ContextSourceOverrides, LaunchPromptInput,
    SkillSyncOptions, Surface,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::discover_skill;
use crate::lf::output::Colors;
use crate::lf::{CronCommand, PmCommand, PmTaskCommand, PrCommand, ReleaseCommand, WtCommand};
use crate::ops::OpsError;
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, current_pr, land, next_branch,
    plan_rebase, rebase_class_name, rebase_strategy_name, rebase_with_recovery, release_bump,
    release_check, release_notes, release_run, release_status, release_tag, submit, AbandonOptions,
    CommitOptions, CronSpec, LandOptions, NextOptions, PrOptions, Progress, RebaseOptions,
    SystemLaunchctl,
};
use anyhow::{anyhow, Result};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

pub fn run_pr(cmd: Option<&PrCommand>, cli_model: Option<&str>) -> Result<()> {
    let progress = CliProgress;
    match cmd {
        None | Some(PrCommand::Status) => pr_status(),
        Some(PrCommand::Open { model, title, body }) => open_pr(
            title.clone(),
            body.clone(),
            model.as_deref().or(cli_model),
            &progress,
        ),
        Some(PrCommand::Submit {
            strict,
            create_pr,
            worktree,
            message,
            title,
            body,
        }) => submit_current(
            &LandOptions {
                strict: *strict,
                local: false,
                create_pr: *create_pr,
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
            create_pr,
            worktree,
            message,
            title,
            body,
        }) => land_current(
            &LandOptions {
                strict: *strict,
                local: *local,
                create_pr: *create_pr,
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
    }
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

pub fn run_rebase(onto: Option<&str>, plan_only: bool) -> Result<()> {
    let progress = &CliProgress;
    let repo_root = find_repo_root()?;
    let started = Instant::now();
    let plan = plan_rebase(&repo_root, onto)?;
    let onto_ref = plan.base_ref.clone();
    if plan_only {
        print_rebase_plan(&plan);
        return Ok(());
    }
    match rebase_with_recovery(
        &repo_root,
        &RebaseOptions {
            onto: onto_ref.clone(),
            push: true,
        },
        progress,
    ) {
        Ok(()) => {
            record_ops_metric(
                &repo_root,
                serde_json::json!({
                    "op": "rebase",
                    "branch": plan.branch,
                    "base_ref": plan.base_ref,
                    "stack_parent": plan.stack_parent,
                    "class": rebase_class_name(&plan.class),
                    "strategy": rebase_strategy_name(&plan.strategy),
                    "unique_commits": plan.unique_commits,
                    "changed_files": plan.changed_files.len(),
                    "protected": plan.protected,
                    "scratch_stashed": plan.scratch_stashed,
                    "agent_launched": false,
                    "duration_ms": started.elapsed().as_millis(),
                    "exit_status": "ok",
                }),
            );
            Ok(())
        }
        Err(OpsError::RebaseConflict { onto, detail }) => {
            let context = format!(
                "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\n</lf:rebase-conflict>"
            );
            progress.status("Launching rebase agent to resolve conflicts...");
            launch_skill_agent(&repo_root, "rebase", Some(&context))
        }
        Err(err) => Err(err.into()),
    }
}

fn print_rebase_plan(plan: &crate::ops::RebasePlan) {
    println!("branch: {}", plan.branch);
    println!("base: {}", plan.base_ref);
    if let Some(parent) = plan.stack_parent.as_deref() {
        println!("stack_parent: {parent}");
    }
    println!("class: {}", rebase_class_name(&plan.class));
    println!("strategy: {}", rebase_strategy_name(&plan.strategy));
    println!("unique_commits: {}", plan.unique_commits);
    println!("changed_files: {}", plan.changed_files.len());
    println!("protected: {}", plan.protected);
    println!("agent_launched: false");
}

/// Run a PR-mutating op; on a rebase conflict, launch the rebase agent to
/// resolve it and retry once. A second conflict is a real error.
fn with_rebase_retry<T>(
    repo_root: &Path,
    label: &str,
    progress: &impl Progress,
    op: impl Fn(&Path) -> Result<T, OpsError>,
) -> Result<T> {
    match op(repo_root) {
        Ok(value) => Ok(value),
        Err(OpsError::RebaseConflict { onto, detail }) => {
            let context = format!(
                "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\n</lf:rebase-conflict>"
            );
            progress.status("Launching rebase agent to resolve conflicts...");
            launch_skill_agent(repo_root, "rebase", Some(&context))?;
            progress.status(&format!("Retrying {label} after rebase..."));
            op(repo_root).map_err(Into::into)
        }
        Err(err) => Err(err.into()),
    }
}

fn land_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    // The wave home stays put on land — no rotation, no cd.
    with_rebase_retry(&repo_root, "land", progress, |repo| {
        land(repo, options, progress)
    })?;
    Ok(())
}

fn submit_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    with_rebase_retry(&repo_root, "submit", progress, |repo| {
        submit(repo, options, progress)
    })?;
    progress.status("Ready to land — click merge on the PR once checks pass.");
    Ok(())
}

fn open_pr(
    title: Option<String>,
    body: Option<String>,
    agent_override: Option<&str>,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = with_rebase_retry(&repo_root, "PR creation", progress, |repo| {
        create_or_update_pr(
            repo,
            &PrOptions {
                title: title.clone(),
                body: body.clone(),
                agent: agent_override.map(str::to_string),
            },
            progress,
        )
    })?;
    println!("{}", result.url);
    Ok(())
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

pub fn run_sync() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let ok = crate::engine::git::sync_main(&repo_root, &main_branch)?;
    if !ok {
        return Err(anyhow!("working tree dirty; sync aborted"));
    }
    Ok(())
}

pub fn run_next(create_pr: bool, no_rebase: bool, agent_override: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = next_branch(
        &repo_root,
        &NextOptions {
            create_pr,
            rebase: !no_rebase,
            wave_name: None,
            agent: agent_override.map(str::to_string),
        },
        &CliProgress,
    )?;
    println!("{}", result.new_branch);
    Ok(())
}

pub fn run_advance(wave: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let wave = crate::ops::util::resolve_wave_name(&repo_root, wave)
        .ok_or_else(|| anyhow!("cannot determine wave name (pass --wave)"))?;
    let new_branch = crate::ops::advance_branch(&repo_root, &wave)?;
    println!("{new_branch}");
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
    let list_all_waves = || -> Result<Vec<String>> {
        let wave_dir = repo_root.join("wave");
        if !wave_dir.is_dir() {
            return Err(anyhow!("no wave/ directory found"));
        }
        let mut waves = Vec::new();
        for entry in std::fs::read_dir(&wave_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    waves.push(name.to_string());
                }
            }
        }
        waves.sort();
        if waves.is_empty() {
            return Err(anyhow!("no waves found in wave/"));
        }
        Ok(waves)
    };

    match cmd {
        PmCommand::Init {
            wave,
            wave_flag,
            all,
        } => {
            let targets = if *all {
                list_all_waves()?
            } else {
                vec![wave
                    .clone()
                    .or_else(|| wave_flag.clone())
                    .or_else(|| crate::ops::util::resolve_wave_name(&repo_root, None))
                    .ok_or_else(|| anyhow!("cannot determine wave name"))?]
            };
            for wave in targets {
                let result = crate::ops::pm::pm_init(
                    &repo_root,
                    &crate::ops::pm::PmInitOptions { wave: Some(wave) },
                    progress,
                )?;
                let state = if result.created {
                    "created"
                } else {
                    "already linked"
                };
                println!(
                    "{}: Linear project {} ({state})",
                    result.wave, result.project_id
                );
            }
        }
        PmCommand::Show { wave, project } => {
            let result = crate::ops::pm::pm_show(
                &repo_root,
                &crate::ops::pm::PmShowOptions {
                    wave: wave.clone(),
                    project: project.clone(),
                },
                progress,
            )?;
            print_pm_show_result(&result);
        }
        PmCommand::Update {
            wave,
            project,
            id,
            title,
            notes,
            status,
            pr,
        } => {
            let result = crate::ops::pm::pm_update(
                &repo_root,
                &crate::ops::pm::PmUpdateOptions {
                    wave: wave.clone(),
                    project: project.clone(),
                    id: id.clone(),
                    title: title.clone(),
                    notes: notes.clone(),
                    status: status.clone(),
                    pr: pr.clone(),
                },
                progress,
            )?;
            let verb = if result.created { "created" } else { "updated" };
            let closed = if result.completed { ", closed" } else { "" };
            let linked = match result.linked_pr {
                Some(pr) => format!(", linked {pr}"),
                None => String::new(),
            };
            println!("{}: {verb} task {}{closed}{linked}", result.wave, result.id);
        }
        PmCommand::Status { wave } => {
            let result = crate::ops::pm::pm_status(
                &repo_root,
                &crate::ops::pm::PmStatusOptions { wave: wave.clone() },
                progress,
            )?;
            if result.waves.is_empty() {
                println!("no PM-linked waves");
            } else {
                for wave in result.waves {
                    let linear_project = wave.project_name.as_deref().unwrap_or("-");
                    println!(
                        "{}: Linear project `{linear_project}` ({}) — {} open / {} total, {} unassigned",
                        wave.wave, wave.project, wave.open, wave.total, wave.unassigned
                    );
                    for (project, open) in wave.open_by_project {
                        println!("  {project:<28} {open} open");
                    }
                }
            }
            for project in result.stranded_projects {
                println!(
                    "stranded: Linear project `{}` ({}) — no local wave points here",
                    project.name, project.id
                );
            }
        }
        PmCommand::Rename { wave, title } => {
            let result = crate::ops::pm::pm_rename(
                &repo_root,
                &crate::ops::pm::PmRenameOptions {
                    wave: wave.clone(),
                    title: title.clone(),
                },
                progress,
            )?;
            println!(
                "{}: renamed Linear project {} to `{}`",
                result.wave, result.project, result.title
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
                        wave: wave.clone(),
                        project: project.clone(),
                        id: None,
                        title: Some(title.clone()),
                        notes: notes.clone(),
                        status: None,
                        pr: None,
                    },
                    progress,
                )?;
                let suffix = project
                    .as_deref()
                    .map(|project| format!(" project:{project}"))
                    .unwrap_or_default();
                println!("{}: created task {}{suffix}", result.wave, result.id);
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
                        wave: wave.clone(),
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
                        wave: wave.clone(),
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
                        wave: wave.clone(),
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
        PmCommand::Doctor => {
            let result = crate::ops::pm::pm_sync(
                &repo_root,
                &crate::ops::pm::PmSyncOptions { plan: true },
                progress,
            )?;
            print_pm_sync_result(&result);
        }
        PmCommand::Sync { plan } => {
            let result = crate::ops::pm::pm_sync(
                &repo_root,
                &crate::ops::pm::PmSyncOptions { plan: *plan },
                progress,
            )?;
            print_pm_sync_result(&result);
        }
    }
    Ok(())
}

fn print_pm_show_result(result: &crate::ops::pm::PmShowResult) {
    if result.items.is_empty() {
        let suffix = result
            .local_project
            .as_deref()
            .map(|project| format!(" project:{project}"))
            .unwrap_or_default();
        println!("{}{}: no Linear tasks", result.wave, suffix);
        return;
    }

    if result.local_project.is_some() {
        for item in &result.items {
            println!("{}", crate::ops::pm::format_task_item(item));
        }
        return;
    }

    let mut grouped: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
    let mut unassigned = Vec::new();
    for item in &result.items {
        let projects = crate::ops::pm::item_project_labels(item);
        if projects.is_empty() {
            unassigned.push(item);
        } else {
            for project in projects {
                grouped.entry(project).or_default().push(item);
            }
        }
    }

    for (project, items) in grouped {
        println!("project:{project}");
        for item in items {
            println!("  {}", crate::ops::pm::format_task_item(item));
        }
    }
    if !unassigned.is_empty() {
        println!("unassigned:");
        for item in unassigned {
            println!("  {}", crate::ops::pm::format_task_item(item));
        }
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
            let spec = CronSpec {
                wave: wave.clone(),
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
    let prs = release_check(&repo_root, target_name)?;

    if prs.is_empty() {
        eprintln!("No PRs merged since last tag.");
        std::process::exit(1);
    }

    let is_tty = std::io::stdout().is_terminal();
    if is_tty {
        for pr in &prs {
            println!(
                "#{:<6} {} (+{} -{}, {} files)",
                pr.number, pr.title, pr.additions, pr.deletions, pr.changed_files
            );
        }
        println!("\n{} PR(s) merged since last tag.", prs.len());
    } else {
        let json = serde_json::to_string_pretty(&prs)?;
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
    let result = release_run(&repo_root, input, target_name, progress)?;

    println!("Released {} ({})", result.tag, result.target);
    if let Some(url) = result.workflow_url.as_deref() {
        println!("Workflow URL: {url}");
    }
    println!(
        "GitHub Release: {}",
        if result.release_exists { "yes" } else { "no" }
    );
    Ok(())
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
        WtCommand::Create {
            name,
            child,
            sibling: _,
            plan,
        } => wt_create(name, child.as_deref(), *plan),
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::Up => wt_up(),
        WtCommand::Down { name } => wt_down(name.as_deref()),
        WtCommand::List { format, .. } => wt_list(format.as_deref()),
        WtCommand::Remove { name, force } => wt_remove(name, *force),
        WtCommand::Prune {
            dry_run,
            include_fresh,
        } => wt_prune(*dry_run, *include_fresh),
        WtCommand::Ci { watch, logs } => wt_ci(*watch, *logs),
    }
}

fn wt_create(name: &str, child: Option<&str>, dry_run: bool) -> Result<()> {
    let started = Instant::now();
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let segment = WorktreeSegment::parse(name)?;
    let current = current_branch(&repo_root)?;
    // Sibling (the default) roots from the default branch. A child stacks under
    // its parent — opt-in only via --child, so ad-hoc worktrees never nest off
    // the current feature branch by accident.
    let request = if let Some(parent) = child {
        let parent = if parent == "__current__" {
            current
                .as_deref()
                .ok_or_else(|| anyhow!("not on a branch"))?
                .to_string()
        } else {
            parent.to_string()
        };
        PlacementRequest::Stack { parent, segment }
    } else {
        PlacementRequest::Main { segment }
    };

    let default_branch = get_default_branch(&main_repo)?;
    let sync_default_base = match &request {
        PlacementRequest::Main { .. } => true,
        PlacementRequest::Stack { .. } => false,
    };
    if sync_default_base {
        let _ = sync_main(&main_repo, &default_branch);
    }

    let placement = plan_placement(&main_repo, request)?;

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
            "stack_parent": placement.parent_branch,
            "strategy": placement_strategy_name(&placement.strategy),
            "stack_depth": placement.stack_depth,
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
        println!("Base: {}", base_branch);
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
    if let Some(parent) = plan.parent_branch.as_deref() {
        println!("parent: {parent}");
    }
    println!("worktree: {}", plan.worktree_path.display());
    println!("stack_depth: {}", plan.stack_depth);
    println!("strategy: {}", placement_strategy_name(&plan.strategy));
}

fn placement_strategy_name(strategy: &PlacementStrategy) -> &'static str {
    match strategy {
        PlacementStrategy::CreateRoot => "create_root",
        PlacementStrategy::CreateStackChild => "create_stack_child",
        PlacementStrategy::CheckoutExisting => "checkout_existing",
        PlacementStrategy::UseExistingWorktree => "use_existing_worktree",
    }
}

fn record_ops_metric(repo: &Path, mut event: serde_json::Value) {
    let Some(object) = event.as_object_mut() else {
        return;
    };
    object.insert(
        "ts".to_string(),
        serde_json::Value::String(chrono::Utc::now().to_rfc3339()),
    );
    let path = repo.join(".lf").join("metrics").join("ops.jsonl");
    let Some(parent) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    else {
        return;
    };
    if serde_json::to_writer(&mut file, &event).is_ok() {
        let _ = writeln!(file);
    }
}

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
        // Path-guessing only applies to a bare wave/dir name. A full `user/…`
        // branch spec must resolve via an exact branch match (handled above) or
        // the wave-name match below — never by landing in whatever worktree
        // happens to occupy the guessed directory.
        let target = worktree_path(&main_repo, name);
        if target.exists() && !name.contains('/') {
            target
        } else {
            let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());
            let mut matches = worktrees
                .into_iter()
                .filter(|wt| {
                    let wt_name = wave_name_from_worktree_and_main(&wt.path, &main_repo);
                    // Match a chain leaf or wave name too, so `fix-auth` finds
                    // the `…bugs.fix-auth…` worktree without the full chain.
                    let id = wt
                        .branch
                        .as_deref()
                        .and_then(|branch| WaveId::parse(branch, &user));
                    wt_name.as_deref() == Some(name)
                        || wt_name
                            .as_ref()
                            .map(|n| n.starts_with(&format!("{name}.")))
                            .unwrap_or(false)
                        || wt
                            .path
                            .file_name()
                            .map(|n| n.to_string_lossy() == name)
                            .unwrap_or(false)
                        || id
                            .as_ref()
                            .map(|id| id.leaf() == name || id.wave_name() == name)
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            if matches.len() == 1 {
                matches.remove(0).path
            } else if matches.is_empty() {
                return Err(anyhow!("no worktree found for '{}'", name));
            } else {
                return Err(anyhow!("multiple worktrees match '{}'", name));
            }
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

/// The parent branch of `branch`: its chain minus the last segment, or the
/// default branch for a bare wave (or a branch that isn't a wave at all).
fn parent_branch_of(branch: &str, user: &str, default_branch: &str) -> String {
    WaveId::parse(branch, user)
        .and_then(|id| id.parent())
        .unwrap_or_else(|| default_branch.to_string())
}

/// `lf wt up` — skill to the parent worktree in the stack (toward main).
fn wt_up() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let default_branch = get_default_branch(&main_repo)?;
    let current = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
    if current == default_branch {
        return Err(anyhow!("already at the root ({default_branch})"));
    }
    let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());
    let parent = parent_branch_of(&current, &user, &default_branch);

    let target = list_worktrees(&main_repo)?
        .into_iter()
        .find(|wt| wt.branch.as_deref() == Some(parent.as_str()))
        .map(|wt| wt.path)
        .ok_or_else(|| anyhow!("no worktree for parent branch '{parent}'"))?;
    cd_directive(&target)
}

/// `lf wt down [name]` — skill to a child worktree (away from main). When
/// there is more than one child, `name` picks it by leaf.
fn wt_down(name: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let default_branch = get_default_branch(&main_repo)?;
    let current = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
    let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());

    let mut children: Vec<_> = list_worktrees(&main_repo)?
        .into_iter()
        .filter(|wt| {
            let branch = match wt.branch.as_deref() {
                Some(branch) if branch != current => branch,
                _ => return false,
            };
            parent_branch_of(branch, &user, &default_branch) == current
        })
        .collect();

    if let Some(name) = name {
        children.retain(|wt| {
            wt.branch
                .as_deref()
                .and_then(|branch| WaveId::parse(branch, &user))
                .map(|id| id.leaf() == name)
                .unwrap_or(false)
        });
    }

    match children.len() {
        0 => Err(anyhow!(
            "no child worktree{}",
            name.map(|n| format!(" named '{n}'")).unwrap_or_default()
        )),
        1 => cd_directive(&children.remove(0).path),
        _ => {
            let leaves: Vec<String> = children
                .iter()
                .filter_map(|wt| {
                    wt.branch
                        .as_deref()
                        .and_then(|branch| WaveId::parse(branch, &user))
                        .map(|id| id.leaf().to_string())
                })
                .collect();
            Err(anyhow!(
                "{} children — pick one: lf wt down <{}>",
                leaves.len(),
                leaves.join("|")
            ))
        }
    }
}

fn wt_list(format: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let default_branch = get_default_branch(&main_repo)?;
    let _ = sync_main(&main_repo, &default_branch);
    let worktrees = list_worktrees(&main_repo)?;

    if matches!(format, Some("json")) {
        let json = serde_json::to_string_pretty(&worktrees)?;
        println!("{}", json);
        return Ok(());
    }

    let c = Colors::new();
    let user = git_user(&main_repo).unwrap_or_else(|_| "user".to_string());

    // Collect display info for all worktrees. `depth`/`sort_key` come from the
    // branch's WaveId chain so children render indented under their parents.
    struct Row {
        depth: usize,
        label: String,
        stamp: Option<String>,
        sort_key: String,
        is_current: bool,
        is_main: bool,
        merged: bool,
        squash_merged: bool,
        fresh: bool,
        dirty: bool,
        remote_gone: bool,
        diff_stat: String,
    }

    let mut rows: Vec<Row> = worktrees
        .iter()
        .map(|wt| {
            let is_main = wt.branch.as_deref() == Some(&default_branch);
            let id = wt
                .branch
                .as_deref()
                .and_then(|branch| WaveId::parse(branch, &user));
            let (depth, label, stamp, sort_key) = if is_main {
                (0, default_branch.clone(), None, String::new())
            } else if let Some(id) = &id {
                (
                    id.depth(),
                    id.leaf().to_string(),
                    id.timestamp().map(str::to_string),
                    id.chain_str(),
                )
            } else {
                let name = wave_name_from_worktree(&wt.path).unwrap_or_else(|| {
                    wt.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "?".to_string())
                });
                (1, name.clone(), None, name)
            };
            let is_current = wt.path == repo_root;
            let diff_stat = if is_main {
                String::new()
            } else {
                wt_diff_stat(&main_repo, wt.branch.as_deref(), &default_branch)
            };
            Row {
                depth,
                label,
                stamp,
                sort_key,
                is_current,
                is_main,
                merged: wt.merged,
                squash_merged: wt.squash_merged,
                fresh: wt.fresh,
                dirty: wt.dirty,
                remote_gone: wt.remote_gone,
                diff_stat,
            }
        })
        .collect();

    // Main first (empty key), then a pre-order tree walk by chain.
    rows.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));

    // Displayed name = indent (one level per stacked tier) + leaf + worker stamp.
    let display_name = |row: &Row| -> String {
        let indent = "  ".repeat(row.depth.saturating_sub(1));
        match &row.stamp {
            Some(ts) => format!("{indent}{} {ts}", row.label),
            None => format!("{indent}{}", row.label),
        }
    };
    let max_name = rows
        .iter()
        .map(|r| display_name(r).len())
        .max()
        .unwrap_or(0);

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
        wave_name_from_worktree(&wt.path).as_deref() == Some(name)
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

fn wt_prune(dry_run: bool, include_fresh: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;
    let current_path = repo_root;
    let default_branch = get_default_branch(&main_repo)?;
    let _ = sync_main(&main_repo, &default_branch);
    let prune_output = Command::new("git")
        .arg("-C")
        .arg(&main_repo)
        .args(["worktree", "prune"])
        .output()?;
    if !prune_output.status.success() {
        return Err(anyhow!(
            "git worktree prune failed: {}",
            String::from_utf8_lossy(&prune_output.stderr).trim()
        ));
    }

    let worktrees = list_worktrees(&main_repo)?;
    let targets: Vec<_> = worktrees
        .into_iter()
        .filter(|wt| wt.path != current_path)
        .filter(|wt| {
            if wt.fresh {
                // Fresh worktrees: only with --include-fresh, never if dirty
                return include_fresh && !wt.dirty;
            }
            if !wt.prunable {
                return false;
            }
            // Merged/squash-merged/remote-gone: prune even if dirty
            // (landed-dirty or abandoned branch)
            true
        })
        .collect();

    if targets.is_empty() {
        println!("No prunable worktrees.");
        return Ok(());
    }

    if dry_run {
        for wt in &targets {
            let reason = if wt.merged {
                "merged"
            } else if wt.fresh {
                "fresh"
            } else if wt.squash_merged {
                "squash-merged"
            } else if wt.remote_gone {
                "remote-gone"
            } else {
                "prunable"
            };
            println!(
                "  {} ({reason})  {}",
                wt.branch.as_deref().unwrap_or("detached"),
                wt.path.display()
            );
        }
        return Ok(());
    }

    for wt in targets {
        crate::engine::git::worktree_remove(&main_repo, &wt.path)?;
        if let Some(branch) = wt.branch {
            if branch != default_branch {
                let _ = delete_local_branch(&main_repo, &branch);
            }
        }
        println!("Removed {}", wt.path.display());
    }
    Ok(())
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
        println!("\n--- Failed check logs ---\n");
        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                &branch,
                "--json",
                "statusCheckRollup",
                "-q",
                ".statusCheckRollup[] | select(.conclusion == \"FAILURE\" or .conclusion == \"failure\") | .detailsUrl",
            ])
            .current_dir(&repo_root)
            .output()?;
        if output.status.success() {
            let urls = String::from_utf8_lossy(&output.stdout);
            for url in urls.lines().filter(|line| !line.trim().is_empty()) {
                let _ = Command::new("gh")
                    .args(["run", "view", url])
                    .current_dir(&repo_root)
                    .status();
            }
        }
    }

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("ci checks failed"))
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
fn launch_skill_agent(repo_root: &Path, skill_name: &str, context: Option<&str>) -> Result<()> {
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
                diff_files: Some(true),
                ..Default::default()
            },
            ..LaunchPromptInput::default()
        },
    )?;

    let process = ProcessConfig {
        auto: true,
        stream: true,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    let result = launch_agent(&prepared.config, &process, &capabilities)?;
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
