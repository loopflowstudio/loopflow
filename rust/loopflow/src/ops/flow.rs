use std::path::Path;

use clap::Parser;

use crate::engine::flow::Op;
use crate::engine::git::get_default_branch;
use crate::lf::{Cli, Commands, PrCommand, ReleaseCommand};
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;
use crate::ops::{
    abandon_branch, commit_workflow, create_or_update_pr, land, rebase_with_recovery, release_bump,
    release_check, release_notes, release_publish, release_run, release_status, release_tag,
    submit, AbandonOptions, CommitOptions, LandOptions, PrOptions, RebaseOptions,
};

pub fn execute_flow_ops(repo: &Path, item: &Op, progress: &impl Progress) -> OpsResult<()> {
    let mut argv = vec!["lf".to_string(), item.command.clone()];
    argv.extend(item.args.iter().cloned());

    let cli = Cli::try_parse_from(argv)
        .map_err(|err| OpsError::Message(format!("invalid op item: {err}")))?;

    match cli.command {
        Some(Commands::Pr { cmd: Some(pr) }) => execute_pr(repo, pr, progress),
        Some(Commands::Rebase {
            plan,
            manual,
            continue_rebase,
            abort,
            adopt,
            onto,
        }) => {
            if manual || continue_rebase || abort || adopt {
                return Err(OpsError::Message(
                    "manual rebase recovery is only available from the CLI".to_string(),
                ));
            }
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
                    fork_base: None,
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
        Some(Commands::Doctor { json }) => crate::lf::commands::doctor::run(json)
            .map_err(|error| OpsError::Message(error.to_string())),
        _ => Err(unsupported()),
    }
}

fn execute_pr(repo: &Path, cmd: PrCommand, progress: &impl Progress) -> OpsResult<()> {
    match cmd {
        PrCommand::Land {
            strict,
            local,
            create_pr,
            complete,
            next,
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
                    complete,
                    next_slug: next,
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
            complete,
            next,
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
                    complete,
                    next_slug: next,
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
        // A flow `op:` runs headless, so both publish and open only publish —
        // presentation is a human-initiated CLI concern, never an automation step.
        PrCommand::Publish {
            model: _,
            title,
            body,
        }
        | PrCommand::Open {
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
        PrCommand::Next { slug } => {
            crate::ops::task::pr_next(repo, slug.as_deref())?;
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
        ReleaseCommand::Publish {
            tag,
            notes,
            assets,
            finalize,
        } => release_publish(repo, &tag, notes.as_deref(), &assets, finalize),
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
        "op item must be one of pr open, pr submit, pr land, pr abandon, rebase, commit, release, or doctor"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::NullProgress;

    #[test]
    fn authored_flow_cannot_dispatch_evidence_receipt_command() {
        let item = Op {
            command: "receipt".to_string(),
            args: vec!["show".to_string(), "chat_turn:turn-3".to_string()],
        };

        let error = execute_flow_ops(Path::new("."), &item, &NullProgress)
            .expect_err("removed evidence command must not dispatch");
        assert!(error.to_string().contains("op item must be one of"));
    }
}
