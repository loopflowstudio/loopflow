use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};

use serde::{Deserialize, Serialize};

use super::write::SessionOpenIntent;
use crate::lf::SessionCommand;
use crate::lf::SessionConnectionArgs;
use crate::lf::SessionListArgs;
use crate::ops::human_session::{SessionRecord, SessionState};
use crate::session_record::SessionTitleSource;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    if let SessionCommand::Serve { terminal, argv } = command {
        return crate::transport::serve(argv, *terminal);
    }
    if let SessionCommand::Card { id, .. } = command {
        return super::screens::print(super::read::Read::SessionCard { id: id.clone() });
    }
    if let SessionCommand::List(args) = command {
        return list_sessions(args);
    }
    if let SessionCommand::Usage(args) = command {
        return super::history::usage(args);
    }
    if let SessionCommand::Retention { json } = command {
        return super::history_transcript::retention(*json);
    }
    match command {
        SessionCommand::History { cmd: Some(cmd), .. } => return super::history::run(cmd),
        SessionCommand::History {
            id: None,
            feed,
            cmd: None,
            ..
        } => return super::activity::run(feed),
        _ => {}
    }
    let runtime = tokio::runtime::Runtime::new()?;
    if matches!(
        command,
        SessionCommand::Resume { .. } | SessionCommand::New { .. }
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
        SessionCommand::Serve { .. } => {
            unreachable!("serve owns its process before any runtime")
        }
        SessionCommand::Steer { id, message, json } => {
            use crate::engine::session::SteerScope;
            let store = open_shared_store().await?;
            // Global `--repo` has already entered the selected repository.
            let scope = match (id.as_deref(), cli.wave.as_deref()) {
                (Some(id), _) => SteerScope::Session(id),
                (None, Some(wave)) => SteerScope::Wave(wave),
                (None, None) => SteerScope::Repository,
            };
            let directory = std::env::current_dir()?;
            let steer = crate::engine::session::steer(&store, &directory, scope, message).await?;
            if *json {
                println!("{}", serde_json::to_string(&steer)?);
            } else {
                println!("Saved {}", steer.id);
            }
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
                .map(crate::id::AgentProcessId::parse)
                .transpose()?;
            crate::ops::human_session::observe_program_status(
                &store,
                id,
                terminal,
                agent_process.as_ref(),
            )
            .await
        }
        SessionCommand::History { id: None, .. } => {
            unreachable!("the history feed and its commands return before any runtime")
        }
        SessionCommand::History {
            id: Some(id),
            feed,
            after,
            ..
        } => {
            let store = open_shared_store().await?;
            let Some(session) = crate::ops::human_session::session_by_id(&store, id).await? else {
                bail!("Session {id} was not found");
            };
            let events = store
                .sqlite
                .session_history(&session.id, *after, feed.limit)?;
            if feed.json {
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
        SessionCommand::Card { .. }
        | SessionCommand::List(_)
        | SessionCommand::Usage(_)
        | SessionCommand::Retention { .. } => {
            unreachable!("reads return before any runtime")
        }
        SessionCommand::Move {
            selector,
            all,
            json,
        } => {
            use crate::ops::human_session::moves_bulk::{move_sessions, parse_target};
            let target = parse_target(cli.agent.as_deref())?;
            let filter = crate::session::SessionFilter {
                blocked: selector.blocked,
                on: selector.on.clone(),
                repo: if *all {
                    None
                } else {
                    crate::repository::CanonicalRepo::containing(&std::env::current_dir()?)?
                        .map(|repo| repo.to_string())
                },
                ..Default::default()
            };
            let store = open_shared_store().await?;
            let report = move_sessions(&store, &filter, &target).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                report.print();
            }
            Ok(())
        }
        SessionCommand::Resume {
            id,
            connection,
            fork,
        } => {
            resume(
                cli,
                connection,
                connection.target(cli, id.as_deref())?,
                *fork,
            )
            .await
        }
        SessionCommand::New {
            id,
            connection,
            request_id,
        } => {
            new_thread(
                cli,
                connection,
                connection.target(cli, id.as_deref())?,
                request_id.as_deref(),
            )
            .await
        }
        SessionCommand::Rename { .. } => unreachable!("rename uses shared mutation dispatch"),
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
        SessionCommand::Bind { .. } => unreachable!("binding uses shared mutation dispatch"),
    }
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
    pub agent_process_id: Option<crate::id::AgentProcessId>,
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

/// `lf session list`.
pub fn list_sessions(args: &SessionListArgs) -> anyhow::Result<()> {
    let filter = list_filter(args, &std::env::current_dir()?)?;
    if args.subscribe {
        return subscribe_list(filter, super::subscribe::Transport::Stdio).map(drop);
    }
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        list(&store, args.json, &filter).await
    })
}

/// `lf session list`'s selection: the repository containing `directory`
/// unless `--all`.
pub(crate) fn list_filter(
    args: &SessionListArgs,
    directory: &std::path::Path,
) -> anyhow::Result<crate::session::SessionFilter> {
    Ok(crate::session::SessionFilter {
        blocked: args.selector.blocked,
        on: args.selector.on.clone(),
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

/// What a `lf session list --subscribe` request can select instead.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SessionWatchArgs {
    /// Canonical repository; `null` reads every repository.
    repo: Option<String>,
    task: Option<String>,
    search: Option<String>,
    /// `null` selects both Sessions and Runs.
    interactive: Option<bool>,
}

pub(crate) fn subscribe_list(
    filter: crate::session::SessionFilter,
    transport: super::subscribe::Transport,
) -> anyhow::Result<Option<super::subscribe::Subscription>> {
    let args = SessionWatchArgs {
        repo: filter.repo.clone(),
        task: filter.task.clone(),
        search: filter.search.clone(),
        interactive: filter.interactive,
    };
    super::subscribe::start(
        args,
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
                    Some(store) => read_list(&store, &filter).await,
                    None => Ok(serde_json::Value::Array(Vec::new())),
                }
            }
        },
        transport,
    )
}

async fn rename_session(
    id: &str,
    name: &[String],
    source: SessionTitleSource,
) -> anyhow::Result<SessionRecord> {
    let requested = required_text(name, "Session name")?;
    let store = open_shared_store().await?;
    crate::ops::human_session::rename(&store, id, &requested, source).await
}

/// `lf session rename <id> <name>`: the renamed Session.
pub(crate) fn renamed(id: &str, name: &str) -> anyhow::Result<SessionRecord> {
    tokio::runtime::Runtime::new()?.block_on(rename_session(
        id,
        &[name.to_owned()],
        SessionTitleSource::Human,
    ))
}

/// `lf session bind <id> --task <task> --json`: the bound Session, or with
/// `preview` (`--dry-run`) the Task a bind would assign, assigning nothing.
pub(crate) fn bound(id: &str, task: &str, preview: bool) -> anyhow::Result<serde_json::Value> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        Ok(if preview {
            serde_json::to_value(
                crate::ops::human_session::preview_binding(&store, id, task).await?,
            )?
        } else {
            serde_json::to_value(crate::ops::human_session::bind(&store, id, task).await?)?
        })
    })
}

/// The report `lf session move --all --json` prints for the selector and agent.
pub(crate) fn moved(
    blocked: bool,
    on: Option<&str>,
    agent: &str,
) -> anyhow::Result<crate::ops::human_session::moves_bulk::MoveReport> {
    use crate::ops::human_session::moves_bulk::{move_sessions, parse_target};
    let target = parse_target(Some(agent))?;
    let filter = crate::session::SessionFilter {
        blocked,
        on: on.map(str::to_owned),
        repo: None,
        ..Default::default()
    };
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        move_sessions(&store, &filter, &target).await
    })
}

/// What `lf session list --json` prints for `filter`.
pub(crate) fn read_once(
    filter: &crate::session::SessionFilter,
) -> anyhow::Result<serde_json::Value> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        read_list(&store, filter).await
    })
}

/// What `lf session list --json` prints: a page with `--page`, else every
/// Session record.
async fn read_list(
    store: &Arc<Store>,
    filter: &crate::session::SessionFilter,
) -> anyhow::Result<serde_json::Value> {
    if filter.after.is_none() {
        return Ok(serde_json::to_value(
            crate::ops::human_session::list(store, filter).await?,
        )?);
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
            entries: entries
                .into_iter()
                .map(serde_json::to_value)
                .collect::<Result<_, _>>()?,
            next,
        },
    )?)
}

async fn list(
    store: &Arc<Store>,
    json: bool,
    filter: &crate::session::SessionFilter,
) -> anyhow::Result<()> {
    if json || filter.after.is_some() {
        println!(
            "{}",
            serde_json::to_string_pretty(&super::subscribe::selected_answer(
                &read_list(store, filter).await?
            )?)?
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

async fn resume(
    cli: &crate::lf::Cli,
    connection: &SessionConnectionArgs,
    target: crate::ops::human_session::SessionTarget<'_>,
    fork: bool,
) -> anyhow::Result<()> {
    // Prepared connections have already returned through shared mutation dispatch.
    let message = &connection.message;
    anyhow::ensure!(
        message.is_none() || cli.batch,
        "a next-turn message requires -b"
    );
    anyhow::ensure!(
        !cli.batch || message.is_some(),
        "a headless connection requires --message TEXT"
    );
    anyhow::ensure!(
        cli.batch || cli.output_schema.is_none(),
        "response schemas require a headless Run (-b)"
    );
    let progress = crate::startup::Startup::new("Finding conversation");
    let store = open_shared_store().await?;
    let cwd = std::env::current_dir()?;
    drop(progress);
    if let Some(message) = message {
        let id = select(&store, &cwd, target).await?;
        let session = crate::ops::human_session::session_by_id(&store, &id)
            .await?
            .context("Session not found")?;
        let lock_id = session.id.to_string();
        let launch = tokio::task::spawn_blocking(move || {
            crate::ops::human_session::lock_session_process(&lock_id)
        })
        .await??;
        let session = crate::ops::human_session::admit_workspace(&store, session).await?;
        drop(launch);
        let id = session.id;
        return tokio::task::block_in_place(|| crate::lf::commands::run::resume(&id, message, cli));
    }
    let session = reopen(
        &store,
        &cwd,
        target,
        connection,
        fork,
        connection.json || cli.json,
    )
    .await?;
    print_opened(connection.json || cli.json, &session)
}

/// The conversation `lf resume` run from `directory` selects.
async fn select(
    store: &Arc<Store>,
    directory: &Path,
    target: crate::ops::human_session::SessionTarget<'_>,
) -> anyhow::Result<String> {
    let selected = crate::ops::human_session::select_connection(
        store,
        directory,
        target,
        SessionOpenIntent::Resume,
    )
    .await?;
    if selected.started {
        crate::journal::request::diagnostic(format_args!(
            "Nothing to resume here; starting a new conversation."
        ));
    }
    Ok(selected.id)
}

/// `lf session resume` without a message: the selected conversation, opened
/// here, and its provider started unless `--prepare` was requested.
async fn reopen(
    store: &Arc<Store>,
    directory: &Path,
    target: crate::ops::human_session::SessionTarget<'_>,
    connection: &SessionConnectionArgs,
    fork: bool,
    json: bool,
) -> anyhow::Result<SessionRecord> {
    let intent = connection.intent();
    if intent != SessionOpenIntent::Resume {
        anyhow::ensure!(!fork, "non-replacing opening cannot fork a conversation");
        let selected =
            crate::ops::human_session::select_connection(store, directory, target, intent).await?;
        return crate::ops::human_session::open_without_replacing(
            store,
            &selected.id,
            intent,
            !connection.prepare,
            json,
        )
        .await;
    }
    let id = select(store, directory, target).await?;
    crate::ops::human_session::open(store, directory, &id, fork, !connection.prepare, json).await
}

/// What `lf session resume|new --prepare` run from `directory` records and
/// prints: the conversation and the command that opens it, with no provider
/// started.
pub(crate) fn prepared(cli: &crate::lf::Cli, directory: &Path) -> anyhow::Result<SessionRecord> {
    let Some(crate::lf::Commands::Session { cmd }) = &cli.command else {
        bail!("not a Session command");
    };
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        match cmd {
            SessionCommand::Resume {
                id,
                connection,
                fork,
            } if connection.prepare => {
                let target = connection.target(cli, id.as_deref())?;
                reopen(&store, directory, target, connection, *fork, false).await
            }
            SessionCommand::New {
                id,
                connection,
                request_id,
            } if connection.prepare => {
                let target = connection.target(cli, id.as_deref())?;
                restarted(
                    &store,
                    directory,
                    target,
                    false,
                    false,
                    request_id.as_deref(),
                    cli.agent.as_deref(),
                )
                .await
            }
            _ => bail!("only `--prepare` opens a Session without starting its provider"),
        }
    })
}

/// `lf session new`: the scope's conversation continues on a fresh provider
/// thread, seeded with what the last thread knew.
async fn new_thread(
    cli: &crate::lf::Cli,
    connection: &SessionConnectionArgs,
    target: crate::ops::human_session::SessionTarget<'_>,
    request_id: Option<&str>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        connection.message.is_none(),
        "`lf session new` starts an interactive thread; continue it with `lf -b resume --message`"
    );
    let store = open_shared_store().await?;
    let record = restarted(
        &store,
        &std::env::current_dir()?,
        target,
        true,
        connection.json || cli.json,
        request_id,
        cli.agent.as_deref(),
    )
    .await?;
    print_opened(connection.json || cli.json, &record)
}

/// The conversation `lf session new` run from `directory` selects, on a fresh
/// provider thread started unless `launch` is false.
async fn restarted(
    store: &Arc<Store>,
    directory: &Path,
    target: crate::ops::human_session::SessionTarget<'_>,
    launch: bool,
    json: bool,
    request_id: Option<&str>,
    agent: Option<&str>,
) -> anyhow::Result<SessionRecord> {
    let selected = crate::ops::human_session::select_connection(
        store,
        directory,
        target,
        SessionOpenIntent::Resume,
    )
    .await?;
    let request_id = request_id
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let next = crate::ops::human_session::new_thread(
        store,
        &selected.id,
        agent,
        launch,
        json,
        &request_id,
    )
    .await?;
    if let Some(result) = store
        .sqlite
        .creation_receipt(&request_id)?
        .and_then(|r| r.result)
    {
        return Ok(serde_json::from_value(result)?);
    }
    crate::ops::human_session::surface(store, &next).await
}

/// Finish a successful Session connection after its thin relay has returned.
pub fn print_returned(id: &str) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        let session = crate::ops::human_session::session_by_id(&store, id)
            .await?
            .context("Session not found")?;
        let record = crate::ops::human_session::surface(&store, &session).await?;
        println!("{}", serde_json::to_string_pretty(&record)?);
        Ok(())
    })
}

fn print_opened(json: bool, session: &SessionRecord) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(session)?);
    }
    Ok(())
}

async fn rename(
    id: &str,
    name: &[String],
    suggest: bool,
    json: bool,
    output: &mut dyn std::io::Write,
) -> anyhow::Result<()> {
    let source = if suggest {
        SessionTitleSource::Generated
    } else {
        SessionTitleSource::Human
    };
    let session = rename_session(id, name, source).await?;
    if json {
        writeln!(output, "{}", serde_json::to_string_pretty(&session)?)?;
    } else if suggest && session.title_source == SessionTitleSource::Human {
        writeln!(
            output,
            "Session {} keeps its human-assigned name {:?}.",
            session.id, session.title
        )?;
    } else {
        writeln!(
            output,
            "Session {} is named {:?}.",
            session.id, session.title
        )?;
    }
    Ok(())
}

fn required_text(args: &[String], label: &str) -> anyhow::Result<String> {
    let text = args.join(" ").trim().to_string();
    if text.is_empty() {
        return Err(crate::engine::Error::new(
            crate::engine::ErrorCode::Invalid,
            format!("{label} cannot be empty"),
        )
        .into());
    }
    Ok(text)
}

pub(crate) async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    Ok(Arc::new(
        open_store(&config)
            .await
            .context("open the shared Loopflow store")?,
    ))
}

pub(super) fn is_mutation(command: &SessionCommand) -> bool {
    matches!(
        command,
        SessionCommand::Rename { .. }
            | SessionCommand::Bind { .. }
            | SessionCommand::Resume {
                connection: SessionConnectionArgs { prepare: true, .. },
                ..
            }
            | SessionCommand::New {
                connection: SessionConnectionArgs { prepare: true, .. },
                ..
            }
    )
}

pub(super) fn mutate(
    cli: &crate::lf::Cli,
    directory: &Path,
    command: &SessionCommand,
    io: &mut super::mutation::CommandIo<'_>,
) -> anyhow::Result<()> {
    if let SessionCommand::Resume { connection, .. } | SessionCommand::New { connection, .. } =
        command
    {
        anyhow::ensure!(
            connection.message.is_none(),
            "prepared Sessions do not accept a message"
        );
        let session = prepared(cli, directory)?;
        if connection.json || cli.json {
            writeln!(io.output, "{}", serde_json::to_string_pretty(&session)?)?;
        } else {
            writeln!(
                io.output,
                "{}",
                shlex::try_join(session.open_argv.iter().map(String::as_str))?
            )?;
        }
        return Ok(());
    }
    tokio::runtime::Runtime::new()?.block_on(async {
        match command {
            SessionCommand::Rename {
                id,
                name,
                suggest,
                json,
            } => rename(id, name, *suggest, *json, io.output).await,
            SessionCommand::Bind {
                id,
                task,
                dry_run,
                json,
            } => {
                let store = open_shared_store().await?;
                if *dry_run {
                    let preview =
                        crate::ops::human_session::preview_binding(&store, id, task).await?;
                    if *json {
                        writeln!(io.output, "{}", serde_json::to_string_pretty(&preview)?)?;
                    } else {
                        writeln!(
                            io.output,
                            "{} → {} ({}) [{}]. Not assigned.",
                            preview.session_id, preview.identifier, preview.title, preview.task_id
                        )?;
                    }
                    return Ok(());
                }
                let session = crate::ops::human_session::bind(&store, id, task).await?;
                if *json {
                    writeln!(io.output, "{}", serde_json::to_string_pretty(&session)?)?;
                } else {
                    writeln!(
                        io.output,
                        "Session {} belongs to {}.",
                        session.id,
                        session.work_path.as_deref().unwrap_or(task)
                    )?;
                }
                Ok(())
            }
            _ => unreachable!("is_mutation selects Session mutations"),
        }
    })
}
