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
                    "{TASK_PR_COLUMNS} WHERE id IN (SELECT parent_pr_id FROM tasks
                     WHERE id IN (SELECT value FROM json_each(?1)))"
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

fn states_in(conn: &Connection, ids: &str) -> StoreResult<HashMap<TaskId, TaskState>> {
    let sql = format!(
        "SELECT t.id,{} FROM tasks t WHERE t.id IN (SELECT value FROM json_each(?1))",
        super::durable::task_state_sql("t")
    );
    let labels = keyed(conn, &sql, ids, |row| {
        Ok((
            TaskId::from_raw(row.get::<_, String>(0)?),
            row.get::<_, String>(1)?,
        ))
    })?;
    labels
        .into_iter()
        .map(|(id, state)| {
            TaskState::parse(&state)
                .map(|state| (id, state))
                .ok_or_else(|| StoreError::InvalidData(format!("invalid Task state {state:?}")))
        })
        .collect()
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
