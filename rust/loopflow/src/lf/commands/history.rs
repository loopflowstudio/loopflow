use anyhow::Context;
use clap::Subcommand;

use crate::lf::commands::session_history::SessionView;
use loopflow_execution::store::{open_store, storage_config_from_env};

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
    #[arg(short = 's', long, requires = "json")]
    pub subscribe: bool,
    #[command(flatten)]
    pub selection: crate::lf::Selection,
}

/// `session.usage`: provider-authored usage from recorded Session inputs.
#[derive(Debug, clap::Args)]
pub struct UsageArgs {
    /// All recorded turn usage for one local or copied Session, with its origin
    #[arg(long, conflicts_with_all = ["days","weekly","binds","parent","wave","project","task","context"])]
    pub session: Option<String>,
    /// Emit Session usage evidence as JSON
    #[arg(long)]
    pub json: bool,
    /// Observation window, in days (zero means all time)
    #[arg(long, default_value_t = 30)]
    pub days: u32,
    /// Report context cost and turn time by week since 2026-09-30
    #[arg(long, conflicts_with_all = ["days", "parent"])]
    pub weekly: bool,
    /// Compare Task and Wave usage under prospective and post-hoc bind attribution
    #[arg(long, conflicts_with_all = ["days", "parent", "weekly", "context", "wave", "project", "task"])]
    pub binds: bool,
    /// Inputs issued by this Session or retained capture
    #[arg(long, conflicts_with_all = ["wave", "project", "task"])]
    pub parent: Option<String>,
    /// Limit to Session inputs attributed to one Wave
    #[arg(long)]
    pub wave: Option<String>,
    /// Limit to Session inputs attributed to one Project
    #[arg(long)]
    pub project: Option<String>,
    /// Limit to Session inputs attributed to one Task
    #[arg(long)]
    pub task: Option<String>,
    /// Break each step's submitted input down by source, flagged against budgets
    #[arg(long)]
    pub context: bool,
}

#[derive(Debug, Subcommand)]
pub enum HistoryCommand {
    /// Inspect a Session by identity
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
    /// Export Session history, transcripts, usage and captures as JSON lines
    Export {
        /// Resume at the checkpoint from an earlier export
        #[arg(long)]
        after: Option<String>,
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long)]
        since: String,
    },
}

impl HistoryCommand {
    /// Whether this command changes state, and so writes a journal trace.
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Show { .. } | Self::Search { .. } | Self::Linear { .. } | Self::Export { .. } => {
                false
            }
            Self::Replay { .. } => true,
        }
    }
}

fn export(since: &str, after: Option<&str>) -> anyhow::Result<()> {
    let since = loopflow_execution::output::parse_since(since, time::OffsetDateTime::now_utc())?;
    let store = loopflow_execution::runtime::block_on(open_store(&storage_config_from_env()?))?;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    loopflow_execution::machine_records::export(
        &store.sqlite,
        &loopflow_execution::store::lf_home_dir(),
        since.unix_timestamp(),
        after.map(serde_json::from_str).transpose()?,
        &mut out,
    )?;
    std::io::Write::flush(&mut out)?;
    Ok(())
}

/// `lf session usage`.
pub fn usage(args: &UsageArgs) -> anyhow::Result<()> {
    match args {
        UsageArgs {
            session: Some(session),
            json,
            ..
        } => super::usage::session(session, *json),
        UsageArgs {
            json, binds: true, ..
        } => super::bind_attribution::run(*json),
        UsageArgs {
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
        UsageArgs {
            json,
            days,
            parent,
            wave,
            project,
            task,
            session: None,
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
    }
}

pub fn run(command: &HistoryCommand) -> anyhow::Result<()> {
    match command {
        HistoryCommand::Replay { run } => super::replay::run(run),
        HistoryCommand::Linear { since, json } => super::linear_requests::run(since, *json),
        HistoryCommand::Export { since, after } => export(since, after.as_deref()),
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
    }
}

fn show(id: &str, json: bool, input: Option<&str>, view: SessionView) -> anyhow::Result<()> {
    print!("{}", render(id, json, input, view)?);
    Ok(())
}

pub(crate) fn render(
    id: &str,
    json: bool,
    input: Option<&str>,
    view: SessionView,
) -> anyhow::Result<String> {
    let store = loopflow_execution::runtime::block_on(open_store(&storage_config_from_env()?))?;
    if loopflow_execution::runtime::block_on(store.session(id))?.is_some() {
        if let Some(input) = input {
            let captured =
                loopflow_execution::store::sessions::input_history(&store.sqlite, input)?;
            anyhow::ensure!(
                captured.session_id == id,
                "Input does not belong to Session {id}"
            );
        }
        return super::session_history::render(input.unwrap_or(id), view, json);
    }
    let copied = loopflow_execution::store::machines::copied_history(&store.sqlite, id)?
        .with_context(|| format!("Session {id} was not found"))?;
    let origin = copied.session.origin_name();
    match (view, input) {
        (SessionView::Final, _) => anyhow::bail!(
            "The copy of {id} from {origin} keeps no final answer here; `--transcript` shows how it ended"
        ),
        (SessionView::Transcript, _) => {
            super::history_transcript::copied(&store.sqlite, &copied, json)
        }
        (SessionView::Summary, None) if json => {
            Ok(format!("{}\n", serde_json::to_string(&copied)?))
        }
        (SessionView::Summary, None) => Ok(format!(
            "{}\n",
            loopflow_execution::machine_records::render(
                &copied,
                &loopflow_execution::store::lf_home_dir()
            )
        )),
        // An input's own views read its copied capture.
        (SessionView::Summary | SessionView::Context | SessionView::Instructions, input) => {
            let mut captures = copied
                .events
                .iter()
                .filter_map(|event| event.artifact_key.as_deref())
                .filter(|key| input.is_none_or(|input| key.starts_with(input)));
            let key = captures.next_back().with_context(|| match input {
                Some(_) => format!("Input does not belong to Session {id}"),
                None => format!("The copy of {id} from {origin} recorded no input"),
            })?;
            super::session_history::render(key, view, json)
        }
    }
}
