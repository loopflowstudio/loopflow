//! Recovery of this Machine's work, run by the Machine server's recovery pass.
//! [`settle`] closes AgentProcesses whose lf died, releases AgentProcess
//! ancestry that was never recorded, carries a Task's run again
//! when its ship process exited or its `lf task run` died, starts a run that ended waiting on
//! a usage limit, and gives back rented Machines whose work drained or whose
//! soft deadline came. [`retain`] keeps the store and receipts to what is
//! still needed. [`due_task_conversations`] names the Task
//! conversations to start, and [`launch_failure`] what would stop one.

use loopflow_model::id::LfSessionId;
use loopflow_storage::sqlite::session_recovery::ConversationToStart;
use loopflow_storage::sqlite::SqliteStore;

use crate::journal::ProcessIdentityEvidence;

/// Settle this Machine's work, and say when the soonest Task run waiting its
/// restart backoff comes due. Failures are logged; the next pass tries again.
pub fn settle() -> Option<i64> {
    match crate::harness::agent_process::settle_agent_processes() {
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

/// Close Processes whose OS process is gone, forget finished ones nothing
/// needs, merge per-receipt usage and reclaim pages; then remove receipts of
/// Processes proven dead. Retention closes rows from receipt evidence, so the
/// receipts go after it. Failures are logged; the next pass tries again.
pub fn retain() {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    match crate::store::database_path_from_env() {
        Ok(database) if database.exists() => {
            if let Err(error) = crate::store::retention::prune(&database, now) {
                tracing::warn!(%error, "store retention");
            }
        }
        Ok(_) => return,
        Err(error) => {
            tracing::warn!(%error, "store retention has no store");
            return;
        }
    }
    let home = crate::store::lf_home_dir();
    if let Err(error) = crate::journal::remove_dead_process_receipts_at(&home) {
        tracing::warn!(%error, "dead Process receipts");
    }
}

/// Names the Session the recovery pass is starting, in the environment of the
/// `lf session resume` it runs. Any other start of a Session is a person's.
pub const RECOVERING_ENV: &str = "LF_RECOVERING_SESSION";

/// A person started `session` (`lf resume`, Desktop, the phone), unless this
/// process is the pass's own start of it: the pass's count of starts after a
/// death begins again, and an earlier start's failure is forgotten.
pub fn record_start(store: &SqliteStore, session: &LfSessionId) -> crate::store::StoreResult<()> {
    if std::env::var(RECOVERING_ENV).is_ok_and(|recovering| *session == recovering) {
        return Ok(());
    }
    store.reset_conversation_starts(session)
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
/// waits. The first start after a death is immediate; later ones wait the
/// landing's repair backoff (1 minute, doubling to 30) from the last.
pub fn due_task_conversations(
    store: &SqliteStore,
    now: i64,
) -> crate::store::StoreResult<DueConversations> {
    let mut due = DueConversations::default();
    for conversation in store.conversations_to_start()? {
        let died = conversation.agent.is_none()
            || conversation.ended_at.is_some()
            || conversation.identity.is_some_and(|(pid, start)| {
                crate::journal::process_identity_evidence(pid, start)
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
/// progress, last at `last_started_at`, may start again; `None`: at once. Task
/// conversations and Task runs share this schedule.
pub fn next_start_at(attempts: u32, last_started_at: Option<i64>) -> Option<i64> {
    match (attempts, last_started_at) {
        (0, _) | (_, None) => None,
        (attempts, Some(at)) => Some(at + crate::ops::pr_landing::repair_backoff(attempts)),
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::OptionalExtension;

    /// A Process whose receipt proves it dead is closed, forgotten and loses
    /// its receipt; one whose only receipt is an unlinked PID-named file stays
    /// unfinished with its receipt, since nothing proves its exit.
    #[test]
    fn retain_settles_proven_dead_processes_and_keeps_unknown_ones() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let database = ledger.home().join("loopflow.db");
        crate::store::opening::open_ephemeral(&database).unwrap();
        let conn = rusqlite::Connection::open(&database).unwrap();
        let mut child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .unwrap();
        let pid = child.id();
        child.kill().unwrap();
        child.wait().unwrap();
        let directory = ledger.home().join("runtime/exec-processes");
        std::fs::create_dir_all(&directory).unwrap();
        // Started since boot, so only the missing receipt link leaves it unknown.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        for linked in [false, true] {
            let id = loopflow_model::id::LfProcessId::new();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,?2)",
                rusqlite::params![id, now],
            )
            .unwrap();
            let name = if linked {
                id.to_string()
            } else {
                pid.to_string()
            };
            let receipt = directory.join(format!("{name}.json"));
            std::fs::write(
                &receipt,
                serde_json::to_vec(&serde_json::json!({
                    "schema_version": 1, "trace_id": id, "exec_id": id,
                    "pid": pid, "started_at": 1,
                }))
                .unwrap(),
            )
            .unwrap();

            super::retain();

            let row: Option<Option<String>> = conn
                .query_row("SELECT outcome FROM processes WHERE id=?1", [&id], |row| {
                    row.get(0)
                })
                .optional()
                .unwrap();
            assert_eq!(row, (!linked).then_some(None), "linked: {linked}");
            assert_eq!(receipt.exists(), !linked, "linked: {linked}");
            let _ = std::fs::remove_file(receipt);
        }
    }
}
