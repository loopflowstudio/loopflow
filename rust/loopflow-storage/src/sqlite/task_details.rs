//! What a plan reads about each of its Tasks, one statement per concern over
//! every Task in scope, so a reading's cost does not grow a statement per Task.
use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::children::{
    ids_json, map_task_event_row, map_task_pr_row, tracker_states_in, TASK_PR_COLUMNS,
};
use super::task_pause::PauseCleanup;
use super::SqliteStore;
use crate::execution::{TaskSection, TaskState, WorkingGroup};
use crate::planning_sync::TrackerActions;
use crate::work::task::{Task, TaskEvent, TaskId, TaskPr, TaskPrId};
use crate::{StoreError, StoreResult, TrackerState};
use loopflow_model::durable::{RepositoryId, TaskOwnership};

/// Durable facts about a set of Tasks, read in one snapshot.
#[derive(Debug)]
pub struct TaskDetailFacts {
    pub working_groups: HashMap<String, WorkingGroup>,
    pub states: HashMap<TaskId, TaskState>,
    /// Unsettled pauses, with the first worker OS session awaiting cleanup when recorded.
    pub pause_cleanup: HashMap<TaskId, PauseCleanup>,
    pub open_children: HashMap<TaskId, u32>,
    pub active_prs: HashMap<TaskId, TaskPr>,
    /// Predecessor PRs the Tasks are stacked on.
    pub parent_prs: HashMap<TaskPrId, TaskPr>,
    /// A pending completion request's reason.
    pub completion_pending: HashMap<TaskId, String>,
    pub issue_urls: HashMap<TaskId, String>,
    /// Each owned Task's ownership; unowned Tasks are absent.
    pub ownership: HashMap<TaskId, TaskOwnership>,
    pub tracker_states: HashMap<TaskId, TrackerState>,
    pub latest_events: HashMap<TaskId, TaskEvent>,
    /// The name of the graph each Task took up.
    pub graph_names: HashMap<TaskId, String>,
    /// The step each Task stands at on its graph; absent while a run carries it.
    pub steps: HashMap<TaskId, String>,
    /// Tasks with auto on.
    pub auto_on: HashSet<TaskId>,
    pub tracker_actions: HashMap<TaskId, TrackerActions>,
    pub repository_paths: HashMap<RepositoryId, String>,
}

impl SqliteStore {
    pub fn saved_task_detail_facts(
        &self,
        tasks: &[&Task],
        ships: ShipsBeforeReview<'_>,
    ) -> StoreResult<(loopflow_model::durable::MachineId, TaskDetailFacts)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids: Vec<TaskId> = tasks.iter().map(|task| task.id.clone()).collect();
        let json = ids_json(ids.iter().map(TaskId::as_str));
        let states = states_in(&snapshot, &json)?;
        let paused: Vec<_> = states
            .iter()
            .filter(|(_, state)| **state == TaskState::Paused)
            .map(|(id, _)| id.as_str())
            .collect();
        let pause_cleanup = if paused.is_empty() {
            HashMap::new()
        } else {
            keyed(
                &snapshot,
                "SELECT task_id,json_extract(stopping_sessions,'$[0].session') FROM task_pauses
                 WHERE task_id IN (SELECT value FROM json_each(?1)) AND stopping_sessions IS NOT NULL",
                &ids_json(paused),
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), PauseCleanup { worker_session: row.get(1)? })),
            )?
        };
        let (local, ownership) = super::task_ownership::ownerships_in(&snapshot, &ids)?;
        let auto_on = auto_on_in(&snapshot, &json)?;
        let working = tasks.iter().filter_map(|task| {
            let state = states.get(&task.id)?.lifecycle(task.state.as_deref());
            let owner = ownership.get(&task.id);
            let remote = owner.is_some_and(|owner| owner.runs_on(&local).is_some());
            (TaskSection::of(state, owner.is_some(), remote) == TaskSection::Working).then(|| {
                (
                    task.id.as_str(),
                    state.observed_here(remote),
                    auto_on.contains(&task.id),
                )
            })
        });
        let facts = TaskDetailFacts {
            working_groups: working_groups_in(&snapshot, working, ships)?,
            states,
            pause_cleanup,
            open_children: keyed(
                &snapshot,
                &format!(
                    "SELECT parent_task_id, COUNT(*) FROM tasks t
                     WHERE parent_task_id IN (SELECT value FROM json_each(?1)) AND {}
                     GROUP BY parent_task_id",
                    super::children::open_task_children_where()
                ),
                &json,
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), row.get::<_, u32>(1)?)),
            )?,
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
            ownership,
            tracker_states: tracker_states_in(&snapshot, tasks, true)?,
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
            auto_on,
            tracker_actions: super::planning_sync::task_tracker_actions_in(&snapshot, &ids)?,
            repository_paths: repository_paths_in(&snapshot)?,
        };
        snapshot.commit()?;
        Ok((local, facts))
    }
}

/// Execution interprets the cursor and the CLI recognizes commands that land a PR.
pub type ShipsBeforeReview<'a> = &'a mut dyn FnMut(&str, &str, Option<&str>) -> StoreResult<bool>;

pub(super) fn working_groups_in<'a>(
    conn: &Connection,
    working: impl Iterator<Item = (&'a str, Option<TaskState>, bool)>,
    ships: ShipsBeforeReview<'_>,
) -> StoreResult<HashMap<String, WorkingGroup>> {
    let mut groups = HashMap::new();
    let mut inspect = Vec::new();
    for (id, state, auto) in working {
        let group = match (state, auto) {
            (Some(TaskState::Paused), _) => WorkingGroup::Paused,
            (_, true) => WorkingGroup::Shipping,
            (observed, false) => {
                if observed.is_some() {
                    inspect.push(id);
                }
                WorkingGroup::Active
            }
        };
        groups.insert(id.to_owned(), group);
    }
    if !inspect.is_empty() {
        let mut query = conn.prepare(
            "SELECT task_id,graph,step,run_start FROM task_graphs
             WHERE task_id IN (SELECT value FROM json_each(?1))",
        )?;
        let mut rows = query.query([ids_json(inspect)])?;
        while let Some(row) = rows.next()? {
            if ships(
                &row.get::<_, String>(1)?,
                &row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?.as_deref(),
            )? {
                groups.insert(row.get(0)?, WorkingGroup::Shipping);
            }
        }
    }
    Ok(groups)
}

/// The Tasks among `ids` whose auto setting is on.
pub(super) fn auto_on_in(conn: &Connection, ids: &str) -> StoreResult<HashSet<TaskId>> {
    task_set(
        conn,
        "SELECT task_id FROM task_auto
         WHERE task_id IN (SELECT value FROM json_each(?1)) AND state='on'",
        ids,
    )
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

/// Each listed Task's state, in one statement; `ids` is a JSON array.
pub(super) fn states_in(conn: &Connection, ids: &str) -> StoreResult<HashMap<TaskId, TaskState>> {
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
