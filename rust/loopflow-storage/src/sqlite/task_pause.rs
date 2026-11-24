//! A pause belongs to the Task on the Machine running it. The interrupt and
//! the identities needed to finish a cut-short stop commit together.
use loopflow_model::durable::TaskId;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::{children, durable, SqliteStore};
use crate::execution::TaskState;
use crate::process::Starter;
use crate::rows::now_unix;
use crate::work::task::TaskEventKind;
use crate::{StoreError, StoreResult};

/// Unsettled cleanup may still have a recorded worker OS session.
#[derive(Debug, Clone, Copy)]
pub struct PauseCleanup {
    pub worker_session: Option<u32>,
}

/// Members observed together in a worker's private OS session, before its
/// leader was asked to exit. A surviving member pins the session on retry.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoppingSession {
    pub session: u32,
    pub members: Vec<(u32, i64)>,
}

/// The Task holding this primary conversation, if it is paused.
pub(super) fn paused_primary_sql(session: &str) -> String {
    format!(
        "SELECT h.task_id FROM task_primary_sessions tp
             JOIN task_pauses h ON h.task_id=tp.task_id WHERE tp.session_id={session}"
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPause {
    pub at: i64,
    pub by: Starter,
}

pub(super) fn require_settled_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let pending: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_pauses WHERE task_id=?1 AND stopping_processes IS NOT NULL)",
        [task.as_str()], |row| row.get(0))?;
    if pending {
        return Err(StoreError::InvalidData(format!(
            "Task is still stopping; repeat `lf task pause {task}` to finish cleanup"
        )));
    }
    Ok(())
}

pub(super) fn clear_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    require_settled_in(conn, task)?;
    Ok(conn.execute("DELETE FROM task_pauses WHERE task_id=?1", [task.as_str()])? != 0)
}

pub fn run_refusal(issue: &str) -> String {
    format!("Task is paused; run `lf task unpause {issue}` to resume it")
}

pub(super) fn require_unpaused_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let issue: Option<String> = conn.query_row(
        "SELECT t.issue_identifier FROM task_pauses p JOIN tasks t ON t.id=p.task_id WHERE p.task_id=?1",
        [task.as_str()], |row| row.get(0)).optional()?;
    if let Some(issue) = issue {
        return Err(StoreError::InvalidData(run_refusal(&issue)));
    }
    Ok(())
}

fn stopping_processes_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<Vec<StoppingSession>>> {
    let jobs: Option<String> = conn.query_row(
        "SELECT stopping_processes FROM task_pauses WHERE task_id=?1 AND stopping_processes IS NOT NULL",
        [task.as_str()], |r| r.get(0)).optional()?;
    Ok(jobs.map(|json| serde_json::from_str(&json)).transpose()?)
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

    pub fn stopping_processes(&self, task: &TaskId) -> StoreResult<Option<Vec<StoppingSession>>> {
        stopping_processes_in(&self.conn.lock().expect("store mutex poisoned"), task)
    }

    /// The portable part of a pause. Process identities belong to the source Machine.
    pub fn task_pause(&self, task: &TaskId) -> StoreResult<Option<TaskPause>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                "SELECT paused_at,paused_by FROM task_pauses WHERE task_id=?1",
                [task.as_str()],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        row.map(|(at, by)| {
            Ok(TaskPause {
                at,
                by: serde_json::from_str(&by)?,
            })
        })
        .transpose()
    }

    pub fn place_moved_task_pause(
        &self,
        task: &TaskId,
        pause: Option<&TaskPause>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        clear_in(&tx, task)?;
        if let Some(pause) = pause {
            tx.execute(
                "INSERT INTO task_pauses(task_id,paused_at,paused_by) VALUES(?1,?2,?3)",
                params![task.as_str(), pause.at, serde_json::to_string(&pause.by)?],
            )?;
        }
        tx.commit()?;
        Ok(())
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
                return Ok(stopping_processes_in(&tx, task)?.unwrap_or_default());
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
        if !clear_in(&tx, task)? {
            return Err(StoreError::InvalidData("Task is not paused".into()));
        }
        settle_carrier(&tx, task)?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::sqlite::durable::task_state_sql;

    #[test]
    fn completing_reopening_and_abandoning_a_task_clear_its_hold() {
        use crate::execution::TaskState;
        use crate::process::Starter;
        use loopflow_model::durable::WorkRef;

        let (_home, store, fixture, _) = crate::test_fixture::task();
        let id = fixture.id;
        let task = store
            .create_task(&crate::work::task::NewTask {
                id: id.clone(),
                repository_id: fixture.repository_id,
                parent_task_id: None,
                project_id: None,
                title: "Paused work".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .take_up_task_graph(&id, r#"{"name":"code","steps":[]}"#)
            .unwrap();
        let pause = || {
            store
                .begin_task_pause(&id, &Starter::Person { name: None }, vec![])
                .unwrap();
            store.settle_task_pause(&id, false).unwrap();
        };
        pause();
        store.reopen_task(&id, None).unwrap();
        assert!(store.task_pause(&id).unwrap().is_none());

        pause();
        let request = store.request_task_completion(&id, None).unwrap().unwrap();
        store.complete_task(&task, request).unwrap();
        assert_eq!(store.task_state(&id).unwrap(), TaskState::Done);
        assert!(store.task_pause(&id).unwrap().is_none());

        store.reopen_task(&id, None).unwrap();
        pause();
        store
            .abandon(&WorkRef::Task(id.clone()), "superseded")
            .unwrap();
        assert_eq!(store.task_state(&id).unwrap(), TaskState::Abandoned);
        assert!(store.task_pause(&id).unwrap().is_none());
    }

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
             ANALYZE main;").unwrap();
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
        let count = conn.query_row(&query, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(count, 1000);
        eprintln!("{count} paused among 10,000 Tasks in {:?}", now.elapsed());
    }
}
