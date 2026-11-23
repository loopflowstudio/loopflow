//! Saved selection and layout intent share one transaction and one Work snapshot.
use super::sessions::{SessionProjection, SessionProjectionInputs};
use super::SqliteStore;
use crate::execution::{TaskSection, TaskState};

use crate::work::selection::{
    move_into_place, Arrangement, Dashboard, DashboardId, DashboardReading, DropSide, Selection,
    SelectionError, SelectionExpression, SelectionId, SelectionResolution, SelectionState,
    SelectionWork, SlotLayout, ViewPlan, WorkWorkspaceKey, WorkWorkspaceKind,
};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::RepositoryId;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap};

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn catalog_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<Vec<Dashboard>> {
    let mut statement = conn.prepare("SELECT s.id,s.revision,a.selection_json,a.arrangement_json,s.name,s.pinned,s.slots_json FROM dashboards s JOIN dashboard_arrangements a ON a.repository_id=s.repository_id AND a.dashboard_id=s.id AND a.expression_key=s.expression_key WHERE s.repository_id=?1 ORDER BY s.id")?;
    let rows = statement.query_map([repository.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, bool>(5)?,
            row.get::<_, Option<String>>(6)?,
        ))
    })?;
    rows.map(|row| {
        let (id, revision, selection, arrangement, name, pinned, slots) = row?;
        Ok(Dashboard {
            repository: repository.to_string(),
            id: DashboardId(id),
            revision,
            name,
            pinned,
            slots: slots.as_deref().map(serde_json::from_str).transpose()?,
            selection: serde_json::from_str(&selection)?,
            arrangement: serde_json::from_str(&arrangement)?,
        })
    })
    .collect()
}

fn arrangement_in(
    conn: &Connection,
    repository: &RepositoryId,
    id: &DashboardId,
    expression_key: &str,
) -> StoreResult<Option<(Selection, Arrangement)>> {
    let row: Option<(String, String)> = conn.query_row(
        "SELECT selection_json,arrangement_json FROM dashboard_arrangements WHERE repository_id=?1 AND dashboard_id=?2 AND expression_key=?3",
        params![repository.as_str(), id.0, expression_key], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional()?;
    row.map(|(selection, arrangement)| {
        Ok((
            serde_json::from_str(&selection)?,
            serde_json::from_str(&arrangement)?,
        ))
    })
    .transpose()
}

fn work_in(
    conn: &Connection,
    repository: &RepositoryId,
    projection: &SessionProjection,
) -> StoreResult<Vec<SelectionWork>> {
    let repo = repository_path_in(conn, repository)?;
    let mut tasks = task_rows_in(conn, repository, &repo, projection)?;
    let order = TaskOrder::from_ids(task_order_in(conn, repository)?);
    tasks.sort_by_cached_key(|task| order.key(&task.work.key.id, task.rank, task.started_at));
    let mut work: Vec<_> = tasks.into_iter().map(|task| task.work).collect();

    let mut waves = super::waves_in(conn, Some(&repo))?;
    waves.sort_by(|a, b| (a.name(), a.id().as_str()).cmp(&(b.name(), b.id().as_str())));
    let key = |kind, id| WorkWorkspaceKey {
        repository: repository.to_string(),
        kind,
        id,
    };
    // Tasks precede the alphabetized Waves, then the repository.
    for wave in waves {
        work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
            parent_wave: wave.parent_wave_id().cloned(),
            name: wave.name().to_string(),
            states: BTreeSet::new(),
            activity_known: false,
        });
    }
    work.push(SelectionWork {
        key: key(WorkWorkspaceKind::Repository, repository.to_string()),
        parent_wave: None,
        name: repo,
        states: BTreeSet::new(),
        activity_known: false,
    });
    Ok(work)
}

struct SelectionTask {
    work: SelectionWork,
    rank: u32,
    /// Only Working Tasks participate in the start-time order.
    started_at: Option<i64>,
}

fn task_rows_in(
    conn: &Connection,
    repository: &RepositoryId,
    repo: &str,
    projection: &SessionProjection,
) -> StoreResult<Vec<SelectionTask>> {
    let tasks = super::children::repository_tasks_in(conn, repository)?;
    let activity = task_activity_in(conn, repo, projection)?;
    // State, publication and ownership for every Task in a fixed number of
    // statements: a query per Task costs thousands on a large repository.
    let ids: Vec<_> = tasks.iter().map(|task| task.id.clone()).collect();
    let ids_json = super::children::ids_json(ids.iter().map(|id| id.as_str()));
    let task_states = super::task_details::states_in(conn, &ids_json)?;
    let auto_on = super::task_details::auto_on_in(conn, &ids_json)?;
    let published = super::task_started::published_in(conn, &ids)?;
    let owners = super::task_ownership::owners_in(conn, &ids)?;
    let mut rows = Vec::with_capacity(tasks.len());
    for task in tasks {
        let state = *task_states
            .get(&task.id)
            .ok_or_else(|| invalid(format!("Task {} has no state", task.id)))?;
        let activity = activity.get(&task.id);
        let states = task_selection_states(
            state,
            owners.contains_key(&task.id),
            published.contains(&task.id),
            auto_on.contains(&task.id),
            task.state.as_deref(),
            activity,
        );
        rows.push(SelectionTask {
            rank: task.rank,
            started_at: None,
            work: SelectionWork {
                key: WorkWorkspaceKey {
                    repository: repository.to_string(),
                    kind: WorkWorkspaceKind::Task,
                    id: task.id.to_string(),
                },
                parent_wave: task.wave_id,
                name: task.identifier,
                states,
                activity_known: activity.is_none_or(|activity| activity.known),
            },
        });
    }
    let starts = start_times_in(
        conn,
        rows.iter()
            .filter(|row| row.work.states.contains(&SelectionState::Working))
            .map(|row| row.work.key.id.as_str()),
    )?;
    for task in &mut rows {
        task.started_at = starts.get(&task.work.key.id).copied();
    }
    Ok(rows)
}

fn repository_path_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<String> {
    conn.query_row(
        "SELECT repo FROM repository_plans WHERE id=?1",
        [repository.as_str()],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

/// Conversation activity is independent of Task lifecycle and ownership.
#[derive(Debug, Clone)]
pub struct TaskActivity {
    states: BTreeSet<SelectionState>,
    known: bool,
}

/// Shared repository ordering and conversation facts; read once for each plan.
#[derive(Debug)]
pub struct TaskSelectionReading {
    pub order: TaskOrder,
    pub activity: HashMap<crate::execution::TaskId, TaskActivity>,
}

#[derive(Debug)]
pub struct TaskOrder(HashMap<String, usize>);

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TaskPosition {
    New(Reverse<i64>),
    Saved(usize),
    Planning(u32),
}

impl TaskOrder {
    fn from_ids(ids: impl IntoIterator<Item = String>) -> Self {
        Self(
            ids.into_iter()
                .enumerate()
                .map(|(position, id)| (id, position))
                .collect(),
        )
    }

    pub fn key(&self, id: &str, rank: u32, started_at: Option<i64>) -> (TaskPosition, String) {
        let position = match started_at {
            None => TaskPosition::Planning(rank),
            Some(at) => self.working_position(id, at),
        };
        (position, id.to_owned())
    }

    fn working_position(&self, id: &str, at: i64) -> TaskPosition {
        self.0
            .get(id)
            .copied()
            .map_or(TaskPosition::New(Reverse(at)), TaskPosition::Saved)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TaskOrderEdit<'a> {
    Move {
        task: &'a str,
        target: &'a str,
        side: Option<DropSide>,
    },
    SortByStatus,
}

impl TaskOrderEdit<'_> {
    fn apply(self, tasks: Vec<SelectionTask>, saved: &[String]) -> StoreResult<Vec<String>> {
        let mut working: Vec<_> = tasks
            .into_iter()
            .filter_map(|task| Some((task.started_at?, task.work)))
            .collect();
        match self {
            Self::Move { task, target, side } => {
                let saved = TaskOrder::from_ids(saved.iter().cloned());
                working.sort_by_cached_key(|(at, row)| {
                    (saved.working_position(&row.key.id, *at), row.key.id.clone())
                });
                let position = |id: &str| {
                    working
                        .iter()
                        .position(|(_, row)| row.key.id == id)
                        .ok_or_else(|| SelectionError::NotWorking(id.to_owned()))
                };
                let from = position(task)?;
                let to = position(target)?;
                move_into_place(&mut working, from, to, side);
            }
            Self::SortByStatus => working.sort_by_cached_key(|(at, row)| {
                (status_bucket(&row.states), Reverse(*at), row.key.id.clone())
            }),
        }
        Ok(working.into_iter().map(|(_, row)| row.key.id).collect())
    }
}

fn status_bucket(states: &BTreeSet<SelectionState>) -> u8 {
    if states.contains(&SelectionState::Blocked) {
        0
    } else if states.contains(&SelectionState::Available) {
        1
    } else if states.contains(&SelectionState::Busy) {
        3
    } else {
        2
    }
}

fn start_times_in<'a>(
    conn: &Connection,
    ids: impl Iterator<Item = &'a str>,
) -> StoreResult<HashMap<String, i64>> {
    let mut statement = conn.prepare(
        "SELECT t.id, COALESCE(
            (SELECT MIN(o.at) FROM task_ownership o WHERE o.task_id=t.id AND o.kind != 'unclaim'),
            (SELECT e.created_at FROM task_events e WHERE e.task_id=t.id
             AND json_extract(e.kind_json,'$.kind')='started' ORDER BY e.id LIMIT 1),
            t.created_at)
         FROM tasks t WHERE t.id IN (SELECT value FROM json_each(?1))",
    )?;
    let rows = statement.query_map([super::children::ids_json(ids)], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The same lifecycle, section, auto setting and activity set for roadmap rows
/// and selections.
pub fn task_selection_states(
    runtime: TaskState,
    owned: bool,
    published: bool,
    auto: bool,
    plan_state: Option<&str>,
    activity: Option<&TaskActivity>,
) -> BTreeSet<SelectionState> {
    let state = runtime.lifecycle(plan_state);
    let mut states = activity.map_or_else(BTreeSet::new, |activity| activity.states.clone());
    if (owned || published) && !state.is_terminal() {
        states.insert(SelectionState::Open);
    }
    if auto && !state.is_terminal() {
        states.insert(SelectionState::Auto);
    }
    states.insert(match state {
        TaskState::Unstarted => SelectionState::Unstarted,
        TaskState::Idle => SelectionState::Idle,
        TaskState::Active => SelectionState::Active,
        TaskState::Moving => SelectionState::Moving,
        TaskState::Done | TaskState::Abandoned => SelectionState::Done,
    });
    states.insert(match TaskSection::of(runtime, owned, plan_state) {
        TaskSection::Backlog => SelectionState::Backlog,
        TaskSection::Working => SelectionState::Working,
        TaskSection::Done | TaskSection::Abandoned => SelectionState::Done,
    });
    states
}

fn task_activity_in(
    conn: &Connection,
    repo: &str,
    projection: &SessionProjection,
) -> StoreResult<HashMap<crate::execution::TaskId, TaskActivity>> {
    let sessions = super::sessions::session_summaries_in(
        conn,
        &crate::session::SessionFilter {
            repo: Some(repo.to_owned()),
            limit: 0,
            after: Some(String::new()),
            ..Default::default()
        },
        projection,
    )?;
    let mut activity = HashMap::<_, TaskActivity>::new();
    for session in sessions
        .iter()
        .filter(|session| session.completed_at.is_none())
    {
        for task in &session.task_ids {
            let activity = activity
                .entry(task.clone())
                .or_insert_with(|| TaskActivity {
                    states: BTreeSet::new(),
                    known: true,
                });
            // A conversation Loopflow never attached cannot be running here.
            if session.agent_process_id.is_some()
                && session.agent_process_evidence == crate::ProcessIdentityEvidence::Unknown
            {
                activity.known = false;
            }
            if session.available {
                activity.states.insert(SelectionState::Available);
            }
            for report in session
                .program_status
                .iter()
                .flat_map(|status| &status.records)
            {
                match report.state {
                    loopflow_model::program_status::State::Working => {
                        activity.states.insert(SelectionState::Busy);
                    }
                    loopflow_model::program_status::State::Blocked => {
                        activity.states.insert(SelectionState::Blocked);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(activity)
}

fn task_order_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<Vec<String>> {
    let order: Option<String> = conn
        .query_row(
            "SELECT task_ids_json FROM task_orders WHERE repository_id=?1",
            [repository.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    Ok(order
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?
        .unwrap_or_default())
}

pub fn repository_default() -> Selection {
    Selection {
        id: SelectionId("repository-default".into()),
        expression: SelectionExpression::Repository,
    }
}

fn resolve_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: &Selection,
    catalog: &[Dashboard],
    projection: &SessionProjection,
) -> StoreResult<SelectionResolution> {
    selection
        .resolve(
            repository.as_str(),
            &work_in(conn, repository, projection)?,
            &definitions(catalog),
        )
        .map_err(StoreError::from)
}

fn definitions(catalog: &[Dashboard]) -> BTreeMap<DashboardId, Selection> {
    catalog
        .iter()
        .map(|saved| (saved.id.clone(), saved.selection.clone()))
        .collect()
}

/// Selection IDs of the built-in plan behind each Wave view.
pub const WAVE_DEFAULT_PREFIX: &str = "wave-default:";

fn plan_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: Selection,
    saved: Option<&Dashboard>,
    queries: &BTreeMap<DashboardId, Selection>,
    work: &[SelectionWork],
) -> StoreResult<ViewPlan> {
    use crate::work::selection::{ViewContent, ViewLocation};
    let resolution = selection
        .resolve(repository.as_str(), work, queries)
        .map_err(StoreError::from)?;
    let mut arrangement = saved.map_or_else(
        || Arrangement::automatic(&[]),
        |saved| saved.arrangement.clone(),
    );
    arrangement.reconcile(&resolution);
    let selected: BTreeSet<_> = arrangement
        .order
        .iter()
        .map(|member| member.work.clone())
        .collect();
    let mut contents = Vec::new();
    for key in selected {
        let facts = work.iter().find(|item| item.key == key);
        let mut content = ViewContent {
            work: key.clone(),
            title: facts.map_or_else(|| key.id.clone(), |item| item.name.clone()),
            states: facts.map_or_else(BTreeSet::new, |item| item.states.clone()),
            identifier: None,
            graph_step: None,
            primary_session: None,
            location: None,
            argv: None,
            unavailable: Vec::new(),
        };
        if facts.is_none() {
            content
                .unavailable
                .push("Work is unavailable; retained intent only".into());
        } else if key.kind == WorkWorkspaceKind::Task {
            let task_id = loopflow_model::durable::TaskId::parse(&key.id).map_err(invalid)?;
            let task = super::children::task_on(conn, &task_id)?.ok_or(StoreError::NotFound)?;
            content.identifier = Some(task.identifier);
            content.title = task.title;
            let primary = super::sessions::primary_session_in(
                conn,
                &crate::session::PrimaryScope::Task(task_id.clone()),
            )?;
            content.primary_session = primary.as_ref().map(|session| session.id.to_string());
            if primary.is_none() {
                content
                    .unavailable
                    .push("No primary Session is recorded".into());
            }
            if let Some(checkout) = super::children::task_checkout_in(conn, &task_id)? {
                content.location = Some(ViewLocation {
                    machine_id: super::durable::map_local_machine(conn)?.id.to_string(),
                    cwd: checkout.worktree.display().to_string(),
                });
            }
            if content.location.is_none() {
                content
                    .unavailable
                    .push("Recorded execution location is unavailable".into());
            }
        }
        contents.push(content);
    }
    // A Wave view's built-in plan fills its footer strip, the repository's included
    // (the repository page is its default Wave); saved and unsaved queries start golden.
    let default_slots = if selection.id.0.starts_with(WAVE_DEFAULT_PREFIX)
        || selection.id == repository_default().id
    {
        SlotLayout::strip(3)
    } else {
        SlotLayout::golden()
    };
    Ok(ViewPlan {
        name: saved.and_then(|saved| saved.name.clone()),
        pinned: saved.is_some_and(|saved| saved.pinned),
        dashboard_id: saved.map(|saved| saved.id.clone()),
        repository: repository.to_string(),
        repo_path: repository_path_in(conn, repository)?,
        selection,
        saved_revision: saved.map(|saved| saved.revision),
        resolution,
        projection: arrangement.project(),
        arrangement,
        slots: saved
            .and_then(|saved| saved.slots.clone())
            .unwrap_or(default_slots),
        contents,
        // The application resolves the claimant: it reads Git, not the store.
        viewer: None,
    })
}

/// Resolve the captured cursor in execution, where the Graph's step names live.
type RunningStep = fn(&str, &str) -> StoreResult<Option<String>>;

fn fill_graph_steps_in(
    conn: &Connection,
    plans: &mut [ViewPlan],
    running_step: RunningStep,
) -> StoreResult<()> {
    let tasks: BTreeSet<_> = plans
        .iter()
        .flat_map(|plan| &plan.contents)
        .filter(|content| {
            content.work.kind == WorkWorkspaceKind::Task && content.identifier.is_some()
        })
        .map(|content| content.work.id.as_str())
        .collect();
    let mut query = conn.prepare(
        "SELECT task_id,step,CASE WHEN run_start IS NOT NULL THEN graph END,run_start
         FROM task_graphs WHERE task_id IN (SELECT value FROM json_each(?1))",
    )?;
    let mut rows = query.query([super::children::ids_json(tasks)])?;
    let mut steps = HashMap::<String, String>::new();
    while let Some(row) = rows.next()? {
        let parked = row.get(1)?;
        let step = match row.get::<_, Option<String>>(3)? {
            Some(cursor) => running_step(&row.get::<_, String>(2)?, &cursor)?.unwrap_or(parked),
            None => parked,
        };
        steps.insert(row.get(0)?, step);
    }
    for content in plans.iter_mut().flat_map(|plan| &mut plan.contents) {
        content.graph_step = steps.get(&content.work.id).cloned();
    }
    Ok(())
}

fn plan_selection_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: Selection,
    work: &[SelectionWork],
) -> StoreResult<ViewPlan> {
    let catalog = catalog_in(conn, repository)?;
    let saved = selection
        .referenced_query()
        .and_then(|id| catalog.iter().find(|saved| &saved.id == id));
    plan_in(
        conn,
        repository,
        saved.map_or(selection, |saved| saved.selection.clone()),
        saved,
        &definitions(&catalog),
        work,
    )
}

#[derive(Debug)]
pub struct DashboardEditInputs {
    pub catalog: Vec<Dashboard>,
    pub work: Vec<SelectionWork>,
    pub arrangement: Option<Arrangement>,
}

impl SqliteStore {
    pub fn dashboard_arrangement(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expression: &str,
    ) -> StoreResult<Option<(Selection, Arrangement)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        arrangement_in(&conn, repository, id, expression)
    }

    pub fn projected_plan_selection(
        &self,
        repository: &RepositoryId,
        selection: Selection,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
        running_step: RunningStep,
    ) -> StoreResult<Option<ViewPlan>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let work = work_in(&snapshot, repository, projection)?;
        let mut plan = plan_selection_in(&snapshot, repository, selection, &work)?;
        fill_graph_steps_in(&snapshot, std::slice::from_mut(&mut plan), running_step)?;
        Ok(Some(plan))
    }

    /// One consistent reading of the same selection across `repos` (canonical
    /// repository paths); a repository with no recorded Work identity has no
    /// plan, and neither does one that holds none of the Waves it scopes to.
    pub fn projected_all_view_plans(
        &self,
        source: Option<&str>,
        repos: &[String],
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
        running_step: RunningStep,
    ) -> StoreResult<Option<Vec<ViewPlan>>> {
        let selection = match source {
            Some(source) => Selection::parse(source, None).map_err(StoreError::from)?,
            None => repository_default(),
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let waves = selection.waves();
        let mut plans = Vec::new();
        for repo in repos {
            if let Some(repository) = super::repositories::repository_id_in(&snapshot, repo)? {
                let work = work_in(&snapshot, &repository, projection)?;
                if !waves.is_empty()
                    && !work.iter().any(|item| {
                        item.key.kind == WorkWorkspaceKind::Wave
                            && waves.iter().any(|wave| {
                                item.key.id == *wave || item.name.eq_ignore_ascii_case(wave)
                            })
                    })
                {
                    continue;
                }
                plans.push(plan_selection_in(
                    &snapshot,
                    &repository,
                    selection.clone(),
                    &work,
                )?);
            }
        }
        fill_graph_steps_in(&snapshot, &mut plans, running_step)?;
        Ok(Some(plans))
    }

    /// The observer and one-shot plans expose the same repository/default and saved views.
    pub fn projected_selection_plans(
        &self,
        repository: &RepositoryId,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
        running_step: RunningStep,
    ) -> StoreResult<Option<Vec<ViewPlan>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let catalog = catalog_in(&snapshot, repository)?;
        let queries = definitions(&catalog);
        let work = work_in(&snapshot, repository, projection)?;
        let mut plans = vec![plan_in(
            &snapshot,
            repository,
            repository_default(),
            None,
            &queries,
            &work,
        )?];
        for wave in work
            .iter()
            .filter(|item| item.key.kind == WorkWorkspaceKind::Wave)
        {
            plans.push(plan_in(
                &snapshot,
                repository,
                Selection {
                    id: crate::work::selection::SelectionId(format!(
                        "{WAVE_DEFAULT_PREFIX}{}",
                        wave.key.id
                    )),
                    expression: crate::work::selection::SelectionExpression::Wave {
                        wave: loopflow_model::id::WaveId::parse(&wave.key.id).map_err(invalid)?,
                    },
                },
                None,
                &queries,
                &work,
            )?);
        }
        for saved in &catalog {
            plans.push(plan_in(
                &snapshot,
                repository,
                saved.selection.clone(),
                Some(saved),
                &queries,
                &work,
            )?);
        }
        fill_graph_steps_in(&snapshot, &mut plans, running_step)?;
        Ok(Some(plans))
    }

    /// A read composes with the Work observer's pinned snapshot. No acquisition.
    pub fn projected_resolve_selection(
        &self,
        repository: &RepositoryId,
        selection: &Selection,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<SelectionResolution>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        Ok(Some(resolve_in(
            &snapshot,
            repository,
            selection,
            &catalog_in(&snapshot, repository)?,
            projection,
        )?))
    }

    /// Shared conversation activity and ordering, without loading unrelated Tasks.
    pub fn projected_task_selection_reading(
        &self,
        repository: &RepositoryId,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<TaskSelectionReading>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let repo = repository_path_in(&snapshot, repository)?;
        Ok(Some(TaskSelectionReading {
            order: TaskOrder::from_ids(task_order_in(&snapshot, repository)?),
            activity: task_activity_in(&snapshot, &repo, projection)?,
        }))
    }

    pub fn dashboards(&self, repository: &RepositoryId) -> StoreResult<Vec<Dashboard>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        catalog_in(&conn, repository)
    }

    pub fn projected_read_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<Option<DashboardReading>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let catalog = catalog_in(&snapshot, repository)?;
        let Some(saved) = catalog.iter().find(|saved| &saved.id == id) else {
            return Ok(Some(None));
        };
        let resolution = resolve_in(
            &snapshot,
            repository,
            &saved.selection,
            &catalog,
            projection,
        )?;
        let mut arrangement = saved.arrangement.clone();
        arrangement.reconcile(&resolution);
        Ok(Some(Some(DashboardReading {
            saved: saved.clone(),
            resolution,
            arrangement,
        })))
    }

    /// Read saved editing inputs with the same validated Session projection.
    pub fn dashboard_edit_inputs(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expression: &str,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<DashboardEditInputs>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        Ok(Some(DashboardEditInputs {
            catalog: catalog_in(&snapshot, repository)?,
            work: work_in(&snapshot, repository, projection)?,
            arrangement: arrangement_in(&snapshot, repository, id, expression)?
                .map(|(_, arrangement)| arrangement),
        }))
    }

    pub fn commit_dashboard(
        &self,
        repository: &RepositoryId,
        expected_revision: Option<i64>,
        saved: Dashboard,
    ) -> StoreResult<Dashboard> {
        let id = &saved.id;
        let expression_key = saved.selection.expression_key();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Every Dashboard write advances the revision, so an unchanged one
        // means the catalog and arrangement read above are still current.
        let current: Option<i64> = tx
            .query_row(
                "SELECT revision FROM dashboards WHERE repository_id=?1 AND id=?2",
                params![repository.as_str(), id.0],
                |row| row.get(0),
            )
            .optional()?;
        if current != expected_revision {
            return Err(StoreError::Stale(
                "Dashboard changed; read it again before editing".into(),
            ));
        }
        tx.execute("INSERT INTO dashboards(repository_id,id,revision,expression_key,name,pinned) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(repository_id,id) DO UPDATE SET revision=excluded.revision,expression_key=excluded.expression_key",
            params![repository.as_str(),id.0,saved.revision,expression_key,saved.name,saved.pinned])?;
        tx.execute("INSERT INTO dashboard_arrangements(repository_id,dashboard_id,expression_key,selection_json,arrangement_json) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(repository_id,dashboard_id,expression_key) DO UPDATE SET selection_json=excluded.selection_json,arrangement_json=excluded.arrangement_json",
            params![repository.as_str(),id.0,expression_key,serde_json::to_string(&saved.selection)?,serde_json::to_string(&saved.arrangement)?])?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn set_dashboard_metadata(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expected_revision: i64,
        name: Option<&str>,
        pinned: Option<bool>,
    ) -> StoreResult<Dashboard> {
        if name.is_some_and(|name| name.trim().is_empty()) {
            return Err(invalid("Dashboard name must be nonempty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute("UPDATE dashboards SET name=coalesce(?1,name),pinned=coalesce(?2,pinned),revision=revision+1 WHERE repository_id=?3 AND id=?4 AND revision=?5",
            params![name,pinned,repository.as_str(),id.0,expected_revision])?;
        if changed == 0 {
            return Err(StoreError::Stale(
                "Dashboard changed; read it again before editing".into(),
            ));
        }
        let saved = catalog_in(&tx, repository)?
            .into_iter()
            .find(|saved| &saved.id == id)
            .ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(saved)
    }

    /// Replace a Dashboard's slot layout; None restores the default.
    pub fn set_dashboard_slots(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expected_revision: i64,
        slots: Option<&SlotLayout>,
    ) -> StoreResult<Dashboard> {
        if let Some(slots) = slots {
            slots.validate().map_err(StoreError::from)?;
        }
        let json = slots.map(serde_json::to_string).transpose()?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = tx.execute("UPDATE dashboards SET slots_json=?1,revision=revision+1 WHERE repository_id=?2 AND id=?3 AND revision=?4",
            params![json,repository.as_str(),id.0,expected_revision])?;
        if changed == 0 {
            return Err(StoreError::Stale(
                "Dashboard changed; read it again before editing".into(),
            ));
        }
        let saved = catalog_in(&tx, repository)?
            .into_iter()
            .find(|saved| &saved.id == id)
            .ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(saved)
    }

    pub fn task_start_times<'a>(
        &self,
        ids: impl Iterator<Item = &'a str>,
    ) -> StoreResult<HashMap<String, i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        start_times_in(&conn, ids)
    }

    /// Prepare the edit from one reading. The saved IDs are checked again at commit.
    pub fn prepare_task_order(
        &self,
        repository: &RepositoryId,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
        edit: TaskOrderEdit<'_>,
    ) -> StoreResult<Option<(Vec<String>, Vec<String>)>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let repo = repository_path_in(&snapshot, repository)?;
        let saved = task_order_in(&snapshot, repository)?;
        let tasks = task_rows_in(&snapshot, repository, &repo, projection)?;
        let order = edit.apply(tasks, &saved)?;
        Ok(Some((saved, order)))
    }

    pub fn commit_task_order(
        &self,
        repository: &RepositoryId,
        read: &[String],
        order: &[String],
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if task_order_in(&tx, repository)? != read {
            return Ok(false);
        }
        tx.execute(
            "INSERT INTO task_orders(repository_id,task_ids_json) VALUES(?1,?2) ON CONFLICT(repository_id) DO UPDATE SET task_ids_json=excluded.task_ids_json",
            params![repository.as_str(), serde_json::to_string(order)?],
        )?;
        tx.commit()?;
        Ok(true)
    }

    pub fn delete_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expected_revision: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let deleted = conn.execute(
            "DELETE FROM dashboards WHERE repository_id=?1 AND id=?2 AND revision=?3",
            params![repository.as_str(), id.0, expected_revision],
        )?;
        if deleted == 0 {
            return Err(StoreError::Stale(
                "Dashboard changed; read it again before deleting".into(),
            ));
        }
        Ok(())
    }
}
