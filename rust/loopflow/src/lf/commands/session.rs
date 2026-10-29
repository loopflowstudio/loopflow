use std::sync::Arc;

use anyhow::{bail, Context};

use serde::{Deserialize, Serialize};

use super::waves::Shape;
use crate::lf::SessionCommand;
use crate::lf::SessionConnectionArgs;
use crate::lf::SessionListArgs;
use crate::ops::human_session::{OpenMode, SessionRecord, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    if let SessionCommand::Serve { terminal, argv } = command {
        return crate::transport::serve(argv, *terminal);
    }
    if let SessionCommand::List(args) = command {
        return list_sessions(args, Shape::Tasks);
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
        SessionCommand::List(_) => unreachable!("lists return before any runtime"),
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
        SessionCommand::Send { id, task, to } => {
            let store = open_shared_store().await?;
            let to = crate::ops::task_ownership::resolve_machine(&store.sqlite, to)?;
            let moved = match (id, task) {
                (_, Some(task)) => {
                    crate::ops::human_session::moves::send_task(&store, task, &to).await?
                }
                (Some(id), None) => crate::ops::human_session::moves::send(&store, id, &to).await?,
                (None, None) => unreachable!("Clap requires a Session or a Task"),
            };
            println!("{}", serde_json::to_string(&moved)?);
            Ok(())
        }
        SessionCommand::Receive {
            check: true, task, ..
        } => {
            let store = open_shared_store().await?;
            if let Some(task) = task {
                let id = crate::durable::TaskId::parse(task)?;
                crate::ops::human_session::moves::known_task(&store, &id, task).await?;
            }
            Ok(())
        }
        SessionCommand::Receive { json, .. } => {
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)?;
            let moved: crate::ops::human_session::moves::Move =
                serde_json::from_str(&input).context("read the moved Session")?;
            let store = open_shared_store().await?;
            let task = moved.task.as_ref().map(|task| task.identifier.clone());
            let session = crate::ops::human_session::moves::receive(&store, moved).await?;
            let Some(session) = session else {
                let task = task.expect("a move without a Session moves a Task");
                if *json {
                    println!("{}", serde_json::json!({ "task": task }));
                } else {
                    println!("Task {task} is here; `lf task run {task}` continues it");
                }
                return Ok(());
            };
            let record = crate::ops::human_session::surface(&store, &session).await?;
            if *json {
                println!("{}", serde_json::to_string(&record)?);
            } else {
                println!(
                    "Session {} is here in {}; `lf resume {}` continues it",
                    record.title, record.cwd, record.id
                );
            }
            Ok(())
        }
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

/// `lf session list --json`: one Session and the Work it belongs to. Wire
/// type; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRow {
    #[serde(flatten)]
    pub session: SessionFacts,
    pub task: Option<crate::durable::TaskId>,
    /// Wave name, as `--wave` takes it.
    pub wave: Option<String>,
}

/// One Session's own facts; `lf task status` lists its Task's Sessions with them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionFacts {
    pub id: crate::id::LfSessionId,
    pub title: String,
    /// The provider its current input launched with; `null` before a first launch.
    pub provider: Option<String>,
    /// A Session a person works in; `false` for a headless Run.
    pub interactive: bool,
    /// What its AgentProcess shows: active, detached, closed or unknown.
    pub state: SessionState,
    /// Ready for a person's input.
    pub available: bool,
    pub cwd: String,
    /// The provider's current AgentProcess, as `lf ps` lists it.
    pub agent_process_id: Option<crate::id::LfProcessId>,
}

impl SessionRow {
    fn of(session: SessionRecord, waves: &std::collections::HashMap<String, String>) -> Self {
        Self {
            task: match &session.work {
                Some(crate::durable::WorkRef::Task(task)) => Some(task.clone()),
                _ => None,
            },
            wave: session
                .wave_id
                .as_ref()
                .and_then(|wave| waves.get(&wave.to_string()).cloned()),
            session: SessionFacts::of(session),
        }
    }
}

impl SessionFacts {
    pub(crate) fn of(session: SessionRecord) -> Self {
        Self {
            id: session.id,
            title: session.title,
            provider: session.provider,
            interactive: session.interactive,
            state: session.state,
            available: session.availability.is_some(),
            cwd: session.cwd,
            agent_process_id: session.agent_process_id,
        }
    }
}

/// `lf session list` (`Shape::Tasks`) and `lf desktop sessions` (`Shape::Desktop`).
pub fn list_sessions(args: &SessionListArgs, shape: Shape) -> anyhow::Result<()> {
    let filter = list_filter(args, &std::env::current_dir()?)?;
    if args.watch {
        return watch_list(filter, shape, super::watch::Transport::Stdio).map(drop);
    }
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        list(&store, args.json || shape == Shape::Desktop, &filter, shape).await
    })
}

/// `lf session list`'s selection: the repository containing `directory`
/// unless `--all`.
pub(crate) fn list_filter(
    args: &SessionListArgs,
    directory: &std::path::Path,
) -> anyhow::Result<crate::session::SessionFilter> {
    Ok(crate::session::SessionFilter {
        available: args.available,
        repo: if args.all {
            None
        } else {
            crate::repository::CanonicalRepo::containing(directory)?.map(|repo| repo.to_string())
        },
        task: args.task.clone(),
        orphan: args.orphan,
        search: args.search.clone(),
        interactive: args.interactive.interactive(),
        history: args.history,
        limit: args.limit,
        offset: args.offset,
        after: args.page.then(|| args.after.clone().unwrap_or_default()),
    })
}

/// What a `lf session list --watch` request can select instead.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionWatchArgs {
    /// Canonical repository; `null` reads every repository.
    repo: Option<String>,
    task: Option<String>,
    search: Option<String>,
    /// `null` selects interactive and headless Sessions.
    interactive: Option<bool>,
}

pub(crate) fn watch_list(
    filter: crate::session::SessionFilter,
    shape: Shape,
    transport: super::watch::Transport,
) -> anyhow::Result<Option<super::watch::Subscription>> {
    let args = SessionWatchArgs {
        repo: filter.repo.clone(),
        task: filter.task.clone(),
        search: filter.search.clone(),
        interactive: filter.interactive,
    };
    super::watch::start(
        args,
        super::watch::Watched::every(super::watch::ACTIVITY_CLOCK),
        move |store, args| {
            let filter = crate::session::SessionFilter {
                repo: args.repo,
                task: args.task,
                search: args.search,
                interactive: args.interactive,
                ..filter.clone()
            };
            async move {
                match store {
                    Some(store) => read_list(&store, &filter, shape).await,
                    None => Ok(serde_json::Value::Array(Vec::new())),
                }
            }
        },
        transport,
    )
}

/// What `lf session list --json` prints: a page with `--page`, else every row.
async fn read_list(
    store: &Arc<Store>,
    filter: &crate::session::SessionFilter,
    shape: Shape,
) -> anyhow::Result<serde_json::Value> {
    let waves = match shape {
        Shape::Desktop => None,
        Shape::Tasks => Some(
            store
                .list_waves(None)
                .await?
                .iter()
                .map(|wave| (wave.id().to_string(), wave.name().to_owned()))
                .collect::<std::collections::HashMap<_, _>>(),
        ),
    };
    let rows = |sessions: Vec<SessionRecord>| -> anyhow::Result<Vec<serde_json::Value>> {
        sessions
            .into_iter()
            .map(|session| match &waves {
                None => serde_json::to_value(session),
                Some(waves) => serde_json::to_value(SessionRow::of(session, waves)),
            })
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    };
    if filter.after.is_none() {
        return Ok(serde_json::to_value(rows(
            crate::ops::human_session::list(store, filter).await?,
        )?)?);
    }
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
    Ok(serde_json::to_value(
        crate::ops::human_session::SessionPage {
            entries: rows(entries)?,
            next,
        },
    )?)
}

async fn list(
    store: &Arc<Store>,
    json: bool,
    filter: &crate::session::SessionFilter,
    shape: Shape,
) -> anyhow::Result<()> {
    if json || filter.after.is_some() {
        println!(
            "{}",
            serde_json::to_string_pretty(&read_list(store, filter, shape).await?)?
        );
        return Ok(());
    }
    let sessions = crate::ops::human_session::list(store, filter).await?;
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
                match &session.account {
                    Some(account) => format!("{}  on {account}", session.title),
                    None => session.title.clone(),
                }
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
