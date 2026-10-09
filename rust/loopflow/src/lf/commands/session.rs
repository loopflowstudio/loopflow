use std::sync::Arc;

use anyhow::{bail, Context};

use crate::lf::SessionCommand;
use crate::ops::human_session::{OpenMode, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let worktree = match command {
        SessionCommand::Open { json: false, .. }
        | SessionCommand::Resume { .. }
        | SessionCommand::ServeConversation { .. } => Some(crate::repo::working_directory()?),
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
        SessionCommand::ObserveStatus {
            id,
            terminal,
            generation,
        } => {
            let store = open_shared_store().await?;
            crate::ops::human_session::observe_program_status(&store, id, terminal, *generation)
                .await
        }
        SessionCommand::Resume { id, message } => {
            anyhow::ensure!(
                message.is_none(),
                "a resume message is a headless turn: lf -b session resume ID MESSAGE"
            );
            let store = open_shared_store().await?;
            let id = crate::ops::human_session::select_connection(
                &store,
                &std::env::current_dir()?,
                id.as_deref(),
            )
            .await?;
            open(&store, &id, false, OpenMode::Refuse).await
        }
        SessionCommand::History {
            id,
            json,
            after,
            limit,
        } => {
            let store = open_shared_store().await?;
            let Some(session) = crate::ops::human_session::session_by_id(&store, id).await? else {
                bail!("Session {id} was not found");
            };
            let events = store.sqlite.session_history(&session.id, *after, *limit)?;
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
            waiting,
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
                &crate::session::SessionFilter {
                    waiting: *waiting,
                    repo: if *all {
                        None
                    } else {
                        crate::repository::CanonicalRepo::current()?.map(|repo| repo.to_string())
                    },
                    task: task.clone(),
                    orphan: *orphan,
                    search: search.clone(),
                    interactive: interactive.interactive(),
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
            let store = open_shared_store().await?;
            open(&store, id, *json, OpenMode::from_flags(*replace, *try_open)).await
        }
        SessionCommand::Ensure {
            wave,
            task,
            choose,
            json,
        } => {
            use crate::ops::human_session::primary;
            let store = open_shared_store().await?;
            let repo = crate::repo::find_repo_root()?;
            let session = match task {
                Some(task) => primary::ensure_task(&store, &repo, task, choose.as_deref()).await?,
                None => primary::ensure(&store, &repo, wave.as_deref()).await?,
            };
            report_primary(&session, *json)
        }
        SessionCommand::Replace { id, json } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::primary::replace(&store, id).await?;
            report_primary(&session, *json)
        }
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
                        preview.session_id, preview.identifier, preview.title, preview.task_id
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
        SessionCommand::ServeConversation { input } => {
            let store = open_shared_store().await?;
            crate::ops::human_session::serve_conversation(&store, input).await
        }
    }
}

async fn list(
    store: &Arc<Store>,
    json: bool,
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
        let mut entries = crate::ops::human_session::list(store, &selection).await?;
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
    let sessions = crate::ops::human_session::list(store, filter).await?;
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
                    _ if session.attention.is_some() => "waiting",
                    SessionState::Unknown => "unknown",
                    SessionState::Active => "active",
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

async fn open(
    store: &crate::store::SharedStore,
    id: &str,
    json: bool,
    mode: OpenMode,
) -> anyhow::Result<()> {
    let session = crate::ops::human_session::open(store, id, mode, !json).await?;
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
