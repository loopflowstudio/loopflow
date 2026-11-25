//! Acquire configuration outside SQL and retry commands whose saved scope changed.
use crate::durable::{OwningMachineSource, TaskExecutionRoute, TaskExecutionSource};
use crate::store::{StoreError, StoreResult};
use loopflow_model::durable::{
    MachineId, MachineRef, MachineSetting, RepositoryId, TaskId, TaskOwner, TaskOwnershipEntry,
};
use loopflow_storage::sqlite::scope_defaults::TaskScope;
use loopflow_storage::sqlite::task_ownership::{label_owner, OwnershipFacts, Person, You};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::task::{NewTask, Task, TaskPr};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(crate) fn machine_names(store: &SqliteStore) -> StoreResult<HashMap<MachineId, String>> {
    Ok(
        crate::config::placement::global_settings_at(&store.home_dir()?)
            .map_err(invalid)?
            .machines
            .into_iter()
            .map(|(name, declaration)| (declaration.id, name))
            .collect(),
    )
}

pub fn acquire(store: &SqliteStore, scope: TaskScope) -> StoreResult<OwnershipFacts> {
    prepare(store, scope).map(|(facts, _)| facts)
}

/// The person a claim in `repo` names: your display name and Git `user.email`.
fn claimant(repo: &Path, id: Option<String>) -> StoreResult<Person> {
    Ok(Person {
        name: crate::config::participant_name().map_err(invalid)?,
        key: crate::config::participant_email(repo),
        id,
    })
}

/// Who a Claim in `repo` names as owner: you, on this Machine. `None` without
/// a Git `user.email`, which claiming needs.
pub fn viewer(
    store: &SqliteStore,
    repository: &RepositoryId,
    repo: &Path,
) -> StoreResult<Option<TaskOwner>> {
    let scope = store.placement_scope(None, Some(repository))?;
    let person = claimant(repo, scope.person_id)?;
    Ok(person.key.map(|key| TaskOwner {
        person: person.name,
        person_key: Some(key),
        person_id: person.id,
        machine: MachineRef {
            id: Some(scope.local_machine),
            label: Some(local_label()),
        },
    }))
}

/// You in `repo`, whose Git `user.email` is your person key. Reading it
/// spawns `git` and loads Machine configuration: read once per repository.
pub fn you(store: &SqliteStore, repository: &RepositoryId, repo: &Path) -> StoreResult<You> {
    let scope = store.placement_scope(None, Some(repository))?;
    let mut machines: HashSet<MachineId> = machine_names(store)?.into_keys().collect();
    machines.insert(scope.local_machine);
    Ok(You {
        person: claimant(repo, scope.person_id)?,
        machines,
    })
}

fn prepare(
    store: &SqliteStore,
    scope: TaskScope,
) -> StoreResult<(OwnershipFacts, OwningMachineSource)> {
    let repo = &scope
        .placement
        .chain
        .last()
        .expect("Task scope includes its repository")
        .repo;
    let home = store.home_dir()?;
    let (machine, source) =
        crate::store::placement::owning_machine(&home, &scope.placement, MachineSetting::Tasks)?;
    Ok((
        OwnershipFacts {
            person: claimant(Path::new(repo), scope.placement.person_id.clone())?,
            planning_connected: crate::ops::tracker::connected(repo),
            declared_machines: machine_names(store)?,
            local_label: local_label(),
            owning_machine: machine,
            scope,
        },
        source,
    ))
}

pub fn claim_task(store: &SqliteStore, task: &TaskId) -> StoreResult<Option<TaskOwnershipEntry>> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.claim_task_prepared(task, &facts)? {
            return Ok(result);
        }
    }
}

pub fn move_claim(
    store: &SqliteStore,
    task: &TaskId,
    target: &MachineId,
) -> StoreResult<TaskOwnershipEntry> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.move_claim_prepared(task, target, &facts)? {
            return Ok(result);
        }
    }
}

pub fn begin_task_move(
    store: &SqliteStore,
    task: &TaskId,
    target: &MachineId,
) -> StoreResult<TaskOwnershipEntry> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.begin_task_move_prepared(task, target, &facts)? {
            return Ok(result);
        }
    }
}

pub(crate) fn hand_back(
    store: &SqliteStore,
    task: &TaskId,
    sent: &TaskOwnershipEntry,
) -> StoreResult<TaskOwnershipEntry> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.hand_back_prepared(task, sent, &facts)? {
            return Ok(result);
        }
    }
}

pub(crate) fn ingest_ownership_marker(
    store: &SqliteStore,
    task: &TaskId,
    marker: &str,
) -> StoreResult<bool> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.ingest_ownership_marker_prepared(task, marker, &facts)? {
            return Ok(result);
        }
    }
}

pub fn unassign_task(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    loop {
        let scope = store.task_scope(task)?;
        let facts = acquire(store, scope)?;
        if let Some(result) = store.unassign_task_prepared(task, &facts)? {
            return Ok(result);
        }
    }
}

pub fn place_task(
    store: &SqliteStore,
    task_id: &TaskId,
    worktree: &std::path::Path,
    workspace_slug: &str,
    pr: &TaskPr,
) -> StoreResult<Task> {
    let _admission = super::admission::lock_checkout(store, worktree)?;
    loop {
        let scope = store.task_scope(task_id)?;
        let facts = acquire(store, scope)?;
        if let Some(result) =
            store.place_task_prepared(task_id, worktree, workspace_slug, pr, &facts)?
        {
            return Ok(result);
        }
    }
}

#[allow(clippy::too_many_arguments)] // These effects share the promotion transaction.
pub(crate) fn promote_task(
    store: &SqliteStore,
    task_id: &TaskId,
    creation: Option<&NewTask>,
    creation_wave: Option<&loopflow_model::id::WaveId>,
    placement: Option<&(PathBuf, TaskPr)>,
    session: Option<&crate::session::LfSession>,
    destination: Option<&Path>,
    receipt: Option<&loopflow_storage::sqlite::creation::TaskCreationRecord>,
) -> StoreResult<Task> {
    let caller = if session.is_some() {
        super::sessions::session_caller()?
    } else {
        super::sqlite::SessionCaller {
            agent: None,
            capture_key: None,
        }
    };
    loop {
        let scope = match creation {
            Some(input) => store.promotion_scope(input, creation_wave)?,
            None => store.task_scope(task_id)?,
        };
        let facts = acquire(store, scope)?;
        if let Some(result) = store.promote_task_prepared(
            task_id,
            creation,
            creation_wave,
            placement,
            session,
            destination,
            receipt,
            &facts,
            &caller,
        )? {
            return Ok(result);
        }
    }
}

pub(crate) type TaskRoute = fn(&SqliteStore, &TaskId) -> StoreResult<TaskExecutionRoute>;

pub fn task_execution_route(store: &SqliteStore, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
    loop {
        let scope = store.task_scope(task)?;
        let (facts, source) = prepare(store, scope)?;
        if let Some(saved) = store.task_execution_route_prepared(task, &facts)? {
            return Ok(TaskExecutionRoute {
                task_id: task.clone(),
                peer_selector: saved.peer_selector,
                checkout: saved.checkout,
                repository_id: saved.repository_id,
                machine_id: saved.machine_id,
                source: if saved.claimed {
                    TaskExecutionSource::Owner
                } else {
                    TaskExecutionSource::OwningMachine(source)
                },
            });
        }
    }
}

/// A move can still carry a pause from its source until the payload is sent.
pub fn task_pause_route(store: &SqliteStore, task: &TaskId) -> StoreResult<TaskExecutionRoute> {
    let mut route = task_execution_route(store, task)?;
    let here = super::machines::local_machine(store)?.id;
    if route.machine_id != here && store.task_is_leaving(task)? {
        route.machine_id = here;
    }
    Ok(route)
}

pub(crate) fn held_planning(
    store: &SqliteStore,
    machine: &MachineId,
) -> StoreResult<(
    loopflow_storage::sqlite::task_ownership::HeldPlanningScope,
    HashMap<TaskId, bool>,
)> {
    let scope = store.held_planning_scope(machine)?;
    let mut connected = HashMap::new();
    let planning = scope
        .repositories
        .iter()
        .map(|(task, (_, repo))| {
            let value = *connected
                .entry(repo)
                .or_insert_with(|| crate::ops::tracker::connected(repo));
            (task.clone(), value)
        })
        .collect();
    Ok((scope, planning))
}

pub fn set_rental_state(
    store: &SqliteStore,
    id: &loopflow_storage::rental::RentalId,
    state: loopflow_storage::rental::RentalState,
) -> StoreResult<(loopflow_storage::rental::Rental, Vec<TaskId>)> {
    loop {
        let rental = store.rental(id)?;
        let prepared = match (&rental.machine_id, state.is_live()) {
            (Some(machine), false) => Some(held_planning(store, machine)?),
            _ => None,
        };
        if let Some(result) = store.set_rental_state_prepared(id, state, prepared.as_ref())? {
            return Ok(result);
        }
    }
}

/// The local Machine has no registered label; its short hostname names it
/// here. It is never published: only a public name leaves this Machine.
pub fn local_label() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    host.split('.').next().unwrap_or(&host).to_string()
}

pub fn task_ownership(
    store: &SqliteStore,
    task: &TaskId,
) -> StoreResult<loopflow_model::durable::TaskOwnership> {
    let (local, mut value) = store.saved_task_ownership(task)?;
    let mut names = machine_names(store)?;
    names.insert(local, local_label());
    label_ownership(&mut value, &names);
    Ok(value)
}

pub fn task_detail_facts(
    store: &SqliteStore,
    tasks: &[&Task],
    merges: fn(&[String]) -> bool,
) -> StoreResult<loopflow_storage::sqlite::TaskDetailFacts> {
    use loopflow_storage::sqlite::{GraphDetail, GraphDetailInput};
    let mut cached = crate::task_graph::cached_merges(merges);
    let mut graph_detail = |input: GraphDetailInput<'_>| -> StoreResult<GraphDetail> {
        let graph: crate::task_graph::CapturedGraph = serde_json::from_str(input.graph)?;
        let cursor = input.cursor.map(serde_json::from_str).transpose()?;
        if input.paused {
            Ok(GraphDetail::ResumesOnUnpause(graph.resumes_on_unpause(
                input.step,
                cursor.as_ref(),
                input.auto,
                &mut cached,
            )))
        } else {
            Ok(GraphDetail::Ships(graph.ships_before_review(
                input.step,
                cursor.as_ref(),
                input.auto,
                &mut cached,
            )))
        }
    };
    let (local, mut facts) = store.saved_task_detail_facts(tasks, &mut graph_detail)?;
    let mut names = machine_names(store)?;
    names.insert(local, local_label());
    for ownership in facts.ownership.values_mut() {
        label_ownership(ownership, &names);
    }
    Ok(facts)
}

/// Each Machine the ownership names, as `names` names it here.
fn label_ownership(
    ownership: &mut loopflow_model::durable::TaskOwnership,
    names: &HashMap<MachineId, String>,
) {
    for owner in [&mut ownership.owner, &mut ownership.superseded_by]
        .into_iter()
        .flatten()
    {
        label_owner(owner, names);
    }
    for entry in &mut ownership.log {
        if let Some(label) = entry.machine_id.as_ref().and_then(|id| names.get(id)) {
            entry.machine_label = Some(label.clone());
        }
    }
}

pub(crate) fn apply_tracker_event(
    store: &SqliteStore,
    task_id: &TaskId,
    event: &loopflow_storage::planning::TrackerEvent,
    observed_at: time::OffsetDateTime,
    planning_connected: bool,
) -> StoreResult<loopflow_storage::work::task::TrackerEventOutcome> {
    let mut names = machine_names(store)?;
    names.insert(
        crate::store::machines::local_machine(store)?.id,
        local_label(),
    );
    store.apply_tracker_event_prepared(task_id, event, observed_at, planning_connected, &names)
}

#[cfg(test)]
mod tests;
