use std::num::NonZeroU32;

use anyhow::Context;
use clap::Subcommand;

use crate::lf::commands::session_history::SessionView;
use crate::lf::output::print_process;
use crate::process::{
    LfProcessCursor, LfProcessFilter, LfProcessOutcomeFilter, LfProcessWorkFilter,
};
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
    /// Print a JSON line now and each time the history changes, until stdin closes
    #[arg(long, requires = "json")]
    pub watch: bool,
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
        #[arg(long, conflicts_with = "final_answer")]
        json: bool,
        /// Print a Session's last recorded provider conclusion
        #[arg(long = "final")]
        final_answer: bool,
        /// Inspect an exact retained input belonging to this Session
        #[arg(long)]
        input: Option<String>,
        /// Print only what the step's submitted input was made of, by source
        #[arg(long, conflicts_with_all = ["final_answer", "instructions"])]
        context: bool,
        /// Explain recorded instruction inclusion and omission
        #[arg(long, conflicts_with_all = ["final_answer", "context"])]
        instructions: bool,
        /// Print only the Session's conversation, as its providers kept it
        #[arg(long, conflicts_with_all = ["final_answer", "context", "instructions", "input"])]
        transcript: bool,
    },
    /// Show how much Session history providers still keep, and what they remove next
    Retention {
        #[arg(long)]
        json: bool,
    },
    /// Find text in the conversations of recent Sessions
    Search {
        text: String,
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long, default_value = "7d")]
        since: String,
        /// Scope to one Wave by name
        #[arg(long)]
        wave: Option<String>,
        /// Scope to one Project by slug
        #[arg(long)]
        project: Option<String>,
        /// Scope to one Task by Linear identifier
        #[arg(long)]
        task: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
        #[arg(long)]
        json: bool,
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
    /// Show where this Machine's Linear requests went, by hour, command and operation
    #[command(
        after_help = "Requests are recorded only on a Machine whose ~/.lf/config.yaml sets\n\n  logs:\n    linear_requests: true"
    )]
    Linear {
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long, default_value = "1h")]
        since: String,
        #[arg(long)]
        json: bool,
    },
    /// Launch a new execution from a captured provider request, while this Machine retains its capture
    Replay {
        /// Captured input identity or an unambiguous displayed prefix
        run: String,
    },
    /// Write this Machine's Sessions, Processes and captures as JSON lines,
    /// for the Machine that copies them before giving this one back
    Export {
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long)]
        since: String,
    },
}

fn export(since: &str) -> anyhow::Result<()> {
    let since = super::util::parse_since(since, time::OffsetDateTime::now_utc())?;
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(open_store(&storage_config_from_env()?))?;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    crate::machine_records::export(
        &store.sqlite,
        &crate::store::lf_home_dir(),
        since.unix_timestamp(),
        &mut out,
    )?;
    std::io::Write::flush(&mut out)?;
    Ok(())
}

fn parse_cursor(value: &str) -> Result<LfProcessCursor, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

pub fn run(command: &HistoryCommand) -> anyhow::Result<()> {
    match command {
        HistoryCommand::Replay { run } => super::replay::run(run),
        HistoryCommand::Linear { since, json } => super::linear_requests::run(since, *json),
        HistoryCommand::Export { since } => export(since),
        HistoryCommand::Retention { json } => super::history_transcript::retention(*json),
        HistoryCommand::Search {
            text,
            since,
            wave,
            project,
            task,
            limit,
            json,
        } => super::history_transcript::search(
            text,
            since,
            super::WorkFilter {
                wave: wave.as_deref(),
                project: project.as_deref(),
                task: task.as_deref(),
            },
            *limit,
            *json,
        ),
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
            final_answer,
            input,
            context,
            instructions,
            transcript,
        } => {
            let view = [
                (*final_answer, SessionView::Final),
                (*context, SessionView::Context),
                (*instructions, SessionView::Instructions),
                (*transcript, SessionView::Transcript),
            ]
            .into_iter()
            .find_map(|(asked, view)| asked.then_some(view))
            .unwrap_or(SessionView::Summary);
            show(id, *json, input.as_deref(), view)
        }
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
                parent_lf_process_id: match parent {
                    Some(parent) => Some(
                        store
                            .resolve_process(parent)
                            .await?
                            .context("Parent Process was not found")?
                            .id,
                    ),
                    None => None,
                },
                caller_session_id: caller.clone().map(Into::into),
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

/// What `lf history show` found: this Machine's own record, or a copy.
#[derive(Debug)]
enum Shown {
    Local(Option<crate::process::LfProcess>),
    Copied(crate::machine_records::CopiedHistory),
}

fn show(id: &str, json: bool, input: Option<&str>, view: SessionView) -> anyhow::Result<()> {
    let shown = tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        let process = store.resolve_process(id).await?;
        let session = store.session(id).await?;
        anyhow::ensure!(
            process.is_none() || session.is_none(),
            "Identity matches both a process and a Session; use an exact identity"
        );
        if process.is_none() && session.is_none() {
            // A Session copied from a given-back Machine, when nothing here is it.
            let copied = store.sqlite.copied_history(id)?;
            let copied =
                copied.with_context(|| format!("Process or Session {id} was not found"))?;
            return Ok(Shown::Copied(copied));
        }
        if let Some(input) = input {
            anyhow::ensure!(session.is_some(), "--input requires a Session identity");
            let captured = store.sqlite.input_history(input)?;
            anyhow::ensure!(
                captured.session_id == id,
                "Input does not belong to Session {id}"
            );
        }
        Ok::<_, anyhow::Error>(Shown::Local(process))
    })?;
    let process = match shown {
        Shown::Local(process) => process,
        Shown::Copied(copied) => {
            anyhow::ensure!(
                !json && input.is_none() && view == SessionView::Summary,
                "Session {id} is a copy from {}; it reads as text",
                copied.session.origin_name()
            );
            println!("{}", copied.render(&crate::store::lf_home_dir()));
            return Ok(());
        }
    };
    match process {
        Some(process) => {
            anyhow::ensure!(
                view == SessionView::Summary,
                "--final, --context, --instructions and --transcript inspect Session evidence; Processes record command outcomes"
            );
            if !json {
                print_process(&process);
            }
            println!("{}", serde_json::to_string_pretty(&process)?);
            Ok(())
        }
        None => super::session_history::inspect(input.unwrap_or(id), view, json),
    }
}
