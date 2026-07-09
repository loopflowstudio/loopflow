use std::path::Path;

use clap::Parser;

use crate::engine::flow::Op;
use crate::engine::git::{get_default_branch, sync_main};
use crate::engine::{sync_skills, SkillSyncOptions};
use crate::lf::{Cli, Commands, PrCommand, ReleaseCommand};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, land, next_branch, rebase_with_recovery,
    release_bump, release_check, release_notes, release_run, release_status, release_tag, submit,
    AbandonOptions, CommitOptions, LandOptions, NextOptions, PrOptions, RebaseOptions,
};

pub fn execute_flow_ops(repo: &Path, item: &Op, progress: &impl Progress) -> OpsResult<()> {
    let mut argv = vec!["lf".to_string(), item.command.clone()];
    argv.extend(item.args.iter().cloned());

    let cli = Cli::try_parse_from(argv)
        .map_err(|err| OpsError::Message(format!("invalid op item: {err}")))?;

    match cli.command {
        Some(Commands::Pr { cmd: Some(pr) }) => execute_pr(repo, pr, progress),
        Some(Commands::Rebase { plan, onto }) => {
            if plan {
                return Ok(());
            }
            let base = get_default_branch(repo)?;
            let onto_ref = onto.unwrap_or_else(|| format!("origin/{base}"));
            rebase_with_recovery(
                repo,
                &RebaseOptions {
                    onto: onto_ref,
                    push: true,
                },
                progress,
            )?;
            Ok(())
        }
        Some(Commands::Sync) => {
            let base = get_default_branch(repo)?;
            if !sync_main(repo, &base)? {
                return Err(OpsError::Message(
                    "working tree dirty; sync aborted".to_string(),
                ));
            }
            Ok(())
        }
        Some(Commands::SyncSkills { yes: _, no_prune }) => {
            sync_skills(&SkillSyncOptions {
                prune: !no_prune,
                global_home: None,
            })?;
            Ok(())
        }
        Some(Commands::Advance { wave }) => {
            let wave = crate::ops::util::resolve_wave_name(repo, wave.as_deref())
                .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
            let branch = crate::ops::advance_branch(repo, &wave)?;
            progress.status(&format!("Advanced to branch: {branch}"));
            Ok(())
        }
        Some(Commands::Next {
            create_pr,
            no_rebase,
        }) => {
            next_branch(
                repo,
                &NextOptions {
                    create_pr,
                    rebase: !no_rebase,
                    wave_name: None,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        Some(Commands::Commit {
            message,
            push,
            no_add,
        }) => {
            commit_workflow(
                repo,
                &CommitOptions {
                    add: !no_add,
                    push,
                    create_draft_pr: true,
                    message,
                    ..CommitOptions::for_task("commit")
                },
                progress,
            )?;
            Ok(())
        }
        Some(Commands::Release { cmd }) => execute_release(repo, cmd, progress),
        _ => Err(unsupported()),
    }
}

fn execute_pr(repo: &Path, cmd: PrCommand, progress: &impl Progress) -> OpsResult<()> {
    match cmd {
        PrCommand::Land {
            strict,
            local,
            create_pr,
            worktree,
            message,
            title,
            body,
        } => {
            land(
                repo,
                &LandOptions {
                    strict,
                    local,
                    create_pr,
                    worktree,
                    commit_message: message,
                    pr_title: title,
                    pr_body: body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        PrCommand::Submit {
            strict,
            create_pr,
            worktree,
            message,
            title,
            body,
        } => {
            submit(
                repo,
                &LandOptions {
                    strict,
                    local: false,
                    create_pr,
                    worktree,
                    commit_message: message,
                    pr_title: title,
                    pr_body: body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        PrCommand::Open {
            model: _,
            title,
            body,
        } => {
            create_or_update_pr(
                repo,
                &PrOptions {
                    title,
                    body,
                    agent: None,
                },
                progress,
            )?;
            Ok(())
        }
        PrCommand::Abandon { force, branch } => {
            abandon_branch(repo, &AbandonOptions { branch, force }, progress)?;
            Ok(())
        }
        PrCommand::Status => Err(unsupported()),
    }
}

fn execute_release(repo: &Path, cmd: ReleaseCommand, progress: &impl Progress) -> OpsResult<()> {
    match cmd {
        ReleaseCommand::Run { version, target } => {
            release_run(
                repo,
                version.as_deref().unwrap_or("patch"),
                target.as_deref(),
                progress,
            )?;
            Ok(())
        }
        ReleaseCommand::Check { target } => {
            release_check(repo, target.as_deref())?;
            Ok(())
        }
        ReleaseCommand::Notes {
            version,
            prev_tag,
            target,
        } => {
            release_notes(
                repo,
                &version,
                prev_tag.as_deref(),
                target.as_deref(),
                progress,
            )?;
            Ok(())
        }
        ReleaseCommand::Bump { version, target } => {
            release_bump(repo, &version, target.as_deref(), progress)
        }
        ReleaseCommand::Tag { version, target } => {
            release_tag(repo, &version, target.as_deref())?;
            Ok(())
        }
        ReleaseCommand::Status { target } => {
            release_status(repo, target.as_deref())?;
            Ok(())
        }
    }
}

/// Flow `op:` items drive the mechanical verbs only; anything that launches an
/// agent, reads interactively, or manages waves has no place in a flow step.
fn unsupported() -> OpsError {
    OpsError::Message(
        "op item must be one of pr open, pr submit, pr land, pr abandon, rebase, sync, \
         sync-skills, advance, next, commit, or release"
            .to_string(),
    )
}
