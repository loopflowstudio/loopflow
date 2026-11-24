//! Application selection acquisition and Dashboard editing.
use crate::store::session_projection::Reading;
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::RepositoryId;
use loopflow_storage::sqlite::selections::{
    repository_default, GraphTile, GraphTileInput, TaskSelectionReading,
};
use loopflow_storage::sqlite::sessions::{SessionProjection, SessionProjectionInputs};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::selection::{
    Arrangement, ArrangementEdit, ArrangementMode, Dashboard, DashboardId, DashboardReading,
    Selection, SelectionResolution, ViewPlan,
};

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

// A process can change between observation and pinning the SQL snapshot.
fn read_projected<T>(
    store: &SqliteStore,
    mut read: impl FnMut(&SessionProjectionInputs, &SessionProjection) -> StoreResult<Option<T>>,
) -> StoreResult<T> {
    loop {
        let reading = Reading::acquire(store, loopflow_storage::rows::now_unix(), None)?;
        if let Some(value) = read(&reading.inputs, &reading.values)? {
            return Ok(value);
        }
    }
}

pub fn plan_selection(
    store: &SqliteStore,
    repository: &RepositoryId,
    selection: Selection,
    merges: fn(&[String]) -> bool,
) -> StoreResult<ViewPlan> {
    let mut tile = graph_tile_reader(merges);
    let plan = read_projected(store, |inputs, projection| {
        store.projected_plan_selection(repository, selection.clone(), inputs, projection, &mut tile)
    })?;
    prepare_plan(store, plan)
}

pub fn all_view_plans(
    store: &SqliteStore,
    source: Option<&str>,
    repos: &[String],
    merges: fn(&[String]) -> bool,
) -> StoreResult<Vec<ViewPlan>> {
    let mut tile = graph_tile_reader(merges);
    let plans = read_projected(store, |inputs, projection| {
        store.projected_all_view_plans(source, repos, inputs, projection, &mut tile)
    })?;
    plans
        .into_iter()
        .map(|plan| prepare_plan(store, plan))
        .collect()
}

pub fn selection_plans(
    store: &SqliteStore,
    repository: &RepositoryId,
    merges: fn(&[String]) -> bool,
) -> StoreResult<Vec<ViewPlan>> {
    let mut tile = graph_tile_reader(merges);
    let plans = read_projected(store, |inputs, projection| {
        store.projected_selection_plans(repository, inputs, projection, &mut tile)
    })?;
    plans
        .into_iter()
        .map(|plan| prepare_plan(store, plan))
        .collect()
}

pub fn resolve_selection(
    store: &SqliteStore,
    repository: &RepositoryId,
    selection: &Selection,
) -> StoreResult<SelectionResolution> {
    read_projected(store, |inputs, projection| {
        store.projected_resolve_selection(repository, selection, inputs, projection)
    })
}

pub fn task_selection_reading(
    store: &SqliteStore,
    repository: &RepositoryId,
) -> StoreResult<TaskSelectionReading> {
    read_projected(store, |inputs, projection| {
        store.projected_task_selection_reading(repository, inputs, projection)
    })
}

pub fn read_dashboard(
    store: &SqliteStore,
    repository: &RepositoryId,
    id: &DashboardId,
) -> StoreResult<Option<DashboardReading>> {
    read_projected(store, |inputs, projection| {
        store.projected_read_dashboard(repository, id, inputs, projection)
    })
}

/// Resolve text/defaults and saved intent without creating Work or acquiring a client.
pub fn view_plan(
    store: &SqliteStore,
    repository: &RepositoryId,
    source: Option<&str>,
    merges: fn(&[String]) -> bool,
) -> StoreResult<ViewPlan> {
    let selection = match source {
        Some(source) => Selection::parse(source, None).map_err(StoreError::from)?,
        None => repository_default(),
    };
    plan_selection(store, repository, selection, merges)
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

fn graph_tile_reader(
    merges: fn(&[String]) -> bool,
) -> impl FnMut(GraphTileInput<'_>) -> StoreResult<GraphTile> {
    let mut cached = crate::task_graph::cached_merges(merges);
    move |input| {
        let graph: crate::task_graph::CapturedGraph = serde_json::from_str(input.graph)?;
        let cursor: Option<crate::graph::runner::ExecutionCursor> =
            input.cursor.map(serde_json::from_str).transpose()?;
        let running_step = cursor
            .as_ref()
            .and_then(|cursor| graph.step_name_of(cursor));
        let ships = input.need_ship
            && graph.ships_before_review(input.step, cursor.as_ref(), input.auto, &mut cached);
        Ok(GraphTile {
            running_step,
            ships,
        })
    }
}

fn prepare_plan(store: &SqliteStore, mut plan: ViewPlan) -> StoreResult<ViewPlan> {
    plan.viewer = crate::store::ownership::viewer(
        store,
        &RepositoryId::from_raw(plan.repository.clone()),
        std::path::Path::new(&plan.repo_path),
    )?;
    for content in &mut plan.contents {
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
    Ok(plan)
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
    let expression = selection.expression_key();
    let inputs = read_projected(store, |inputs, projection| {
        store.dashboard_edit_inputs(repository, id, &expression, inputs, projection)
    })?;
    let catalog = inputs.catalog;
    let prior = catalog.iter().find(|saved| &saved.id == id);
    if prior.map(|saved| saved.revision) != expected_revision {
        return Err(StoreError::Stale(
            "Dashboard changed; read it again before editing".into(),
        ));
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
        .or_else(|| prior.map(|saved| saved.arrangement.clone()))
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
        name: prior.and_then(|saved| saved.name.clone()),
        pinned: prior.is_some_and(|saved| saved.pinned),
        slots: prior.and_then(|saved| saved.slots.clone()),
        selection: selection.clone(),
        arrangement,
    };

    store.commit_dashboard(repository, expected_revision, saved)
}
#[cfg(test)]
mod tests;
