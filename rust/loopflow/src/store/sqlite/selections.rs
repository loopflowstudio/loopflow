//! Saved selection and layout intent share one transaction and one Work snapshot.
mod panes;
use super::SqliteStore;
use crate::durable::{RepositoryId, TaskState};
use crate::store::{StoreError, StoreResult};
use crate::work::selection::{
    move_into_place, Arrangement, ArrangementEdit, ArrangementMode, Dashboard, DashboardId,
    DashboardReading, Selection, SelectionExpression, SelectionId, SelectionResolution,
    SelectionState, SelectionWork, SlotLayout, WorkWorkspaceKey, WorkWorkspaceKind,
};
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

fn work_in(conn: &Connection, repository: &RepositoryId) -> StoreResult<Vec<SelectionWork>> {
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
    let mut work = vec![SelectionWork {
        key: key(WorkWorkspaceKind::Repository, repository.to_string()),
        parent_wave: None,
        open: None,
        name: repo.clone(),
        states: BTreeSet::new(),
        activity_known: false,
    }];
    for wave in &waves {
        work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Wave, wave.id().to_string()),
            parent_wave: wave.parent_wave_id().cloned(),
            open: None,
            name: wave.name().to_string(),
            states: BTreeSet::new(),
            activity_known: false,
        });
    }
    let mut tasks = super::children::tasks_in(conn, None)?;
    tasks.retain(|task| &task.repository_id == repository);
    tasks.sort_by(|a, b| (a.rank, a.id.as_str()).cmp(&(b.rank, b.id.as_str())));
    let sessions = super::sessions::session_summaries_in(
        conn,
        &crate::session::SessionFilter {
            repo: Some(repo),
            limit: 0,
            after: Some(String::new()),
            ..Default::default()
        },
        time::OffsetDateTime::now_utc().unix_timestamp(),
        "[]",
    )?;
    let mut sessions_by_task = HashMap::<_, Vec<_>>::new();
    for session in sessions
        .iter()
        .filter(|session| session.completed_at.is_none())
    {
        for task in &session.task_ids {
            sessions_by_task.entry(task).or_default().push(session);
        }
    }
    for task in tasks {
        let started = super::chapters::task_started_or_published_in(conn, &task.id)?;
        let state = super::durable::task_state_in(conn, &task.id)?;
        let open = started && !state.is_terminal();
        let mut states = BTreeSet::new();
        if open {
            states.insert(SelectionState::Open);
        }
        states.insert(match state {
            TaskState::Unstarted => SelectionState::Unstarted,
            TaskState::Idle => SelectionState::Idle,
            TaskState::Active => SelectionState::Active,
            TaskState::Moving => SelectionState::Moving,
            TaskState::Shipping => SelectionState::Shipping,
            TaskState::Done | TaskState::Abandoned => SelectionState::Done,
        });
        if !started && !state.is_terminal() {
            states.insert(SelectionState::Backlog);
        }
        let mut activity_known = true;
        for session in sessions_by_task.get(&task.id).into_iter().flatten() {
            // A conversation Loopflow never attached cannot be running here.
            if session.agent_process_id.is_some()
                && session.agent_process_evidence
                    == crate::journal::ProcessIdentityEvidence::Unknown
            {
                activity_known = false;
            }
            if session.available {
                states.insert(SelectionState::Available);
            }
            if let Some(status) = &session.program_status {
                for report in &status.records {
                    match report.state {
                        crate::program_status::State::Working => {
                            states.insert(SelectionState::Busy);
                        }
                        crate::program_status::State::Blocked => {
                            states.insert(SelectionState::Blocked);
                        }
                        _ => {}
                    }
                }
            }
        }
        work.push(SelectionWork {
            key: key(WorkWorkspaceKind::Task, task.id.to_string()),
            parent_wave: task.wave_id,
            open: Some(open),
            name: task.identifier,
            states,
            activity_known,
        });
    }
    // The person's Task order comes first; Tasks it does not name follow, waiting
    // first. Stable sorting retains rank/ID within each attention class.
    let order = task_order_in(conn, repository)?;
    work.sort_by_key(|item| match item.key.kind {
        WorkWorkspaceKind::Task => {
            let waiting = item.states.contains(&SelectionState::Available)
                || item.states.contains(&SelectionState::Blocked);
            let position = order.iter().position(|id| id == &item.key.id);
            (0, position.unwrap_or(usize::MAX), u8::from(!waiting))
        }
        WorkWorkspaceKind::Wave => (1, 0, 0),
        WorkWorkspaceKind::Repository => (2, 0, 0),
    });
    Ok(work)
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

fn repository_default() -> Selection {
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
) -> StoreResult<SelectionResolution> {
    selection
        .resolve(
            repository.as_str(),
            &work_in(conn, repository)?,
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
const WAVE_DEFAULT_PREFIX: &str = "wave-default:";

fn plan_in(
    conn: &Connection,
    repository: &RepositoryId,
    selection: Selection,
    saved: Option<&Dashboard>,
    queries: &BTreeMap<DashboardId, Selection>,
    work: &[SelectionWork],
) -> StoreResult<crate::work::selection::ViewPlan> {
    use crate::work::selection::{PaneIntent, ViewContent, ViewLocation, ViewPlan};
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
        let pane_layout = panes::layout_in(conn, &key)?;
        let face = pane_layout.as_ref().and_then(|layout| {
            layout
                .zoomed
                .as_ref()
                .and_then(|id| layout.tree.pane(id))
                .cloned()
        });
        let mut content = ViewContent {
            pane_revision: pane_layout.as_ref().map(|layout| layout.revision),
            face,
            work: key.clone(),
            title: facts.map_or_else(|| key.id.clone(), |item| item.name.clone()),
            states: facts.map_or_else(BTreeSet::new, |item| item.states.clone()),
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
            let task_id = crate::durable::TaskId::parse(&key.id).map_err(invalid)?;
            let task = super::children::task_on(conn, &task_id)?.ok_or(StoreError::NotFound)?;
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
                if let Some(machine) = checkout.machine_id {
                    content.location = Some(ViewLocation {
                        machine_id: machine.to_string(),
                        cwd: checkout.worktree.display().to_string(),
                    });
                }
            }
            if content.location.is_none() {
                content
                    .unavailable
                    .push("Recorded execution location is unavailable".into());
            }
            if content.face.is_none() {
                content.face = primary.map(|primary| PaneIntent::Session {
                    session_id: Some(primary.id.into()),
                    location: content.location.clone(),
                });
            }
            if let Some(PaneIntent::Session {
                session_id: Some(session_id),
                location: Some(location),
            }) = &content.face
            {
                match crate::ops::human_session::human_session_argv(session_id) {
                    Ok(mut argv) => {
                        // Keep the existing resume command and data-selection fences.
                        let at = argv.len() - 3;
                        argv.splice(
                            at..at,
                            [
                                "--machine".into(),
                                location.machine_id.clone(),
                                "--repository".into(),
                                repository.to_string(),
                            ],
                        );
                        content.argv = Some(argv);
                    }
                    Err(error) => content.unavailable.push(error.to_string()),
                }
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
        selection,
        saved_revision: saved.map(|saved| saved.revision),
        resolution,
        projection: arrangement.project(),
        arrangement,
        slots: saved
            .and_then(|saved| saved.slots.clone())
            .unwrap_or(default_slots),
        contents,
    })
}

impl SqliteStore {
    /// Resolve text/defaults and saved intent without creating Work or acquiring a client.
    pub fn view_plan(
        &self,
        repository: &RepositoryId,
        source: Option<&str>,
    ) -> StoreResult<crate::work::selection::ViewPlan> {
        let selection = match source {
            Some(source) => Selection::parse(source, None).map_err(StoreError::from)?,
            None => repository_default(),
        };
        self.plan_selection(repository, selection)
    }

    pub fn plan_selection(
        &self,
        repository: &RepositoryId,
        selection: Selection,
    ) -> StoreResult<crate::work::selection::ViewPlan> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let catalog = catalog_in(&snapshot, repository)?;
        let work = work_in(&snapshot, repository)?;
        let saved = selection
            .referenced_query()
            .and_then(|id| catalog.iter().find(|saved| &saved.id == id));
        plan_in(
            &snapshot,
            repository,
            saved.map_or(selection, |saved| saved.selection.clone()),
            saved,
            &definitions(&catalog),
            &work,
        )
    }

    /// The observer and one-shot plans expose the same repository/default and saved views.
    pub fn selection_plans(
        &self,
        repository: &RepositoryId,
    ) -> StoreResult<Vec<crate::work::selection::ViewPlan>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let catalog = catalog_in(&snapshot, repository)?;
        let queries = definitions(&catalog);
        let work = work_in(&snapshot, repository)?;
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
                        wave: crate::id::WaveId::parse(&wave.key.id).map_err(invalid)?,
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
        Ok(plans)
    }

    pub fn edit_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        source: &str,
        expected_revision: Option<i64>,
    ) -> StoreResult<Dashboard> {
        // Editing must remain possible when the old query has become ambiguous.
        let previous = self
            .dashboards(repository)?
            .into_iter()
            .find(|saved| &saved.id == id);
        if previous.as_ref().map(|saved| saved.revision) != expected_revision {
            return Err(invalid("Dashboard changed; read it again before editing"));
        }
        let mut selection =
            Selection::parse(source, previous.as_ref().map(|saved| &saved.selection))
                .map_err(StoreError::from)?;
        if previous.is_none() {
            // Identical text in separately authored queries still names separate sources.
            selection.id = crate::work::selection::SelectionId(format!("query:{}", id.0));
        }
        let recovered = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            arrangement_in(&conn, repository, id, &selection.expression_key())?
        };
        let mut arrangement = if let Some((authored, arrangement)) = recovered {
            selection = authored;
            arrangement
        } else {
            previous.map_or_else(|| Arrangement::automatic(&[]), |saved| saved.arrangement)
        };
        let resolution = self.resolve_selection(repository, &selection)?;
        arrangement.reconcile(&resolution);
        self.save_dashboard(
            repository,
            id,
            expected_revision,
            &selection,
            &arrangement.to_edit(),
        )
    }

    /// A read composes with the Work observer's pinned snapshot. No acquisition.
    pub fn resolve_selection(
        &self,
        repository: &RepositoryId,
        selection: &Selection,
    ) -> StoreResult<SelectionResolution> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        resolve_in(
            &snapshot,
            repository,
            selection,
            &catalog_in(&snapshot, repository)?,
        )
    }

    pub fn dashboards(&self, repository: &RepositoryId) -> StoreResult<Vec<Dashboard>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        catalog_in(&conn, repository)
    }

    pub fn read_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
    ) -> StoreResult<Option<DashboardReading>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let catalog = catalog_in(&snapshot, repository)?;
        let Some(saved) = catalog.iter().find(|saved| &saved.id == id) else {
            return Ok(None);
        };
        let resolution = resolve_in(&snapshot, repository, &saved.selection, &catalog)?;
        let mut arrangement = saved.arrangement.clone();
        arrangement.reconcile(&resolution);
        Ok(Some(DashboardReading {
            saved: saved.clone(),
            resolution,
            arrangement,
        }))
    }

    /// Compare-and-swap both authored values. A stale or invalid edit writes nothing.
    /// The writer owns `mode`: a changed order touches the arrangement; hiding never does.
    pub fn save_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expected_revision: Option<i64>,
        selection: &Selection,
        edit: &ArrangementEdit,
    ) -> StoreResult<Dashboard> {
        if id.0.is_empty() {
            return Err(invalid("Dashboard ID must be nonempty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let catalog = catalog_in(&tx, repository)?;
        let prior = catalog.iter().find(|saved| &saved.id == id).cloned();
        if prior.as_ref().map(|saved| saved.revision) != expected_revision {
            return Err(invalid("Dashboard changed; read it again before editing"));
        }
        let work = work_in(&tx, repository)?;
        let mut definitions = definitions(&catalog);
        definitions.insert(id.clone(), selection.clone());
        // Any cycle introduced by this edit must pass through this selection.
        // Unrelated Dashboards may have ambiguous references and still be repairable.
        let resolution = selection
            .resolve(repository.as_str(), &work, &definitions)
            .map_err(StoreError::from)?;
        let expression_key = selection.expression_key();
        let mut expected = arrangement_in(&tx, repository, id, &expression_key)?
            .map(|(_, arrangement)| arrangement)
            .or_else(|| prior.as_ref().map(|saved| saved.arrangement.clone()))
            .unwrap_or_else(|| Arrangement::automatic(&[]));
        expected.reconcile(&resolution);
        let arrangement = Arrangement {
            order: edit.order.clone(),
            hidden: edit.hidden.clone(),
            mode: if edit.order == expected.order {
                expected.mode
            } else {
                ArrangementMode::Touched
            },
        };
        arrangement
            .validate(&expected.order)
            .map_err(StoreError::from)?;
        let saved = Dashboard {
            repository: repository.to_string(),
            id: id.clone(),
            revision: expected_revision.unwrap_or(0) + 1,
            name: prior.as_ref().and_then(|saved| saved.name.clone()),
            pinned: prior.as_ref().is_some_and(|saved| saved.pinned),
            slots: prior.as_ref().and_then(|saved| saved.slots.clone()),
            selection: selection.clone(),
            arrangement,
        };
        tx.execute("INSERT INTO dashboards(repository_id,id,revision,expression_key,name,pinned) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(repository_id,id) DO UPDATE SET revision=excluded.revision,expression_key=excluded.expression_key",
            params![repository.as_str(),id.0,saved.revision,expression_key,saved.name,saved.pinned])?;
        tx.execute("INSERT INTO dashboard_arrangements(repository_id,dashboard_id,expression_key,selection_json,arrangement_json) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(repository_id,dashboard_id,expression_key) DO UPDATE SET selection_json=excluded.selection_json,arrangement_json=excluded.arrangement_json",
            params![repository.as_str(),id.0,expression_key,serde_json::to_string(selection)?,serde_json::to_string(&saved.arrangement)?])?;
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
            return Err(invalid("Dashboard changed; read it again before editing"));
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
            return Err(invalid("Dashboard changed; read it again before editing"));
        }
        let saved = catalog_in(&tx, repository)?
            .into_iter()
            .find(|saved| &saved.id == id)
            .ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(saved)
    }

    /// Move a Working Tasks member to `to`'s place in the repository's one Task
    /// order, which the sidebar and every grid read. Returns the written order.
    pub fn reorder_task(
        &self,
        repository: &RepositoryId,
        task: &str,
        to: &str,
    ) -> StoreResult<Vec<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let work = work_in(&tx, repository)?;
        let resolution = repository_default()
            .resolve(repository.as_str(), &work, &BTreeMap::new())
            .map_err(StoreError::from)?;
        let mut order = resolution.task_ids();
        let from = order
            .iter()
            .position(|id| id == task)
            .ok_or(StoreError::NotFound)?;
        let target = order
            .iter()
            .position(|id| id == to)
            .ok_or(StoreError::NotFound)?;
        move_into_place(&mut order, from, target);
        tx.execute(
            "INSERT INTO task_orders(repository_id,task_ids_json) VALUES(?1,?2) ON CONFLICT(repository_id) DO UPDATE SET task_ids_json=excluded.task_ids_json",
            params![repository.as_str(), serde_json::to_string(&order)?],
        )?;
        tx.commit()?;
        Ok(order)
    }

    /// Move `work`'s visible occurrence into `to`'s place in a Dashboard's order,
    /// under the same revision check as every Dashboard edit.
    pub fn reorder_dashboard(
        &self,
        repository: &RepositoryId,
        id: &DashboardId,
        expected_revision: i64,
        work: &WorkWorkspaceKey,
        to: &WorkWorkspaceKey,
    ) -> StoreResult<Dashboard> {
        let reading = self
            .read_dashboard(repository, id)?
            .ok_or(StoreError::NotFound)?;
        let mut arrangement = reading.arrangement;
        let from = arrangement.winner(work).ok_or(StoreError::NotFound)?;
        let target = arrangement.winner(to).ok_or(StoreError::NotFound)?;
        move_into_place(&mut arrangement.order, from, target);
        // A concurrent edit since this read advances the revision and is rejected there.
        self.save_dashboard(
            repository,
            id,
            Some(expected_revision),
            &reading.saved.selection,
            &arrangement.to_edit(),
        )
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
            return Err(invalid("Dashboard changed; read it again before deleting"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use crate::durable::RepositoryId;
    use crate::work::selection::{
        Arrangement, ArrangementMode, DashboardId, Selection, SelectionExpression, SelectionId,
        WorkWorkspaceKind,
    };

    fn fixture() -> (tempfile::TempDir, SqliteStore, RepositoryId) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!(
                "../../../tests/fixtures/saved_selection_work.sql"
            ))
            .unwrap();
        let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
        (dir, store, repo)
    }

    fn selection() -> Selection {
        Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Repository,
        }
    }

    #[test]
    fn selection_edits_can_repair_ambiguity_without_blocking_other_dashboards() {
        let (_dir, store, repo) = fixture();
        let id = DashboardId("picked".into());
        let saved = store.edit_dashboard(&repo, &id, "task:#1", None).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET issue_identifier='OTHER-1' WHERE issue_identifier='FIX-2'",
                [],
            )
            .unwrap();
        assert!(store.read_dashboard(&repo, &id).is_err());

        let independent = store
            .edit_dashboard(&repo, &DashboardId("backlog".into()), "state:backlog", None)
            .unwrap();
        assert_eq!(independent.arrangement.order.len(), 1);
        let repaired = store
            .edit_dashboard(&repo, &id, "task:FIX-1", Some(saved.revision))
            .unwrap();
        assert_eq!(repaired.arrangement, saved.arrangement);
        assert_eq!(
            store.read_dashboard(&repo, &id).unwrap().unwrap().saved,
            repaired
        );
    }

    #[test]
    fn selection_revisit_recovers_authored_geometry_and_stale_edits_change_nothing() {
        let (_dir, store, repo) = fixture();
        let id = DashboardId("revisit".into());
        let first = store
            .edit_dashboard(&repo, &id, "state:open or state:backlog", None)
            .unwrap();
        let mut arranged = first.arrangement.clone();
        arranged.hide(&arranged.order[0].clone(), true).unwrap();
        arranged.order.reverse();
        let first = store
            .save_dashboard(
                &repo,
                &id,
                Some(first.revision),
                &first.selection,
                &arranged.to_edit(),
            )
            .unwrap();
        let next = store
            .edit_dashboard(&repo, &id, "state:done", Some(first.revision))
            .unwrap();
        let restored = store
            .edit_dashboard(
                &repo,
                &id,
                "  state:open   or state:backlog  ",
                Some(next.revision),
            )
            .unwrap();
        assert_eq!(restored.selection, first.selection);
        assert_eq!(restored.arrangement, first.arrangement);
        assert_eq!(restored.arrangement.order, arranged.order);
        let revisions = store.revisions().unwrap();
        assert!(store
            .edit_dashboard(&repo, &id, "state:deferred", Some(next.revision))
            .is_err());
        assert!(store
            .save_dashboard(
                &repo,
                &id,
                Some(next.revision),
                &next.selection,
                &next.arrangement.to_edit()
            )
            .is_err());
        let invalid = Arrangement::automatic(&[]);
        assert!(store
            .save_dashboard(
                &repo,
                &id,
                Some(restored.revision),
                &restored.selection,
                &invalid.to_edit()
            )
            .is_err());
        assert_eq!(store.revisions().unwrap(), revisions);
        assert_eq!(
            store.read_dashboard(&repo, &id).unwrap().unwrap().saved,
            restored
        );
        assert_eq!(
            store
                .conn
                .lock()
                .unwrap()
                .query_row("SELECT count(*) FROM dashboard_arrangements", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn dashboard_order_moves_under_its_revision_and_only_reordering_touches() {
        let (_dir, store, repo) = fixture();
        let id = DashboardId("ordered".into());
        let saved = store
            .edit_dashboard(&repo, &id, "task:FIX-1 or task:FIX-2 or task:FIX-3", None)
            .unwrap();
        let [a, b, c] = [0, 1, 2].map(|i| saved.arrangement.order[i].clone());
        assert_eq!(saved.arrangement.mode, ArrangementMode::Auto);

        // Hiding and restoring keep the automatic order.
        let mut hidden = saved.arrangement.clone();
        hidden.hide(&b, true).unwrap();
        let hidden = store
            .save_dashboard(
                &repo,
                &id,
                Some(saved.revision),
                &saved.selection,
                &hidden.to_edit(),
            )
            .unwrap();
        assert_eq!(hidden.arrangement.mode, ArrangementMode::Auto);
        let mut restored = hidden.arrangement.clone();
        restored.hide(&b, false).unwrap();
        let restored = store
            .save_dashboard(
                &repo,
                &id,
                Some(hidden.revision),
                &saved.selection,
                &restored.to_edit(),
            )
            .unwrap();
        assert_eq!(restored.arrangement.mode, ArrangementMode::Auto);

        let moved = store
            .reorder_dashboard(&repo, &id, restored.revision, &c.work, &a.work)
            .unwrap();
        assert_eq!(moved.arrangement.order, [c.clone(), a.clone(), b.clone()]);
        assert_eq!(moved.arrangement.mode, ArrangementMode::Touched);
        assert_eq!(moved.revision, restored.revision + 1);

        let revisions = store.revisions().unwrap();
        assert!(store
            .reorder_dashboard(&repo, &id, restored.revision, &a.work, &c.work)
            .is_err());
        assert_eq!(store.revisions().unwrap(), revisions);
        assert_eq!(
            store.read_dashboard(&repo, &id).unwrap().unwrap().saved,
            moved
        );
    }

    #[test]
    fn one_task_order_reorders_working_tasks_and_every_plan_and_arrivals_append() {
        let (_dir, store, repo) = fixture();
        let (one, two, three) = (
            "task_11111111111111111111111111111111",
            "task_22222222222222222222222222222222",
            "task_33333333333333333333333333333333",
        );
        // With a checkout, FIX-1 is idle and Working.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/repo/fix-1' WHERE issue_identifier='FIX-1'",
                [],
            )
            .unwrap();
        assert_eq!(
            store.view_plan(&repo, None).unwrap().resolution.task_ids(),
            [one, two]
        );
        assert_eq!(store.reorder_task(&repo, two, one).unwrap(), [two, one]);
        assert_eq!(
            store.view_plan(&repo, None).unwrap().resolution.task_ids(),
            [two, one]
        );
        let plans = store.selection_plans(&repo).unwrap();
        let wave = plans
            .iter()
            .find(|plan| {
                plan.selection.id.0.starts_with("wave-default:")
                    && plan.resolution.members.len() > 1
            })
            .unwrap();
        // The Wave strip lists Working Tasks; unstarted FIX-3 waits in Backlog.
        assert_eq!(wave.resolution.task_ids(), [two, one]);

        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO task_events(task_id,kind_json,created_at) VALUES('{three}','{{\"kind\":\"started\"}}',2); INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here) VALUES('own_33333333333333333333333333333333','{three}','recorded',NULL,NULL,2,2000,1); UPDATE tasks SET worktree='/repo/fix-3' WHERE id='{three}';"
            ))
            .unwrap();
        assert_eq!(
            store.view_plan(&repo, None).unwrap().resolution.task_ids(),
            [two, one, three]
        );
        assert_eq!(
            store.reorder_task(&repo, three, two).unwrap(),
            [three, two, one]
        );

        let revisions = store.revisions().unwrap();
        // A done Task is not a Working Tasks member; nothing is written.
        assert!(store
            .reorder_task(&repo, "task_44444444444444444444444444444444", one)
            .is_err());
        assert_eq!(store.revisions().unwrap(), revisions);
        assert_eq!(
            store.view_plan(&repo, None).unwrap().resolution.task_ids(),
            [three, two, one]
        );
    }

    #[test]
    fn wave_default_plan_uses_three_slot_strip() {
        use crate::work::selection::{SlotLayout, SlotNode, SplitAxis};
        let (_dir, store, repo) = fixture();
        let plans = store.selection_plans(&repo).unwrap();
        let wave = plans
            .iter()
            .find(|plan| plan.selection.id.0.starts_with(super::WAVE_DEFAULT_PREFIX))
            .expect("fixture has a Wave");
        assert_eq!(wave.slots, SlotLayout::strip(3));
        let SlotNode::Split { axis, weights, .. } = &wave.slots.root else {
            panic!("a strip splits");
        };
        assert_eq!(*axis, SplitAxis::Vertical);
        assert_eq!(weights, &[1.0; 3]);
        assert_eq!(wave.slots.slots(), [0, 1, 2]);
        // The repository page is a Wave view too.
        assert_eq!(plans[0].slots, SlotLayout::strip(3));
    }

    #[test]
    fn resizing_two_slots_reweights_only_the_split_between_them() {
        use crate::work::selection::{SlotLayout, SlotNode};
        let golden = SlotLayout::golden();
        // Slots 1 and 2 share the inner top split; the bottom band and slot 0 keep theirs.
        let resized = golden.resized(1, 2, 0.75).unwrap();
        let SlotNode::Split { children, .. } = &resized.root else {
            panic!("golden root splits");
        };
        let SlotNode::Split {
            children: top,
            weights: top_weights,
            ..
        } = &children[0]
        else {
            panic!("golden top splits");
        };
        assert_eq!(top_weights, &[0.618, 1.0 - 0.618]);
        let SlotNode::Split { weights, .. } = &top[1] else {
            panic!("slots 1 and 2 split");
        };
        assert_eq!(weights, &[1.5, 0.5]);
        let SlotNode::Split { children: band, .. } = &golden.root else {
            panic!("golden root splits");
        };
        assert_eq!(children[1], band[1]);
        // Slots 0 and 5 meet only at the root.
        let SlotNode::Split { weights, .. } = &golden.resized(0, 5, 0.5).unwrap().root else {
            panic!("golden root splits");
        };
        assert_eq!(weights, &[0.5, 0.5]);
        assert!(golden.resized(3, 3, 0.5).is_none());
        assert!(golden.resized(3, 9, 0.5).is_none());
    }

    #[test]
    fn dashboard_slots_round_trip_survive_query_edits_and_reject_invalid_layouts() {
        use crate::work::selection::{SlotLayout, SlotNode, SplitAxis};
        let (_dir, store, repo) = fixture();
        assert_eq!(
            store.view_plan(&repo, Some("state:open")).unwrap().slots,
            SlotLayout::golden()
        );
        assert_eq!(SlotLayout::golden().slots().len(), 7);
        let id = DashboardId("slots".into());
        let saved = store
            .edit_dashboard(&repo, &id, "state:open", None)
            .unwrap();
        assert_eq!(saved.slots, None);
        let query = format!("query:{}", id.0);
        assert_eq!(
            store.view_plan(&repo, Some(&query)).unwrap().slots,
            SlotLayout::golden()
        );

        let two = SlotLayout {
            root: SlotNode::Split {
                axis: SplitAxis::Vertical,
                weights: vec![2.0, 1.0],
                children: vec![SlotNode::Slot { slot: 1 }, SlotNode::Slot { slot: 0 }],
            },
        };
        let arranged = store
            .set_dashboard_slots(&repo, &id, saved.revision, Some(&two))
            .unwrap();
        assert_eq!(arranged.revision, saved.revision + 1);
        assert_eq!(store.view_plan(&repo, Some(&query)).unwrap().slots, two);
        let edited = store
            .edit_dashboard(&repo, &id, "state:backlog", Some(arranged.revision))
            .unwrap();
        assert_eq!(edited.slots, Some(two.clone()));

        let revisions = store.revisions().unwrap();
        let duplicate = SlotLayout {
            root: SlotNode::Split {
                axis: SplitAxis::Vertical,
                weights: vec![1.0, 1.0],
                children: vec![SlotNode::Slot { slot: 0 }, SlotNode::Slot { slot: 0 }],
            },
        };
        assert!(store
            .set_dashboard_slots(&repo, &id, edited.revision, Some(&duplicate))
            .is_err());
        assert!(store
            .set_dashboard_slots(&repo, &id, arranged.revision, None)
            .is_err());
        assert_eq!(store.revisions().unwrap(), revisions);
        let reset = store
            .set_dashboard_slots(&repo, &id, edited.revision, None)
            .unwrap();
        assert_eq!(reset.slots, None);
        assert_eq!(
            store.view_plan(&repo, Some(&query)).unwrap().slots,
            SlotLayout::golden()
        );
    }

    #[test]
    fn repository_queries_include_tasks_without_a_wave() {
        let (_dir, store, repo) = fixture();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET project_id=NULL WHERE issue_identifier='FIX-2'",
                [],
            )
            .unwrap();
        let plan = store.view_plan(&repo, Some("task:FIX-2")).unwrap();
        assert_eq!(plan.contents.len(), 1);
        assert_eq!(plan.contents[0].title, "Idle");
        assert!(store
            .view_plan(&repo, None)
            .unwrap()
            .contents
            .iter()
            .any(|content| content.work == plan.contents[0].work));
        let wave = store
            .view_plan(&repo, Some("wave:Product state:open"))
            .unwrap();
        assert!(!wave
            .contents
            .iter()
            .any(|content| content.work == plan.contents[0].work));
    }

    #[test]
    fn dashboards_leave_repository_and_wave_defaults_unchanged() {
        let (_dir, store, repo) = fixture();
        let defaults = store.selection_plans(&repo).unwrap();
        let id = DashboardId("backlog".into());
        let saved = store
            .edit_dashboard(&repo, &id, "state:backlog", None)
            .unwrap();
        let named = store
            .set_dashboard_metadata(&repo, &id, saved.revision, Some("Later"), Some(true))
            .unwrap();
        let revisions = store.revisions().unwrap();
        let dashboard = store.view_plan(&repo, Some("query:backlog")).unwrap();
        assert_eq!(dashboard.dashboard_id, Some(id.clone()));
        assert_eq!(dashboard.name.as_deref(), Some("Later"));
        assert!(dashboard.pinned);
        assert_eq!(dashboard.contents.len(), 1);
        assert!(dashboard.contents[0].primary_session.is_none());
        let plans = store.selection_plans(&repo).unwrap();
        assert_eq!(plans.len(), defaults.len() + 1);
        for (before, after) in defaults.iter().zip(&plans) {
            assert_eq!(before.selection, after.selection);
            assert_eq!(before.resolution, after.resolution);
            assert_eq!(before.arrangement, after.arrangement);
            assert!(after.dashboard_id.is_none());
        }
        assert_eq!(
            store.view_plan(&repo, None).unwrap().resolution,
            defaults[0].resolution
        );
        assert_eq!(store.revisions().unwrap(), revisions);
        assert!(store
            .set_dashboard_metadata(&repo, &id, saved.revision, Some("stale"), Some(false),)
            .is_err());
        assert_eq!(store.dashboards(&repo).unwrap(), [named]);
        assert_eq!(store.revisions().unwrap(), revisions);
    }

    #[test]
    fn selection_pane_ownership_is_repository_scoped_for_every_work_kind() {
        use crate::store::StoreError;
        use crate::work::selection::{WorkPaneLayout, WorkWorkspaceKey};
        let (_dir, store, repo) = fixture();
        let other = store.ensure_repository("/other").unwrap();
        for (kind, id) in [
            (WorkWorkspaceKind::Repository, repo.to_string()),
            (
                WorkWorkspaceKind::Wave,
                "11111111-1111-1111-1111-111111111111".into(),
            ),
            // Backlog Work still owns panes even though it is outside the default view.
            (
                WorkWorkspaceKind::Task,
                "task_33333333333333333333333333333333".into(),
            ),
        ] {
            let mut layout = WorkPaneLayout {
                work: WorkWorkspaceKey {
                    repository: repo.to_string(),
                    kind: kind.clone(),
                    id,
                },
                revision: 0,
                tree: designated_tree(&kind),
                focused: None,
                zoomed: None,
                hidden: Default::default(),
                drafts: vec![],
            };
            assert!(store.pane_layout(&layout.work).unwrap().is_none());
            let saved = store.save_pane_layout(&layout).unwrap();
            assert_eq!(
                store.pane_layout(&layout.work).unwrap(),
                Some(saved.clone())
            );
            if layout.work.kind == WorkWorkspaceKind::Task {
                // Leaving a Project changes membership, not the Task's pane owner.
                store
                    .conn
                    .lock()
                    .unwrap()
                    .execute(
                        "UPDATE tasks SET project_id=NULL WHERE id=?1",
                        [&layout.work.id],
                    )
                    .unwrap();
                assert_eq!(
                    store.pane_layout(&layout.work).unwrap(),
                    Some(saved.clone())
                );
                assert_eq!(store.save_pane_layout(&saved).unwrap().revision, 2);
            }
            let revision = store.revisions().unwrap();
            layout.work.repository = other.to_string();
            assert!(matches!(
                store.pane_layout(&layout.work),
                Err(StoreError::NotFound)
            ));
            assert!(matches!(
                store.save_pane_layout(&layout),
                Err(StoreError::NotFound)
            ));
            assert_eq!(store.revisions().unwrap(), revision);
            layout.work.repository = repo.to_string();
            match layout.work.kind {
                WorkWorkspaceKind::Task => {
                    store
                        .conn
                        .lock()
                        .unwrap()
                        .execute(
                            "UPDATE tasks SET planning_deleted_at=1 WHERE id=?1",
                            [&layout.work.id],
                        )
                        .unwrap();
                }
                WorkWorkspaceKind::Wave => {
                    // Use the empty Wave so the later Task case remains available.
                    layout.work.id = "22222222-2222-2222-2222-222222222222".into();
                    store
                        .conn
                        .lock()
                        .unwrap()
                        .execute(
                            "UPDATE waves SET retired_at=1, superseded_by_wave_id='11111111-1111-1111-1111-111111111111', retirement_reason='Merged into Product' WHERE id=?1",
                            [&layout.work.id],
                        )
                        .unwrap();
                }
                WorkWorkspaceKind::Repository => continue,
            }
            assert!(matches!(
                store.pane_layout(&layout.work),
                Err(StoreError::NotFound)
            ));
        }
    }

    fn designated_tree(kind: &WorkWorkspaceKind) -> crate::work::selection::PaneTree {
        use crate::work::selection::{PaneIntent, PaneTree, SplitAxis};
        let pane = |id: &str, content| PaneTree::Pane {
            id: id.into(),
            content,
        };
        let session = pane(
            "session",
            PaneIntent::Session {
                session_id: None,
                location: None,
            },
        );
        if *kind != WorkWorkspaceKind::Task {
            return session;
        }
        let split = |first, second| PaneTree::Split {
            axis: SplitAxis::Vertical,
            ratio: 0.5,
            first: Box::new(first),
            second: Box::new(second),
        };
        split(
            session,
            split(
                pane(
                    "shell",
                    PaneIntent::Shell {
                        tabs: vec![],
                        selected: None,
                    },
                ),
                pane(
                    "browser",
                    PaneIntent::Browser {
                        location: None,
                        tabs: vec![],
                        selected: None,
                    },
                ),
            ),
        )
    }

    #[test]
    fn selection_designated_panes_restore_and_invalid_writes_are_atomic() {
        use crate::work::selection::{BrowserTab, PaneIntent, PaneTree, WorkPaneLayout};
        let (dir, store, repo) = fixture();
        let mut layout: WorkPaneLayout = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/pane_layout.json"
        ))
        .unwrap();
        layout.work.repository = repo.to_string();
        layout.revision = 0;
        // The fixture's conversation is not recorded here; the pane shows the start state.
        layout.tree = match layout.tree {
            PaneTree::Split {
                axis,
                ratio,
                second,
                ..
            } => PaneTree::Split {
                axis,
                ratio,
                first: Box::new(PaneTree::Pane {
                    id: "pane-0".into(),
                    content: PaneIntent::Session {
                        session_id: None,
                        location: None,
                    },
                }),
                second,
            },
            tree => tree,
        };
        let saved = store.save_pane_layout(&layout).unwrap();
        let reader = SqliteStore::open_read_only(&dir.path().join("db")).unwrap();
        assert_eq!(
            reader.pane_layout(&saved.work).unwrap(),
            Some(saved.clone())
        );
        let revision = store.revisions().unwrap();
        let Some(PaneIntent::Shell { tabs, selected }) = saved.tree.pane("pane-1") else {
            panic!("shell")
        };
        assert_eq!(
            tabs.iter().map(|tab| tab.id.as_str()).collect::<Vec<_>>(),
            ["shell-1", "shell-2"]
        );
        assert_eq!(selected.as_deref(), Some("shell-2"));
        let Some(PaneIntent::Browser { tabs, selected, .. }) = saved.tree.pane("pane-2") else {
            panic!("browser")
        };
        assert!(matches!(&tabs[0], BrowserTab::Files { tabs, .. } if tabs.len() == 2));
        assert_eq!(selected.as_deref(), Some("files"));
        assert_eq!(saved.drafts[0].text, "unsaved edits");
        for kind in [WorkWorkspaceKind::Repository, WorkWorkspaceKind::Wave] {
            let mut invalid = saved.clone();
            invalid.work.kind = kind;
            assert!(store.save_pane_layout(&invalid).is_err());
        }
        // A fourth pane, a missing pane, a stale tab selection and a non-web page are rejected.
        let mut invalid = saved.clone();
        invalid.tree = PaneTree::Split {
            axis: crate::work::selection::SplitAxis::Vertical,
            ratio: 0.5,
            first: Box::new(invalid.tree),
            second: Box::new(PaneTree::Pane {
                id: "second-shell".into(),
                content: PaneIntent::Shell {
                    tabs: vec![],
                    selected: None,
                },
            }),
        };
        assert!(store.save_pane_layout(&invalid).is_err());
        let mut invalid = saved.clone();
        if let PaneTree::Split { second, .. } = &invalid.tree {
            invalid.tree = (**second).clone();
        }
        invalid.focused = None;
        assert!(store.save_pane_layout(&invalid).is_err());
        let edit = |change: &dyn Fn(&mut PaneIntent)| {
            let mut invalid = saved.clone();
            fn visit(tree: &mut PaneTree, change: &dyn Fn(&mut PaneIntent)) {
                match tree {
                    PaneTree::Pane { content, .. } => change(content),
                    PaneTree::Split { first, second, .. } => {
                        visit(first, change);
                        visit(second, change);
                    }
                }
            }
            visit(&mut invalid.tree, change);
            invalid
        };
        assert!(store
            .save_pane_layout(&edit(
                &|content| if let PaneIntent::Shell { selected, .. } = content {
                    *selected = Some("removed".into());
                }
            ))
            .is_err());
        assert!(store
            .save_pane_layout(&edit(
                &|content| if let PaneIntent::Browser { tabs, .. } = content {
                    tabs.push(BrowserTab::Url {
                        id: "page-2".into(),
                        url: "file:///etc/passwd".into(),
                    });
                }
            ))
            .is_err());
        assert_eq!(store.revisions().unwrap(), revision);
        assert_eq!(store.pane_layout(&saved.work).unwrap(), Some(saved));
    }

    #[test]
    fn selection_panes_restore_drafts_and_reject_stale_or_foreign_intent() {
        use crate::work::selection::{
            BrowserTab, FileDraft, PaneIntent, PaneTree, ViewLocation, WorkPaneLayout,
            WorkWorkspaceKey,
        };
        let (dir, store, repo) = fixture();
        store.test_session(
            "selection-primary",
            &crate::session_record::new_artifact_key(),
        );
        store.conn.lock().unwrap().execute_batch("UPDATE agent_sessions SET repo='/repo',task_id='task_22222222222222222222222222222222',wave_id='11111111-1111-1111-1111-111111111111' WHERE id='selection-primary'; UPDATE tasks SET primary_session_id='selection-primary' WHERE id='task_22222222222222222222222222222222';").unwrap();
        let location = ViewLocation {
            machine_id: "machine_recorded".into(),
            cwd: "/repo/task".into(),
        };
        let with_session = |session_id: Option<&str>| {
            fn visit(tree: &mut PaneTree, session: Option<&str>, location: &ViewLocation) {
                match tree {
                    PaneTree::Pane { content, .. } => match content {
                        PaneIntent::Session { session_id, .. } => {
                            *session_id = session.map(Into::into)
                        }
                        PaneIntent::Browser {
                            location: place,
                            tabs,
                            selected,
                        } => {
                            *place = Some(location.clone());
                            *tabs = vec![BrowserTab::Files {
                                id: "files".into(),
                                tabs: vec![crate::work::selection::FileTab {
                                    id: "file-1".into(),
                                    path: "src/lib.rs".into(),
                                }],
                                selected: Some("file-1".into()),
                            }];
                            *selected = Some("files".into());
                        }
                        PaneIntent::Shell { .. } => {}
                    },
                    PaneTree::Split { first, second, .. } => {
                        visit(first, session, location);
                        visit(second, session, location);
                    }
                }
            }
            let mut tree = designated_tree(&WorkWorkspaceKind::Task);
            visit(&mut tree, session_id, &location);
            tree
        };
        let layout = WorkPaneLayout {
            work: WorkWorkspaceKey {
                repository: repo.to_string(),
                kind: WorkWorkspaceKind::Task,
                id: "task_22222222222222222222222222222222".into(),
            },
            revision: 0,
            tree: with_session(None),
            focused: Some("browser".into()),
            zoomed: Some("browser".into()),
            hidden: Default::default(),
            drafts: vec![FileDraft {
                location: location.clone(),
                path: "src/lib.rs".into(),
                base_revision: "disk-revision".into(),
                base_content: "base".into(),
                text: "unsaved edits".into(),
            }],
        };
        let saved = store.save_pane_layout(&layout).unwrap();
        let plan = store
            .view_plan(&repo, Some(&format!("task:{}", layout.work.id)))
            .unwrap();
        assert_eq!(plan.contents[0].pane_revision, Some(saved.revision));
        assert!(matches!(
            plan.contents[0].face,
            Some(PaneIntent::Browser { .. })
        ));
        assert!(plan.contents[0].argv.is_none());
        for id in ["first", "second"] {
            store
                .edit_dashboard(
                    &repo,
                    &DashboardId(id.into()),
                    &format!("task:{}", layout.work.id),
                    None,
                )
                .unwrap();
        }
        let first = store
            .read_dashboard(&repo, &DashboardId("first".into()))
            .unwrap()
            .unwrap();
        let mut hidden = first.arrangement;
        hidden.hide(&hidden.order[0].clone(), true).unwrap();
        store
            .save_dashboard(
                &repo,
                &first.saved.id,
                Some(first.saved.revision),
                &first.saved.selection,
                &hidden.to_edit(),
            )
            .unwrap();
        let revisions = store.revisions().unwrap();
        let reader = SqliteStore::open_read_only(&dir.path().join("db")).unwrap();
        let first = reader.view_plan(&repo, Some("query:first")).unwrap();
        let second = reader.view_plan(&repo, Some("query:second")).unwrap();
        assert!(first.projection.visible.is_empty());
        assert!(!second.projection.visible.is_empty());
        assert_ne!(first.dashboard_id, second.dashboard_id);
        assert_eq!(first.contents, second.contents);
        assert_eq!(first.contents[0].pane_revision, Some(saved.revision));
        assert_eq!(
            reader.pane_layout(&layout.work).unwrap(),
            Some(saved.clone())
        );
        assert!(store.save_pane_layout(&layout).is_err());
        let mut invalid = saved.clone();
        invalid.hidden.insert("browser".into());
        assert!(store.save_pane_layout(&invalid).is_err());
        invalid = saved.clone();
        invalid.drafts[0].path = "../outside".into();
        assert!(store.save_pane_layout(&invalid).is_err());
        invalid = saved.clone();
        invalid.tree = with_session(Some("unknown-session"));
        assert!(store.save_pane_layout(&invalid).is_err());
        invalid = saved.clone();
        invalid.work.id = "task_99999999999999999999999999999999".into();
        assert!(store.save_pane_layout(&invalid).is_err());
        assert_eq!(store.revisions().unwrap(), revisions);
        assert_eq!(store.pane_layout(&saved.work).unwrap(), Some(saved.clone()));
        let mut unzoomed = saved;
        unzoomed.zoomed = None;
        let mut unzoomed = store.save_pane_layout(&unzoomed).unwrap();
        let plan = store
            .view_plan(&repo, Some(&format!("task:{}", unzoomed.work.id)))
            .unwrap();
        assert!(
            matches!(&plan.contents[0].face, Some(PaneIntent::Session {session_id: Some(session_id), ..}) if session_id == "selection-primary")
        );
        unzoomed.tree = with_session(Some("selection-primary"));
        let mut session_layout = store.save_pane_layout(&unzoomed).unwrap();
        session_layout.work.id = "task_11111111111111111111111111111111".into();
        session_layout.revision = 0;
        assert!(store.save_pane_layout(&session_layout).is_err());
    }

    #[test]
    fn query_selection_identical_saved_expressions_keep_distinct_sources() {
        let (_dir, store, repo) = fixture();
        for id in ["first", "second"] {
            store
                .edit_dashboard(&repo, &DashboardId(id.into()), "state:open", None)
                .unwrap();
        }
        let first = store.view_plan(&repo, Some("query:first")).unwrap();
        let second = store.view_plan(&repo, Some("query:second")).unwrap();
        assert_eq!(
            first.resolution.members[0].work,
            second.resolution.members[0].work
        );
        assert_ne!(
            first.resolution.members[0].source,
            second.resolution.members[0].source
        );
        let edited = store
            .edit_dashboard(
                &repo,
                &DashboardId("first".into()),
                "state:open or state:backlog",
                first.saved_revision,
            )
            .unwrap();
        assert_eq!(edited.arrangement.order[0], first.resolution.members[0]);
    }

    #[test]
    fn query_selection_uses_current_reported_attention_and_prunes_ended_activity() {
        let (_dir, store, repo) = fixture();
        store.test_session(
            "selection-session",
            &crate::session_record::new_artifact_key(),
        );
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE agent_sessions SET repo='/repo', task_id='task_22222222222222222222222222222222', wave_id='11111111-1111-1111-1111-111111111111' WHERE id='selection-session'", []).unwrap();
        }
        let attachment = store
            .claim_session_attachment("selection-session", None, &store.test_lf_process(), true)
            .unwrap();
        store
            .record_agent_process_identity(
                "selection-session",
                &attachment,
                std::process::id(),
                crate::journal::process_started_at(std::process::id())
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        store
            .begin_program_status(
                "selection-session",
                Some(&attachment.agent_process_id),
                "selection-surface",
            )
            .unwrap();
        let report = crate::program_status::Records {
            seen: true,
            records: vec![crate::program_status::Report {
                state: crate::program_status::State::Blocked,
                id: None,
                kind: Some(crate::program_status::Kind::Question),
                progress: None,
                app: None,
                title: None,
                msg: None,
            }],
        };
        store
            .record_program_status(
                "selection-session",
                Some(&attachment.agent_process_id),
                "selection-surface",
                1,
                &report,
            )
            .unwrap();
        let available = Selection::parse("state:available", None).unwrap();
        let members = store.resolve_selection(&repo, &available).unwrap();
        assert_eq!(members.members.len(), 1);
        assert_eq!(
            members.members[0].work.id,
            "task_22222222222222222222222222222222"
        );
        let default = store.resolve_selection(&repo, &selection()).unwrap();
        assert_eq!(default.members[0].work, members.members[0].work);
        let id = DashboardId("available".into());
        store
            .save_dashboard(
                &repo,
                &id,
                None,
                &available,
                &Arrangement::automatic(&members.members).to_edit(),
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE processes SET completed_at=2,outcome='succeeded' WHERE id=?1",
                [&attachment.agent_process_id],
            )
            .unwrap();
        let ended = store.read_dashboard(&repo, &id).unwrap().unwrap();
        assert!(ended.resolution.unavailable.is_empty());
        assert!(ended.arrangement.order.is_empty());
        // The same Task is still open; only its current attention departed.
        assert_eq!(
            store
                .resolve_selection(&repo, &selection())
                .unwrap()
                .members
                .len(),
            3
        );
    }

    #[test]
    fn stale_conversations_leave_task_activity_known() {
        let (_dir, store, repo) = fixture();
        let task = "task_22222222222222222222222222222222";
        let running = Selection::parse("state:busy", None).unwrap();
        for id in ["never-attached", "before-boot"] {
            store.test_session(id, &crate::session_record::new_artifact_key());
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET repo='/repo', task_id=?2, wave_id='11111111-1111-1111-1111-111111111111' WHERE id=?1",
                    [id, task],
                )
                .unwrap();
        }
        let unattached = store.resolve_selection(&repo, &running).unwrap();
        let attachment = store
            .claim_session_attachment("before-boot", None, &store.test_lf_process(), true)
            .unwrap();
        let started: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT started_at FROM processes WHERE id=?1",
                [&attachment.agent_process_id],
                |row| row.get(0),
            )
            .unwrap();
        crate::journal::set_test_machine_booted_at(Some(started - 1));
        let unknown = store.resolve_selection(&repo, &running).unwrap();
        crate::journal::set_test_machine_booted_at(Some(started + 1));
        let restarted = store.resolve_selection(&repo, &running).unwrap();
        crate::journal::set_test_machine_booted_at(None);
        // Only the attached Process without OS identity, begun since boot, is unknown.
        assert!(unattached.unavailable.is_empty());
        assert_eq!(unknown.unavailable, ["Task FIX-2 activity is unavailable"]);
        assert!(restarted.unavailable.is_empty());
    }

    #[test]
    fn a_published_pr_starts_a_task_for_queries_as_for_its_runtime() {
        let (_dir, store, repo) = fixture();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,
                    parent_pr_id,publication_requested_at,created_at,updated_at)
                 SELECT 'pr_backlog',id,1,'backlog',branch,base_commit,parent_pr_id,1,1,1
                 FROM tasks WHERE issue_identifier='FIX-3'",
                [],
            )
            .unwrap();
        assert!(store
            .view_plan(&repo, Some("state:backlog"))
            .unwrap()
            .contents
            .is_empty());
        assert!(store
            .view_plan(&repo, Some("state:open"))
            .unwrap()
            .contents
            .iter()
            .any(|content| content.title == "Backlog"));
    }

    #[test]
    fn query_plans_include_backlog_without_creating_execution_and_keep_saved_edits() {
        let (_dir, store, repo) = fixture();
        let before = store.revisions().unwrap();
        let plan = store.view_plan(&repo, Some("state:backlog")).unwrap();
        assert_eq!(plan.contents.len(), 1);
        let backlog = &plan.contents[0];
        assert_eq!(backlog.title, "Backlog");
        assert!(backlog.primary_session.is_none());
        assert!(backlog.location.is_none());
        assert!(backlog.argv.is_none());
        assert_eq!(backlog.unavailable.len(), 2);
        assert_eq!(store.revisions().unwrap(), before);
        let default = store.view_plan(&repo, None).unwrap();
        assert_eq!(
            default.resolution.members[0].work.kind,
            WorkWorkspaceKind::Task
        );
        assert_eq!(
            default.resolution.members.last().unwrap().work.kind,
            WorkWorkspaceKind::Wave
        );
        let id = DashboardId("picked".into());
        let first = store
            .edit_dashboard(&repo, &id, "task:FIX-1 or task:FIX-2", None)
            .unwrap();
        let original = first.arrangement.order.clone();
        let mut arrangement = first.arrangement.clone();
        arrangement.hide(&original[0], true).unwrap();
        arrangement.order.reverse();
        let touched = store
            .save_dashboard(
                &repo,
                &id,
                Some(first.revision),
                &first.selection,
                &arrangement.to_edit(),
            )
            .unwrap();
        let second = store
            .edit_dashboard(
                &repo,
                &id,
                "task:FIX-3 or task:FIX-1 or task:FIX-2",
                Some(touched.revision),
            )
            .unwrap();
        assert_eq!(&second.arrangement.order[..2], arrangement.order);
        assert!(second.arrangement.hidden.contains(&original[0]));
        assert!(store
            .edit_dashboard(&repo, &id, "state:open", Some(first.revision))
            .is_err());
        assert!(store
            .edit_dashboard(&repo, &id, "query:picked", Some(second.revision))
            .is_err());
        let plan = store.view_plan(&repo, Some("query:picked")).unwrap();
        assert_eq!(plan.saved_revision, Some(second.revision));
        assert_eq!(plan.arrangement, second.arrangement);
        let plans = store.selection_plans(&repo).unwrap();
        let observed = plans
            .iter()
            .find(|plan| plan.dashboard_id.as_ref() == Some(&id))
            .unwrap();
        assert_eq!(observed.selection, plan.selection);
        assert_eq!(observed.resolution, plan.resolution);
        assert_eq!(observed.arrangement, plan.arrangement);
        assert_eq!(observed.contents, plan.contents);
        assert_eq!(store.dashboards(&repo).unwrap(), [second]);
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM task_ownership WHERE kind!='recorded'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn released_work_upgrades_to_saved_intent_without_rewriting_work() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        let conn = rusqlite::Connection::open(&path).unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "saved_selections");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/saved_selection_work.sql"
        ))
        .unwrap();
        crate::store::migrations::upgrade_from_draft(&conn, "saved_selections");
        // The idle Task has a checkout; the deferred one isn't Working.
        conn.execute(
            "UPDATE tasks SET worktree='/repo/fix-2' WHERE issue_identifier='FIX-2'",
            [],
        )
        .unwrap();
        drop(conn);
        let store = SqliteStore {
            conn: std::sync::Arc::new(std::sync::Mutex::new(
                rusqlite::Connection::open(&path).unwrap(),
            )),
        };
        let repo = RepositoryId::parse("repo_11111111111111111111111111111111").unwrap();
        let selection = selection();
        let result = store.resolve_selection(&repo, &selection).unwrap();
        assert_eq!(result.members.len(), 3);
        let id = DashboardId::new();
        let saved = store
            .save_dashboard(
                &repo,
                &id,
                None,
                &selection,
                &Arrangement::automatic(&result.members).to_edit(),
            )
            .unwrap();
        let reader = SqliteStore::open_read_only(&path).unwrap();
        assert_eq!(
            reader.read_dashboard(&repo, &id).unwrap().unwrap().saved,
            saved
        );
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            4
        );
        crate::store::migrations::validate_persisted_json(&conn).unwrap();
    }

    #[test]
    fn saved_selections_upgrade_reload_and_edit_atomically_without_execution() {
        let (_dir, store, repo) = fixture();
        let selection = selection();
        let id = DashboardId::new();
        let result = store.resolve_selection(&repo, &selection).unwrap();
        assert!(result.unavailable.is_empty());
        assert_eq!(result.members.len(), 3); // Two Waves and the idle Task; FIX-1 has no checkout.
        assert_eq!(
            result
                .members
                .iter()
                .filter(|member| member.work.kind == WorkWorkspaceKind::Task)
                .map(|member| member.work.id.as_str())
                .collect::<Vec<_>>(),
            ["task_22222222222222222222222222222222"]
        );
        let hidden = result.members[0].clone();
        let mut arrangement = Arrangement::automatic(&result.members);
        arrangement.hide(&hidden, true).unwrap();
        let first = store
            .save_dashboard(&repo, &id, None, &selection, &arrangement.to_edit())
            .unwrap();
        // A second connection reads real lfdb bytes, without opening a native surface.
        let reader = SqliteStore::open_read_only(&_dir.path().join("db")).unwrap();
        let loaded = reader.read_dashboard(&repo, &id).unwrap().unwrap();
        assert_eq!(loaded.saved, first);
        assert_eq!(loaded.arrangement, arrangement);
        arrangement.order.reverse();
        let saved = store
            .save_dashboard(
                &repo,
                &id,
                Some(first.revision),
                &selection,
                &arrangement.to_edit(),
            )
            .unwrap();
        assert!(store
            .save_dashboard(
                &repo,
                &id,
                Some(first.revision),
                &selection,
                &arrangement.to_edit()
            )
            .is_err());
        let incomplete = Arrangement::automatic(&result.members[1..]);
        assert!(store
            .save_dashboard(
                &repo,
                &id,
                Some(saved.revision),
                &selection,
                &incomplete.to_edit()
            )
            .is_err());
        assert_eq!(reader.dashboards(&repo).unwrap(), [saved]);
        let mut restored = reader
            .read_dashboard(&repo, &id)
            .unwrap()
            .unwrap()
            .arrangement;
        restored.hide(&hidden, false).unwrap();
        assert_eq!(restored.order, arrangement.order);
        let conn = store.conn.lock().unwrap();
        for table in ["agent_sessions", "processes", "graph_processes"] {
            assert_eq!(
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                    .unwrap(),
                0
            );
        }
        crate::store::migrations::validate_persisted_json(&conn).unwrap();
    }

    #[test]
    fn nested_query_failure_retains_prior_members_but_confirmed_departure_prunes() {
        let (_dir, store, repo) = fixture();
        let base = selection();
        let base_id = DashboardId::new();
        let resolution = store.resolve_selection(&repo, &base).unwrap();
        store
            .save_dashboard(
                &repo,
                &base_id,
                None,
                &base,
                &Arrangement::automatic(&resolution.members).to_edit(),
            )
            .unwrap();
        let nested = Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Query {
                query: base_id.clone(),
            },
        };
        let nested_id = DashboardId::new();
        let result = store.resolve_selection(&repo, &nested).unwrap();
        let arrangement = Arrangement::automatic(&result.members);
        store
            .save_dashboard(&repo, &nested_id, None, &nested, &arrangement.to_edit())
            .unwrap();
        // The named query edit would introduce a cycle through the dependent.
        let cycle = Selection {
            id: SelectionId::new(),
            expression: SelectionExpression::Query {
                query: nested_id.clone(),
            },
        };
        assert!(store
            .save_dashboard(&repo, &base_id, Some(1), &cycle, &arrangement.to_edit())
            .is_err());
        assert!(store.delete_dashboard(&repo, &base_id, 2).is_err());
        assert_eq!(
            store
                .read_dashboard(&repo, &nested_id)
                .unwrap()
                .unwrap()
                .arrangement,
            arrangement
        );
        assert!(store.read_dashboard(&repo, &base_id).unwrap().is_some());
        store.delete_dashboard(&repo, &base_id, 1).unwrap();
        assert!(store.read_dashboard(&repo, &base_id).unwrap().is_none());
        assert!(store.delete_dashboard(&repo, &base_id, 1).is_err());
        let unavailable = store.read_dashboard(&repo, &nested_id).unwrap().unwrap();
        assert!(!unavailable.resolution.unavailable.is_empty());
        assert_eq!(unavailable.arrangement, arrangement);
        store
            .save_dashboard(
                &repo,
                &base_id,
                None,
                &base,
                &Arrangement::automatic(&resolution.members).to_edit(),
            )
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=1 WHERE issue_identifier='FIX-2'",
                [],
            )
            .unwrap();
        let current = store.read_dashboard(&repo, &nested_id).unwrap().unwrap();
        assert!(current.resolution.unavailable.is_empty());
        assert_eq!(current.arrangement.order.len(), 2);
        assert_eq!(current.saved.arrangement.order.len(), 3); // Reading does not write.
    }

    #[test]
    fn selection_reads_compose_with_the_work_snapshot_and_report_failed_reads() {
        let (_dir, store, repo) = fixture();
        let _snapshot = store.read_snapshot().unwrap();
        let result = store.resolve_selection(&repo, &selection()).unwrap();
        assert_eq!(result.members.len(), 3);
        let missing = RepositoryId::new();
        assert!(store.resolve_selection(&missing, &selection()).is_err());
    }
}
