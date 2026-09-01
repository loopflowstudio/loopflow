use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};

use crate::durable::WorkRef;
use crate::lf::SessionCommand;
use crate::ops::human_session::{FlowDecision, OpenMode, SessionKind, SessionRecord, SessionState};
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(command))
}

async fn run_async(command: &SessionCommand) -> anyhow::Result<()> {
    match command {
        SessionCommand::List { json, all } => {
            let store = open_shared_store().await?;
            list(&store, *json, *all).await
        }
        SessionCommand::Open {
            id,
            json,
            replace,
            try_open,
        } => {
            let mode = if *replace {
                OpenMode::Replace
            } else if *try_open {
                OpenMode::Try
            } else {
                OpenMode::Refuse
            };
            open(id, *json, mode).await
        }
        SessionCommand::Complete { id } => complete(id).await,
        SessionCommand::Ready { summary } => {
            let text = required_text(summary, "ready summary")?;
            let store = open_shared_store().await?;
            crate::ops::human_session::mark_ready(&store, &text).await?;
            println!("Session is ready for human action.");
            Ok(())
        }
        SessionCommand::Approve { id, summary } => {
            decide_flow(id, FlowDecision::Approve, summary, "approval summary").await
        }
        SessionCommand::Iterate { id, direction } => {
            decide_flow(id, FlowDecision::Iterate, direction, "iteration direction").await
        }
        SessionCommand::ServeFlow {
            task_id,
            flow,
            node_id,
            skill,
            iteration,
        } => {
            let store = open_shared_store().await?;
            crate::ops::human_session::serve_flow(
                store,
                task_id.clone(),
                flow.clone(),
                node_id.clone(),
                skill.clone(),
                *iteration,
            )
            .await
        }
        SessionCommand::ServeAsk { id } => crate::ops::human_session::serve_ask(id).await,
        SessionCommand::StopRun { run_id } => crate::ops::human_session::stop_run(run_id),
    }
}

async fn list(store: &Arc<Store>, json: bool, all: bool) -> anyhow::Result<()> {
    let mut sessions = crate::ops::human_session::list(store).await?;
    sessions = scope_to_repo(sessions, all);
    if json {
        println!("{}", serde_json::to_string_pretty(&sessions)?);
    } else if sessions.is_empty() {
        println!("No Sessions.");
    } else {
        for session in sessions {
            println!(
                "{}  {:<7} {}  {}",
                session.id,
                match session.state {
                    SessionState::Waiting => "waiting",
                    SessionState::Active => "active",
                    SessionState::Ready => "ready",
                    SessionState::Closed => "closed",
                },
                session.work.as_ref().map(WorkRef::id).unwrap_or("run"),
                session.title
            );
        }
    }
    Ok(())
}

async fn open(id: &str, json: bool, mode: OpenMode) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    let session = crate::ops::human_session::open(&store, id, mode, !json).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&session)?);
    }
    Ok(())
}

async fn complete(id: &str) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    let session = crate::ops::human_session::complete(&store, id).await?;
    match session.kind {
        SessionKind::Interactive => println!(
            "Session {} completed; its provider history remains resumable.",
            session.id
        ),
        SessionKind::Ask => println!(
            "Ask session completed: {}",
            session
                .ready_summary
                .expect("completed Ask Session has a ready summary")
        ),
        SessionKind::Flow => unreachable!("FlowStep Sessions cannot complete"),
    }
    Ok(())
}

async fn decide_flow(
    id: &str,
    decision: FlowDecision,
    args: &[String],
    label: &str,
) -> anyhow::Result<()> {
    let text = required_text(args, label)?;
    let store = open_shared_store().await?;
    crate::ops::human_session::decide(&store, id, decision, &text).await?;
    println!(
        "{}",
        match decision {
            FlowDecision::Approve => "Task FlowStep approved; the Task may continue.",
            FlowDecision::Iterate => {
                "Task FlowStep returned to autonomous work for another iteration."
            }
        }
    );
    Ok(())
}

fn scope_to_repo(sessions: Vec<SessionRecord>, all: bool) -> Vec<SessionRecord> {
    if all {
        return sessions;
    }
    let Some(scope) = crate::repository::CanonicalRepo::current() else {
        return sessions;
    };
    sessions
        .into_iter()
        .filter(|session| scope.contains(Path::new(&session.cwd)))
        .collect()
}

fn required_text(args: &[String], label: &str) -> anyhow::Result<String> {
    let text = args.join(" ").trim().to_string();
    if text.is_empty() {
        bail!("{label} cannot be empty");
    }
    Ok(text)
}

async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    Ok(Arc::new(
        open_store(&config)
            .await
            .context("open the shared Loopflow store")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::scope_to_repo;

    #[test]
    fn an_empty_session_list_stays_empty() {
        assert!(scope_to_repo(Vec::new(), true).is_empty());
    }
}
