//! Application selection acquisition and Dashboard editing.
use crate::store::session_projection::Reading;
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::RepositoryId;
use loopflow_storage::sqlite::selections::{
    repository_default, TaskSelectionReading, WorkingOrder,
};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::selection::{
    move_into_place, Arrangement, ArrangementEdit, ArrangementMode, Dashboard, DashboardId,
    DashboardReading, DropSide, Selection, SelectionResolution, ViewPlan,
};
fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub fn plan_selection(
    store: &SqliteStore,
    repository: &RepositoryId,
    selection: Selection,
) -> StoreResult<ViewPlan> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) = store.projected_plan_selection(
            repository,
            selection.clone(),
            &reading.inputs,
            &reading.values,
            running_step,
        )? {
            let mut plan = result;
            prepare_plan(store, &mut plan)?;
            return Ok(plan);
        }
    }
}

pub fn all_view_plans(
    store: &SqliteStore,
    source: Option<&str>,
    repos: &[String],
) -> StoreResult<Vec<ViewPlan>> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) = store.projected_all_view_plans(
            source,
            repos,
            &reading.inputs,
            &reading.values,
            running_step,
        )? {
            let mut plans = result;
            for plan in &mut plans {
                prepare_plan(store, plan)?;
            }
            return Ok(plans);
        }
    }
}

pub fn selection_plans(
    store: &SqliteStore,
    repository: &RepositoryId,
) -> StoreResult<Vec<ViewPlan>> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) = store.projected_selection_plans(
            repository,
            &reading.inputs,
            &reading.values,
            running_step,
        )? {
            let mut plans = result;
            for plan in &mut plans {
                prepare_plan(store, plan)?;
            }
            return Ok(plans);
        }
    }
}

pub fn resolve_selection(
    store: &SqliteStore,
    repository: &RepositoryId,
    selection: &Selection,
) -> StoreResult<SelectionResolution> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) = store.projected_resolve_selection(
            repository,
            selection,
            &reading.inputs,
            &reading.values,
        )? {
            return Ok(result);
        }
    }
}

pub fn task_selection_reading(
    store: &SqliteStore,
    repository: &RepositoryId,
) -> StoreResult<TaskSelectionReading> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) =
            store.projected_task_selection_reading(repository, &reading.inputs, &reading.values)?
        {
            return Ok(result);
        }
    }
}

pub fn read_dashboard(
    store: &SqliteStore,
    repository: &RepositoryId,
    id: &DashboardId,
) -> StoreResult<Option<DashboardReading>> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(result) =
            store.projected_read_dashboard(repository, id, &reading.inputs, &reading.values)?
        {
            return Ok(result);
        }
    }
}

/// Resolve text/defaults and saved intent without creating Work or acquiring a client.
pub fn view_plan(
    store: &SqliteStore,
    repository: &RepositoryId,
    source: Option<&str>,
) -> StoreResult<loopflow_storage::work::selection::ViewPlan> {
    let selection = match source {
        Some(source) => Selection::parse(source, None).map_err(StoreError::from)?,
        None => repository_default(),
    };
    plan_selection(store, repository, selection)
}

pub fn edit_dashboard(
    store: &SqliteStore,
    repository: &RepositoryId,
    id: &DashboardId,
    source: &str,
    expected_revision: Option<i64>,
) -> StoreResult<Dashboard> {
    // Editing must remain possible when the old query has become ambiguous.
    let previous = store
        .dashboards(repository)?
        .into_iter()
        .find(|saved| &saved.id == id);
    if previous.as_ref().map(|saved| saved.revision) != expected_revision {
        return Err(StoreError::Stale(
            "Dashboard changed; read it again before editing".into(),
        ));
    }
    let mut selection = Selection::parse(source, previous.as_ref().map(|saved| &saved.selection))
        .map_err(StoreError::from)?;
    if previous.is_none() {
        // Identical text in separately authored queries still names separate sources.
        selection.id = loopflow_storage::work::selection::SelectionId(format!("query:{}", id.0));
    }
    let recovered = store.dashboard_arrangement(repository, id, &selection.expression_key())?;
    let mut arrangement = if let Some((authored, arrangement)) = recovered {
        selection = authored;
        arrangement
    } else {
        previous.map_or_else(|| Arrangement::automatic(&[]), |saved| saved.arrangement)
    };
    let resolution = resolve_selection(store, repository, &selection)?;
    arrangement.reconcile(&resolution);
    save_dashboard(
        store,
        repository,
        id,
        expected_revision,
        &selection,
        &arrangement.to_edit(),
    )
}

fn running_step(graph: &str, cursor: &str) -> StoreResult<Option<String>> {
    let graph: crate::task_graph::CapturedGraph = serde_json::from_str(graph)?;
    Ok(graph.step_name_of(&serde_json::from_str(cursor)?))
}

fn prepare_plan(store: &SqliteStore, plan: &mut ViewPlan) -> StoreResult<()> {
    plan.viewer = crate::store::ownership::viewer(
        store,
        &RepositoryId::from_raw(plan.repository.clone()),
        std::path::Path::new(&plan.repo_path),
    )?;
    for content in plan.contents.iter_mut().filter(|content| {
        content.work.kind == loopflow_storage::work::selection::WorkWorkspaceKind::Task
            && content.identifier.is_some()
    }) {
        if let (Some(session_id), Some(location)) = (&content.primary_session, &content.location) {
            match crate::ops::human_session::human_session_argv(session_id) {
                Ok(mut argv) => {
                    // Keep the existing resume command and data-selection fences.
                    let at = argv.len() - 3;
                    argv.splice(
                        at..at,
                        [
                            "--machine".into(),
                            location.machine_id.clone(),
                            "--repo-id".into(),
                            plan.repository.clone(),
                        ],
                    );
                    content.argv = Some(argv);
                }
                Err(error) => content.unavailable.push(error.to_string()),
            }
        }
    }
    Ok(())
}

/// Prepare without a write lock; SQL checks the saved revision at commit.
/// Changed order touches the arrangement; hiding alone retains its mode.
pub fn save_dashboard(
    store: &SqliteStore,
    repository: &RepositoryId,
    id: &DashboardId,
    expected_revision: Option<i64>,
    selection: &Selection,
    edit: &ArrangementEdit,
) -> StoreResult<Dashboard> {
    if id.0.is_empty() {
        return Err(invalid("Dashboard ID must be nonempty"));
    }
    let stale = || StoreError::Stale("Dashboard changed; read it again before editing".into());
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        let Some(inputs) = store.dashboard_edit_inputs(
            repository,
            id,
            &selection.expression_key(),
            &reading.inputs,
            &reading.values,
        )?
        else {
            continue;
        };
        let catalog = inputs.catalog;
        let prior = catalog.iter().find(|saved| &saved.id == id).cloned();
        if prior.as_ref().map(|saved| saved.revision) != expected_revision {
            return Err(stale());
        }
        let work = inputs.work;
        let mut definitions: std::collections::BTreeMap<_, _> = catalog
            .iter()
            .map(|saved| (saved.id.clone(), saved.selection.clone()))
            .collect();
        definitions.insert(id.clone(), selection.clone());
        // Any cycle introduced by this edit must pass through this selection.
        // Unrelated Dashboards may have ambiguous references and still be repairable.
        let resolution = selection
            .resolve(repository.as_str(), &work, &definitions)
            .map_err(StoreError::from)?;
        let mut expected = inputs
            .arrangement
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

        return store.commit_dashboard(repository, expected_revision, saved);
    }
}

/// Moves `task` beside `to` in the shared Working order. The inner `Err` names
/// whichever of the two is not a Working Task here; nothing is written then.
pub fn reorder_task<'a>(
    store: &SqliteStore,
    repository: &RepositoryId,
    task: &'a str,
    to: &'a str,
    side: Option<DropSide>,
) -> StoreResult<Result<Vec<String>, &'a str>> {
    edit_working_order(store, repository, WorkingOrder::Saved, |order| {
        let position = |id: &'a str| order.iter().position(|working| working == id).ok_or(id);
        let from = position(task)?;
        let target = position(to)?;
        move_into_place(order, from, target, side);
        Ok(())
    })
}

pub fn sort_tasks_by_status(
    store: &SqliteStore,
    repository: &RepositoryId,
) -> StoreResult<Vec<String>> {
    edit_working_order(store, repository, WorkingOrder::ByStatus, |_| Ok(()))?
}

fn edit_working_order<E>(
    store: &SqliteStore,
    repository: &RepositoryId,
    order_by: WorkingOrder,
    edit: impl Fn(&mut Vec<String>) -> Result<(), E>,
) -> StoreResult<Result<Vec<String>, E>> {
    for _ in 0..3 {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        let Some((read, work)) =
            store.task_order_inputs(repository, &reading.inputs, &reading.values, order_by)?
        else {
            continue;
        };
        let mut order: Vec<_> = work
            .into_iter()
            .filter(|row| {
                row.states
                    .contains(&loopflow_storage::work::selection::SelectionState::Working)
            })
            .map(|row| row.key.id)
            .collect();
        if let Err(absent) = edit(&mut order) {
            return Ok(Err(absent));
        }
        if store.commit_task_order(repository, &read, &order)? {
            return Ok(Ok(order));
        }
    }
    Err(StoreError::Stale(
        "the Task order kept changing; read it again before reordering".into(),
    ))
}

#[cfg(test)]
mod tests;
