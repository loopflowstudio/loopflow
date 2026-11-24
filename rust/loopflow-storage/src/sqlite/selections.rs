//! Saved selection and layout intent share one transaction and one Work snapshot.
use super::sessions::{SessionProjection, SessionProjectionInputs};
use super::SqliteStore;
use crate::execution::{TaskSection, TaskState};

use crate::work::selection::{
    Arrangement, Dashboard, DashboardId, DashboardReading, Selection, SelectionExpression,
    SelectionId, SelectionResolution, SelectionState, SelectionWork, SlotLayout, WorkWorkspaceKey,
    WorkWorkspaceKind,
};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{ProjectId, RepositoryId};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
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
    let repo: String = conn.query_row(
        "SELECT repo FROM repository_plans WHERE id=?1",
        [repository.as_str()],
        |row| row.get(0),
    )?;
    let mut waves = super::waves_in(conn, Some(&repo))?;
    waves.sort_by(|a, b| (a.name(), a.id().as_str()).cmp(&(b.name(), b.id().as_str())));
    let key = |kind, id| WorkWorkspaceKey {
        repository: repository.to_string(),
        kind,
        id,
    };
    let repository_work = SelectionWork {
        key: key(WorkWorkspaceKind::Repository, repository.to_string()),
        parent_wave: None,
        name: repo.clone(),
        states: BTreeSet::new(),
        activity_known: false,
    };
    let mut wave_work = Vec::with_capacity(waves.len());
    for wave in &waves {
        wave_work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
            parent_wave: wave.parent_wave_id().cloned(),
            name: wave.name().to_string(),
            states: BTreeSet::new(),
            activity_known: false,
        });
    }
    let tasks = super::children::repository_tasks_in(conn, repository)?;
    let reading = task_selection_in(conn, &repo, projection)?;
    // State, publication and ownership for every Task in a fixed number of
    // statements: a query per Task costs thousands on a large repository.
    let ids: Vec<_> = tasks.iter().map(|task| task.id.clone()).collect();
    let ids_json = super::children::ids_json(ids.iter().map(|id| id.as_str()));
    let task_states = super::task_details::states_in(conn, &ids_json)?;
    let auto_on = super::task_details::auto_on_in(conn, &ids_json)?;
    let published = super::task_started::published_in(conn, &ids)?;
    let (local, owners) = super::task_ownership::ownerships_in(conn, &ids)?;
    let mut task_work = Vec::with_capacity(tasks.len());
    for task in tasks {
        let state = *task_states
            .get(&task.id)
            .ok_or_else(|| invalid(format!("Task {} has no state", task.id)))?;
        let ownership = owners.get(&task.id);
        let owned = ownership.is_some();
        let runs_elsewhere = ownership.is_some_and(|claim| claim.runs_on(&local).is_some());
        let lifecycle = state.lifecycle(task.state.as_deref());
        let states = task_selection_states(
            lifecycle.observed_here(runs_elsewhere),
            TaskSection::of(lifecycle, owned, runs_elsewhere),
            owned || published.contains(&task.id),
            auto_on.contains(&task.id),
            reading.activity.get(&task.id),
        );
        let activity_known = reading
            .activity
            .get(&task.id)
            .is_none_or(|activity| activity.known);
        let order = reading.order.key(
            task.project_id.as_ref().map(ProjectId::as_str),
            task.rank,
            task.id.as_str(),
        );
        task_work.push((
            order,
            SelectionWork {
                key: key(WorkWorkspaceKind::Task, task.id.to_string()),
                parent_wave: task.wave_id,
                name: task.identifier,
                states,
                activity_known,
            },
        ));
    }
    task_work.sort_by(|left, right| left.0.cmp(&right.0));
    // Tasks precede the already alphabetized Waves, then the repository.
    Ok(task_work
        .into_iter()
        .map(|(_, task)| task)
        .chain(wave_work)
        .chain([repository_work])
        .collect())
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

/// Rank order: Tasks follow their Project's rank, then their own. Standalone
/// Tasks follow every Project.
#[derive(Debug)]
pub struct TaskOrder(HashMap<String, u32>);

impl TaskOrder {
    pub fn key(&self, project: Option<&str>, rank: u32, id: &str) -> impl Ord {
        let project =
            project.map(|id| (self.0.get(id).copied().unwrap_or(u32::MAX), id.to_owned()));
        (project.is_none(), project, rank, id.to_owned())
    }
}

/// The same lifecycle, section, auto setting and activity set for roadmap rows
/// and selections.
pub fn task_selection_states(
    state: Option<TaskState>,
    section: TaskSection,
    claimed_or_published: bool,
    auto: bool,
    activity: Option<&TaskActivity>,
) -> BTreeSet<SelectionState> {
    let mut states = activity.map_or_else(BTreeSet::new, |activity| activity.states.clone());
    let open = matches!(section, TaskSection::Backlog | TaskSection::Working);
    if claimed_or_published && open {
        states.insert(SelectionState::Open);
    }
    if auto && open {
        states.insert(SelectionState::Auto);
    }
    states.extend(state.map(|state| match state {
        TaskState::Unstarted => SelectionState::Unstarted,
        TaskState::Idle => SelectionState::Idle,
        TaskState::Active => SelectionState::Active,
        TaskState::Moving => SelectionState::Moving,
        TaskState::Done | TaskState::Abandoned => SelectionState::Done,
    }));
    states.insert(match section {
        TaskSection::Backlog => SelectionState::Backlog,
        TaskSection::Working => SelectionState::Working,
        TaskSection::Done | TaskSection::Abandoned => SelectionState::Done,
    });
    states
}

fn task_selection_in(
    conn: &Connection,
    repo: &str,
    projection: &SessionProjection,
) -> StoreResult<TaskSelectionReading> {
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
        if let Some(task) = session.member_task() {
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
    Ok(TaskSelectionReading {
        order: TaskOrder(project_ranks_in(conn, repo)?),
        activity,
    })
}

fn project_ranks_in(conn: &Connection, repo: &str) -> StoreResult<HashMap<String, u32>> {
    let mut query = conn.prepare(
        "SELECT p.id,p.planning_rank FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1",
    )?;
    let rows = query.query_map([repo], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect::<Result<_, _>>().map_err(Into::into)
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
) -> StoreResult<crate::work::selection::ViewPlan> {
    use crate::work::selection::{ViewContent, ViewLocation, ViewPlan};
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
    let repo_path = conn
        .query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [repository.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    Ok(ViewPlan {
        name: saved.and_then(|saved| saved.name.clone()),
        pinned: saved.is_some_and(|saved| saved.pinned),
        dashboard_id: saved.map(|saved| saved.id.clone()),
        repository: repository.to_string(),
        repo_path,
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

fn plan_selection_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: Selection,
    projection: &SessionProjection,
) -> StoreResult<crate::work::selection::ViewPlan> {
    let catalog = catalog_in(conn, repository)?;
    let work = work_in(conn, repository, projection)?;
    let saved = selection
        .referenced_query()
        .and_then(|id| catalog.iter().find(|saved| &saved.id == id));
    plan_in(
        conn,
        repository,
        saved.map_or(selection, |saved| saved.selection.clone()),
        saved,
        &definitions(&catalog),
        &work,
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
    ) -> StoreResult<Option<crate::work::selection::ViewPlan>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        Ok(Some(plan_selection_in(
            &snapshot, repository, selection, projection,
        )?))
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
    ) -> StoreResult<Option<Vec<crate::work::selection::ViewPlan>>> {
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
                if !waves.is_empty()
                    && !work_in(&snapshot, &repository, projection)?
                        .iter()
                        .any(|item| {
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
                    projection,
                )?);
            }
        }
        Ok(Some(plans))
    }

    /// The observer and one-shot plans expose the same repository/default and saved views.
    pub fn projected_selection_plans(
        &self,
        repository: &RepositoryId,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<Vec<crate::work::selection::ViewPlan>>> {
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
        let repo: String = snapshot.query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [repository.as_str()],
            |row| row.get(0),
        )?;
        Ok(Some(task_selection_in(&snapshot, &repo, projection)?))
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

#[cfg(test)]
mod tests {
    use super::task_selection_states;
    use crate::execution::{TaskSection, TaskState};
    use crate::work::selection::SelectionState;
    use std::collections::BTreeSet;

    #[test]
    fn task_selection_states_show_only_observed_runtime_states() {
        use SelectionState::{Auto, Open, Working};
        for state in [TaskState::Unstarted, TaskState::Idle, TaskState::Active] {
            let states = task_selection_states(
                state.observed_here(true),
                TaskSection::Working,
                true,
                true,
                None,
            );
            assert_eq!(states, BTreeSet::from([Open, Working, Auto]));
        }
        for (state, expected) in [
            (TaskState::Moving, SelectionState::Moving),
            (TaskState::Done, SelectionState::Done),
            (TaskState::Abandoned, SelectionState::Done),
        ] {
            let section = TaskSection::of(state, true, true);
            let states =
                task_selection_states(state.observed_here(true), section, true, true, None);
            assert!(states.contains(&expected));
        }
    }
}
