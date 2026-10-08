//! Foreground delivery consumes the same receipts as local planning writers.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::durable::WorkRef;
use crate::planning::PlanningChange;
use crate::pm::linear::LinearClient;
use crate::store::sqlite::planning_changes::PlanningChanges;
use crate::store::{PmTaskRecord, Store};
use crate::work::task::Task;

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(crate) async fn sync_repository_fields(store: &Store, task: &Task) -> OpsResult<()> {
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task Wave is missing"))?;
    if !super::linear_observe::connected(wave.repo()) {
        return Ok(());
    }
    let owners = store
        .sqlite
        .planning_field_owners(wave.repo())
        .map_err(message)?;
    let results = futures_util::future::join_all(
        owners
            .iter()
            .map(|owner| sync_fields(store, Path::new(wave.repo()), owner)),
    )
    .await;
    for result in results {
        if let Err(error) = result {
            tracing::debug!(%error, "planning field delivery pending");
        }
    }
    Ok(())
}

pub(crate) async fn sync_fields(store: &Store, repo: &Path, work: &WorkRef) -> OpsResult<()> {
    if !super::linear_observe::connected(&repo.to_string_lossy()) {
        return Ok(());
    }
    let owner = match work {
        WorkRef::Task(id) => PlanningChanges::Task(id),
        WorkRef::Project(id) => PlanningChanges::Project(id),
        _ => return Err(message("field delivery requires a Task or Project")),
    };
    let (kind, id) = owner.owner();
    // Other connections serialize effects only. Saves and inbound observations
    // remain independent, including while a request is waiting for a reply.
    let directory = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/planning-fields");
    std::fs::create_dir_all(&directory)?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{kind}-{id}.lock")))?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let changes = pending(store, owner)?;
    let attempt = async {
        let client = super::pm::linear_client(repo).await?;
        // Also reconcile an older attempted receipt whose successor is no longer pending.
        observe(store, repo, owner, &client).await?;
        for change in changes.iter().filter(|change| change.field != "deleted") {
            let attempt = async {
                let (external, content, revision) = observe(store, repo, owner, &client).await?;
                let Some(current) = pending(store, owner)?
                    .into_iter()
                    .find(|c| c.id == change.id)
                else {
                    return Ok(());
                };
                let input = match owner {
                    PlanningChanges::Task(_) => task_input(store, &current)?,
                    PlanningChanges::Project(_) => client
                        .project_field_input(&current.field, &current.value, &content)
                        .await
                        .map_err(message)?,
                };
                if !store
                    .sqlite
                    .attempt_planning_field(owner, &current, revision.as_deref())
                    .map_err(message)?
                {
                    return Ok(());
                }
                client
                    .deliver_planning_field(
                        &external,
                        matches!(owner, PlanningChanges::Project(_)),
                        input,
                    )
                    .await
                    .map_err(message)?;
                observe(store, repo, owner, &client).await?;
                if pending(store, owner)?.iter().any(|c| c.id == change.id) {
                    return Err(message("Field write remains uncertain; receipt retained"));
                }
                Ok(())
            };
            if let Err(error) = attempt.await {
                store
                    .sqlite
                    .planning_field_error(owner, change, &error.to_string())
                    .map_err(message)?;
            }
        }
        Ok(())
    };
    let result = tokio::time::timeout(Duration::from_secs(5), attempt)
        .await
        .map_err(|_| message("Field delivery timed out; attempted receipts remain uncertain"))
        .and_then(|result| result);
    if let Err(error) = &result {
        for change in &changes {
            store
                .sqlite
                .planning_field_error(owner, change, &error.to_string())
                .map_err(message)?;
        }
    }
    result
}

fn pending(store: &Store, owner: PlanningChanges<'_>) -> OpsResult<Vec<PlanningChange>> {
    match owner {
        PlanningChanges::Task(id) => store.sqlite.pending_task_changes(id),
        PlanningChanges::Project(id) => store.sqlite.pending_project_changes(id),
    }
    .map_err(message)
}

async fn observe(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &LinearClient,
) -> OpsResult<(String, String, Option<String>)> {
    match owner {
        PlanningChanges::Task(id) => {
            let task = store
                .get_task(id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Task is missing"))?;
            let external = task.plan.linear_id()?.as_str().to_owned();
            let (item, project) = client
                .issue_ownership(&external)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Linked Linear issue is unavailable"))?;
            let revision = item.revision.clone();
            store
                .sqlite
                .put_pm_task(
                    &repo.to_string_lossy(),
                    "linear",
                    &PmTaskRecord {
                        item,
                        project,
                        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    },
                    None,
                )
                .map_err(message)?;
            let accepted = store
                .sqlite
                .planning_task(id)
                .map_err(message)?
                .record
                .ok_or_else(|| message("Task planning is unavailable"))?;
            if accepted.item.revision != revision {
                return Err(message(
                    "Newer or unresolved Linear planning retained; field delivery deferred",
                ));
            }
            Ok((external, String::new(), revision))
        }
        PlanningChanges::Project(id) => {
            let project = store
                .get_project(id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project is missing"))?;
            let external = project.plan.linear_id()?.as_str().to_owned();
            let (observed, content) = client
                .project_content_observation(&external)
                .await
                .map_err(message)?;
            let wave = store
                .get_wave(&project.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project Wave is missing"))?;
            let initiative = super::pm::read_initiative(repo, wave.slug())
                .ok_or_else(|| message("Wave has no Linear Initiative"))?;
            let accepted = store
                .sqlite
                .put_pm_project(
                    &project.wave_id,
                    "linear",
                    &initiative,
                    &observed,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
                .map_err(message)?;
            if accepted.revision != observed.revision {
                return Err(message(
                    "Newer Linear Project observation retained; field delivery deferred",
                ));
            }
            Ok((external, content, observed.revision))
        }
    }
}

fn task_input(store: &Store, change: &PlanningChange) -> OpsResult<Value> {
    match change.field.as_str() {
        "name" => Ok(json!({"title": change.value})),
        "description" => Ok(json!({"description": change.value})),
        "assignee" => Ok(json!({"assigneeId": change.value})),
        "project_id" => {
            let id = crate::durable::ProjectId::parse(
                change
                    .value
                    .as_str()
                    .ok_or_else(|| message("missing destination Project"))?,
            )
            .map_err(message)?;
            let project = store
                .sqlite
                .project(&id)
                .map_err(message)?
                .ok_or_else(|| message("destination Project is missing"))?;
            Ok(json!({"projectId":project.plan.linear_id()?.as_str()}))
        }
        _ => Err(message(format!(
            "{} remains pending: relative order requires a Project-wide delivery",
            change.field
        ))),
    }
}
