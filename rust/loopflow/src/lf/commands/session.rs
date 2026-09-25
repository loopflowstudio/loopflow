use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};

use crate::lf::SessionCommand;
use crate::ops::human_session::{OpenMode, SessionKind, SessionRecord, SessionState};
use crate::run_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let session_id = match command {
        SessionCommand::Complete { id } => Some(id),
        _ => None,
    };
    let Some(session_id) = session_id else {
        return runtime.block_on(run_async(command));
    };
    let store = runtime.block_on(open_shared_store())?;
    let Some(worktree) = runtime.block_on(crate::ops::human_session::completion_worktree(
        &store, session_id,
    ))?
    else {
        return runtime.block_on(run_async(command));
    };
    let argv = std::env::args().collect::<Vec<_>>();
    crate::journal::with_runtime(&worktree, &argv, || runtime.block_on(run_async(command)))
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
        SessionCommand::Rename {
            id,
            name,
            suggest,
            json,
        } => rename(id, name, *suggest, *json).await,
        SessionCommand::Ready { summary } => {
            let text = required_text(summary, "ready summary")?;
            let store = open_shared_store().await?;
            crate::ops::human_session::mark_ready(&store, &text).await?;
            println!("Session is ready for your review.");
            Ok(())
        }
        SessionCommand::ServeFlow {
            task_id,
            invocation_id,
            flow,
            node_id,
            skill,
            iteration,
        } => {
            let store = open_shared_store().await?;
            crate::ops::human_session::serve_flow(
                store,
                task_id.clone(),
                invocation_id.clone(),
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
    sessions = scope_to_repo(sessions, all)?;
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
                session.work_path.as_deref().unwrap_or("Repository"),
                session.title
            );
            for action in session.actions {
                println!(
                    "  {} — {}",
                    action.label,
                    action.unavailable_reason.as_deref().unwrap_or(&action.help)
                );
            }
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
        SessionKind::Flow => println!("Review completed; feedback returned to the Flow."),
    }
    Ok(())
}

async fn rename(id: &str, name: &[String], suggest: bool, json: bool) -> anyhow::Result<()> {
    let requested = required_text(name, "Session name")?;
    let source = if suggest {
        SessionTitleSource::Generated
    } else {
        SessionTitleSource::Human
    };
    let store = open_shared_store().await?;
    let session = crate::ops::human_session::rename(&store, id, &requested, source).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&session)?);
    } else if suggest && session.title_source == SessionTitleSource::Human {
        println!(
            "Session {} keeps its human-assigned name {:?}.",
            session.id, session.title
        );
    } else {
        println!("Session {} is named {:?}.", session.id, session.title);
    }
    Ok(())
}

fn scope_to_repo(sessions: Vec<SessionRecord>, all: bool) -> anyhow::Result<Vec<SessionRecord>> {
    if all {
        return Ok(sessions);
    }
    let Some(scope) = crate::repository::CanonicalRepo::current()? else {
        return Ok(sessions);
    };
    Ok(sessions
        .into_iter()
        .filter(|session| scope.contains(Path::new(&session.cwd)))
        .collect())
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
        assert!(scope_to_repo(Vec::new(), true).unwrap().is_empty());
    }
}
