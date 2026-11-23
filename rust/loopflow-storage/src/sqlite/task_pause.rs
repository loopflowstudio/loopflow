//! A pause belongs to the Task on the Machine running it. The interrupt and
//! the identities needed to finish a cut-short stop commit together.
use loopflow_model::durable::TaskId;
use loopflow_model::id::AgentProcessId;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::{children, durable, sessions::SESSION_GRAPH, SqliteStore};
use crate::execution::TaskState;
use crate::process::Starter;
use crate::rows::now_unix;
use crate::work::task::TaskEventKind;
use crate::{StoreError, StoreResult};

/// Members observed together in a worker's private OS session, before its
/// leader was asked to exit. A surviving member pins the session on retry.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoppingSession {
    pub session: u32,
    pub members: Vec<(u32, i64)>,
}

pub(super) fn require_unpaused_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let issue: Option<String> = conn.query_row(
        "SELECT t.issue_identifier FROM task_pauses p JOIN tasks t ON t.id=p.task_id WHERE p.task_id=?1",
        [task.as_str()], |row| row.get(0)).optional()?;
    if let Some(issue) = issue {
        return Err(StoreError::InvalidData(format!(
            "Task is paused; run `lf task unpause {issue}` to resume it"
        )));
    }
    Ok(())
}

fn settle_carrier(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    conn.execute(
        "UPDATE task_graphs SET run_pid=NULL,run_os_started_at=NULL,run_died_at=NULL
                  WHERE task_id=?1 AND (run_pid IS NOT NULL OR run_died_at IS NOT NULL)",
        [task.as_str()],
    )?;
    Ok(())
}

impl SqliteStore {
    pub fn require_task_unpaused(&self, task: &TaskId) -> StoreResult<()> {
        require_unpaused_in(&self.conn.lock().expect("store mutex poisoned"), task)
    }

    /// Only providers launched for a step of this Task's Graph own build jobs.
    pub fn task_worker_agents(&self, task: &TaskId) -> StoreResult<Vec<AgentProcessId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT s.agent_process_id FROM agent_sessions s INDEXED BY agent_sessions_unended_agent
             WHERE s.agent_process_id IS NOT NULL AND s.agent_ended_at IS NULL
             AND s.agent_started_at IS NOT NULL AND s.completed_at IS NULL
             AND (SELECT g.task_id FROM graph_processes g WHERE g.lf_process_id={SESSION_GRAPH})=?1"
        ))?;
        let rows = query.query_map([task.as_str()], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn begin_task_pause(
        &self,
        task: &TaskId,
        by: &Starter,
        jobs: Vec<StoppingSession>,
    ) -> StoreResult<Vec<StoppingSession>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        match durable::task_state_in(&tx, task)? {
            TaskState::Unstarted => {
                return Err(StoreError::InvalidData(
                    "Task has not started; nothing to pause".into(),
                ))
            }
            TaskState::Done | TaskState::Abandoned => {
                return Err(StoreError::InvalidData(
                    "Task is closed; nothing to pause".into(),
                ))
            }
            TaskState::Paused => {
                let saved: Option<String> = tx.query_row(
                    "SELECT stopping_processes FROM task_pauses WHERE task_id=?1",
                    [task.as_str()],
                    |r| r.get(0),
                )?;
                return Ok(saved
                    .map(|s| serde_json::from_str(&s))
                    .transpose()?
                    .unwrap_or_default());
            }
            _ => {}
        }
        tx.execute("INSERT INTO task_pauses(task_id,paused_at,paused_by,stopping_processes) VALUES(?1,?2,?3,?4)",
            params![task.as_str(), now_unix(), serde_json::to_string(by)?, serde_json::to_string(&jobs)?])?;
        children::insert_task_event_in(&tx, task, &TaskEventKind::Interrupt)?;
        tx.commit()?;
        Ok(jobs)
    }

    pub fn settle_task_pause(&self, task: &TaskId, draining: bool) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !draining {
            settle_carrier(&tx, task)?;
        }
        tx.execute("UPDATE agent_limit_recoveries SET retry_at=NULL,updated_at=?2
                    WHERE retry_at IS NOT NULL AND lf_process_id IN (
                      SELECT gs.lf_process_id FROM graph_processes g JOIN graph_process_steps gs ON gs.graph_lf_process_id=g.lf_process_id WHERE g.task_id=?1)",
                    params![task.as_str(), now_unix()])?;
        tx.execute("UPDATE task_pauses SET stopping_processes=NULL WHERE task_id=?1 AND stopping_processes IS NOT NULL", [task.as_str()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn release_task_pause(&self, task: &TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let pending: Option<bool> = tx
            .query_row(
                "SELECT stopping_processes IS NOT NULL FROM task_pauses WHERE task_id=?1",
                [task.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        match pending {
            None => return Err(StoreError::InvalidData("Task is not paused".into())),
            Some(true) => {
                return Err(StoreError::InvalidData(format!(
                    "Task is still stopping; repeat `lf task pause {task}` to finish cleanup"
                )))
            }
            Some(false) => (),
        }
        settle_carrier(&tx, task)?;
        tx.execute("DELETE FROM task_pauses WHERE task_id=?1", [task.as_str()])?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sqlite::durable::task_state_sql;

    #[test]
    fn board_looks_up_each_hold_by_task_on_a_ten_thousand_task_machine() {
        let (_home, store, seed) = crate::test_fixture::store_with_task();
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i+1 FROM n WHERE i<9999)
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             SELECT printf('task_%032x',i),t.repository_id,printf('OPS-%d',i),1
             FROM n JOIN tasks t ON t.id=?1",
            [seed.as_str()],
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO task_checkouts(task_id,worktree,branch)
             SELECT id,'/work/'||id,id FROM tasks WHERE rowid%10=0 AND issue_identifier LIKE 'OPS-%';
             INSERT INTO task_pauses(task_id,paused_at,paused_by)
             SELECT task_id,1,'{\"kind\":\"person\",\"name\":\"test\"}' FROM task_checkouts WHERE worktree LIKE '/work/%';
             ANALYZE;").unwrap();
        let query = format!(
            "SELECT count(*) FROM tasks t WHERE ({})='paused'",
            task_state_sql("t")
        );
        let plan = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {query}"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(
            plan.iter().any(|row| row.contains(
                "SEARCH p USING COVERING INDEX sqlite_autoindex_task_pauses_1 (task_id=?)"
            )),
            "{plan:?}"
        );
        let now = std::time::Instant::now();
        let count = conn
            .query_row(&query, [], |r| r.get::<_, usize>(0))
            .unwrap();
        assert_eq!(count, 1000);
        eprintln!("{count} paused among 10,000 Tasks in {:?}", now.elapsed());
    }
}
