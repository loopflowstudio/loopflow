use std::sync::Arc;

use anyhow::{bail, Context};

use crate::lf::SessionCommand;
use crate::lf::SessionConnectionArgs;
use crate::ops::human_session::{OpenMode, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    if matches!(
        command,
        SessionCommand::Connect {
            connection: SessionConnectionArgs { prepare: false, .. },
            ..
        }
    ) {
        let cwd = crate::repo::working_directory()?;
        let argv = std::env::args().collect::<Vec<_>>();
        crate::journal::with_runtime(&cwd, &argv, || runtime.block_on(run_async(command, cli)))
    } else {
        runtime.block_on(run_async(command, cli))
    }
}

async fn run_async(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    match command {
        SessionCommand::Attach { id, observe } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
            let owner = store
                .sqlite
                .session_transport(&session.id)?
                .context("Session has no retained execution")?;
            crate::transport::attach(&owner, *observe)
        }
        SessionCommand::Steer { id, message } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
            let author = crate::config::participant_name()?
                .unwrap_or_else(|| "attribution unresolved".into());
            let steer = store
                .sqlite
                .save_session_steer(&session.id, &author, message)?;
            println!("Saved {}", steer.id);
            Ok(())
        }
        SessionCommand::Steers {
            id,
            json,
            retry,
            after,
            limit,
            source,
        } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
            if let Some(steer) = retry {
                store
                    .sqlite
                    .retry_session_steer(&session.id, &crate::id::SteerId::parse(steer)?)?;
            }
            if let Some(source) = source {
                let steer = store
                    .sqlite
                    .session_steer_source(&session.id, &crate::id::SteerId::parse(source)?)?;
                println!("{}", serde_json::to_string_pretty(&steer)?);
                return Ok(());
            }
            let status = store.sqlite.session_steers(&session.id, *after, *limit)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("{}; {} pending", status.capability, status.pending);
                if let Some(after) = status.next_after {
                    println!("Next page: --after {after}");
                }
                for row in status.items {
                    println!("{} {}", row.steer.id, row.evidence);
                }
            }
            Ok(())
        }
        SessionCommand::ObserveStatus {
            id,
            terminal,
            agent_process,
        } => {
            let store = open_shared_store().await?;
            let agent_process = agent_process
                .as_deref()
                .map(crate::id::LfProcessId::parse)
                .transpose()?;
            crate::ops::human_session::observe_program_status(
                &store,
                id,
                terminal,
                agent_process.as_ref(),
            )
            .await
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
            available,
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
                    available: *available,
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
        SessionCommand::Connect {
            id,
            connection,
            takeover,
            try_open,
        } => {
            connect(
                cli,
                connection,
                connection.target(cli, id.as_deref())?,
                OpenMode::from_flags(*takeover, *try_open),
            )
            .await
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
                    _ if session.attention.is_some() => "available",
                    SessionState::Unknown => "unknown",
                    SessionState::Active => "active",
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

async fn connect(
    cli: &crate::lf::Cli,
    connection: &SessionConnectionArgs,
    target: crate::ops::human_session::SessionTarget<'_>,
    mode: OpenMode,
) -> anyhow::Result<()> {
    let SessionConnectionArgs {
        message,
        prepare,
        json,
        ..
    } = connection;
    anyhow::ensure!(
        !*prepare || message.is_none(),
        "--prepare cannot be used with --message"
    );
    anyhow::ensure!(
        message.is_none() || cli.batch,
        "a next-turn message requires -b"
    );
    anyhow::ensure!(
        !cli.batch || *prepare || message.is_some(),
        "a headless connection requires --message TEXT"
    );
    anyhow::ensure!(
        *prepare || cli.batch || cli.output_schema.is_none(),
        "response schemas require a headless Session (-b)"
    );
    let progress = crate::startup::Startup::new("Finding conversation");
    let store = open_shared_store().await?;
    let id = crate::ops::human_session::select_connection(
        &store,
        &std::env::current_dir()?,
        target,
        connection.fresh,
    )
    .await?;
    drop(progress);
    if let Some(message) = message {
        let id = crate::ops::human_session::session_by_id(&store, &id)
            .await?
            .context("Session not found")?
            .id;
        return tokio::task::block_in_place(|| crate::lf::commands::run::resume(&id, message, cli));
    }
    let session = crate::ops::human_session::open(&store, &id, mode, !*prepare).await?;
    if *json || cli.json {
        println!("{}", serde_json::to_string_pretty(&session)?);
    } else if *prepare {
        println!(
            "{}",
            shlex::try_join(session.open_argv.iter().map(String::as_str))?
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
