//! Recovery of this Machine's work. The Machine server's recovery pass starts
//! Task conversations that died ([`due_task_conversations`]). Lazy recovery,
//! run by lf reads of this Machine's Tasks or processes, settles
//! AgentProcesses whose lf died, starts a shipping Task's ship process again
//! when it has exited, starts a run that ended waiting on a usage limit, and
//! gives back rented Machines whose work drained or whose soft deadline came.

use loopflow_storage::sqlite::session_recovery::RecoverableConversation;
use loopflow_storage::sqlite::SqliteStore;

use crate::journal::ProcessIdentityEvidence;

/// Recover this Machine's work. Failures are logged, never fatal to the read.
pub fn recover() {
    if let Err(error) = crate::harness::agent_process::reap_agent_processes(false) {
        tracing::warn!("AgentProcess settlement: {error}");
    }
    if let Err(error) = crate::ops::task::ship::restart_ships() {
        tracing::warn!("ship restart: {error}");
    }
    if let Err(error) = crate::ops::task::limit_recovery::restart_waiting_runs() {
        tracing::warn!("limit recovery: {error}");
    }
    if let Err(error) = crate::rental::recovery::check() {
        tracing::warn!("Rentals: {error:#}");
    }
}

/// The Task conversations to start again now, most recently active Task
/// first: open Tasks claimed here whose conversation's agent died rather than
/// being quit, and whose backoff has passed. An agent still unfinished in the
/// store counts as died only when its recorded process is gone; unknown
/// liveness waits. The first start after a death is immediate; later ones wait
/// the landing's repair backoff (1 minute, doubling to 30) from the last.
pub fn due_task_conversations(
    store: &SqliteStore,
    now: i64,
) -> crate::store::StoreResult<Vec<RecoverableConversation>> {
    let mut due = Vec::new();
    for conversation in store.recoverable_task_conversations()? {
        let died = conversation.ended_at.is_some()
            || conversation.identity.is_some_and(|(pid, start)| {
                crate::journal::process_identity_evidence(pid, start)
                    == ProcessIdentityEvidence::Dead
            });
        let waited = match (conversation.recovery_attempts, conversation.recovered_at) {
            (0, _) | (_, None) => true,
            (attempts, Some(at)) => at + crate::ops::pr_landing::repair_backoff(attempts) <= now,
        };
        if died && waited && store.task_claimed_here(&conversation.task)? {
            due.push(conversation);
        }
    }
    Ok(due)
}
