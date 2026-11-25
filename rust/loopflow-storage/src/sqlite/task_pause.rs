//! A pause belongs to the Task on the Machine running it. The Graph stop and
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPause {
    pub at: i64,
    pub by: Starter,
}

pub(super) fn require_settled_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let pending: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_pauses WHERE task_id=?1 AND stopping_sessions IS NOT NULL)",
        [task.as_str()], |row| row.get(0))?;
    if pending {
        let issue = children::short_task_id(conn, task)?.unwrap_or_else(|| task.to_string());
        return Err(StoreError::InvalidData(format!(
            "Task is still stopping; repeat `lf task pause {issue}` to finish cleanup"
        )));
    }
    Ok(())
}

pub(super) fn clear_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    require_settled_in(conn, task)?;
    Ok(conn.execute("DELETE FROM task_pauses WHERE task_id=?1", [task.as_str()])? != 0)
}

pub fn run_refusal(issue: &str) -> String {
    format!("Task is paused; run `lf task unpause {issue}` to lift the hold")
}

pub(super) fn is_paused_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_pauses WHERE task_id=?1)",
        [task.as_str()],
        |row| row.get(0),
    )?)
}

pub(super) fn require_unpaused_in(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    if is_paused_in(conn, task)? {
        let issue = children::short_task_id(conn, task)?.unwrap_or_else(|| task.to_string());
        return Err(StoreError::InvalidData(run_refusal(&issue)));
    }
    Ok(())
}

fn stopping_sessions_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<Vec<StoppingSession>>> {
    let sessions: Option<String> = conn.query_row(
        "SELECT stopping_sessions FROM task_pauses WHERE task_id=?1 AND stopping_sessions IS NOT NULL",
        [task.as_str()], |r| r.get(0)).optional()?;
    Ok(sessions
        .map(|json| serde_json::from_str(&json))
        .transpose()?)
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

    pub fn stopping_sessions(&self, task: &TaskId) -> StoreResult<Option<Vec<StoppingSession>>> {
        stopping_sessions_in(&self.conn.lock().expect("store mutex poisoned"), task)
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
        sessions: Vec<StoppingSession>,
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
                return Ok(stopping_sessions_in(&tx, task)?.unwrap_or_default());
            }
            _ => {}
        }
        tx.execute("INSERT INTO task_pauses(task_id,paused_at,paused_by,stopping_sessions) VALUES(?1,?2,?3,?4)",
            params![task.as_str(), now_unix(), serde_json::to_string(by)?, serde_json::to_string(&sessions)?])?;
        children::insert_task_event_in(&tx, task, &TaskEventKind::Paused)?;
        tx.commit()?;
        Ok(sessions)
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
        tx.execute("UPDATE task_pauses SET stopping_sessions=NULL WHERE task_id=?1 AND stopping_sessions IS NOT NULL", [task.as_str()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn release_task_pause(&self, task: &TaskId, settle_step: Option<&str>) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !clear_in(&tx, task)? {
            return Err(StoreError::InvalidData("Task is not paused".into()));
        }
        match settle_step {
            Some(step) => {
                super::task_work::set_step_in(&tx, task, step)?;
            }
            None => settle_carrier(&tx, task)?,
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::run_refusal;
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
        // The refusal names the Task the way people type it, not its stored key.
        let refusal = store.require_task_unpaused(&id).unwrap_err().to_string();
        assert!(
            refusal.ends_with(&run_refusal(&task.identifier)),
            "{refusal}"
        );
        store.reopen_task(&id, None, false).unwrap();
        assert!(store.task_pause(&id).unwrap().is_none());

        pause();
        let request = store.request_task_completion(&id, None).unwrap().unwrap();
        store.complete_task(&task, request).unwrap();
        assert_eq!(store.task_state(&id).unwrap(), TaskState::Done);
        assert!(store.task_pause(&id).unwrap().is_none());

        store.reopen_task(&id, None, false).unwrap();
        pause();
        store
            .abandon(&WorkRef::Task(id.clone()), "superseded")
            .unwrap();
        assert_eq!(store.task_state(&id).unwrap(), TaskState::Abandoned);
        assert!(store.task_pause(&id).unwrap().is_none());
    }

    #[test]
    fn arriving_at_end_while_paused_keeps_the_hold_without_requesting_completion() {
        use crate::execution::TaskState;
        use crate::graph_process::ExecutionStart;
        use crate::process::Starter;
        use crate::sqlite::task_work::END;
        use loopflow_model::id::{LfProcessId, TraceId};

        let (_home, store, fixture, _) = crate::test_fixture::task();
        let id = fixture.id;
        store
            .create_task(&crate::work::task::NewTask {
                id: id.clone(),
                repository_id: fixture.repository_id,
                parent_task_id: None,
                project_id: None,
                title: "Draining final step".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        store
            .take_up_task_graph(&id, r#"{"name":"landing","steps":[]}"#)
            .unwrap();
        let carrier = LfProcessId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
                rusqlite::params![carrier, TraceId::new()],
            )
            .unwrap();
        }
        assert!(store
            .start_task_graph_run(
                &id,
                "start",
                None,
                r#"{"index":0}"#,
                (&carrier, &ExecutionStart::reserved(1)),
            )
            .unwrap());
        store
            .begin_task_pause(&id, &Starter::Person { name: None }, vec![])
            .unwrap();

        assert!(store.arrive_task_graph(&id, &carrier, END).unwrap());
        assert_eq!(store.task_state(&id).unwrap(), TaskState::Paused);
        assert!(store.task_pause(&id).unwrap().is_some());
        assert!(store.task_completion_pending(&id).unwrap().is_none());
        let row = store.task_graph_row(&id).unwrap().unwrap();
        assert_eq!(row.step, END);
        assert!(row.run.is_none());

        store.settle_task_pause(&id, false).unwrap();
        // Settling a dead stop cursor on release clears the hold and run cursor together.
        assert!(store
            .start_task_graph_run(
                &id,
                END,
                None,
                r#"{"index":0}"#,
                (&carrier, &ExecutionStart::reserved(1)),
            )
            .unwrap());
        store.release_task_pause(&id, Some("ship-review")).unwrap();
        assert!(store.task_pause(&id).unwrap().is_none());
        let settled = store.task_graph_row(&id).unwrap().unwrap();
        assert_eq!(settled.step, "ship-review");
        assert!(settled.run.is_none());
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
