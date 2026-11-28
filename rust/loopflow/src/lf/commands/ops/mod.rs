use crate::lf::{PrCommand, ReleaseCommand, WtCommand};
use anyhow::{anyhow, Result};
use loopflow_execution::engine::Diagnostics;
use loopflow_execution::git::current_branch;
use loopflow_execution::git::worktree_name::WorktreeName;
use loopflow_execution::git::worktrees::{
    diff_shortstats, list_worktrees, list_worktrees_timed, main_repo_root, prune_worktrees,
    sibling_worktree_name, sibling_worktree_name_with_main, PullRequestState, WorktreeOwnership,
    WorktreePruneReport,
};
use loopflow_execution::naming::git_user;
use loopflow_execution::ops::branch_prune::{
    prune_branches, BranchCopy, EntryStatus as BranchPruneStatus, TaskBranches,
};
use loopflow_execution::ops::{
    current_pr, preview_release_notes, release_bump, release_check, release_issues, release_notes,
    release_promote, release_publish, release_regress, release_status, release_tag, Progress,
};
use loopflow_execution::output::{column_width, Colors};
use loopflow_execution::repo::find_repo_root;
use loopflow_execution::skills::SkillSyncOptions;
use loopflow_execution::skills::{
    installed_skill_directories, installed_skill_registries, sync_skills,
};
use loopflow_execution::work::wave::context::{
    planning_wave, resolve_managed_wave_sync, WaveResolveError,
};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

mod review;

pub fn run_pr(cmd: Option<&PrCommand>, cli_agent: Option<&str>) -> Result<()> {
    use loopflow_execution::engine::pr as engine;
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
            loopflow_execution::ops::present_pr_review(&published.url).map_err(|err| {
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
fn print_published(
    published: &loopflow_execution::engine::pr::Published,
    json: bool,
) -> Result<()> {
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

pub fn run_sync(
    input: &loopflow_execution::engine::checkout::SyncInput<'_>,
    json: bool,
) -> Result<()> {
    let repo_root =
        loopflow_execution::repo::require_repo_root(&std::env::current_dir()?, "lf sync")?;
    let outcome =
        loopflow_execution::engine::checkout::carry_sync(&repo_root, input, &CliProgress)?;
    if json {
        println!("{}", serde_json::to_string(&outcome)?);
    } else {
        println!("{}", outcome.render());
    }
    Ok(())
}

/// `lf sync` in `repo_root`. A sync that moves the branch first announces
/// itself to the launcher that started it detached, if one did.
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
        let directories = installed_skill_directories(false)
            .into_iter()
            .chain(installed_skill_registries())
            .map(|directory| format!("~/{directory}"))
            .collect::<Vec<_>>()
            .join(", ");
        if !std::io::stdin().is_terminal() {
            return Err(anyhow!(
                "skill sync writes under {directories}; rerun with --yes to confirm"
            ));
        }
        let progress = CliProgress;
        if !progress.confirm(&format!(
            "Write loopflow-generated skills under {directories}?"
        )) {
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
    let committed = loopflow_execution::engine::checkout::commit(
        &repo_root,
        message,
        push,
        no_add,
        paths,
        agent_override,
        &Diagnostics,
    )?;
    if json {
        println!("{}", serde_json::to_string(&committed)?);
    }
    Ok(())
}

/// `lf commit --step <skill>`: everything a finished skill step left, as one commit.
pub fn run_step_commit(step: &str, agent_override: Option<&str>) -> Result<()> {
    let repo_root = find_repo_root()?;
    loopflow_execution::ops::commit_step(&repo_root, step, agent_override, &Diagnostics)?;
    Ok(())
}

pub fn connect_wave(
    repo_root: &std::path::Path,
    wave: Option<&str>,
    project: Option<&str>,
    all: bool,
    team_key: Option<&str>,
    team_name: Option<&str>,
    json: bool,
) -> Result<()> {
    let results = loopflow_execution::engine::wave::planning_connect(
        repo_root,
        wave,
        project,
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
        let project = match (&result.project_id, result.created) {
            (Some(id), false) => format!("Linear Project `{}` ({id}) linked", result.project_name),
            (Some(id), true) => format!("Linear Project `{}` ({id}) made", result.project_name),
            (None, _) => format!(
                "Linear Project `{}` is not made yet; `{}` makes it",
                result.project_name,
                loopflow_execution::ops::tracker::make_project_command(&result.wave)
            ),
        };
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
        println!("{}: {project}{team_state}", result.wave);
        if let loopflow_execution::ops::wave_lifecycle::AuthoredFiles::Uncommitted { reason } =
            &result.files
        {
            println!(".lf/config.yaml is not committed on the main checkout: {reason}");
        }
    }
    Ok(())
}

pub fn sync_planning(
    repo: &std::path::Path,
    wave: Option<&str>,
    all: bool,
    json: bool,
) -> Result<()> {
    let wave = if all {
        None
    } else {
        planning_wave(repo, wave)?
    };
    let result = loopflow_execution::ops::linear::refresh(
        repo,
        &loopflow_execution::ops::linear::LinearSyncOptions { wave },
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

/// Refresh the selected Wave, or exchange the repository plan by its carriers.
pub fn refresh_status(directory: &Path, wave: Option<&str>) -> Result<Option<String>> {
    let repo = crate::lf::commands::tracker::status_repository(directory)?;
    let Some(tracker) = loopflow_execution::ops::tracker::queued_tracker(&repo) else {
        crate::lf::commands::tracker::refresh(&repo)?;
        return Ok(None);
    };
    let wave = resolve_managed_wave_sync(Some(&repo), wave).map_err(|error| match error {
        WaveResolveError::NoContext => anyhow!("Select a Wave to refresh"),
        error => error.into(),
    })?;
    loopflow_execution::runtime::block_on(loopflow_execution::ops::planning_sync::refresh_wave(
        &wave, tracker,
    ))?;
    Ok(Some(wave.slug().to_owned()))
}

pub fn reteam(repo: &std::path::Path, apply: bool, json: bool) -> Result<()> {
    let result = loopflow_execution::engine::wave::planning_reteam(repo, apply, &CliProgress)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        print_pm_reteam_result(&result);
    }
    Ok(())
}

fn print_pm_reteam_result(result: &loopflow_execution::ops::linear::LinearReteamResult) {
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

fn print_pm_sync_result(result: &loopflow_execution::ops::linear::LinearSyncResult) {
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
        return Err(loopflow_execution::process::CommandExit(1).into());
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
    let outcome = loopflow_execution::engine::repo::carry_release(
        &repo_root,
        version_input,
        target_name,
        progress,
    )?;
    if json {
        println!("{}", serde_json::to_string(&outcome)?);
        return Ok(());
    }
    match outcome {
        loopflow_execution::ops::ReleaseRunOutcome::NoChanges {
            target, latest_tag, ..
        } => {
            let latest = latest_tag.as_deref().unwrap_or("(none)");
            println!("No release ({target}): no merged PRs since {latest}");
        }
        loopflow_execution::ops::ReleaseRunOutcome::Released(receipt) => {
            print_release_receipt("Released", &receipt);
        }
        loopflow_execution::ops::ReleaseRunOutcome::Resumed(receipt) => {
            print_release_receipt("Resumed", &receipt);
        }
    }
    Ok(())
}

fn print_release_receipt(action: &str, receipt: &loopflow_execution::ops::ReleaseReceipt) {
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
        Some(status) => println!("Release notes: {status}"),
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
            stack,
            json,
        } => {
            let repo_root = find_repo_root()?;
            let checkout = loopflow_execution::engine::checkout::create(
                &repo_root,
                name,
                *plan,
                *persistent,
                *stack,
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
    checkout: &loopflow_execution::engine::checkout::Checkout,
) -> Result<()> {
    use loopflow_execution::git::worktrees::PlacementStrategy;
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
/// worktree. Single source of truth: `loopflow_execution::ops::telemetry`.
use loopflow_execution::ops::telemetry::record_ops_metric;

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
    let report =
        loopflow_execution::ops::wt_timing::report(&loopflow_execution::store::lf_home_dir())?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    if report.groups.is_empty() {
        println!("No `lf wt list` invocations recorded in {}", report.path);
        return Ok(());
    }
    let seconds = |ms: u64| format!("{:.2}s", ms as f64 / 1000.0);
    let phase = |spread: Option<loopflow_execution::ops::wt_timing::Spread>| {
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
            "  median / p95:  startup {}   local git {}   remote {}",
            phase(Some(group.startup_ms)),
            phase(group.local_git_ms),
            phase(group.remote_ms)
        );
        println!(
            "  failed {}   interrupted {}   remote timed out {}   remote unavailable {}",
            group.failed, group.interrupted, group.remote_timed_out, group.remote_unavailable
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
    loopflow_execution::ops::wt_timing::begin(&main_repo, json, sync);
    // `wt list` is an inspection surface and stays side-effect free by default:
    // merge/fresh flags reflect the last-synced main. `--sync` is the explicit,
    // self-owned mutation that fetches origin and integrates main first — a
    // read never fetches, resets, or stashes the canonical checkout behind the
    // user's back.
    if sync {
        loopflow_execution::ops::checkout::refresh_main(
            &main_repo,
            &loopflow_execution::ops::NullProgress,
        )?;
    }
    let listing = list_worktrees_timed(&main_repo)?;
    loopflow_execution::ops::wt_timing::listed(&listing);
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
    loopflow_execution::ops::wt::delete_worktree(&find_repo_root()?, name, force, &CliProgress)?;
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
    let report = loopflow_execution::git::build_cleanup::prune_builds(
        &main,
        &repo,
        &registries,
        loopflow_execution::git::build_cleanup::BuildCleanupPolicy {
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
    let selected = loopflow_execution::store::default_db_path();
    let production = loopflow_execution::store::production_database_path();
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
        loopflow_execution::ops::checkout::refresh_main(main_repo, &CliProgress)?;
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
    let with_saved = |target: &loopflow_execution::git::worktrees::WorktreePruneTarget| {
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
        process_paths: loopflow_execution::git::build_cleanup::running_paths()?,
        ..WorktreeOwnership::default()
    };
    let mut named = Vec::new();
    match loopflow_execution::runtime::block_on(loopflow_execution::store::open_existing_store()) {
        Ok(Some(store)) => {
            let tasks = loopflow_execution::runtime::block_on(store.list_tasks(None))
                .map_err(|error| anyhow!("cannot verify Task ownership before pruning: {error}"))?;
            let local = loopflow_execution::store::machines::local_machine(&store.sqlite)?.id;
            for task in tasks {
                let work = loopflow_execution::runtime::block_on(
                    store.work_for_child(&loopflow_storage::child::ChildRef::Task(task.id.clone())),
                )
                .map_err(|error| anyhow!("cannot resolve Task Work: {error}"))?;
                let status = loopflow_execution::runtime::block_on(store.work_status(&work))
                    .map_err(|error| anyhow!("cannot read Task Work status: {error}"))?;
                let open = !matches!(
                    status,
                    loopflow_model::durable::WorkStatus::Done
                        | loopflow_model::durable::WorkStatus::Abandoned
                );
                let prs = loopflow_execution::runtime::block_on(store.task_prs(&task.id))
                    .map_err(|error| anyhow!("cannot read Task PRs: {error}"))?;
                named.push((task.branch, open));
                named.extend(prs.into_iter().map(|pr| (pr.branch, open)));
                // A move pushed the branch before handing the Task off.
                let elsewhere = if open {
                    let owned = loopflow_execution::store::ownership::task_ownership(
                        &store.sqlite,
                        &task.id,
                    )?;
                    owned.owner_elsewhere(&local).map(|machine| machine.name())
                } else {
                    None
                };
                if let Some(worktree) = task.worktree {
                    if let Some(machine) = elsewhere {
                        ownership
                            .departed_checkouts
                            .insert(worktree, (task.identifier, machine));
                    } else if open {
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
    let production = loopflow_execution::store::production_database_path();
    if production.exists() {
        let unreadable = |error: loopflow_execution::store::StoreError| {
            anyhow!(
                "cannot verify Task ownership from {} before pruning: {error}",
                production.display()
            )
        };
        for path in loopflow_execution::store::read_nonterminal_task_worktrees(&production)
            .map_err(unreadable)?
        {
            ownership.finished_checkouts.remove(&path);
            ownership.unfinished_checkouts.entry(path).or_insert(None);
        }
        named.extend(
            loopflow_execution::store::read_task_branches(&production).map_err(unreadable)?,
        );
    }
    let TaskBranches { live, finished } = named.into_iter().collect();
    ownership.live_branches = live;
    ownership.finished_branches = finished;
    Ok(ownership)
}

fn pr_checks(repo_root: &Path, watch: bool, logs: bool, required: bool, json: bool) -> Result<()> {
    use loopflow_execution::engine::pr::ChecksState;
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
            Err(loopflow_execution::process::CommandExit(status.code().unwrap_or(1) as u8).into())
        };
    }
    let checks = loopflow_execution::engine::pr::checks(repo_root, required)
        .map_err(loopflow_execution::engine::pr::exit_of)?;
    if json {
        print_json(&checks)?;
    } else {
        print!("{}", checks.report);
    }
    match checks.state {
        ChecksState::Passing => Ok(()),
        ChecksState::Pending => {
            eprintln!("CI checks are pending.");
            Err(loopflow_execution::process::CommandExit(checks.exit_code()).into())
        }
        ChecksState::Failing => {
            if logs {
                print_failed_check_logs(repo_root, &checks.branch)?;
            }
            Err(loopflow_execution::process::CommandExit(checks.exit_code()).into())
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
