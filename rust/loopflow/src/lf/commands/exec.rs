use std::num::NonZeroU32;

use anyhow::Context;
use clap::Subcommand;

use crate::exec::{Exec, ExecCursor, ExecFilter, ExecOutcomeFilter, ExecWorkFilter};
use crate::repository::CanonicalRepo;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, Subcommand)]
pub enum ExecCommand {
    /// List a bounded page of recorded commands, newest first
    List {
        #[arg(long)]
        json: bool,
        /// Include all repositories
        #[arg(long)]
        all: bool,
        #[arg(long, default_value = "100")]
        limit: NonZeroU32,
        /// Continue with the previous page's next object, encoded as JSON
        #[arg(long, value_parser = parse_cursor)]
        after: Option<ExecCursor>,
        /// Direct children of an exact or unambiguous parent Exec
        #[arg(long)]
        parent: Option<String>,
        /// Commands issued by this AgentSession
        #[arg(long)]
        caller: Option<String>,
        /// Literal command text, ignoring ASCII case
        #[arg(long)]
        search: Option<String>,
        #[arg(long, value_parser = ["succeeded", "failed", "interrupted", "unknown"])]
        outcome: Option<String>,
        /// Recorded work for a Task, including completed Tasks
        #[arg(long, conflicts_with = "wave")]
        task: Option<String>,
        /// Recorded work for a Wave
        #[arg(long)]
        wave: Option<String>,
    },
    /// Show one command by exact ID or unambiguous prefix
    Show {
        id: String,
        #[arg(long)]
        json: bool,
    },
}

fn parse_cursor(value: &str) -> Result<ExecCursor, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

pub fn run(command: &ExecCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        match command {
            ExecCommand::Show { id, json } => {
                let exec = store
                    .resolve_exec(id)
                    .await?
                    .context("Exec was not found")?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&exec)?);
                } else {
                    print_exec(&exec);
                    println!("{}", serde_json::to_string_pretty(&exec)?);
                }
            }
            ExecCommand::List {
                json,
                all,
                limit,
                after,
                parent,
                caller,
                search,
                outcome,
                task,
                wave,
            } => {
                let repo = if *all {
                    None
                } else {
                    CanonicalRepo::current()?.map(|repo| repo.to_string())
                };
                let performed_work = match (task, wave) {
                    (Some(task), _) => Some(ExecWorkFilter::Task(
                        store
                            .sqlite
                            .resolve_task_id(task, repo.as_deref())?
                            .context("Task was not found")?,
                    )),
                    (_, Some(wave)) => Some(ExecWorkFilter::Wave(
                        store
                            .sqlite
                            .resolve_wave_id(wave, repo.as_deref())?
                            .context("Wave was not found")?,
                    )),
                    _ => None,
                };
                let filter = ExecFilter {
                    repo,
                    parent_exec_id: match parent {
                        Some(parent) => Some(
                            store
                                .resolve_exec(parent)
                                .await?
                                .context("Parent Exec was not found")?
                                .id,
                        ),
                        None => None,
                    },
                    caller_session_id: caller.clone(),
                    command_contains: search.clone(),
                    outcome: outcome.as_deref().map(|value| match value {
                        "succeeded" => ExecOutcomeFilter::Succeeded,
                        "failed" => ExecOutcomeFilter::Failed,
                        "interrupted" => ExecOutcomeFilter::Interrupted,
                        _ => ExecOutcomeFilter::Unknown,
                    }),
                    performed_work,
                    ..Default::default()
                };
                let page = store.execs(&filter, after.as_ref(), *limit).await?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&page)?);
                } else {
                    for exec in &page.entries {
                        print_exec(exec);
                    }
                    if let Some(next) = page.next {
                        println!(
                            "Continue with --after '{}' and the same filters",
                            serde_json::to_string(&next)?
                        );
                    }
                }
            }
        }
        Ok(())
    })
}

fn print_exec(exec: &Exec) {
    let command = exec.command.as_deref().unwrap_or("unknown command");
    let display = serde_json::from_str::<Vec<String>>(command)
        .map(|argv| argv.join(" "))
        .unwrap_or_else(|_| command.to_string());
    println!(
        "{}  {}  {}  {}",
        exec.id,
        exec.started_at,
        exec.outcome.as_deref().unwrap_or("unknown"),
        display
    );
}
