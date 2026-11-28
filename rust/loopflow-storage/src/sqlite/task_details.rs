//! What a plan reads about each of its Tasks, one statement per concern over
//! every Task in scope, so a reading's cost does not grow a statement per Task.
use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection};

use super::children::{
    ids_json, map_task_event_row, map_task_pr_row, tracker_states_in, TASK_PR_COLUMNS,
};
use super::task_pause::PauseCleanup;
use super::{AutoState, SqliteStore};
use crate::execution::{TaskSection, TaskState, WorkingGroup};
use crate::planning_sync::TrackerActions;
use crate::work::task::{Task, TaskEvent, TaskId, TaskPr, TaskPrId};
use crate::{StoreError, StoreResult, TrackerState};
use loopflow_model::durable::{RepositoryId, TaskOwnership};

/// Durable facts about a set of Tasks, read in one snapshot.
#[derive(Debug)]
pub struct TaskDetailFacts {
    pub working_groups: HashMap<TaskId, WorkingGroup>,
    pub states: HashMap<TaskId, TaskState>,
    /// Unsettled pauses, with the first worker OS session awaiting cleanup when recorded.
    pub pause_cleanup: HashMap<TaskId, PauseCleanup>,
    /// Paused Tasks whose `lf task unpause --run` restarts work instead of staying parked.
    pub resumes_on_unpause: HashSet<TaskId>,
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
    /// Each Task parked at a breakpoint: its breakpoint name and saved arrival JSON.
    pub breakpoints: HashMap<TaskId, (String, String)>,
    /// Auto marks set on these Tasks.
    pub auto: HashMap<TaskId, AutoState>,
    pub tracker_actions: HashMap<TaskId, TrackerActions>,
    pub repository_paths: HashMap<RepositoryId, String>,
}

impl SqliteStore {
    pub fn saved_task_detail_facts(
        &self,
        tasks: &[&Task],
        graph_detail: GraphDetailReader<'_>,
    ) -> StoreResult<(loopflow_model::durable::MachineId, TaskDetailFacts)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids: Vec<TaskId> = tasks.iter().map(|task| task.id.clone()).collect();
        let json = ids_json(ids.iter().map(TaskId::as_str));
        let states = states_in(&snapshot, &json)?;
        let mut resumes_on_unpause: HashSet<TaskId> = states
            .iter()
            .filter(|(_, state)| **state == TaskState::Paused)
            .map(|(id, _)| id.clone())
            .collect();
        let pause_cleanup = if resumes_on_unpause.is_empty() {
            HashMap::new()
        } else {
            keyed(
                &snapshot,
                "SELECT task_id,json_extract(stopping_sessions,'$[0].session') FROM task_pauses
                 WHERE task_id IN (SELECT value FROM json_each(?1)) AND stopping_sessions IS NOT NULL",
                &ids_json(resumes_on_unpause.iter().map(TaskId::as_str)),
                |row| Ok((TaskId::from_raw(row.get::<_, String>(0)?), PauseCleanup { worker_session: row.get(1)? })),
            )?
        };
        let (local, ownership) = super::task_ownership::ownerships_in(&snapshot, &ids)?;
        let auto = auto_states_in(&snapshot, &json)?;
        let mut working_groups = HashMap::new();
        let mut inspect = HashSet::new();
        for task in tasks {
            let Some(state) = states.get(&task.id) else {
                continue;
            };
            let state = state.lifecycle(task.state.as_deref());
            let owner = ownership.get(&task.id);
            let remote = owner.is_some_and(|owner| owner.runs_on(&local).is_some());
            if TaskSection::of(state, owner.is_some(), remote) != TaskSection::Working {
                continue;
            }
            let observed = state.observed_here(remote);
            let (group, check_graph) = WorkingGroup::before_graph(
                observed == Some(TaskState::Paused),
                auto.get(&task.id).copied() == Some(AutoState::On),
                observed.is_some(),
            );
            working_groups.insert(task.id.clone(), group);
            if check_graph {
                inspect.insert(task.id.as_str());
            }
        }
        let mut graph_names = HashMap::new();
        let mut steps = HashMap::new();
        let mut breakpoints = HashMap::new();
        {
            let need_graph = ids_json(
                inspect
                    .iter()
                    .copied()
                    .chain(resumes_on_unpause.iter().map(TaskId::as_str)),
            );
            let mut query = snapshot.prepare(
                "SELECT task_id,json_extract(graph,'$.name'),step,run_start,
                        CASE WHEN task_id IN (SELECT value FROM json_each(?2)) THEN graph END,breakpoint
                 FROM task_graphs WHERE task_id IN (SELECT value FROM json_each(?1))",
            )?;
            let mut rows = query.query(params![&json, need_graph])?;
            while let Some(row) = rows.next()? {
                let id = TaskId::from_raw(row.get::<_, String>(0)?);
                graph_names.insert(id.clone(), row.get::<_, String>(1)?);
                let step: String = row.get(2)?;
                let run_start: Option<String> = row.get(3)?;
                let breakpoint: Option<String> = row.get(5)?;
                if let Some(graph) = row.get::<_, Option<String>>(4)? {
                    match graph_detail(GraphDetailInput {
                        graph: &graph,
                        step: &step,
                        cursor: run_start.as_deref(),
                        auto: auto.get(&id).copied(),
                        paused: resumes_on_unpause.contains(&id),
                        at_breakpoint: breakpoint.is_some(),
                    })? {
                        GraphDetail::Ships(true) => {
                            working_groups.insert(id.clone(), WorkingGroup::Shipping);
                        }
                        GraphDetail::ResumesOnUnpause(false) => {
                            resumes_on_unpause.remove(&id);
                        }
                        GraphDetail::Ships(false) | GraphDetail::ResumesOnUnpause(true) => {}
                    }
                }
                if run_start.is_none() {
                    if let Some(arrival) = breakpoint {
                        breakpoints.insert(id.clone(), (step.clone(), arrival));
                        if states.get(&id) == Some(&TaskState::Active) {
                            if let Some(group) = working_groups.get_mut(&id) {
                                *group = WorkingGroup::Active;
                            }
                        }
                    }
                    steps.insert(id, step);
                }
            }
        }
        let facts = TaskDetailFacts {
            working_groups,
            states,
            pause_cleanup,
            resumes_on_unpause,
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
            graph_names,
            steps,
            breakpoints,
            auto,
            tracker_actions: super::planning_sync::task_tracker_actions_in(&snapshot, &ids)?,
            repository_paths: repository_paths_in(&snapshot)?,
        };
        snapshot.commit()?;
        Ok((local, facts))
    }
}

/// Inputs for inspecting a Task's captured Graph inside `saved_task_detail_facts`.
#[derive(Debug)]
pub struct GraphDetailInput<'a> {
    pub graph: &'a str,
    pub step: &'a str,
    pub cursor: Option<&'a str>,
    pub auto: Option<AutoState>,
    /// Active Working Tasks check Shipping; Paused Tasks check whether unpause resumes.
    pub paused: bool,
    pub at_breakpoint: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum GraphDetail {
    Ships(bool),
    ResumesOnUnpause(bool),
}

/// Execution interprets the cursor and the CLI recognizes commands that land a PR.
pub type GraphDetailReader<'a> =
    &'a mut dyn FnMut(GraphDetailInput<'_>) -> StoreResult<GraphDetail>;

/// Read only the auto marks on the Tasks in this reading.
pub(super) fn auto_states_in(
    conn: &Connection,
    ids: &str,
) -> StoreResult<HashMap<TaskId, AutoState>> {
    let mut statement = conn.prepare(
        "SELECT task_id,state FROM task_auto WHERE task_id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([ids], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (id, state) = row?;
        Ok((TaskId::from_raw(id), AutoState::parse(&state)?))
    })
    .collect()
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
    let mut statement = conn.prepare_cached("SELECT id,repo FROM repository_plans")?;
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
