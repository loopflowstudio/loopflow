use std::num::NonZeroU32;

use anyhow::Context;
use clap::Subcommand;

use crate::exec::{Exec, ExecCursor, ExecFilter, ExecOutcomeFilter, ExecWorkFilter};
use crate::repository::CanonicalRepo;
use crate::store::{open_store, storage_config_from_env};

#[derive(Debug, Subcommand)]
pub enum MonitorCommand {
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
    /// Inspect an Exec or Session by identity
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
    /// Stream what a workspace shows, each part again only when it changes
    Workspace {
        #[arg(long, required = true)]
        json: bool,
        /// Stream NDJSON until stdin closes
        #[arg(long)]
        watch: bool,
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
    /// Show one ordered record of durable Work, Session, PR, and Steer facts
    Activity {
        /// Relative window (7d, 24h, 30m) or RFC3339 start
        #[arg(long, default_value = "7d")]
        since: String,
        /// Maximum rows after Work filters (1-200)
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Scope to one Wave by name
        #[arg(long)]
        wave: Option<String>,
        /// Scope to one Project by slug
        #[arg(long)]
        project: Option<String>,
        /// Scope to one Task by Linear identifier
        #[arg(long)]
        task: Option<String>,
        /// Emit the typed activity snapshot as JSON
        #[arg(long)]
        json: bool,
    },
}

fn parse_cursor(value: &str) -> Result<ExecCursor, String> {
    serde_json::from_str(value).map_err(|error| error.to_string())
}

pub fn run(command: &MonitorCommand) -> anyhow::Result<()> {
    match command {
        MonitorCommand::Active { json, watch, task } => {
            super::runs::list_active(*json, *watch, task.as_deref())
        }
        MonitorCommand::Workspace { watch, .. } => super::workspace_watch::run(*watch),
        MonitorCommand::Ps { json } => super::top::run_ps(*json),
        MonitorCommand::Top { json } => super::top::run_top(*json),
        MonitorCommand::Prune { dry_run, json } => super::top::run_prune(*json, *dry_run),
        MonitorCommand::Usage {
            json, binds: true, ..
        } => super::bind_attribution::run(*json),
        MonitorCommand::Usage {
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
        MonitorCommand::Usage {
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
        MonitorCommand::Activity {
            since,
            limit,
            wave,
            project,
            task,
            json,
        } => super::activity::run(
            since,
            *limit,
            wave.as_deref(),
            project.as_deref(),
            task.as_deref(),
            *json,
        ),
        MonitorCommand::Show {
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
        MonitorCommand::List {
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
            Ok(())
        }),
    }
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

fn show(
    id: &str,
    json: bool,
    events: bool,
    final_answer: bool,
    input: Option<&str>,
    context: bool,
) -> anyhow::Result<()> {
    let exec = tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store(&storage_config_from_env()?).await?;
        let exec = store.resolve_exec(id).await?;
        let session = store.session(id).await?;
        anyhow::ensure!(
            exec.is_none() || session.is_none(),
            "Identity matches both an Exec and a Session; use an exact identity"
        );
        anyhow::ensure!(
            exec.is_some() || session.is_some(),
            "Exec or Session {id} was not found"
        );
        if let Some(input) = input {
            anyhow::ensure!(session.is_some(), "--input requires a Session identity");
            let captured = store.sqlite.input_history(input)?;
            anyhow::ensure!(
                captured.session_id == id,
                "Input does not belong to Session {id}"
            );
        }
        Ok::<_, anyhow::Error>(exec)
    })?;
    match exec {
        Some(exec) => {
            anyhow::ensure!(
                !events && !final_answer && !context,
                "--events, --final and --context inspect Session evidence; Execs record command outcomes"
            );
            if json {
                println!("{}", serde_json::to_string_pretty(&exec)?);
            } else {
                print_exec(&exec);
                println!("{}", serde_json::to_string_pretty(&exec)?);
            }
            Ok(())
        }
        None => super::runs::inspect(input.unwrap_or(id), events, final_answer, context, json),
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
    recent_commands: crate::exec::ExecPage,
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
                SessionState::Ready => (
                    "waiting",
                    session
                        .ready_summary
                        .clone()
                        .unwrap_or_else(|| "Review is ready".into()),
                    format!("lf session connect {}", session.id),
                ),
                _ if live => (
                    "active",
                    "Matching provider process observed".into(),
                    "lf monitor active --json".to_string(),
                ),
                SessionState::Waiting => (
                    "waiting",
                    session.detail.clone(),
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
                &crate::durable::FlowFilter {
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
                    gaps.push(format!(
                        "FlowSession {} detail unavailable: {error}",
                        summary.id
                    ));
                    None
                }
            };
            let (state, reason, next) =
                if let Some(failure) = detail.as_ref().and_then(|detail| detail.failure.as_ref()) {
                    (
                        "blocked",
                        failure.reason.clone(),
                        format!("lf flow show {} --sessions", summary.id),
                    )
                } else if summary.state != crate::session::FlowSummaryState::Current {
                    (
                        "finished",
                        format!("Recorded {:?}", summary.state),
                        format!("lf flow show {} --sessions", summary.id),
                    )
                } else if let Some(session) = &summary.pending_session {
                    (
                        "waiting",
                        "Flow waits for a review or decision".into(),
                        format!("lf session connect {session}"),
                    )
                } else {
                    (
                        "unknown",
                        "Saved progress does not establish a live driver".into(),
                        format!("lf flow show {} --sessions", summary.id),
                    )
                };
            items.push(MonitorItem {
                kind: "flow_session",
                id: summary.id.clone(),
                title: summary
                    .name
                    .clone()
                    .unwrap_or_else(|| "Unnamed Flow".into()),
                state,
                reason,
                next_action: next,
            });
        }
        let recent_commands = store
            .execs(
                &ExecFilter {
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
            print_exec(command);
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
            println!("More Flows: lf flow list --sessions --json");
        }
        if report.recent_commands.next.is_some() {
            println!("More commands: lf monitor list");
        }
    }
    Ok(())
}
