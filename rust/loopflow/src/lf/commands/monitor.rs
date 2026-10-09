use std::num::NonZeroU32;

use clap::Subcommand;

use crate::process::ProcessFilter;
use crate::repository::CanonicalRepo;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, Subcommand)]
pub enum MonitorCommand {
    /// Observe active conversations and missing process evidence
    Active {
        #[arg(long)]
        json: bool,
        /// Stream NDJSON until stdin closes
        #[arg(long, requires = "json")]
        watch: bool,
        #[arg(long)]
        task: Option<String>,
    },
    /// Stream planning and activity for the selected Work, each part again only when it changes
    Work {
        #[arg(long, required = true)]
        json: bool,
        /// Stream NDJSON until stdin closes
        #[arg(long)]
        watch: bool,
    },
    /// Print one parseable snapshot of live Loopflow call trees
    Ps {
        /// Emit the versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Refresh live Loopflow call trees on a terminal; print once when redirected
    Top {
        /// Emit one versioned activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
    /// Reap registered orphan providers and remove dead process receipts
    Prune {
        /// Show exact targets without changing process or receipt state
        #[arg(long)]
        dry_run: bool,
        /// Emit the versioned prune report as JSON
        #[arg(long)]
        json: bool,
    },
}

pub fn run(command: &MonitorCommand) -> anyhow::Result<()> {
    match command {
        MonitorCommand::Active { json, watch, task } => {
            super::session_history::list_active(*json, *watch, task.as_deref())
        }
        MonitorCommand::Work { watch, .. } => super::work_watch::run(*watch),
        MonitorCommand::Ps { json } => super::top::run_ps(*json),
        MonitorCommand::Top { json } => super::top::run_top(*json),
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
    active: crate::session_record::active::ActiveSessionsSnapshot,
    recent_commands: crate::process::ProcessPage,
    sessions_next: Option<String>,
    flows_next: Option<String>,
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
        let home = crate::store::lf_home_dir();
        let active = crate::session_record::active::snapshot(&home, &store, None).await;
        let mut gaps = active.gaps.clone();
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
            sessions.last().map(|session| session.id.clone())
        } else {
            None
        };
        for session in sessions {
            use crate::ops::human_session::SessionState;
            let live = active.sessions.iter().any(|live| live.id == session.id);
            let (state, reason, next) = match session.state {
                SessionState::Closed => (
                    "finished",
                    "Conversation completion is recorded".to_string(),
                    format!("lf session history {}", session.id),
                ),
                _ if live => (
                    "active",
                    "Matching provider process observed".into(),
                    "lf monitor active --json".to_string(),
                ),
                _ if session.attention.is_some() => (
                    "waiting",
                    "Its provider is waiting on a person".into(),
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
                id: session.id,
                title: session.title,
                state,
                reason,
                next_action: next,
            });
        }
        let flows = store
            .flow_inventory(
                &crate::durable::FlowProcessFilter {
                    repo: repo.clone(),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).expect("100 is nonzero"),
            )
            .await?;
        for entry in flows.entries {
            let summary = &entry.summary;
            let detail = match store.flow_detail(&summary.id).await {
                Ok(detail) => detail,
                Err(error) => {
                    gaps.push(format!("Flow {} detail unavailable: {error}", summary.id));
                    None
                }
            };
            let at = detail
                .as_ref()
                .and_then(|detail| detail.steps.last())
                .map(crate::durable::FlowStepProcess::position)
                .unwrap_or_else(|| "an unreadable step".into());
            let (state, reason) = match summary.state {
                crate::session::FlowProcessSummaryState::Completed => {
                    ("finished", "Flow completed".to_string())
                }
                crate::session::FlowProcessSummaryState::Stopped => {
                    ("stopped", format!("Driver exited at {at}"))
                }
                crate::session::FlowProcessSummaryState::Current => {
                    match crate::id::ProcessLfid::parse(&summary.id)
                        .map(|driver| crate::journal::process_evidence(&store.sqlite, &driver))
                    {
                        Ok(crate::journal::ProcessIdentityEvidence::Live) => {
                            ("running", format!("Driver is at {at}"))
                        }
                        Ok(crate::journal::ProcessIdentityEvidence::Dead) => (
                            "stopped",
                            format!("Driver left no exit record; it stopped at {at}"),
                        ),
                        _ => (
                            "unknown",
                            format!("Driver process identity is unknown at {at}"),
                        ),
                    }
                }
            };
            let next = format!("lf flow show {} --processes", summary.id);
            items.push(MonitorItem {
                kind: "flow",
                id: summary.id.clone(),
                title: summary.name.clone(),
                state,
                reason,
                next_action: next,
            });
        }
        let recent_commands = store
            .processes(
                &ProcessFilter {
                    repo,
                    ..Default::default()
                },
                None,
                NonZeroU32::new(20).expect("20 is nonzero"),
            )
            .await?;
        Ok::<_, anyhow::Error>(MonitorOverview {
            observed_at: active.observed_at,
            items,
            active,
            recent_commands,
            sessions_next,
            flows_next: flows.next,
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
            println!("No recorded conversations or Flows in this scope.");
        }
        if report.sessions_next.is_some() {
            println!(
                "More conversations: lf session list --interactive all --history --page --json"
            );
        }
        if report.flows_next.is_some() {
            println!("More Flows: lf flow list --processes --json");
        }
        if report.recent_commands.next.is_some() {
            println!("More commands: lf history list");
        }
    }
    Ok(())
}
