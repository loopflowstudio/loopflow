use std::path::Path;

use clap::Parser;

use crate::engine::flow::Op;
use crate::engine::git::{get_default_branch, sync_main};
use crate::lf::{Cli, Commands, OpsCommand, ReleaseCommand};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, ingest, land, next_branch,
    rebase_with_recovery, release_bump, release_check, release_notes, release_run, release_status,
    release_tag, AbandonOptions, CommitOptions, IngestOptions, LandOptions, NextOptions, PrOptions,
    RebaseOptions,
};

/// Resolve which PM-enabled waves to operate on within a flow context.
/// If no wave is specified and no --all flag, auto-detect from the repo.
fn resolve_pm_waves_for_flow(
    repo: &Path,
    wave: &Option<String>,
    wave_flag: &Option<String>,
    all: bool,
) -> OpsResult<Vec<String>> {
    if all {
        return crate::ops::pm::list_pm_waves(repo);
    }
    let name = wave
        .clone()
        .or_else(|| wave_flag.clone())
        .or_else(|| crate::ops::util::resolve_wave_name(repo, None))
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    Ok(vec![name])
}

pub fn execute_flow_ops(repo: &Path, item: &Op, progress: &impl Progress) -> OpsResult<()> {
    let mut argv = vec!["lf".to_string(), "op".to_string(), item.command.clone()];
    argv.extend(item.args.iter().cloned());

    let cli = Cli::try_parse_from(argv)
        .map_err(|err| OpsError::Message(format!("invalid op item: {err}")))?;
    let op = match cli.command {
        Some(Commands::Op { op }) => op,
        _ => {
            return Err(OpsError::Message(
                "op item must parse as `lf op <command>`".to_string(),
            ));
        }
    };

    execute_parsed_ops(repo, &op, progress)
}

fn execute_parsed_ops(repo: &Path, op: &OpsCommand, progress: &impl Progress) -> OpsResult<()> {
    match op {
        OpsCommand::Land {
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
                    strict: *strict,
                    local: *local,
                    create_pr: *create_pr,
                    worktree: worktree.clone(),
                    commit_message: message.clone(),
                    pr_title: title.clone(),
                    pr_body: body.clone(),
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Rebase { onto } => {
            let base = get_default_branch(repo)?;
            let onto_ref = onto
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("origin/{base}"));
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
        OpsCommand::Pr { title, body } => {
            create_or_update_pr(
                repo,
                &PrOptions {
                    title: title.clone(),
                    body: body.clone(),
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Sync => {
            let base = get_default_branch(repo)?;
            if !sync_main(repo, &base)? {
                return Err(OpsError::Message(
                    "working tree dirty; sync aborted".to_string(),
                ));
            }
            Ok(())
        }
        OpsCommand::Next {
            create_pr,
            no_rebase,
        } => {
            next_branch(
                repo,
                &NextOptions {
                    create_pr: *create_pr,
                    rebase: !*no_rebase,
                    wave_name: None,
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Commit {
            message,
            push,
            no_add,
        } => {
            commit_workflow(
                repo,
                &CommitOptions {
                    add: !*no_add,
                    push: *push,
                    create_draft_pr: true,
                    message: message.clone(),
                    ..CommitOptions::for_task("commit")
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Abandon { force, branch } => {
            abandon_branch(
                repo,
                &AbandonOptions {
                    branch: branch.clone(),
                    force: *force,
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Release { cmd } => match cmd {
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
                    version,
                    prev_tag.as_deref(),
                    target.as_deref(),
                    progress,
                )?;
                Ok(())
            }
            ReleaseCommand::Bump { version, target } => {
                release_bump(repo, version, target.as_deref(), progress)
            }
            ReleaseCommand::Tag { version, target } => {
                release_tag(repo, version, target.as_deref())?;
                Ok(())
            }
            ReleaseCommand::Status { target } => {
                release_status(repo, target.as_deref())?;
                Ok(())
            }
        },
        OpsCommand::Ingest { wave, item } => {
            ingest(
                repo,
                &IngestOptions {
                    wave: wave.clone(),
                    item: item.clone(),
                },
                progress,
            )?;
            Ok(())
        }
        OpsCommand::Push { force } => crate::engine::git::push(repo, *force).map_err(Into::into),
        OpsCommand::Pm { cmd } => {
            use crate::lf::PmCommand;
            match cmd {
                PmCommand::Pull {
                    wave,
                    wave_flag,
                    all,
                } => {
                    let waves = resolve_pm_waves_for_flow(repo, wave, wave_flag, *all)?;
                    for w in waves {
                        crate::ops::pm::pm_pull(
                            repo,
                            &crate::ops::pm::PmPullOptions { wave: w },
                            progress,
                        )?;
                    }
                    Ok(())
                }
                PmCommand::PushDiff {
                    wave,
                    wave_flag,
                    all,
                } => {
                    let waves = resolve_pm_waves_for_flow(repo, wave, wave_flag, *all)?;
                    for w in waves {
                        crate::ops::pm::pm_push_diff(
                            repo,
                            &crate::ops::pm::PmPushDiffOptions { wave: w },
                            progress,
                        )?;
                    }
                    Ok(())
                }
                _ => Err(OpsError::Message(
                    "only pm pull and pm push-diff are supported as flow ops".to_string(),
                )),
            }
        }
        OpsCommand::Cp { .. }
        | OpsCommand::Doctor
        | OpsCommand::Wt { .. }
        | OpsCommand::Shell { .. }
        | OpsCommand::Auth { .. } => Err(OpsError::Message(
            "ops item does not support cp/doctor/wt/shell/auth commands".to_string(),
        )),
    }
}
