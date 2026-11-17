//! Recovery of this Machine's work, run by the Machine server's recovery pass.
//! [`settle`] closes AgentProcesses whose lf died, carries a Task's run again
//! when its ship process exited or its `lf task run` died, starts a run that ended waiting on
//! a usage limit, and gives back rented Machines whose work drained or whose
//! soft deadline came. [`due_task_conversations`] names the Task
//! conversations to start, and [`launch_failure`] what would stop one.

use loopflow_storage::sqlite::session_recovery::RecoverableConversation;
use loopflow_storage::sqlite::SqliteStore;

use crate::journal::ProcessIdentityEvidence;

/// Settle this Machine's work. Failures are logged; the next pass tries again.
pub fn settle() {
    if let Err(error) = crate::harness::agent_process::reap_agent_processes(false) {
        tracing::warn!("AgentProcess settlement: {error}");
    }
    if let Err(error) = crate::ops::task::ship::restart_runs() {
        tracing::warn!("run restart: {error}");
    }
    if let Err(error) = crate::ops::task::limit_recovery::restart_waiting_runs() {
        tracing::warn!("limit recovery: {error}");
    }
    if let Err(error) = crate::rental::recovery::check() {
        tracing::warn!("Rentals: {error:#}");
    }
}

/// Why starting `conversation` would fail before any provider runs: its
/// agent's CLI is not on this Machine's `PATH`. `None` when it can start.
pub fn launch_failure(conversation: &RecoverableConversation) -> Option<String> {
    let provider = conversation.provider.as_deref()?;
    (!crate::agent::check_cli_available(provider))
        .then(|| format!("{provider} is not installed on this Machine's PATH"))
}

/// The Task conversations to start again now, most recently active Task
/// first: open Tasks claimed here whose conversation was never started, or
/// whose agent died rather than being quit, and whose backoff has passed. An agent still unfinished in the
/// store counts as died only when its recorded process is gone; unknown
/// liveness waits. The first start after a death is immediate; later ones wait
/// the landing's repair backoff (1 minute, doubling to 30) from the last.
pub fn due_task_conversations(
    store: &SqliteStore,
    now: i64,
) -> crate::store::StoreResult<Vec<RecoverableConversation>> {
    let mut due = Vec::new();
    for conversation in store.recoverable_task_conversations()? {
        let died = conversation.agent.is_none()
            || conversation.ended_at.is_some()
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
