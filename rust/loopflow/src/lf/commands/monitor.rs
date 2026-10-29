use std::num::NonZeroU32;

use clap::Subcommand;

use crate::lf::output::print_process;

use crate::process::LfProcessFilter;
use crate::repository::CanonicalRepo;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, Subcommand)]
pub enum MonitorCommand {
    /// Print one parseable snapshot of live Loopflow call trees
    Ps {
        /// Emit the versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
        /// Print a JSON line now and each time the call trees change, until stdin closes
        #[arg(long, requires = "json")]
        watch: bool,
    },
    /// Refresh live Loopflow call trees on a terminal; print once when redirected
    Top,
    /// Move idle Sessions still running a superseded lf onto the selected install
    #[command(
        after_help = "A Session moves when nobody is viewing it, no turn, Graph step or headless Run is in progress, and its provider history is saved. Its owner is ended and the Session resumes on the selected lf. A running Graph isn't stopped: its next step launches on the selected lf, and its runner exits with the Graph. Every other process on a superseded lf is listed with why it stays.\n\nExit status: 0 when every movable Session resumed; 1 otherwise."
    )]
    Rollover {
        /// List what would move and what stays without stopping anything
        #[arg(long)]
        dry_run: bool,
        /// Emit the rollover report as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reap orphan providers, remove dead receipts, forget finished Process rows and release unrecorded AgentProcess ancestry
    Prune {
        /// Show exact targets without changing processes, receipts or the store
        #[arg(long)]
        dry_run: bool,
        /// Emit the versioned prune report as JSON
        #[arg(long)]
        json: bool,
    },
}

impl MonitorCommand {
    /// Whether this command changes state, and so records a Process.
    pub fn changes_work(&self) -> bool {
        match self {
            Self::Ps { .. } | Self::Top => false,
            Self::Rollover { dry_run, .. } | Self::Prune { dry_run, .. } => !dry_run,
        }
    }
}

pub fn run(command: &MonitorCommand) -> anyhow::Result<()> {
    match command {
        MonitorCommand::Ps { watch: true, .. } => {
            super::top::watch_ps(super::watch::Transport::Stdio).map(drop)
        }
        MonitorCommand::Ps { json, .. } => super::top::run_ps(*json),
        MonitorCommand::Top => super::top::run_top(),
        MonitorCommand::Rollover { dry_run, json } => super::rollover::run(*dry_run, *json),
        MonitorCommand::Prune { dry_run, json } => super::top::run_prune(*json, *dry_run),
    }
}

#[derive(Debug, serde::Serialize)]
struct MonitorItem {
    kind: &'static str,
    id: String,
    title: String,
    state: &'static str,
    reason: String,
    next_action: String,
}

#[derive(Debug, serde::Serialize)]
struct MonitorOverview {
    observed_at: i64,
    items: Vec<MonitorItem>,
    recent_commands: crate::process::LfProcessPage,
    sessions_next: Option<String>,
    graphs_next: Option<String>,
    gaps: Vec<String>,
}

pub fn overview(json: bool, all: bool) -> anyhow::Result<()> {
    let report = tokio::runtime::Runtime::new()?.block_on(async {
        let store = std::sync::Arc::new(open_store(&storage_config_from_env()?).await?);
        let repo = if all {
            None
        } else {
            CanonicalRepo::current()?.map(|repo| repo.to_string())
        };
        let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        let mut gaps = Vec::new();
        let mut items = Vec::new();
        let filter = crate::session::SessionFilter {
            repo: repo.clone(),
            interactive: None,
            history: true,
            limit: 101,
            after: Some(String::new()),
            ..Default::default()
        };
        let mut sessions = match crate::ops::human_session::list(&store, &filter).await {
            Ok(sessions) => sessions,
            Err(error) => {
                gaps.push(format!("Session inventory unavailable: {error}"));
                Vec::new()
            }
        };
        let sessions_next = if sessions.len() > 100 {
            sessions.truncate(100);
            sessions.last().map(|session| session.id.to_string())
        } else {
            None
        };
        for session in sessions {
            use crate::ops::human_session::SessionState;
            let (state, reason, next) = match session.state {
                SessionState::Closed => (
                    "finished",
                    "Conversation completion is recorded".to_string(),
                    format!("lf session history {}", session.id),
                ),
                SessionState::Active | SessionState::Detached => (
                    "active",
                    "Matching provider process observed".into(),
                    "lf ps".to_string(),
                ),
                _ if session.availability.is_some() => (
                    "available",
                    "Its conversation is available for your input".into(),
                    format!("lf session connect {}", session.id),
                ),
                _ => (
                    "unknown",
                    "No current provider observation or completion receipt".into(),
                    format!("lf session history {}", session.id),
                ),
            };
            items.push(MonitorItem {
                kind: "session",
                id: session.id.into(),
                title: session.title,
                state,
                reason,
                next_action: next,
            });
        }
        let graphs = store
            .graph_inventory(
                &crate::durable::GraphProcessFilter {
                    repo: repo.clone(),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).expect("100 is nonzero"),
            )
            .await?;
        for entry in graphs.entries {
            let summary = &entry.summary;
            let detail = match store.graph_detail(&summary.id).await {
                Ok(detail) => detail,
                Err(error) => {
                    gaps.push(format!("Graph {} detail unavailable: {error}", summary.id));
                    None
                }
            };
            let at = detail
                .as_ref()
                .and_then(|detail| detail.steps.last())
                .map(crate::durable::GraphStepProcess::position)
                .unwrap_or_else(|| "an unreadable step".into());
            let (state, reason) = match summary.state {
                crate::session::GraphProcessSummaryState::Completed => {
                    ("finished", "Graph completed".to_string())
                }
                crate::session::GraphProcessSummaryState::Stopped => {
                    ("stopped", format!("Graph process exited at {at}"))
                }
                crate::session::GraphProcessSummaryState::Current => {
                    match crate::id::LfProcessId::parse(&summary.id)
                        .map(|id| crate::journal::process_evidence(&store.sqlite, &id))
                    {
                        Ok(crate::journal::ProcessIdentityEvidence::Live) => {
                            ("running", format!("Graph process is at {at}"))
                        }
                        Ok(crate::journal::ProcessIdentityEvidence::Dead) => (
                            "stopped",
                            format!("Graph process left no exit record; it stopped at {at}"),
                        ),
                        _ => (
                            "unknown",
                            format!("Graph process identity is unknown at {at}"),
                        ),
                    }
                }
            };
            let next = format!("lf graph show {} --processes", summary.id);
            items.push(MonitorItem {
                kind: "graph",
                id: summary.id.clone(),
                title: summary.name.clone(),
                state,
                reason,
                next_action: next,
            });
        }
        let recent_commands = store
            .processes(
                &LfProcessFilter {
                    repo,
                    ..Default::default()
                },
                None,
                NonZeroU32::new(20).expect("20 is nonzero"),
            )
            .await?;
        Ok::<_, anyhow::Error>(MonitorOverview {
            observed_at,
            items,
            recent_commands,
            sessions_next,
            graphs_next: graphs.next,
            gaps,
        })
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for item in &report.items {
            println!(
                "{}  {}  {} ({})\n  {}\n  {}",
                item.state, item.id, item.title, item.kind, item.reason, item.next_action
            );
        }
        for command in &report.recent_commands.entries {
            print_process(command);
        }
        for gap in &report.gaps {
            println!("Unavailable: {gap}");
        }
        if report.items.is_empty() {
            println!("No recorded conversations or Graphs in this scope.");
        }
        if report.sessions_next.is_some() {
            println!(
                "More conversations: lf session list --interactive all --history --page --json"
            );
        }
        if report.graphs_next.is_some() {
            println!("More Graphs: lf graph list --processes --json");
        }
        if report.recent_commands.next.is_some() {
            println!("More commands: lf history list");
        }
    }
    Ok(())
}
