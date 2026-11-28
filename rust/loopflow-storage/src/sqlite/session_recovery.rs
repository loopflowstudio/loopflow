//! Primary conversations of open Tasks checked out here, for the server's
//! recovery pass to start again, and the counters it keeps on them.

use loopflow_model::durable::TaskId;
use loopflow_model::id::{AgentProcessId, LfSessionId};
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::StoreResult;

/// Starts after which the pass stops trying and the Task says which command
/// continues.
pub const CONVERSATION_STARTS: u32 = 5;

/// The primary conversation of an open Task checked out on this Machine, with
/// what decides whether the server starts it. Recovery launches it in the
/// Session's own directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskConversation {
    pub session: LfSessionId,
    pub task: TaskId,
    /// The agent generation that ended, `None` before the first. A start
    /// names it, so one death never starts two providers.
    pub agent: Option<AgentProcessId>,
    pub start_failure: Option<String>,
    /// When the agent's end was recorded; `None` while its row is unfinished.
    pub ended_at: Option<i64>,
    /// The provider's exit, as its launcher saw it. `None` when nobody saw
    /// the end: the holder died, or the Machine restarted. It is written
    /// just before the end, so it is read on its own.
    pub exit_code: Option<i32>,
    /// The agent's recorded OS identity.
    pub identity: Option<(u32, i64)>,
    pub starts_since_turn: u32,
    pub last_started_at: Option<i64>,
}

/// SQL: Session row `s` is the primary conversation of open Task `t` checked
/// out on this Machine. The recovery pass lists these rows and a Session's
/// summary asks it of one, so both read one definition.
pub(super) fn task_conversation_here_sql(s: &str, t: &str) -> String {
    format!(
        "({open} AND EXISTS(SELECT 1 FROM task_checkouts pc WHERE pc.task_id={t}.id)
          AND EXISTS(SELECT 1 FROM task_primary_sessions tp WHERE tp.session_id={s}.id AND tp.task_id={t}.id)
          AND {s}.completed_at IS NULL AND {s}.interactive=1
          AND {s}.moved_to_machine_id IS NULL)",
        open = super::durable::task_open_sql(t),
    )
}

impl SqliteStore {
    /// Every open Task's primary conversation for Tasks checked out on this
    /// Machine, most recently active Task first. Whether the server starts
    /// one is the caller's rule, not this read's.
    pub fn task_conversations_here(&self) -> StoreResult<Vec<TaskConversation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare_cached(&format!(
            "SELECT s.id,t.id,s.agent_process_id,s.agent_ended_at,s.agent_exit_code,
                s.agent_pid,s.agent_os_started_at,s.starts_since_turn,s.last_started_at,s.start_failure
             FROM task_primary_sessions tp
             JOIN tasks t ON t.id=tp.task_id
             JOIN agent_sessions s ON s.id=tp.session_id
             WHERE {here}
             ORDER BY t.updated_at DESC,t.id",
            here = task_conversation_here_sql("s", "t"),
        ))?;
        let rows = statement.query_map([], |row| {
            Ok(TaskConversation {
                session: row.get(0)?,
                task: TaskId::from_raw(row.get::<_, String>(1)?),
                agent: row.get(2)?,
                ended_at: row.get(3)?,
                exit_code: row.get(4)?,
                identity: row.get::<_, Option<u32>>(5)?.zip(row.get(6)?),
                starts_since_turn: row.get(7)?,
                last_started_at: row.get(8)?,
                start_failure: row.get(9)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Count a start of `session` for its ended `agent` (`None`: it never had
    /// one), and return its number. `None` when another agent has started
    /// since: nothing is counted.
    pub fn record_conversation_start(
        &self,
        session: &LfSessionId,
        agent: Option<&AgentProcessId>,
        at: i64,
    ) -> StoreResult<Option<u32>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "UPDATE agent_sessions SET starts_since_turn=starts_since_turn+1,last_started_at=?3
                 WHERE id=?1 AND agent_process_id IS ?2 RETURNING starts_since_turn",
                params![session.as_str(), agent, at],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// A person started `session` at `at`: it is start zero. The pass's count
    /// begins again, and its next start waits the first backoff from this one,
    /// so it never races the person's own.
    pub fn record_persons_start(&self, session: &LfSessionId, at: i64) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE agent_sessions SET starts_since_turn=0,start_failure=NULL,last_started_at=?2
             WHERE id=?1",
            params![session.as_str(), at],
        )?;
        Ok(())
    }

    /// The first agent of `session` started: the failed starts before it are
    /// forgotten, so it reads as running, not as started again, and its first
    /// death comes back at once.
    pub fn record_first_start(&self, session: &LfSessionId) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE agent_sessions SET starts_since_turn=0,start_failure=NULL,last_started_at=NULL
             WHERE id=?1 AND (starts_since_turn>0 OR start_failure IS NOT NULL)",
            [session.as_str()],
        )?;
        Ok(())
    }

    /// Why the last start of `session` failed, or `None` once one succeeded.
    pub fn record_start_failure(
        &self,
        session: &LfSessionId,
        failure: Option<&str>,
    ) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE agent_sessions SET start_failure=?2 WHERE id=?1 AND start_failure IS NOT ?2",
            params![session.as_str(), failure],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use crate::query_plans::{growing_scans, statements};

    #[test]
    fn task_conversations_here_follows_task_checkout_not_session_cwd() {
        let (_home, store, task) = crate::test_fixture::store_with_task();
        let wave_id = store.task(&task).unwrap().unwrap().wave_id;
        let session = store
            .test_session(
                "session_split_checkout",
                &crate::session_record::new_artifact_key(),
            )
            .id;
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo.task')",
                [task.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,interactive=1,cwd='/repo.research'
                 WHERE id=?1",
                params![
                    session.as_str(),
                    task.as_str(),
                    wave_id.as_ref().map(|w| w.as_str())
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
                params![task.as_str(), session.as_str()],
            )
            .unwrap();
        }

        let unbound = store
            .test_session(
                "session_unbound",
                &crate::session_record::new_artifact_key(),
            )
            .id;
        let summary_predicate = format!(
            "SELECT {here}
             FROM agent_sessions a LEFT JOIN tasks t ON t.id=a.task_id WHERE a.id=?1",
            here = super::task_conversation_here_sql("a", "t"),
        );
        let summary_checked_out_here = |id: &str| -> bool {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(&summary_predicate, [id], |row| row.get(0))
                .unwrap()
        };

        let listed = store.task_conversations_here().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].session, session);
        assert_eq!(listed[0].task, task);
        assert!(summary_checked_out_here(session.as_str()));
        assert!(!summary_checked_out_here(unbound.as_str()));

        // Single-Session summary query plans by index on growing tables.
        let traced = statements(&store, || {
            assert!(summary_checked_out_here(session.as_str()));
        });
        let scans = growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));

        // Removing the local checkout row excludes the Task on both paths.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM task_checkouts WHERE task_id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(store.task_conversations_here().unwrap().is_empty());
        assert!(!summary_checked_out_here(session.as_str()));
    }
}
