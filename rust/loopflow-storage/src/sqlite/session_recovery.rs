//! Task conversations that ended without the person quitting, for the
//! server's recovery pass to start again.

use loopflow_model::durable::TaskId;
use loopflow_model::id::{AgentProcessId, LfSessionId};
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::StoreResult;

/// Starts after which the pass stops trying and the Task says which command
/// continues.
pub const RECOVERY_ATTEMPTS: u32 = 5;

/// An open Task's conversation, in its checkout here, whose current agent is
/// not recorded as quit by the person: it ended some other way, or is
/// unfinished and may have died.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoverableConversation {
    pub session: LfSessionId,
    pub task: TaskId,
    /// The agent generation that ended. A start names it, so one death never
    /// starts two providers.
    pub agent: AgentProcessId,
    /// When the agent's end was recorded; `None` while its row is unfinished.
    pub ended_at: Option<i64>,
    /// The provider's exit, as its launcher recorded it. `None` when nobody saw
    /// the end: the holder died, or the Machine restarted.
    pub exit_code: Option<i32>,
    /// The agent's recorded OS identity, read while its row is unfinished.
    pub identity: Option<(u32, i64)>,
    pub recovery_attempts: u32,
    pub recovered_at: Option<i64>,
}

impl SqliteStore {
    /// Every open Task's primary conversation in this Machine's checkout of
    /// the Task whose agent did not end by the person quitting (exit 0, or 130
    /// for Ctrl+C) and that has starts left, most recently active Task first.
    /// A moved or completed conversation is not one.
    pub fn recoverable_task_conversations(&self) -> StoreResult<Vec<RecoverableConversation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare_cached(&format!(
            "SELECT s.id,t.id,p.id,COALESCE(p.completed_at,l.completed_at),l.exit_code,
                p.pid,p.os_started_at,s.recovery_attempts,s.recovered_at
             FROM task_primary_sessions tp
             JOIN tasks t ON t.id=tp.task_id
             JOIN task_checkouts c ON c.task_id=t.id
             JOIN agent_sessions s ON s.id=tp.session_id
             JOIN processes p ON p.id=s.agent_process_id
             LEFT JOIN processes l ON l.id=p.parent_lf_process_id
             WHERE {open} AND s.completed_at IS NULL AND s.interactive=1
               AND s.moved_to_machine_id IS NULL
               AND rtrim(s.cwd,'/')=rtrim(c.worktree,'/')
               AND s.recovery_attempts<?1
               AND (l.exit_code IS NULL OR l.exit_code NOT IN (0,130))
             ORDER BY t.updated_at DESC,t.id",
            open = super::durable::task_open_sql("t"),
        ))?;
        let rows = statement.query_map([RECOVERY_ATTEMPTS], |row| {
            let ended_at: Option<i64> = row.get(3)?;
            Ok(RecoverableConversation {
                session: row.get(0)?,
                task: TaskId::from_raw(row.get::<_, String>(1)?),
                agent: row.get(2)?,
                ended_at,
                exit_code: row.get(4)?,
                identity: if ended_at.is_none() {
                    row.get::<_, Option<u32>>(5)?.zip(row.get(6)?)
                } else {
                    None
                },
                recovery_attempts: row.get(7)?,
                recovered_at: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Count a start of `session` for its ended `agent`, and return its number.
    /// `None` when another agent has started since: nothing is counted.
    pub fn record_recovery_start(
        &self,
        session: &LfSessionId,
        agent: &AgentProcessId,
        at: i64,
    ) -> StoreResult<Option<u32>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "UPDATE agent_sessions SET recovery_attempts=recovery_attempts+1,recovered_at=?3
                 WHERE id=?1 AND agent_process_id=?2 RETURNING recovery_attempts",
                params![session.as_str(), agent, at],
                |row| row.get(0),
            )
            .optional()?)
    }
}
