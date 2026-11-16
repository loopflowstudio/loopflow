//! What a plan reads about each of its Tasks, one statement per concern over
//! every Task in scope, so a reading's cost does not grow a statement per Task.
use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::children::{
    ids_json, map_task_event_row, map_task_pr_row, task_acquisitions_in, TASK_PR_COLUMNS,
};
use super::SqliteStore;
use crate::durable::{RepositoryId, TaskOwner, TaskState};
use crate::store::planning_sync::PlanningSyncStatus;
use crate::store::{StoreError, StoreResult, TaskAcquisition};
use crate::work::task::{Task, TaskEvent, TaskId, TaskPr, TaskPrId};

/// Durable facts about a set of Tasks, read in one snapshot.
#[derive(Debug)]
pub struct TaskDetailFacts {
    pub states: HashMap<TaskId, TaskState>,
    pub active_prs: HashMap<TaskId, TaskPr>,
    /// Predecessor PRs the Tasks are stacked on.
    pub parent_prs: HashMap<TaskPrId, TaskPr>,
    /// A pending completion request's reason.
    pub completion_pending: HashMap<TaskId, String>,
    pub issue_urls: HashMap<TaskId, String>,
    pub owners: HashMap<TaskId, TaskOwner>,
    pub acquisitions: HashMap<TaskId, TaskAcquisition>,
    pub latest_events: HashMap<TaskId, TaskEvent>,
    /// The name of the graph each Task took up.
    pub graph_names: HashMap<TaskId, String>,
    /// The step each Task stands at on its graph; absent while a run carries it.
    pub steps: HashMap<TaskId, String>,
    /// Tasks handed off to Auto-Ship.
    pub shipped: HashSet<TaskId>,
    pub planning_sync: HashMap<TaskId, PlanningSyncStatus>,
    pub repository_paths: HashMap<RepositoryId, String>,
}

impl SqliteStore {
    pub fn task_detail_facts(&self, tasks: &[&Task]) -> StoreResult<TaskDetailFacts> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids: Vec<TaskId> = tasks.iter().map(|task| task.id.clone()).collect();
        let json = ids_json(ids.iter().map(TaskId::as_str));
        let facts = TaskDetailFacts {
            states: states_in(&snapshot, &json)?,
            active_prs: keyed(
                &snapshot,
                &format!(
                    "{TASK_PR_COLUMNS} WHERE historical=0 AND task_id IN (SELECT value FROM json_each(?1))"
                ),
                &json,
                |row| {
                    let pr = map_task_pr_row(row)?;
                    Ok((pr.task_id.clone(), pr))
                },
            )?,
            parent_prs: keyed(
                &snapshot,
                &format!(
                    "{TASK_PR_COLUMNS} WHERE id IN (SELECT parent_pr_id FROM task_checkouts
                     WHERE task_id IN (SELECT value FROM json_each(?1)))"
                ),
                &json,
                |row| {
                    let pr = map_task_pr_row(row)?;
                    Ok((pr.id.clone(), pr))
                },
            )?,
            completion_pending: keyed(
                &snapshot,
                "SELECT id,COALESCE(completion_error,'Completion requested') FROM tasks
                 WHERE id IN (SELECT value FROM json_each(?1)) AND completion_request IS NOT NULL",
                &json,
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), row.get::<_, String>(1)?)),
            )?,
            issue_urls: keyed(
                &snapshot,
                "SELECT id,planning_url FROM tasks
                 WHERE id IN (SELECT value FROM json_each(?1)) AND planning_url IS NOT NULL",
                &json,
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), row.get::<_, String>(1)?)),
            )?,
            owners: super::task_ownership::owners_in(&snapshot, &ids)?,
            acquisitions: task_acquisitions_in(&snapshot, tasks, true)?,
            latest_events: keyed(
                &snapshot,
                "SELECT id, task_id, kind_json, created_at FROM task_events
                 WHERE id IN (SELECT max(id) FROM task_events
                    WHERE task_id IN (SELECT value FROM json_each(?1)) GROUP BY task_id)",
                &json,
                |row| {
                    let event = map_task_event_row(row)?;
                    Ok((event.task_id.clone(), event))
                },
            )?,
            graph_names: keyed(
                &snapshot,
                "SELECT task_id,json_extract(graph,'$.name') FROM task_graphs
                 WHERE task_id IN (SELECT value FROM json_each(?1))",
                &json,
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), row.get::<_, String>(1)?)),
            )?,
            steps: keyed(
                &snapshot,
                "SELECT task_id,step FROM task_graphs
                 WHERE task_id IN (SELECT value FROM json_each(?1)) AND run_start IS NULL",
                &json,
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), row.get::<_, String>(1)?)),
            )?,
            shipped: task_set(
                &snapshot,
                "SELECT id FROM tasks
                 WHERE id IN (SELECT value FROM json_each(?1)) AND shipped_at IS NOT NULL",
                &json,
            )?,
            planning_sync: super::planning_sync::task_planning_syncs_in(&snapshot, &ids)?,
            repository_paths: repository_paths_in(&snapshot)?,
        };
        snapshot.commit()?;
        Ok(facts)
    }
}

fn keyed<K: std::hash::Hash + Eq, V>(
    conn: &Connection,
    sql: &str,
    ids: &str,
    map: impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<(K, V)>,
) -> StoreResult<HashMap<K, V>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([ids], map)?;
    rows.collect::<rusqlite::Result<_>>()
        .map_err(StoreError::from)
}

fn task_set(conn: &Connection, sql: &str, ids: &str) -> StoreResult<HashSet<TaskId>> {
    let mut statement = conn.prepare(sql)?;
    let rows = statement.query_map([ids], |row| Ok(TaskId::from_raw(row.get::<_, String>(0)?)))?;
    rows.collect::<rusqlite::Result<_>>()
        .map_err(StoreError::from)
}

/// Terminal disposition never destroys the retained execution inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TaskStateInputs {
    pub completed: bool,
    pub abandoned: bool,
    pub completed_at: Option<String>,
    pub planning_state: Option<String>,
    pub completion_request: Option<i64>,
    pub execution: crate::durable::TaskExecution,
}

impl TaskStateInputs {
    pub fn state(&self) -> TaskState {
        if self.abandoned {
            TaskState::Abandoned
        } else if self.completed {
            TaskState::Done
        } else {
            self.execution.state()
        }
    }
}

/// Every requested Task's independently readable inputs, in one statement.
pub(super) fn state_inputs_in(
    conn: &Connection,
    ids: &str,
) -> StoreResult<HashMap<TaskId, TaskStateInputs>> {
    let super::durable::TaskExecutionSql {
        checkout,
        graph_running,
        carrier_running,
        held,
        approved,
    } = super::durable::task_execution_sql("t");
    keyed(conn, &format!(
        "SELECT t.id,t.planning_completed,t.abandoned_at IS NOT NULL,({checkout}),({graph_running}),({held}),({approved}),t.planning_completed_at,t.completion_request,COALESCE(t.planning_state,CASE WHEN t.planning_completed=1 THEN 'completed' END),({carrier_running}) \
         FROM tasks t WHERE t.id IN (SELECT value FROM json_each(?1))"
    ), ids, |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), TaskStateInputs {
        completed: row.get(1)?, abandoned: row.get(2)?,
        completed_at: row.get(7)?, completion_request: row.get(8)?, planning_state: row.get(9)?,
        execution: crate::durable::TaskExecution {
            checkout: row.get(3)?, graph_running: row.get(4)?, carrier_running: row.get(10)?, held: row.get(5)?, approved: row.get(6)?,
        },
    })))
}

/// Each listed Task's state, in one statement; `ids` is a JSON array.
pub(super) fn states_in(conn: &Connection, ids: &str) -> StoreResult<HashMap<TaskId, TaskState>> {
    Ok(state_inputs_in(conn, ids)?
        .into_iter()
        .map(|(id, inputs)| (id, inputs.state()))
        .collect())
}

fn repository_paths_in(conn: &Connection) -> StoreResult<HashMap<RepositoryId, String>> {
    let mut statement = conn.prepare("SELECT id,repo FROM repository_plans")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (id, repo) = row?;
        let id =
            RepositoryId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))?;
        Ok((id, repo))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{state_inputs_in, states_in};
    use crate::durable::{TaskId, TaskState};
    use crate::store::sqlite::SqliteStore;
    use rusqlite::params;

    #[test]
    fn terminal_tasks_retain_execution_and_both_state_readers_agree() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!(
                "../../../tests/fixtures/saved_selection_work.sql"
            ))
            .unwrap();
        let task = TaskId::parse("task_22222222222222222222222222222222").unwrap();
        let process = store.test_lf_process();
        let conn = store.conn.lock().unwrap();
        let ids = super::super::children::ids_json([task.as_str()]);
        // Reopening keeps the recorded structure; these are its open destinations.
        for (checkout, position, running, held, approved, open) in [
            (false, "start", false, false, false, TaskState::Unstarted),
            (false, "start", false, false, true, TaskState::Unstarted),
            (true, "start", false, false, false, TaskState::Idle),
            (true, "start", false, false, true, TaskState::Idle),
            (false, "review", false, true, false, TaskState::Active),
            (false, "review", false, true, true, TaskState::Unstarted),
            (true, "review", false, true, false, TaskState::Active),
            (true, "review", false, true, true, TaskState::Idle),
            (false, "review", true, false, false, TaskState::Moving),
            (false, "review", true, false, true, TaskState::Shipping),
            (true, "review", true, false, false, TaskState::Moving),
            (true, "review", true, false, true, TaskState::Shipping),
        ] {
            for (completed, abandoned) in [(false, false), (true, false), (true, true)] {
                conn.execute(
                    "DELETE FROM task_checkouts WHERE task_id=?1",
                    [task.as_str()],
                )
                .unwrap();
                if checkout {
                    conn.execute(
                        "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,'/repo/fix-2')",
                        [task.as_str()],
                    )
                    .unwrap();
                }
                conn.execute("INSERT OR REPLACE INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,'{}',?2,?3,?4,1)",
                            params![task.as_str(), position, running.then_some("{}"), running.then_some(process.as_str())]).unwrap();
                conn.execute("UPDATE tasks SET planning_completed=?2,abandoned_at=?3,shipped_at=?4,planning_completed_at='2026-11-15T00:00:00Z',completion_request=17 WHERE id=?1",
                            params![task.as_str(), completed, abandoned.then_some(1), approved.then_some(1)]).unwrap();
                let inputs = state_inputs_in(&conn, &ids).unwrap().remove(&task).unwrap();
                assert_eq!(inputs.execution.checkout, checkout);
                assert_eq!(inputs.execution.graph_running, running);
                assert!(!inputs.execution.carrier_running);
                assert_eq!(inputs.execution.held, held);
                assert_eq!(inputs.execution.approved, approved);
                assert_eq!(inputs.completed_at.as_deref(), Some("2026-11-15T00:00:00Z"));
                assert_eq!(inputs.completion_request, Some(17));
                let expected = if abandoned {
                    TaskState::Abandoned
                } else if completed {
                    TaskState::Done
                } else {
                    open
                };
                assert_eq!(inputs.state(), expected);
                assert_eq!(states_in(&conn, &ids).unwrap()[&task], expected);
                assert_eq!(
                    super::super::durable::task_state_in(&conn, &task).unwrap(),
                    expected
                );
                let sql: String = conn
                    .query_row(
                        &format!(
                            "SELECT {} FROM tasks t WHERE id=?1",
                            super::super::durable::task_state_sql("t")
                        ),
                        [task.as_str()],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert_eq!(sql, expected.label());
            }
        }
    }

    #[test]
    fn reopen_releases_only_the_ship_carrier_while_graph_work_keeps_running() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!(
                "../../../tests/fixtures/saved_selection_work.sql"
            ))
            .unwrap();
        let task = TaskId::parse("task_22222222222222222222222222222222").unwrap();
        let ids = super::super::children::ids_json([task.as_str()]);
        let carrier = store.test_lf_process();
        let graph = store.test_lf_process();
        for graph_running in [false, true] {
            {
                let conn = store.conn.lock().unwrap();
                if graph_running {
                    conn.execute("INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at) VALUES(?1,'{}','review','{}',?2,1)",
                        params![task.as_str(), graph.as_str()]).unwrap();
                }
                conn.execute("INSERT INTO task_ship_events(task_id,kind,at,lf_process_id) VALUES(?1,'approved',1,?2)",
                    params![task.as_str(), carrier.as_str()]).unwrap();
                conn.execute(
                    "UPDATE tasks SET shipped_at=1,completion_request=17 WHERE id=?1",
                    [task.as_str()],
                )
                .unwrap();
                let inputs = state_inputs_in(&conn, &ids).unwrap().remove(&task).unwrap();
                assert!(inputs.execution.carrier_running);
                assert_eq!(inputs.execution.graph_running, graph_running);
                assert_eq!(inputs.state(), TaskState::Shipping);
            }
            store.reopen_task(&task, None).unwrap();
            let conn = store.conn.lock().unwrap();
            let inputs = state_inputs_in(&conn, &ids).unwrap().remove(&task).unwrap();
            assert!(!inputs.execution.carrier_running);
            assert!(!inputs.execution.approved);
            assert_eq!(inputs.execution.graph_running, graph_running);
            assert_eq!(
                inputs.state(),
                if graph_running {
                    TaskState::Moving
                } else {
                    TaskState::Idle
                }
            );
            let unfinished: bool = conn
                .query_row(
                    "SELECT completed_at IS NULL FROM processes WHERE id=?1",
                    [carrier.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(unfinished); // Withdrawal is not a process-exit observation.
            let sql: String = conn
                .query_row(
                    &format!(
                        "SELECT {} FROM tasks t WHERE id=?1",
                        super::super::durable::task_state_sql("t")
                    ),
                    [task.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(sql, inputs.state().label());
        }
    }

    #[test]
    fn execution_inputs_use_indexed_reads_at_ten_thousand_tasks() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let repository = store.ensure_repository("/input-plans").unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<10000)
             INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,created_at,updated_at)
             SELECT 'task_'||printf('%032x',i),?1,'INPUT-'||i,'Input Task',1,1 FROM n",
                [repository.as_str()],
            )
            .unwrap();
        let task = TaskId::parse("task_00000000000000000000000000000001").unwrap();
        let traced = crate::store::query_plans::statements(&store, || {
            assert_eq!(store.task_state(&task).unwrap(), TaskState::Unstarted);
            let conn = store.conn.lock().unwrap();
            let ids = super::super::children::ids_json([task.as_str()]);
            assert_eq!(state_inputs_in(&conn, &ids).unwrap().len(), 1);
        });
        let scans = crate::store::query_plans::growing_scans(&store, &traced);
        assert!(scans.is_empty(), "{}", scans.join("\n"));
    }

    #[test]
    fn reopen_preserves_graph_position_and_only_withdraws_conditional_approval() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!(
                "../../../tests/fixtures/saved_selection_work.sql"
            ))
            .unwrap();
        let task = TaskId::parse("task_22222222222222222222222222222222").unwrap();
        let ids = super::super::children::ids_json([task.as_str()]);
        for (completed, pending, expected, approved) in [
            (false, None, TaskState::Idle, true),
            (false, Some(17), TaskState::Active, false),
            (true, None, TaskState::Active, false),
        ] {
            {
                let conn = store.conn.lock().unwrap();
                conn.execute("INSERT OR REPLACE INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,'{}','review',1)", [task.as_str()]).unwrap();
                conn.execute("UPDATE tasks SET planning_completed=?2,planning_state=CASE WHEN ?2 THEN 'completed' ELSE 'started' END,completion_request=?3,planning_completed_at=CASE WHEN ?2 THEN '2026-11-15T00:00:00Z' END,shipped_at=1 WHERE id=?1",
                    params![task.as_str(), completed, pending]).unwrap();
            }
            store.reopen_task(&task, None).unwrap();
            let conn = store.conn.lock().unwrap();
            let inputs = state_inputs_in(&conn, &ids).unwrap().remove(&task).unwrap();
            assert_eq!(inputs.state(), expected);
            assert_eq!(inputs.execution.approved, approved);
            assert!(inputs.execution.checkout);
            assert!(inputs.execution.held);
            assert!(!inputs.execution.graph_running);
            assert!(!inputs.completed);
            assert_eq!(inputs.completed_at, None);
            assert_eq!(inputs.completion_request, None);
        }
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(store.task_state(&task).unwrap(), TaskState::Active);
    }
}
