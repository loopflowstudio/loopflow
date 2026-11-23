//! Recovery of this Machine's work, run by the Machine server's recovery pass.
//! [`settle`] closes AgentProcesses whose lf died, releases AgentProcess
//! ancestry that was never recorded, carries a Task's run again
//! when its `lf task run` died, starts a run that ended waiting on
//! a usage limit, and gives back rented Machines whose work drained or whose
//! soft deadline came.
//! [`due_task_conversations`] names the Task conversations to start, and
//! [`launch_failure`] what would stop one.

use loopflow_model::id::LfSessionId;
use loopflow_storage::sqlite::session_recovery::{TaskConversation, CONVERSATION_STARTS};
use loopflow_storage::sqlite::SqliteStore;

use crate::store::ProcessIdentityEvidence;

/// Settle this Machine's work, and say when the soonest Task run waiting its
/// restart backoff comes due. Failures are logged; the next pass tries again.
pub fn settle() -> Option<i64> {
    let reaped = crate::store::database_path_from_env()
        .map_err(anyhow::Error::from)
        .and_then(|database| crate::harness::agent_process::reap_agent_processes(&database));
    match reaped {
        Ok(report) => {
            for error in report.errors {
                tracing::warn!("AgentProcess settlement: {error}");
            }
        }
        Err(error) => tracing::warn!("AgentProcess settlement: {error}"),
    }
    let runs_due = crate::ops::task::run_recovery::restart_runs()
        .inspect_err(|error| tracing::warn!("run restart: {error}"))
        .ok()
        .flatten();
    if let Err(error) = crate::ops::task::limit_recovery::restart_waiting_runs() {
        tracing::warn!("limit recovery: {error}");
    }
    if let Err(error) = crate::rental::recovery::check() {
        tracing::warn!("Rentals: {error:#}");
    }
    runs_due
}

/// How the recovery pass names each start it makes:
/// `recover:<session>:<ended agent>:<attempt>`. The held terminal's
/// `lf session resume` carries it as `--request-id`.
pub const RECOVER_REQUEST: &str = "recover:";

/// A person started `session` at `at` (`lf resume`, Desktop, the phone),
/// unless `request` is the pass's own start of it: the pass's count of
/// starts after a death begins again, an earlier start's failure is
/// forgotten, and the pass holds off while the person's start comes up.
pub fn record_start(
    store: &SqliteStore,
    session: &LfSessionId,
    request: Option<&str>,
    at: i64,
) -> crate::store::StoreResult<()> {
    if request.is_some_and(|request| request.starts_with(RECOVER_REQUEST)) {
        return Ok(());
    }
    store.record_persons_start(session, at)
}

/// Why starting `conversation` would fail before any provider runs: its
/// agent's CLI is not on this Machine's `PATH`. `None` when it can start.
pub fn launch_failure(conversation: &TaskConversation) -> Option<String> {
    let provider = conversation.provider.as_deref()?;
    let installed = loopflow_harness::HarnessKind::parse(provider)
        .is_some_and(|kind| crate::harness::launcher(kind).installed());
    (!installed).then(|| format!("{provider} is not installed on this Machine's PATH"))
}

/// What decides whether this Machine's server starts a conversation that is
/// down. The recovery pass builds one from each row it lists and a Session's
/// outlook from its summary; both ask [`server_start`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownConversation {
    /// It is an open Task's conversation in the Task's checkout here.
    pub here: bool,
    pub paused: bool,
    /// Its last agent's exit; `None` when nobody saw one.
    pub exit_code: Option<i32>,
    pub starts_since_turn: u32,
    pub last_started_at: Option<i64>,
}

/// Whether the server starts a conversation that is down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStart {
    /// It does: at this time, or at once when nothing started it since it
    /// last made progress.
    At(Option<i64>),
    Left(Left),
}

/// Why the server leaves a conversation that is down to a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Left {
    /// Its Task holds execution until unpause.
    Paused,
    /// The person quit it.
    Quit,
    /// It was started [`CONVERSATION_STARTS`] times without finishing a turn.
    OutOfStarts,
    /// It is not an open Task's conversation in that Task's checkout here.
    Elsewhere,
    /// Another Machine claims its Task.
    NotClaimedHere,
    NoServer,
}

/// The one rule for "the server starts it". `claimed_here` and `server_runs`
/// are asked only when everything on the Session's own row allows a start.
/// The first start after a death is immediate; one after a start waits the
/// landing's repair backoff (1 minute, doubling to 30) from it.
pub fn server_start(
    conversation: &DownConversation,
    claimed_here: impl FnOnce() -> bool,
    server_runs: impl FnOnce() -> bool,
) -> ServerStart {
    let left = if conversation.paused {
        Left::Paused
    } else if loopflow_storage::session::quit_by_person(conversation.exit_code) {
        Left::Quit
    } else if conversation.starts_since_turn >= CONVERSATION_STARTS {
        Left::OutOfStarts
    } else if !conversation.here {
        Left::Elsewhere
    } else if !claimed_here() {
        Left::NotClaimedHere
    } else if !server_runs() {
        Left::NoServer
    } else {
        return ServerStart::At(next_start_at(
            conversation.starts_since_turn,
            conversation.last_started_at,
        ));
    };
    ServerStart::Left(left)
}

/// Whether this Machine claims `task`; a claim that cannot be read is not one.
pub fn claimed_here(store: &SqliteStore, task: &loopflow_model::durable::TaskId) -> bool {
    store
        .task_claimed_here(task)
        .inspect_err(|error| tracing::warn!(%task, %error, "Task claim unreadable"))
        .unwrap_or(false)
}

/// Whether a Machine server answers here.
pub fn server_runs() -> bool {
    crate::machine::socket_path()
        .is_ok_and(|socket| std::os::unix::net::UnixStream::connect(socket).is_ok())
}

/// The Task conversations to start again now, most recently active Task
/// first, and when the soonest one still waiting its backoff comes due.
#[derive(Debug, Default)]
pub struct DueConversations {
    pub now: Vec<TaskConversation>,
    pub next_at: Option<i64>,
}

/// The server's own reading: open Tasks' conversations here that are down
/// and that [`server_start`] says it starts. One was never started, or its
/// agent ended. An agent still unfinished in the store counts as down only
/// when its recorded process is gone; unknown liveness waits.
pub fn due_task_conversations(
    store: &SqliteStore,
    now: i64,
) -> crate::store::StoreResult<DueConversations> {
    let mut due = DueConversations::default();
    for conversation in store.task_conversations_here()? {
        let down = conversation.agent.is_none()
            || conversation.ended_at.is_some()
            || conversation.identity.is_some_and(|(pid, start)| {
                crate::process::os_process::process_identity_evidence(pid, start)
                    == ProcessIdentityEvidence::Dead
            });
        if !down {
            continue;
        }
        let facts = DownConversation {
            here: true,
            paused: false, // The selection excludes held Tasks.
            exit_code: conversation.exit_code,
            starts_since_turn: conversation.starts_since_turn,
            last_started_at: conversation.last_started_at,
        };
        // This is the server asking: one runs.
        let start = server_start(&facts, || claimed_here(store, &conversation.task), || true);
        match start {
            ServerStart::At(Some(at)) if at > now => {
                due.next_at = Some(due.next_at.map_or(at, |soonest| soonest.min(at)));
            }
            ServerStart::At(_) => due.now.push(conversation),
            ServerStart::Left(_) => {}
        }
    }
    Ok(due)
}

/// When something that died, started again `attempts` times since it last made
/// progress, last at `last_started_at`, may start again; `None`: at once, as
/// nothing has started it since. Task conversations and Task runs share this
/// schedule.
pub fn next_start_at(attempts: u32, last_started_at: Option<i64>) -> Option<i64> {
    last_started_at.map(|at| at + crate::ops::pr_landing::repair_backoff(attempts))
}
