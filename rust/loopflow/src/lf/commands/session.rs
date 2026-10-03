use std::sync::Arc;

use anyhow::{bail, Context};

use crate::lf::SessionCommand;
use crate::ops::human_session::{OpenMode, SessionKind, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let worktree = match command {
        SessionCommand::Open { json: false, .. }
        | SessionCommand::ServeConversation { .. }
        | SessionCommand::ServeFlow { .. } => Some(crate::repo::working_directory()?),
        SessionCommand::Complete { id } => {
            let store = runtime.block_on(open_shared_store())?;
            runtime.block_on(crate::ops::human_session::completion_worktree(&store, id))?
        }
        _ => None,
    };
    let Some(worktree) = worktree else {
        return runtime.block_on(run_async(command));
    };
    let argv = std::env::args().collect::<Vec<_>>();
    crate::journal::with_runtime(&worktree, &argv, || runtime.block_on(run_async(command)))
}

async fn run_async(command: &SessionCommand) -> anyhow::Result<()> {
    match command {
        SessionCommand::History {
            id,
            json,
            after,
            limit,
        } => {
            let store = open_shared_store().await?;
            if store.session(id).await?.is_none() {
                bail!("Session {id} was not found");
            }
            let events = store.sqlite.session_history(id, *after, *limit)?;
            if *json {
                println!("{}", serde_json::to_string(&events)?);
            } else {
                for event in events {
                    println!(
                        "{} {} {} {}",
                        event.seq,
                        event.provider_turn.as_deref().unwrap_or("unknown-turn"),
                        event.kind.as_str(),
                        event.payload
                    );
                }
            }
            Ok(())
        }
        SessionCommand::List {
            json,
            all,
            interactive,
            history,
            needs_me,
            limit,
            offset,
            page,
            after,
            task,
            orphan,
            search,
        } => {
            let store = open_shared_store().await?;
            list(
                &store,
                *json,
                *needs_me,
                &crate::session::SessionFilter {
                    repo: if *all {
                        None
                    } else {
                        crate::repository::CanonicalRepo::current()?.map(|repo| repo.to_string())
                    },
                    task: task.clone(),
                    orphan: *orphan,
                    search: search.clone(),
                    interactive: if *needs_me {
                        None
                    } else {
                        interactive.interactive()
                    },
                    history: *history,
                    limit: *limit,
                    offset: *offset,
                    after: page.then(|| after.clone().unwrap_or_default()),
                },
            )
            .await
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
        SessionCommand::Ensure { wave, json } => {
            let store = open_shared_store().await?;
            let repo = crate::repo::find_repo_root()?;
            let session =
                crate::ops::human_session::primary::ensure(&store, &repo, wave.as_deref()).await?;
            report_primary(&session, *json)
        }
        SessionCommand::Replace { id, json } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::primary::replace(&store, id).await?;
            report_primary(&session, *json)
        }
        SessionCommand::Complete { id } => complete(id).await,
        SessionCommand::Rename {
            id,
            name,
            suggest,
            json,
        } => rename(id, name, *suggest, *json).await,
        SessionCommand::Bind {
            id,
            task,
            dry_run,
            json,
        } => {
            let store = open_shared_store().await?;
            if *dry_run {
                let preview = crate::ops::human_session::preview_binding(&store, id, task).await?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&preview)?);
                } else {
                    println!(
                        "{} → {} ({}) [{}]. Not assigned.",
                        preview.session_id,
                        preview.identifier,
                        preview.title,
                        preview.task_id.as_deref().unwrap_or("not yet admitted")
                    );
                }
                return Ok(());
            }
            let session = crate::ops::human_session::bind(&store, id, task).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&session)?);
            } else {
                println!(
                    "Session {} belongs to {}.",
                    session.id,
                    session.work_path.as_deref().unwrap_or(task)
                );
            }
            Ok(())
        }
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
        SessionCommand::ServeConversation { input } => {
            let store = open_shared_store().await?;
            crate::ops::human_session::serve_conversation(&store, input).await
        }
        SessionCommand::StopClient { input } => {
            crate::ops::human_session::stop_session_client(input)
        }
    }
}

async fn list(
    store: &Arc<Store>,
    json: bool,
    needs_me: bool,
    filter: &crate::session::SessionFilter,
) -> anyhow::Result<()> {
    if filter.after.is_some() {
        anyhow::ensure!(
            filter.limit > 0,
            "paged Session inventory requires a positive limit"
        );
        let mut selection = filter.clone();
        selection.limit = filter
            .limit
            .checked_add(1)
            .context("Session page limit is too large")?;
        let mut entries = if needs_me {
            crate::ops::human_session::list_attention(store, &selection).await?
        } else {
            crate::ops::human_session::list(store, &selection).await?
        };
        let next = if entries.len() > filter.limit {
            entries.pop();
            entries.last().map(|session| session.id.clone())
        } else {
            None
        };
        println!(
            "{}",
            serde_json::to_string_pretty(&crate::ops::human_session::SessionPage {
                entries,
                next
            })?
        );
        return Ok(());
    }
    let sessions = if needs_me {
        crate::ops::human_session::list_attention(store, filter).await?
    } else {
        crate::ops::human_session::list(store, filter).await?
    };
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
                    SessionState::Unknown => "unknown",
                    SessionState::Waiting => "waiting",
                    SessionState::Active => "active",
                    SessionState::Ready => "ready",
                    SessionState::Closed => "closed",
                    SessionState::Interrupted => "interrupted",
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

fn report_primary(
    session: &crate::ops::human_session::SessionRecord,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(session)?);
    } else {
        println!(
            "Session {} is {}'s conversation. Open it with `lf session connect {}`.",
            session.id,
            session.work_path.as_deref().unwrap_or("this repository"),
            session.id
        );
    }
    Ok(())
}

async fn complete(id: &str) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    let session = crate::ops::human_session::complete(&store, id).await?;
    match session.kind {
        SessionKind::Conversation => println!(
            "Session {} completed; its provider history remains resumable.",
            session.id
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
