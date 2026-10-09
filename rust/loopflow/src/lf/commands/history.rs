use std::num::NonZeroU32;

use anyhow::Context;
use clap::Subcommand;

use crate::lf::output::print_process;
use crate::process::{LfProcessCursor, LfProcessFilter, LfProcessOutcomeFilter, LfProcessWorkFilter};
use crate::repository::CanonicalRepo;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, clap::Args)]
pub struct HistoryFeed {
    /// Relative window (7d, 24h, 30m) or RFC3339 start
    #[arg(long, default_value = "7d")]
    pub since: String,
    /// Maximum rows after Work filters (1-200)
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    /// Scope to one Wave by name
    #[arg(long)]
    pub wave: Option<String>,
    /// Scope to one Project by slug
    #[arg(long)]
    pub project: Option<String>,
    /// Scope to one Task by Linear identifier
    #[arg(long)]
    pub task: Option<String>,
    /// Emit the typed activity snapshot as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum HistoryCommand {
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
        after: Option<LfProcessCursor>,
        /// Direct children of an exact or unambiguous parent Process
        #[arg(long)]
        parent: Option<String>,
        /// Commands issued by this Session
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
    /// Inspect a process or Session by identity
    Show {
        id: String,
        #[arg(long, conflicts_with_all = ["events", "final_answer"])]
        json: bool,
        /// Print a Session's retained raw input events
        #[arg(long, conflicts_with = "final_answer")]
        events: bool,
        /// Print a Session's last recorded provider conclusion
        #[arg(long = "final")]
        final_answer: bool,
        /// Inspect an exact retained input belonging to this Session
        #[arg(long)]
        input: Option<String>,
        /// Print only what the step's submitted input was made of, by source
        #[arg(long, conflicts_with_all = ["events", "final_answer"])]
        context: bool,
    },
    /// Show direct provider-authored usage from recorded Session inputs
    Usage {
        /// Emit Session usage evidence as JSON
        #[arg(long)]
        json: bool,
        /// Observation window, in days (zero means all time)
        #[arg(long, default_value_t = 30)]
        days: u32,
        /// Report context cost and turn time by week since 2026-09-30
        #[arg(long, conflicts_with_all = ["days", "parent"])]
        weekly: bool,
        /// Compare Task and Wave usage under prospective and post-hoc bind attribution
        #[arg(long, conflicts_with_all = ["days", "parent", "weekly", "context", "wave", "project", "task"])]
        binds: bool,
        /// Inputs issued by this Session or retained capture
        #[arg(long, conflicts_with_all = ["wave", "project", "task"])]
        parent: Option<String>,
        /// Limit to Session inputs attributed to one Wave
        #[arg(long)]
        wave: Option<String>,
        /// Limit to Session inputs attributed to one Project
        #[arg(long)]
        project: Option<String>,
        /// Limit to Session inputs attributed to one Task
        #[arg(long)]
        task: Option<String>,
        /// Break each step's submitted input down by source, flagged against budgets
        #[arg(long)]
        context: bool,
    },
    /// Launch a new execution from an immutable captured provider request
    Replay {
        /// Captured input identity or an unambiguous displayed prefix
        run: String,
    },
}

fn parse_cursor(value: &str) -> Result<LfProcessCursor, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

pub fn run(command: &HistoryCommand) -> anyhow::Result<()> {
    match command {
        HistoryCommand::Replay { run } => super::replay::run(run),
        HistoryCommand::Usage {
            json, binds: true, ..
        } => super::bind_attribution::run(*json),
        HistoryCommand::Usage {
            json,
            weekly: true,
            wave,
            project,
            task,
            ..
        } => super::context_cost::run(
            *json,
            super::WorkFilter {
                wave: wave.as_deref(),
                project: project.as_deref(),
                task: task.as_deref(),
            },
        ),
        HistoryCommand::Usage {
            json,
            days,
            parent,
            wave,
            project,
            task,
            weekly: false,
            binds: false,
            context,
        } => super::usage::run(
            *context,
            *json,
            *days,
            wave.as_deref(),
            project.as_deref(),
            task.as_deref(),
            parent.as_deref(),
        ),
        HistoryCommand::Show {
            id,
            json,
            events,
            final_answer,
            input,
            context,
        } => show(
            id,
            *json,
            *events,
            *final_answer,
            input.as_deref(),
            *context,
        ),
        HistoryCommand::List {
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
        } => tokio::runtime::Runtime::new()?.block_on(async {
            let store = open_store(&storage_config_from_env()?).await?;
            let repo = if *all {
                None
            } else {
                CanonicalRepo::current()?.map(|repo| repo.to_string())
            };
            let performed_work = match (task, wave) {
                (Some(task), _) => Some(LfProcessWorkFilter::Task(
                    store
                        .sqlite
                        .resolve_task_id(task, repo.as_deref())?
                        .context("Task was not found")?,
                )),
                (_, Some(wave)) => Some(LfProcessWorkFilter::Wave(
                    store
                        .sqlite
                        .resolve_wave_id(wave, repo.as_deref())?
                        .context("Wave was not found")?,
                )),
                _ => None,
            };
            let filter = LfProcessFilter {
                repo,
                parent_process_lfid: match parent {
                    Some(parent) => Some(
                        store
                            .resolve_process(parent)
                            .await?
                            .context("Parent Process was not found")?
                            .lfid,
                    ),
                    None => None,
                },
                caller_session_id: caller.clone(),
                command_contains: search.clone(),
                outcome: outcome.as_deref().map(|value| match value {
                    "succeeded" => LfProcessOutcomeFilter::Succeeded,
                    "failed" => LfProcessOutcomeFilter::Failed,
                    "interrupted" => LfProcessOutcomeFilter::Interrupted,
                    _ => LfProcessOutcomeFilter::Unknown,
                }),
                performed_work,
                ..Default::default()
            };
            let page = store.processes(&filter, after.as_ref(), *limit).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&page)?);
            } else {
                for process in &page.entries {
                    print_process(process);
                }
                if let Some(next) = page.next {
                    println!(
                        "Continue with --after '{}' and the same filters",
                        serde_json::to_string(&next)?
                    );
                }
            }
            Ok(())
        }),
    }
}

fn show(
    id: &str,
    json: bool,
    events: bool,
    final_answer: bool,
    input: Option<&str>,
    context: bool,
) -> anyhow::Result<()> {
    let process = tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        let process = store.resolve_process(id).await?;
        let session = store.session(id).await?;
        anyhow::ensure!(
            process.is_none() || session.is_none(),
            "Identity matches both a process and a Session; use an exact identity"
        );
        anyhow::ensure!(
            process.is_some() || session.is_some(),
            "Process or Session {id} was not found"
        );
        if let Some(input) = input {
            anyhow::ensure!(session.is_some(), "--input requires a Session identity");
            let captured = store.sqlite.input_history(input)?;
            anyhow::ensure!(
                captured.session_id == id,
                "Input does not belong to Session {id}"
            );
        }
        Ok::<_, anyhow::Error>(process)
    })?;
    match process {
        Some(process) => {
            anyhow::ensure!(
                !events && !final_answer && !context,
                "--events, --final and --context inspect Session evidence; Processes record command outcomes"
            );
            if !json {
                print_process(&process);
            }
            println!("{}", serde_json::to_string_pretty(&process)?);
            Ok(())
        }
        None => super::session_history::inspect(
            input.unwrap_or(id),
            events,
            final_answer,
            context,
            json,
        ),
    }
}
