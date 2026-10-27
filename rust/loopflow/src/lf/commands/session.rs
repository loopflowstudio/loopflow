use std::sync::Arc;

use anyhow::{bail, Context};

use crate::lf::SessionCommand;
use crate::lf::SessionConnectionArgs;
use crate::ops::human_session::{OpenMode, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    if let SessionCommand::Serve { terminal, argv } = command {
        return crate::transport::serve(argv, *terminal);
    }
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
        SessionCommand::Serve { .. } => unreachable!("serve owns its process before any runtime"),
        SessionCommand::Attach {
            id,
            observe,
            if_free,
            prepare,
            json,
        } => {
            let store = open_shared_store().await?;
            let session = crate::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
            if *prepare {
                let record = crate::ops::human_session::prepare_attachment(
                    &store, &session, *observe, *if_free,
                )
                .await?;
                if *json {
                    println!("{}", serde_json::to_string_pretty(&record)?);
                } else {
                    let record = record.with_context(|| {
                        format!(
                            "Nothing is running to attach to; `lf resume {}` reopens its history",
                            session.id
                        )
                    })?;
                    println!(
                        "{}",
                        shlex::try_join(record.open_argv.iter().map(String::as_str))?
                    );
                }
                return Ok(());
            }
            let owner = store
                .sqlite
                .session_transport(&session.id)?
                .context("Session has no retained execution")?;
            crate::transport::attach(&owner, *observe, !*if_free)
        }
        SessionCommand::Steer { id, message } => {
            let store = open_shared_store().await?;
            let author = crate::config::participant_name()?
                .unwrap_or_else(|| "attribution unresolved".into());
            let Some(id) = id else {
                // Global `--repo` has already entered the selected repository.
                let steer = match cli.wave.as_deref() {
                    Some(wave) => {
                        crate::lf::commands::waves::save_steer(&store, wave, &author, message)
                            .await?
                    }
                    None => {
                        let root = crate::repo::find_repo_root()?;
                        let repo = crate::repository::CanonicalRepo::discover(&root)?;
                        store
                            .sqlite
                            .save_repository_steer(&repo, &author, message)?
                    }
                };
                println!("Saved {}", steer.id);
                return Ok(());
            };
            let session = crate::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
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
            all: true,
            connection,
            ..
        } => {
            let store = open_shared_store().await?;
            let resumed = crate::ops::human_session::restore::resume_all(&store).await?;
            if connection.json || cli.json {
                println!("{}", serde_json::to_string_pretty(&resumed)?);
            } else {
                println!("{}", crate::ops::human_session::restore::render(&resumed));
            }
            Ok(())
        }
        SessionCommand::Connect {
            id,
            connection,
            takeover,
            try_open,
            all: false,
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
            entries.last().map(|session| session.id.to_string())
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
        return Ok(());
    }
    // Sessions copied from given-back Machines are history, never available here.
    let copied = if filter.available || filter.orphan {
        Vec::new()
    } else {
        copied_sessions(store, filter)?
    };
    if sessions.is_empty() && copied.is_empty() {
        println!("No Sessions.");
    } else {
        for session in sessions {
            println!(
                "{}  {:<7} {}  {}",
                session.id,
                match session.state {
                    _ if session.availability.is_some() => "available",
                    SessionState::Unknown => "unknown",
                    SessionState::Active => "active",
                    SessionState::Detached => "detached",
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
        for session in copied {
            println!(
                "{}  {:<7} {}  from {}",
                session.id,
                if session.completed_at.is_some() {
                    "closed"
                } else {
                    "unknown"
                },
                session.title,
                session.origin_name()
            );
        }
    }
    Ok(())
}

/// Copied Sessions the filter's Task or repository selects; none when it names
/// one this Machine does not know.
fn copied_sessions(
    store: &Store,
    filter: &crate::session::SessionFilter,
) -> anyhow::Result<Vec<crate::machine_records::CopiedSession>> {
    let task = match &filter.task {
        Some(selector) => match store.sqlite.resolve_task_id(selector, None)? {
            Some(task) => Some(task),
            None => return Ok(Vec::new()),
        },
        None => None,
    };
    let repository = match &filter.repo {
        Some(repo) => match store.sqlite.repository_id(repo)? {
            Some(id) => Some(id),
            None => return Ok(Vec::new()),
        },
        None => None,
    };
    Ok(store
        .sqlite
        .copied_sessions(task.as_ref(), repository.as_ref())?)
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
        let session = crate::ops::human_session::session_by_id(&store, &id)
            .await?
            .context("Session not found")?;
        let session = crate::ops::human_session::admit_workspace(&store, session).await?;
        let id = session.id;
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
