//! Recovery of this Machine's work, run by the Machine server's recovery pass.
//! [`settle`] closes AgentProcesses whose lf died, releases AgentProcess
//! ancestry that was never recorded, carries a Task's run again
//! when its ship process exited or its `lf task run` died, starts a run that ended waiting on
//! a usage limit, and gives back rented Machines whose work drained or whose
//! soft deadline came. [`retain`] keeps the store to what is still needed.
//! [`due_task_conversations`] names the Task conversations to start, and
//! [`launch_failure`] what would stop one.

use loopflow_model::id::LfSessionId;
use loopflow_storage::sqlite::session_recovery::ConversationToStart;
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
    let runs_due = crate::ops::task::ship::restart_runs()
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

/// End owners whose OS process is gone, forget finished rows nothing needs
/// and reclaim pages. Failures are logged; the next pass tries again.
pub fn retain() {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    match crate::store::database_path_from_env() {
        Ok(database) if database.exists() => {
            if let Err(error) = crate::store::retention::prune(&database, now) {
                tracing::warn!(%error, "store retention");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "store retention has no store"),
    }
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
pub fn launch_failure(conversation: &ConversationToStart) -> Option<String> {
    let provider = conversation.provider.as_deref()?;
    (!crate::agent::check_cli_available(provider))
        .then(|| format!("{provider} is not installed on this Machine's PATH"))
}

/// The Task conversations to start again now, most recently active Task
/// first, and when the soonest one still waiting its backoff comes due.
#[derive(Debug, Default)]
pub struct DueConversations {
    pub now: Vec<ConversationToStart>,
    pub next_at: Option<i64>,
}

/// Open Tasks claimed here whose conversation was never started, or whose
/// agent died rather than being quit. An agent still unfinished in the store
/// counts as died only when its recorded process is gone; unknown liveness
/// waits. The first start after a death is immediate; one after a start
/// waits the landing's repair backoff (1 minute, doubling to 30) from it.
pub fn due_task_conversations(
    store: &SqliteStore,
    now: i64,
) -> crate::store::StoreResult<DueConversations> {
    let mut due = DueConversations::default();
    for conversation in store.conversations_to_start()? {
        let died = conversation.agent.is_none()
            || conversation.ended_at.is_some()
            || conversation.identity.is_some_and(|(pid, start)| {
                crate::process::os_process::process_identity_evidence(pid, start)
                    == ProcessIdentityEvidence::Dead
            });
        if !died || !store.task_claimed_here(&conversation.task)? {
            continue;
        }
        match next_start_at(conversation.starts_since_turn, conversation.last_started_at) {
            Some(at) if at > now => {
                due.next_at = Some(due.next_at.map_or(at, |soonest| soonest.min(at)));
            }
            _ => due.now.push(conversation),
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
