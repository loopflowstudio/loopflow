use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};

use serde::{Deserialize, Serialize};

use crate::lf::SessionCommand;
use crate::lf::SessionConnectionArgs;
use crate::lf::SessionListArgs;
use loopflow_execution::ops::human_session::{
    ConversationLaunch, Opening, SessionEnd, SessionOutlook, SessionRecord, SessionStarts,
    SessionState,
};
use loopflow_execution::session_record::SessionTitleSource;
use loopflow_execution::store::{open_store, storage_config_from_env, Store};

/// Run a `lf session` command. Opening a conversation whose input starts fresh
/// returns its launch, which the caller runs in this process.
pub fn run(
    command: &SessionCommand,
    cli: &crate::lf::Cli,
) -> anyhow::Result<Option<ConversationLaunch>> {
    match command {
        SessionCommand::Resume {
            id,
            connection,
            fork,
            request_id,
        } => tokio::runtime::Runtime::new()?.block_on(resume(
            cli,
            connection,
            connection.target(cli, id.as_deref())?,
            *fork,
            request_id.as_deref(),
        )),
        SessionCommand::New {
            id,
            connection,
            request_id,
        } => tokio::runtime::Runtime::new()?.block_on(new_thread(
            cli,
            connection,
            connection.target(cli, id.as_deref())?,
            request_id.as_deref(),
        )),
        _ => run_command(command, cli).map(|()| None),
    }
}

fn run_command(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    match command {
        SessionCommand::Card { id, .. } => {
            super::screens::print(super::read::Read::SessionCard { id: id.clone() })
        }
        SessionCommand::List(args) => list_sessions(args),
        SessionCommand::Usage(args) => super::history::usage(args),
        SessionCommand::Retention { json } => super::history_transcript::retention(*json),
        SessionCommand::History { cmd: Some(cmd), .. } => super::history::run(cmd),
        SessionCommand::History {
            id: None,
            feed,
            cmd: None,
            ..
        } => super::activity::run(feed),
        _ => tokio::runtime::Runtime::new()?.block_on(run_async(command, cli)),
    }
}

/// `lf steer`: save `message` for a Session or a Task named by `target`, or
/// with none for the Wave or repository the invocation selected. A target
/// that names no Session is a Task. `--json` prints what the server answers:
/// `task.steer` the Task's thread with the entry, `session.steer` the steer.
pub fn steer(
    cli: &crate::lf::Cli,
    target: Option<&str>,
    message: &str,
    json: bool,
) -> anyhow::Result<()> {
    use loopflow_execution::engine::session::SteerScope;
    let directory = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    let (store, session) = runtime.block_on(async {
        let store = open_shared_store().await?;
        let session = match target {
            Some(id) => loopflow_execution::ops::human_session::session_by_id(&store, id).await?,
            None => None,
        };
        anyhow::Ok((store, session))
    })?;
    if let (Some(session), true) = (&session, cli.batch) {
        if steer_turn(cli, &runtime, &store, session, &directory, message)? {
            return Ok(());
        }
    }
    let scope = match (&session, target, cli.wave.as_deref()) {
        (Some(session), _, _) => SteerScope::Session(&session.id),
        (None, Some(issue), wave) => {
            let unknown = || format!("{issue} names no Session and no Task");
            let repo = super::mutation::selected_task_repository(cli, &directory, Some(issue))
                .with_context(unknown)?;
            let thread = runtime.block_on(async {
                let (store, task) =
                    loopflow_execution::ops::task::resolve_saved_task(&repo, wave, issue)
                        .await
                        .with_context(unknown)?;
                let scope = SteerScope::Task { issue, wave };
                let steer =
                    loopflow_execution::engine::session::steer(&store, &repo, scope, message)
                        .await?;
                anyhow::Ok((steer, store.task_comments(&task.id)?))
            })?;
            match json {
                true => println!("{}", serde_json::to_string(&thread.1)?),
                false => println!("Saved {}", thread.0.id),
            }
            return Ok(());
        }
        (None, None, Some(wave)) => SteerScope::Wave(wave),
        // Global `--repo` has already entered the selected repository.
        (None, None, None) if cli.repo.is_some() => SteerScope::Repository,
        (None, None, None) => anyhow::bail!(
            "name who to steer: a Session ID, a Task ID, `--wave WAVE` or `--repo NAME_OR_PATH`"
        ),
    };
    let steer = runtime.block_on(loopflow_execution::engine::session::steer(
        &store, &directory, scope, message,
    ))?;
    match (json, &session) {
        (true, _) => println!("{}", serde_json::to_string(&steer)?),
        (false, Some(session)) => println!(
            "Saved {}; {}",
            steer.id,
            steer_reader(&store.sqlite, session)?
        ),
        (false, None) => println!("Saved {}", steer.id),
    }
    Ok(())
}

/// Who reads a steer saved for `session`, with no turn of its own to run it.
fn steer_reader(
    sqlite: &loopflow_execution::store::sqlite::SqliteStore,
    session: &loopflow_execution::session::LfSession,
) -> anyhow::Result<String> {
    let id = &session.id;
    Ok(
        if loopflow_execution::ops::human_session::agent_running(sqlite, session)? {
            format!("{id} reads it at its next boundary")
        } else if session.interactive {
            format!("the Machine server starts {id} to read it")
        } else if session.graph_lf_process_id.is_some() {
            format!("{id} is a finished worker; nothing reads it until its Graph continues it")
        } else {
            format!("{id} is a finished batch; nothing reads it")
        },
    )
}

/// `lf -b steer SESSION TEXT`: a Session whose agent is stopped reads its
/// unread steers, this one last, in one headless turn here, and the reply
/// prints. False when there is no turn to run here: a running Session reads
/// the steer at its next boundary, and a finished worker only when its own Graph
/// continues it, so the caller only saves it.
fn steer_turn(
    cli: &crate::lf::Cli,
    runtime: &tokio::runtime::Runtime,
    store: &Arc<Store>,
    session: &loopflow_execution::session::LfSession,
    directory: &Path,
    message: &str,
) -> anyhow::Result<bool> {
    use loopflow_execution::ops::human_session as human;
    use loopflow_execution::transport::LaunchInput;
    let kind = LaunchInput {
        batch: true,
        interactive: false,
        has_prompt: true,
    }
    .kind();
    // One process per Session: this turn holds the launch lock to its end.
    let _launch = human::lock_session_process(&session.id)?;
    let sqlite = &store.sqlite;
    if human::agent_running(sqlite, session)?
        || !session.interactive && !super::run::continues_own_step(session)
    {
        return Ok(false);
    }
    let session = runtime.block_on(human::admit_workspace(store, session.clone()))?;
    let author = loopflow_execution::config::participant_name_in(directory)?
        .unwrap_or_else(|| "attribution unresolved".into());
    let steers = sqlite.take_session_steers(&session.id, &author, message)?;
    let prompt = loopflow_execution::steer::render_complete(&steers);
    super::run::resume(sqlite, &session, &prompt, cli, kind)?;
    Ok(true)
}

async fn run_async(command: &SessionCommand, cli: &crate::lf::Cli) -> anyhow::Result<()> {
    match command {
        SessionCommand::Steers {
            id,
            json,
            after,
            limit,
            source,
        } => {
            let store = open_shared_store().await?;
            let session = loopflow_execution::ops::human_session::session_by_id(&store, id)
                .await?
                .context("Session not found")?;
            if let Some(source) = source {
                let steer = store.sqlite.session_steer_source(
                    &session.id,
                    &loopflow_model::id::SteerId::parse(source)?,
                )?;
                println!("{}", serde_json::to_string_pretty(&steer)?);
                return Ok(());
            }
            let status = store.sqlite.session_steers(&session.id, *after, *limit)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("{} pending", status.pending);
                if let Some(after) = status.next_after {
                    println!("Next page: --after {after}");
                }
                for row in status.items {
                    let state = match row.delivered_at {
                        Some(_) => "delivered",
                        None => "pending",
                    };
                    println!("{} {state}", row.steer.id);
                }
            }
            Ok(())
        }
        SessionCommand::ObserveStatus {
            id,
            terminal,
            agent_process,
            stream,
            sequence,
            json,
        } => {
            let records = serde_json::from_reader(std::io::stdin().lock())
                .context("invalid Program Status snapshot")?;
            let recorded = status_recorded(
                id,
                terminal,
                agent_process.as_deref(),
                stream,
                *sequence,
                &records,
            )
            .await?;
            if *json {
                println!("{}", serde_json::to_string(&recorded)?);
            }
            Ok(())
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
            let Some(session) =
                loopflow_execution::ops::human_session::session_by_id(&store, id).await?
            else {
                bail!("Session {id} was not found");
            };
            // Turns change once, when they end, so they are a list beside
            // the observation feed, not entries in it.
            let turns = store.sqlite.session_turns(&session.id)?;
            let events = store
                .sqlite
                .session_history(&session.id, *after, feed.limit)?;
            if feed.json {
                println!(
                    "{}",
                    serde_json::to_string(&serde_json::json!({"turns": turns, "events": events}))?
                );
            } else {
                for turn in turns {
                    let usage = turn.usage.as_ref().map(|usage| &usage.usage);
                    println!(
                        "turn {} {} {} input={} output={}",
                        turn.thread,
                        turn.turn,
                        turn.status.as_deref().unwrap_or("running"),
                        usage
                            .and_then(|usage| usage.total_input_tokens)
                            .map_or("-".into(), |tokens| tokens.to_string()),
                        usage
                            .and_then(|usage| usage.output_tokens)
                            .map_or("-".into(), |tokens| tokens.to_string()),
                    );
                }
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
            use loopflow_execution::ops::human_session::moves_bulk::{move_sessions, parse_target};
            let target = parse_target(cli.agent.as_deref())?;
            let filter = loopflow_execution::session::SessionFilter {
                on: selector.on.clone(),
                repo: if *all {
                    None
                } else {
                    loopflow_execution::repository::containing(&std::env::current_dir()?)?
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
        SessionCommand::Resume { .. } | SessionCommand::New { .. } => {
            unreachable!("`run` opens conversations itself")
        }
        SessionCommand::Rename { .. } => unreachable!("rename uses shared mutation dispatch"),
        SessionCommand::Send {
            id,
            task,
            to,
            arrived,
        } => {
            let store = open_shared_store().await?;
            let to = loopflow_execution::ops::task_ownership::resolve_machine(&store.sqlite, to)?;
            if *arrived {
                let task = task
                    .as_deref()
                    .expect("Clap requires --task with --arrived");
                return loopflow_execution::ops::human_session::moves::sent_task_arrived(
                    &store, task, &to,
                )
                .await;
            }
            let moved = match (id, task) {
                (id, Some(task)) => {
                    loopflow_execution::ops::human_session::moves::send_task(
                        &store,
                        task,
                        &to,
                        id.as_deref(),
                    )
                    .await?
                }
                (Some(id), None) => {
                    loopflow_execution::ops::human_session::moves::send(&store, id, &to).await?
                }
                (None, None) => unreachable!("Clap requires a Session or a Task"),
            };
            println!("{}", serde_json::to_string(&moved)?);
            Ok(())
        }
        SessionCommand::Receive {
            check: true,
            task: Some(task),
            issue: Some(issue),
            repository: Some(repository),
            origin,
            from,
            ..
        } => {
            let store = open_shared_store().await?;
            let path = loopflow_execution::ops::human_session::moves::checkout_of(
                &store,
                repository,
                origin.as_deref(),
            )?;
            let main = loopflow_execution::git::worktrees::main_repo_root(&path)?;
            let id = loopflow_model::durable::TaskId::parse(task)?;
            loopflow_execution::ops::human_session::moves::arriving_task(&store, &main, &id, issue)
                .await?;
            if let Some(from) = from {
                store.sqlite.record_move_checked(&id, from)?;
            }
            Ok(())
        }
        SessionCommand::Receive {
            arrived: Some(task),
            ..
        } => {
            let store = open_shared_store().await?;
            let here = loopflow_execution::store::machines::local_machine(&store.sqlite)?.id;
            anyhow::ensure!(
                store.sqlite.task_move_arrived_on(task, &here)?,
                "Task {task} did not arrive here"
            );
            Ok(())
        }
        SessionCommand::Receive { check: true, .. } => {
            open_shared_store().await?;
            Ok(())
        }
        SessionCommand::Receive { json, .. } => {
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)?;
            let moved: loopflow_execution::ops::human_session::moves::Move =
                serde_json::from_str(&input).context("read the moved Session")?;
            let store = open_shared_store().await?;
            let task = moved.task.as_ref().map(|task| task.identifier.clone());
            let session =
                loopflow_execution::ops::human_session::moves::receive(&store, moved).await?;
            let Some(session) = session else {
                let task = task.expect("a move without a Session moves a Task");
                if *json {
                    println!("{}", serde_json::json!({ "task": task }));
                } else {
                    println!("Task {task} is here; `lf task run {task}` continues it");
                }
                return Ok(());
            };
            let record = loopflow_execution::ops::human_session::surface(&store, &session).await?;
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
    pub id: loopflow_model::id::LfSessionId,
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
    /// The provider's current AgentProcess generation.
    pub agent_process_id: Option<loopflow_model::id::AgentProcessId>,
    /// How its last agent ended, as `lf session list` reads it.
    #[serde(deserialize_with = "Option::deserialize")]
    pub ended: Option<SessionEnd>,
    /// The server's starts of it since it last finished a turn.
    #[serde(deserialize_with = "Option::deserialize")]
    pub start: Option<SessionStarts>,
    /// Whether it comes up while it is not running, and what brings it.
    #[serde(deserialize_with = "Option::deserialize")]
    pub outlook: Option<SessionOutlook>,
    /// All of the above as the server's one sentence; `lf task status`
    /// prints the Task's conversation's as given.
    #[serde(deserialize_with = "Option::deserialize")]
    pub status_line: Option<String>,
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
            ended: session.ended,
            start: session.start,
            outlook: session.outlook,
            status_line: session.status_line,
        }
    }
}

/// `lf session list`.
pub fn list_sessions(args: &SessionListArgs) -> anyhow::Result<()> {
    let filter = list_filter(args, &std::env::current_dir()?)?;
    if args.subscribe {
        // This process reads the subscription itself, so it watches
        // transcripts the way the Machine server does.
        let observations = loopflow_execution::observe::Cache::shared();
        let _transcripts = loopflow_execution::transcript::watch::watch(&observations);
        return subscribe_list(filter, loopflow_execution::read_stream::Transport::Stdio).map(drop);
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
) -> anyhow::Result<loopflow_execution::session::SessionFilter> {
    Ok(loopflow_execution::session::SessionFilter {
        on: args.selector.on.clone(),
        available: args.available,
        raw: !args.no_raw,
        repo: if args.all {
            None
        } else {
            loopflow_execution::repository::containing(directory)?.map(|repo| repo.to_string())
        },
        task: args.task.clone(),
        wave: args
            .wave
            .as_deref()
            .map(|wave| {
                loopflow_execution::work::wave::context::resolve_managed_wave_sync(
                    (!args.all).then_some(directory),
                    Some(wave),
                )
                .map(|wave| wave.id().clone())
            })
            .transpose()?,
        primary: args.primary,
        orphan: args.orphan,
        search: args.search.clone(),
        headless: args.headless,
        limit: args.limit,
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
    /// Include headless Sessions alongside interactive conversations.
    headless: bool,
}

pub(crate) fn subscribe_list(
    filter: loopflow_execution::session::SessionFilter,
    transport: loopflow_execution::read_stream::Transport,
) -> anyhow::Result<Option<loopflow_execution::read_stream::Subscription>> {
    let args = SessionWatchArgs {
        repo: filter.repo.clone(),
        task: filter.task.clone(),
        search: filter.search.clone(),
        headless: filter.headless,
    };
    loopflow_execution::read_stream::start(
        loopflow_execution::read_stream::Watch::State,
        args,
        move |store, args| {
            let filter = loopflow_execution::session::SessionFilter {
                repo: args.repo,
                task: args.task,
                search: args.search,
                headless: args.headless,
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
    loopflow_execution::ops::human_session::rename(&store, id, &requested, source).await
}

/// `lf session rename <id> <name>`: the renamed Session.
pub(crate) fn renamed(id: &str, name: &str) -> anyhow::Result<SessionRecord> {
    tokio::runtime::Runtime::new()?.block_on(rename_session(
        id,
        &[name.to_owned()],
        SessionTitleSource::Human,
    ))
}

/// The snapshot a Session's Program Status now holds.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct StatusRecorded {
    pub stream: String,
    pub sequence: i64,
}

/// `lf session observe-status <id> --terminal <terminal> --stream <stream>
/// --sequence <n>`, the snapshot on stdin.
pub(crate) async fn status_recorded(
    id: &str,
    terminal: &str,
    agent_process: Option<&str>,
    stream: &str,
    sequence: i64,
    records: &loopflow_model::program_status::Records,
) -> anyhow::Result<StatusRecorded> {
    let store = open_shared_store().await?;
    let agent_process = agent_process
        .map(loopflow_model::id::AgentProcessId::parse)
        .transpose()?;
    loopflow_execution::ops::human_session::record_program_status(
        &store,
        id,
        terminal,
        agent_process.as_ref(),
        stream,
        sequence,
        records,
    )
    .await?;
    Ok(StatusRecorded {
        stream: stream.to_owned(),
        sequence,
    })
}

// The bound Session, or the Task a preview would assign.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub(crate) enum SessionBinding {
    Bound(Box<loopflow_execution::ops::human_session::SessionRecord>),
    Preview(Box<loopflow_execution::ops::human_session::SessionBindingPreview>),
}

/// `lf session bind <id> --task <task> --json`: the bound Session, or with
/// `preview` (`--dry-run`) the Task a bind would assign, assigning nothing.
pub(crate) fn bound(id: &str, task: &str, preview: bool) -> anyhow::Result<SessionBinding> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        Ok(if preview {
            SessionBinding::Preview(Box::new(
                loopflow_execution::ops::human_session::preview_binding(&store, id, task).await?,
            ))
        } else {
            SessionBinding::Bound(Box::new(
                loopflow_execution::ops::human_session::bind(&store, id, task).await?,
            ))
        })
    })
}

/// The report `lf session move --all --json` prints for the selector and agent.
pub(crate) fn moved(
    on: Option<&str>,
    agent: &str,
) -> anyhow::Result<loopflow_execution::ops::human_session::moves_bulk::MoveReport> {
    use loopflow_execution::ops::human_session::moves_bulk::{move_sessions, parse_target};
    let target = parse_target(Some(agent))?;
    let filter = loopflow_execution::session::SessionFilter {
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
    filter: &loopflow_execution::session::SessionFilter,
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
    filter: &loopflow_execution::session::SessionFilter,
) -> anyhow::Result<serde_json::Value> {
    if filter.after.is_none() {
        return Ok(serde_json::to_value(
            loopflow_execution::ops::human_session::list(store, filter).await?,
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
    let mut entries = loopflow_execution::ops::human_session::list(store, &selection).await?;
    let next = if entries.len() > filter.limit {
        entries.pop();
        entries.last().map(|session| session.id.to_string())
    } else {
        None
    };
    Ok(serde_json::to_value(
        loopflow_execution::ops::human_session::SessionPage {
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
    filter: &loopflow_execution::session::SessionFilter,
) -> anyhow::Result<()> {
    if json || filter.after.is_some() {
        println!(
            "{}",
            serde_json::to_string_pretty(&loopflow_execution::read_stream::selected_answer(
                &read_list(store, filter).await?
            )?)?
        );
        return Ok(());
    }
    let sessions = loopflow_execution::ops::human_session::list(store, filter).await?;
    // Sessions copied from given-back Machines are history, never available here.
    let copied = if filter.available || filter.orphan {
        Vec::new()
    } else {
        copied_sessions(store, filter)?
    };
    if sessions.is_empty() && copied.is_empty() {
        println!("No Sessions.");
    } else {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        for session in sessions {
            let place = if session.raw {
                format!("raw {}", session.provider.as_deref().unwrap_or_default())
            } else {
                session
                    .work_path
                    .as_deref()
                    .unwrap_or("Repository")
                    .to_owned()
            };
            let mut line = format!(
                "{}  {:<7} {}  {}",
                session.id,
                match session.state {
                    _ if session.availability.is_some() => "available",
                    SessionState::Unknown => "unknown",
                    SessionState::Active => "active",
                    SessionState::Detached => "detached",
                    SessionState::Closed => "closed",
                },
                place,
                match &session.account {
                    Some(account) => format!("{}  historical account: {account}", session.title),
                    None => session.title.clone(),
                }
            );
            if let Some(turn) = &session.turn {
                line.push_str(&format!("  {}", turn.describe(now)));
            }
            println!("{line}");
            if let Some(action) = session.action {
                println!("  {} — {}", action.label, action.help);
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
    filter: &loopflow_execution::session::SessionFilter,
) -> anyhow::Result<Vec<loopflow_execution::machine_records::CopiedSession>> {
    let task = match &filter.task {
        Some(selector) => {
            match loopflow_execution::store::children::resolve_task_id(
                &store.sqlite,
                selector,
                None,
            )? {
                Some(task) => Some(task),
                None => return Ok(Vec::new()),
            }
        }
        None => None,
    };
    let repository = match &filter.repo {
        Some(repo) => match store.sqlite.repository_id(repo)? {
            Some(id) => Some(id),
            None => return Ok(Vec::new()),
        },
        None => None,
    };
    Ok(loopflow_execution::store::machines::copied_sessions(
        &store.sqlite,
        task.as_ref(),
        repository.as_ref(),
    )?)
}

async fn resume(
    cli: &crate::lf::Cli,
    connection: &SessionConnectionArgs,
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    fork: bool,
    request_id: Option<&str>,
) -> anyhow::Result<Option<ConversationLaunch>> {
    // Prepared connections have already returned through shared mutation dispatch.
    anyhow::ensure!(
        !cli.batch && cli.output_schema.is_none(),
        "`lf resume` connects; `lf -b steer SESSION TEXT` runs one more turn here"
    );
    let progress = loopflow_execution::startup::Startup::new("Finding conversation");
    let store = open_shared_store().await?;
    let cwd = std::env::current_dir()?;
    drop(progress);
    let opening = reopen(
        &store,
        &cwd,
        target,
        connection,
        cli.agent.as_deref(),
        request_id,
        fork,
        connection.json || cli.json,
    )
    .await?;
    match opening {
        Opening::Done(session) => {
            print_opened(connection.json || cli.json, &session).map(|()| None)
        }
        Opening::Launch(launch) => Ok(Some(launch)),
    }
}

/// The conversation `lf resume` run from `directory` selects, recorded as
/// started by a person unless `request_id` is the recovery pass's own.
async fn select(
    store: &Arc<Store>,
    directory: &Path,
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    request_id: Option<&str>,
) -> anyhow::Result<String> {
    let selected =
        loopflow_execution::ops::human_session::select_connection(store, directory, target).await?;
    if selected.started {
        loopflow_execution::journal::request::diagnostic(format_args!(
            "Nothing to resume here; starting a new conversation."
        ));
    }
    loopflow_execution::ops::recovery::record_start(
        &store.sqlite,
        &selected.id.as_str().into(),
        request_id,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )?;
    Ok(selected.id)
}

/// `lf session resume` without a message: the selected conversation, opened
/// here, and its provider started unless `--prepare` was requested.
#[allow(clippy::too_many_arguments)]
async fn reopen(
    store: &Arc<Store>,
    directory: &Path,
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    connection: &SessionConnectionArgs,
    agent: Option<&str>,
    request_id: Option<&str>,
    fork: bool,
    json: bool,
) -> anyhow::Result<Opening> {
    let id = select(store, directory, target, request_id).await?;
    loopflow_execution::ops::human_session::open(
        store,
        directory,
        &id,
        agent,
        request_id,
        fork,
        !connection.prepare,
        loopflow_execution::transport::ViewerOptions {
            json,
            unfocused: connection.unfocused,
        },
    )
    .await
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
                ..
            } if connection.prepare => {
                let target = connection.target(cli, id.as_deref())?;
                reopen(
                    &store,
                    directory,
                    target,
                    connection,
                    cli.agent.as_deref(),
                    None,
                    *fork,
                    false,
                )
                .await?
                .prepared()
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
                    Default::default(),
                    request_id.as_deref(),
                    cli.agent.as_deref(),
                )
                .await?
                .prepared()
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
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    request_id: Option<&str>,
) -> anyhow::Result<Option<ConversationLaunch>> {
    let store = open_shared_store().await?;
    let opening = restarted(
        &store,
        &std::env::current_dir()?,
        target,
        true,
        loopflow_execution::transport::ViewerOptions {
            json: connection.json || cli.json,
            unfocused: connection.unfocused,
        },
        request_id,
        cli.agent.as_deref(),
    )
    .await?;
    match opening {
        Opening::Done(record) => print_opened(connection.json || cli.json, &record).map(|()| None),
        Opening::Launch(launch) => Ok(Some(launch)),
    }
}

/// The conversation `lf session new` run from `directory` selects, on a fresh
/// provider thread started unless `launch` is false.
async fn restarted(
    store: &Arc<Store>,
    directory: &Path,
    target: loopflow_execution::ops::human_session::SessionTarget<'_>,
    launch: bool,
    viewer: loopflow_execution::transport::ViewerOptions,
    request_id: Option<&str>,
    agent: Option<&str>,
) -> anyhow::Result<Opening> {
    let selected =
        loopflow_execution::ops::human_session::select_connection(store, directory, target).await?;
    let request_id = request_id
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let (next, launch) = loopflow_execution::ops::human_session::new_thread(
        store,
        &selected.id,
        agent,
        launch,
        viewer,
        &request_id,
    )
    .await?;
    if let Some(launch) = launch {
        return Ok(Opening::Launch(launch));
    }
    if let Some(result) = store
        .sqlite
        .creation_receipt(&request_id)?
        .and_then(|r| r.result)
    {
        return Ok(Opening::Done(serde_json::from_value(result)?));
    }
    loopflow_execution::ops::human_session::surface(store, &next)
        .await
        .map(Opening::Done)
}

/// Finish a successful Session connection after its thin relay has returned.
/// Whether opening a conversation answers with its Session record once the
/// conversation returns, including one launched in this process.
pub fn returns_record(command: &SessionCommand, cli: &crate::lf::Cli) -> bool {
    match command {
        SessionCommand::Resume { connection, .. } | SessionCommand::New { connection, .. } => {
            connection.json || cli.json
        }
        _ => false,
    }
}

pub fn print_returned(id: &str) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        let session = loopflow_execution::ops::human_session::session_by_id(&store, id)
            .await?
            .context("Session not found")?;
        let record = loopflow_execution::ops::human_session::surface(&store, &session).await?;
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
        return Err(loopflow_execution::engine::Error::new(
            loopflow_execution::engine::ErrorCode::Invalid,
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
    io: &mut loopflow_execution::command::CommandIo<'_>,
) -> anyhow::Result<()> {
    if let SessionCommand::Resume { connection, .. } | SessionCommand::New { connection, .. } =
        command
    {
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
                        loopflow_execution::ops::human_session::preview_binding(&store, id, task)
                            .await?;
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
                let session =
                    loopflow_execution::ops::human_session::bind(&store, id, task).await?;
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
