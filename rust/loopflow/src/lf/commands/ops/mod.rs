use crate::engine::agent::{launch_agent, AgentCapabilities, ProcessConfig};
use crate::engine::config::load_config_or_default;
use crate::engine::git::{current_branch, delete_local_branch, get_default_branch, sync_main};
use crate::engine::worktrees::{
    create_with_schema_synced, list_worktrees, main_repo_root, wave_name_from_worktree,
    wave_name_from_worktree_and_main, worktree_path,
};
use crate::engine::{prepare_launch_prompt, ContextSourceOverrides, LaunchPromptInput, Surface};
use crate::lf::commands::util::find_repo_root;
use crate::lf::discovery::discover_step;
use crate::lf::output::Colors;
use crate::lf::{OpsCommand, PmCommand, ReleaseCommand, ShellCommand, WtCommand};
use crate::ops::OpsError;
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, ingest, land, next_branch,
    rebase_with_recovery, release_bump, release_check, release_notes, release_run, release_status,
    release_tag, AbandonOptions, CommitOptions, IngestOptions, LandOptions, NextOptions, PrOptions,
    Progress, RebaseOptions, RotationResult,
};
use anyhow::{anyhow, Result};
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Command;

pub fn run(op: &OpsCommand) -> Result<()> {
    let progress = CliProgress;
    match op {
        OpsCommand::Cp {
            paths,
            exclude,
            lfdocs,
            no_lfdocs,
        } => copy_context(paths, exclude, *lfdocs, *no_lfdocs),
        OpsCommand::Doctor => doctor(),
        OpsCommand::Rebase { onto } => rebase_current(onto.as_deref(), &progress),
        OpsCommand::Push { force } => push_current(*force),
        OpsCommand::Land {
            strict,
            local,
            create_pr,
            worktree,
            message,
            title,
            body,
        } => land_current(
            &LandOptions {
                strict: *strict,
                local: *local,
                create_pr: *create_pr,
                worktree: worktree.clone(),
                commit_message: message.clone(),
                pr_title: title.clone(),
                pr_body: body.clone(),
            },
            &progress,
        ),
        OpsCommand::Pr { title, body } => open_pr(title.clone(), body.clone(), &progress),
        OpsCommand::Sync => sync_current(),
        OpsCommand::Next {
            create_pr,
            no_rebase,
        } => next_branch_cmd(*create_pr, !*no_rebase, &progress),
        OpsCommand::Commit {
            message,
            push,
            no_add,
        } => commit_current(message.as_deref(), *push, !no_add, &progress),
        OpsCommand::Abandon { force, branch } => {
            abandon_current(branch.as_deref(), *force, &progress)
        }
        OpsCommand::Wt { cmd } => run_worktree(cmd),
        OpsCommand::Shell { cmd } => run_shell(cmd),
        OpsCommand::Release { cmd } => match cmd {
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
        },
        OpsCommand::Ingest { wave } => ingest_cmd(wave.as_deref(), &progress),
        OpsCommand::Pm { cmd } => pm_cmd(cmd, &progress),
        OpsCommand::Auth { cmd } => crate::lf::commands::auth::run(cmd),
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

fn rebase_current(onto: Option<&str>, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let base = get_default_branch(&repo_root)?;
    let onto_ref = onto
        .map(|value| value.to_string())
        .unwrap_or_else(|| format!("origin/{base}"));
    match rebase_with_recovery(
        &repo_root,
        &RebaseOptions {
            onto: onto_ref.clone(),
            push: true,
        },
        progress,
    ) {
        Ok(()) => Ok(()),
        Err(OpsError::RebaseConflict { onto, detail }) => {
            let context = format!(
                "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\n</lf:rebase-conflict>"
            );
            progress.status("Launching rebase agent to resolve conflicts...");
            launch_step_agent(&repo_root, "rebase", Some(&context))
        }
        Err(err) => Err(err.into()),
    }
}

fn push_current(force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    crate::engine::git::push(&repo_root, force).map_err(Into::into)
}

fn land_current(options: &LandOptions, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let result = match land(&repo_root, options, progress) {
        Ok(result) => result,
        Err(OpsError::RebaseConflict { onto, detail }) => {
            let context = format!(
                "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\n</lf:rebase-conflict>"
            );
            progress.status("Launching rebase agent to resolve conflicts...");
            launch_step_agent(&repo_root, "rebase", Some(&context))?;
            progress.status("Retrying land after rebase...");
            land(&repo_root, options, progress)?
        }
        Err(err) => return Err(err.into()),
    };

    let cd_target = match &result.rotation {
        Some(RotationResult::Advanced { new_path, .. }) => Some(new_path.clone()),
        Some(RotationResult::Complete { .. }) => Some(main_repo),
        None => None,
    };
    if let Some(target) = cd_target {
        if !write_shell_directive(&format!("cd {}", target.display()))? {
            println!("cd {}", target.display());
        }
    }

    Ok(())
}

fn open_pr(title: Option<String>, body: Option<String>, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = match create_or_update_pr(
        &repo_root,
        &PrOptions {
            title: title.clone(),
            body: body.clone(),
        },
        progress,
    ) {
        Ok(result) => result,
        Err(OpsError::RebaseConflict { onto, detail }) => {
            let context = format!(
                "<lf:rebase-conflict>\nRebase onto: {onto}\n{detail}\n</lf:rebase-conflict>"
            );
            progress.status("Launching rebase agent to resolve conflicts...");
            launch_step_agent(&repo_root, "rebase", Some(&context))?;
            progress.status("Retrying PR creation after rebase...");
            create_or_update_pr(&repo_root, &PrOptions { title, body }, progress)?
        }
        Err(err) => return Err(err.into()),
    };
    println!("{}", result.url);
    Ok(())
}

fn sync_current() -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_branch = get_default_branch(&repo_root)?;
    let ok = crate::engine::git::sync_main(&repo_root, &main_branch)?;
    if !ok {
        return Err(anyhow!("working tree dirty; sync aborted"));
    }
    Ok(())
}

fn next_branch_cmd(create_pr: bool, rebase: bool, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = next_branch(
        &repo_root,
        &NextOptions {
            create_pr,
            rebase,
            wave_name: None,
        },
        progress,
    )?;
    println!("{}", result.new_branch);
    Ok(())
}

fn commit_current(
    message: Option<&str>,
    push: bool,
    add: bool,
    progress: &impl Progress,
) -> Result<()> {
    let repo_root = find_repo_root()?;
    let _ = commit_workflow(
        &repo_root,
        &CommitOptions {
            add,
            push,
            create_draft_pr: true,
            message: message.map(str::to_string),
            ..CommitOptions::for_task("commit")
        },
        progress,
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

fn ingest_cmd(wave: Option<&str>, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let result = ingest(
        &repo_root,
        &IngestOptions {
            wave: wave.map(str::to_string),
        },
        progress,
    )?;
    println!("{}", result.dest.display());
    Ok(())
}

fn pm_cmd(cmd: &PmCommand, progress: &impl Progress) -> Result<()> {
    let repo_root = find_repo_root()?;
    let list_all_waves = || -> Result<Vec<String>> {
        // List all wave directories
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
    let list_pm_waves = || -> Result<Vec<String>> {
        let waves = list_all_waves()?
            .into_iter()
            .filter(|wave| crate::ops::pm::wave_pm_is_enabled(&repo_root, wave))
            .collect::<Vec<_>>();
        if waves.is_empty() {
            return Err(anyhow!("no PM-enabled waves found in wave/"));
        }
        Ok(waves)
    };

    match cmd {
        PmCommand::Init {
            wave,
            wave_flag,
            all,
        } => {
            if *all {
                for wave in list_all_waves()? {
                    let result = crate::ops::pm::pm_init(
                        &repo_root,
                        &crate::ops::pm::PmInitOptions { wave: Some(wave) },
                        progress,
                    )?;
                    println!(
                        "{}: {:?} project {} ({} linked, {} created)",
                        result.wave,
                        result.provider,
                        result.project_id,
                        result.linked,
                        result.created.len()
                    );
                }
            } else {
                let result = crate::ops::pm::pm_init(
                    &repo_root,
                    &crate::ops::pm::PmInitOptions {
                        wave: wave.clone().or_else(|| wave_flag.clone()),
                    },
                    progress,
                )?;
                println!(
                    "{}: {:?} project {} ({} linked, {} created)",
                    result.wave,
                    result.provider,
                    result.project_id,
                    result.linked,
                    result.created.len()
                );
            }
        }
        PmCommand::Import { team_id } => {
            let result = crate::ops::pm::pm_import(
                &repo_root,
                &crate::ops::pm::PmImportOptions {
                    team_id: team_id.clone(),
                },
                progress,
            )?;
            if result.waves_created.is_empty() {
                println!("no projects found");
            } else {
                println!(
                    "imported {} waves ({} items)",
                    result.waves_created.len(),
                    result.items_created
                );
                for wave in &result.waves_created {
                    println!("  wave/{wave}");
                }
            }
        }
        PmCommand::Sync { wave } => {
            let waves = if let Some(name) = wave.as_ref() {
                vec![name.clone()]
            } else {
                list_all_waves()?
            };
            for wave in &waves {
                let result = crate::ops::pm::pm_sync(
                    &repo_root,
                    &crate::ops::pm::PmSyncOptions { wave: wave.clone() },
                    progress,
                )?;
                let total = result.pushed.len() + result.pulled.len();
                if total == 0 && result.conflicts.is_empty() {
                    println!("{wave}: in sync");
                } else {
                    println!(
                        "{wave}: synced {} items ({} pushed, {} pulled, {} conflicts)",
                        total,
                        result.pushed.len(),
                        result.pulled.len(),
                        result.conflicts.len()
                    );
                }
            }
        }
        PmCommand::Pull {
            wave,
            wave_flag,
            all,
        } => {
            let waves = if *all {
                list_pm_waves()?
            } else {
                vec![wave
                    .clone()
                    .or_else(|| wave_flag.clone())
                    .or_else(|| crate::ops::util::resolve_wave_name(&repo_root, None))
                    .ok_or_else(|| anyhow!("cannot determine wave name"))?]
            };
            for wave in waves {
                let result = crate::ops::pm::pm_pull(
                    &repo_root,
                    &crate::ops::pm::PmPullOptions { wave: wave.clone() },
                    progress,
                )?;
                println!(
                    "{}: {:?} project {} ({} files written, {} removed)",
                    result.wave,
                    result.provider,
                    result.project_id,
                    result.local_written,
                    result.local_removed
                );
            }
        }
        PmCommand::Export {
            wave,
            wave_flag,
            all,
        } => {
            let waves = if *all {
                list_pm_waves()?
            } else {
                vec![wave
                    .clone()
                    .or_else(|| wave_flag.clone())
                    .or_else(|| crate::ops::util::resolve_wave_name(&repo_root, None))
                    .ok_or_else(|| anyhow!("cannot determine wave name"))?]
            };
            for wave in waves {
                let result = crate::ops::pm::pm_export(
                    &repo_root,
                    &crate::ops::pm::PmExportOptions { wave: wave.clone() },
                    progress,
                )?;
                println!(
                    "{}: {:?} project {} ({} created, {} updated, {} skipped)",
                    result.wave,
                    result.provider,
                    result.project_id,
                    result.created,
                    result.updated,
                    result.skipped
                );
            }
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
                    let s = &wave.status;
                    println!(
                        "{}: {:?} project {} — local {}, linked {}, remote {}, remote-only {}",
                        wave.wave,
                        s.provider,
                        s.project_id,
                        s.local_total,
                        s.linked,
                        s.remote_total,
                        s.remote_only
                    );
                }
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

fn run_worktree(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create { name, base, stack } => wt_create(name, base.as_deref(), *stack),
        WtCommand::Switch { name } => wt_switch(name),
        WtCommand::List { format, .. } => wt_list(format.as_deref()),
        WtCommand::Remove { name, force } => wt_remove(name, *force),
        WtCommand::Prune {
            dry_run,
            include_fresh,
        } => wt_prune(*dry_run, *include_fresh),
        WtCommand::Ci { watch, logs } => wt_ci(*watch, *logs),
    }
}

fn wt_create(name: &str, base: Option<&str>, stack: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root)?;

    let mut base_branch = base.map(str::to_string);
    if stack {
        let current = current_branch(&repo_root)?.ok_or_else(|| anyhow!("not on a branch"))?;
        if current == "main" || current == "master" {
            return Err(anyhow!("cannot stack on main/master"));
        }
        base_branch = Some(current);
    }

    let config = crate::engine::config::load_config(Some(&main_repo))
        .ok()
        .flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result =
        create_with_schema_synced(&main_repo, name, base_branch.as_deref(), branch_config)?;

    println!("Created worktree: {}", result.path.display());
    if result.branch != name {
        println!("Branch: {}", result.branch);
    }
    if let Some(base_branch) = result.base_branch {
        println!("Base: {}", base_branch);
    }

    if !write_shell_directive(&format!("cd {}", result.path.display()))? {
        println!("cd {}", result.path.display());
        println!("Tip: Run 'lf op shell install' for auto-cd");
    }

    Ok(())
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
        let target = worktree_path(&main_repo, name);
        if target.exists() {
            target
        } else {
            let mut matches = worktrees
                .into_iter()
                .filter(|wt| {
                    let wt_name = wave_name_from_worktree_and_main(&wt.path, &main_repo);
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

    if !write_shell_directive(&format!("cd {}", path.display()))? {
        println!("cd {}", path.display());
    }
    Ok(())
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
    // Collect display info for all worktrees
    struct Row {
        name: String,
        is_current: bool,
        is_main: bool,
        merged: bool,
        squash_merged: bool,
        fresh: bool,
        dirty: bool,
        remote_gone: bool,
        diff_stat: String,
    }

    let rows: Vec<Row> = worktrees
        .iter()
        .map(|wt| {
            let is_main = wt.branch.as_deref() == Some(&default_branch);
            let name = if is_main {
                default_branch.clone()
            } else {
                wave_name_from_worktree(&wt.path).unwrap_or_else(|| {
                    wt.path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "?".to_string())
                })
            };
            let is_current = wt.path == repo_root;
            let diff_stat = if is_main {
                String::new()
            } else {
                wt_diff_stat(&main_repo, wt.branch.as_deref(), &default_branch)
            };
            Row {
                name,
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

    let max_name = rows.iter().map(|r| r.name.len()).max().unwrap_or(0);

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
            row.name,
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

fn run_shell(cmd: &ShellCommand) -> Result<()> {
    match cmd {
        ShellCommand::Init { shell } => shell_init(shell.as_deref()),
        ShellCommand::Install { shell } => shell_install(shell.as_deref()),
        ShellCommand::Directive { command } => shell_directive(command),
    }
}

fn shell_init(shell: Option<&str>) -> Result<()> {
    let shell = shell.unwrap_or("zsh");
    let init = match shell {
        "zsh" => SHELL_INIT_ZSH,
        "bash" => SHELL_INIT_BASH,
        _ => return Err(anyhow!("unsupported shell: {}", shell)),
    };
    println!("{}", init);
    Ok(())
}

fn shell_install(shell: Option<&str>) -> Result<()> {
    let shell = shell
        .map(|value| value.to_string())
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "zsh".to_string());
    let shell_name = if shell.contains("bash") {
        "bash"
    } else {
        "zsh"
    };
    let (config_path, install_line) = match shell_name {
        "bash" => (
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".bashrc"),
            SHELL_INSTALL_LINE_BASH,
        ),
        _ => (
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".zshrc"),
            SHELL_INSTALL_LINE_ZSH,
        ),
    };

    if let Ok(content) = std::fs::read_to_string(&config_path) {
        if content.contains("lf op shell init") {
            println!("Already installed in {}", config_path.display());
            return Ok(());
        }
    }

    std::fs::create_dir_all(
        config_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(".")),
    )?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config_path)?;
    use std::io::Write;
    writeln!(file, "\n{}", install_line)?;
    println!("Installed to {}", config_path.display());
    println!(
        "Restart your shell or run: source {}",
        config_path.display()
    );
    Ok(())
}

fn shell_directive(command: &[String]) -> Result<()> {
    if command.is_empty() {
        return Err(anyhow!("command required"));
    }
    let line = command.join(" ");
    if !write_shell_directive(&line)? {
        println!("{}", line);
    }
    Ok(())
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

const SHELL_INIT_ZSH: &str = r#"# loopflow shell integration for zsh
#
# Enables directory switching after commands that emit shell directives
# (for example `lf op wt create`, `lf op wt switch`, `lf op land`).

if command -v lf >/dev/null 2>&1; then
    lf() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"#;

const SHELL_INIT_BASH: &str = r#"# loopflow shell integration for bash
#
# Enables directory switching after commands that emit shell directives
# (for example `lf op wt create`, `lf op wt switch`, `lf op land`).

if command -v lf >/dev/null 2>&1; then
    lf() {
        local directive_file exit_code=0
        directive_file="$(mktemp)"

        LOOPFLOW_DIRECTIVE_FILE="$directive_file" command lf "$@" || exit_code=$?

        if [[ -s "$directive_file" ]]; then
            source "$directive_file"
            if [[ $exit_code -eq 0 ]]; then
                exit_code=$?
            fi
        fi

        rm -f "$directive_file"
        return "$exit_code"
    }
fi
"#;

const SHELL_INSTALL_LINE_ZSH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf op shell init zsh)\"; fi";
const SHELL_INSTALL_LINE_BASH: &str =
    "if command -v lf >/dev/null 2>&1; then eval \"$(command lf op shell init bash)\"; fi";

// ==========================================================================
// Step agent fallback
// ==========================================================================

/// Launch an agent with a named step when an ops command needs judgment.
///
/// Used when mechanical operations hit a situation that requires agent
/// reasoning — e.g., rebase conflicts that need conflict resolution.
fn launch_step_agent(repo_root: &Path, step_name: &str, context: Option<&str>) -> Result<()> {
    let step = discover_step(repo_root, step_name)?;
    let config = load_config_or_default(Some(repo_root));

    let message = context.map(|value| value.to_string());
    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.to_path_buf(),
            step: Some(step_name.to_string()),
            resolved_step: Some(step),
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
            step_name,
        ));
    }
    Ok(())
}

// ==========================================================================
// lf op cp
// ==========================================================================

fn copy_context(paths: &[String], exclude: &[String], lfdocs: bool, no_lfdocs: bool) -> Result<()> {
    use crate::engine::prompt::{
        count_tokens, default_gather_sources, gather_context, Document, GatherContextOpts,
    };
    use std::collections::HashSet;

    let repo_root = find_repo_root()?;

    // When paths are given, skip lfdocs unless --lfdocs is explicit.
    // When no paths, include lfdocs by default unless --no-lfdocs.
    let has_paths = !paths.is_empty();
    let include_lfdocs = if has_paths { lfdocs } else { !no_lfdocs };

    // Gather context
    let opts = GatherContextOpts {
        repo_root: repo_root.clone(),
        files: paths.to_vec(),
        sources: default_gather_sources(
            include_lfdocs,
            !has_paths, // Use diff files if no paths specified
            false,
        ),
        ..Default::default()
    };

    let components = gather_context(&opts)?.into_components();

    // Collect all documents to format
    let mut all_docs: Vec<Document> = Vec::new();
    all_docs.extend(components.diff_files);
    all_docs.extend(components.docs);

    // Apply exclusion patterns
    if !exclude.is_empty() {
        let exclude_set: HashSet<&str> = exclude.iter().map(|s| s.as_str()).collect();
        all_docs.retain(|doc| !exclude_set.iter().any(|pattern| doc.path.contains(pattern)));
    }

    if all_docs.is_empty() {
        println!("No files to copy.");
        return Ok(());
    }

    // Format files as raw content (similar to Python's format_files_raw)
    let mut output = String::new();
    for doc in &all_docs {
        output.push_str(&format!("=== {} ===\n", doc.path));
        output.push_str(&doc.content);
        if !doc.content.ends_with('\n') {
            output.push('\n');
        }
        output.push('\n');
    }

    // Copy to clipboard
    copy_to_clipboard(&output)?;

    // Display token tree
    let mut total_tokens = 0;
    for doc in &all_docs {
        let tokens = count_tokens(&doc.content);
        total_tokens += tokens;
        println!("{:>6} tokens  {}", tokens, doc.path);
    }
    println!("─────────────");
    println!("{:>6} tokens  total", total_tokens);
    println!("\nCopied to clipboard.");

    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    crate::engine::clipboard::write(text)?;
    Ok(())
}

// ==========================================================================
// lf op doctor
// ==========================================================================

fn doctor() -> Result<()> {
    use std::path::Path;

    let repo_root = find_repo_root().ok();

    // Repo status
    if let Some(ref root) = repo_root {
        let lf_dir = root.join(".lf");
        if lf_dir.join("steps").is_dir() || lf_dir.join("flows").is_dir() {
            println!("✓ task files found");
        } else {
            println!("- no task files (run: lf init)");
        }
    } else {
        println!("- not in a git repo");
    }

    let is_macos = cfg!(target_os = "macos");

    // Optional: npm
    if which("npm") {
        println!("✓ npm");
    } else if is_macos {
        println!("- npm: brew install node");
    } else {
        println!("- npm: https://nodejs.org/");
    }

    // Optional: coding agents
    if check_claude_available() {
        println!("✓ claude");
    } else {
        println!("- claude: lf init");
    }

    if check_codex_available() {
        println!("✓ codex");
    } else {
        println!("- codex: npm install -g @openai/codex");
    }

    if check_gemini_available() {
        println!("✓ gemini");
    } else {
        println!("- gemini: npm install -g @google/gemini-cli");
    }

    // Optional: IDE/terminals (macOS-only apps)
    if is_macos {
        if which("warp") {
            println!("✓ warp");
        } else {
            println!("- warp: brew install --cask warp");
        }

        if which("cursor") {
            println!("✓ cursor");
        } else {
            println!("- cursor: brew install --cask cursor");
        }
    }

    // Optional: superpowers
    let superpowers_path = dirs::home_dir()
        .map(|h| h.join(".superpowers"))
        .unwrap_or_else(|| Path::new("~/.superpowers").to_path_buf());
    if superpowers_path.exists() {
        println!("✓ superpowers");
    } else {
        println!("- superpowers: git clone https://github.com/obra/superpowers ~/.superpowers");
    }

    // Optional: gh for PR creation
    if which("gh") {
        println!("✓ gh");
    } else if is_macos {
        println!("- gh: brew install gh");
    } else {
        println!("- gh: https://cli.github.com/");
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

fn check_claude_available() -> bool {
    // Check for claude CLI
    which("claude")
}

fn check_codex_available() -> bool {
    // Check for codex CLI
    which("codex")
}

fn check_gemini_available() -> bool {
    // Check for gemini CLI
    which("gemini")
}
