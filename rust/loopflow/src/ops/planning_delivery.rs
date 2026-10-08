//! Foreground delivery consumes the same receipts as local planning writers.

use std::fs::File;
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
    let path = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/planning-fields")
        .join(format!("{kind}-{id}.lock"));
    let Some(_lock) = lock_delivery(&path)? else {
        return Ok(());
    };
    let changes = pending(store, owner)?;
    let attempt = async {
        let client = super::pm::linear_client(repo).await?;
        if let Some(change) = changes.iter().find(|change| change.field == "deleted") {
            return sync_deletion(store, repo, owner, &client, change).await;
        }
        // Also reconcile an older attempted receipt whose successor is no longer pending.
        observe(store, repo, owner, &client).await?;
        for change in &changes {
            if let Err(error) = sync_field(store, repo, owner, &client, change).await {
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

async fn sync_deletion(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &LinearClient,
    change: &PlanningChange,
) -> OpsResult<()> {
    let PlanningChanges::Task(id) = owner else {
        return Err(message("Only Task removal has a deletion receipt"));
    };
    let task = store
        .get_task(id)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task is missing"))?;
    let external = task.plan.linear_id()?.as_str();
    let (trashed, trash_revision) = client
        .issue_trash(external)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Linked Linear issue is unavailable; deletion remains uncertain"))?;
    if trashed {
        if !store
            .sqlite
            .acknowledge_task_deletion(id, change, Some(&trash_revision))
            .map_err(message)?
        {
            return Err(message(
                "Newer Linear evidence retained; deletion remains uncertain",
            ));
        }
        return Ok(());
    }
    // Complete active-issue ingestion adopts conflicting Linear edits through
    // the common writer before attempting the saved removal.
    let (_, _, revision) = observe(store, repo, owner, client).await?;
    if revision.as_deref() != Some(trash_revision.as_str()) {
        return Err(message(
            "Linear deletion observations disagree; saved removal retained",
        ));
    }
    store
        .sqlite
        .observe_task_deletion(id, &trash_revision)
        .map_err(message)?;
    if !pending(store, owner)?.iter().any(|c| c.id == change.id) {
        return Ok(());
    }
    let baseline = change
        .base
        .as_ref()
        .and_then(|base| base["revision"].as_str());
    if baseline.is_none() {
        return Err(message(
            "Deletion has no provider baseline; saved removal retained",
        ));
    }
    if !store
        .sqlite
        .attempt_planning_field(owner, change, Some(&trash_revision))
        .map_err(message)?
    {
        return Err(message(
            "Deletion remains uncertain; no repeated mutation issued",
        ));
    }
    client.delete_issue(external).await.map_err(message)?;
    store
        .sqlite
        .acknowledge_task_deletion(id, change, None)
        .map_err(message)?;
    Ok(())
}

// Serialize provider effects only; saves and acquisition never take this lock.
// Keep the file in place so another connection always locks the same inode.
pub(super) fn lock_delivery(path: &Path) -> std::io::Result<Option<File>> {
    std::fs::create_dir_all(path.parent().expect("delivery lock has a parent directory"))?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    match fs2::FileExt::try_lock_exclusive(&lock) {
        Ok(()) => Ok(Some(lock)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(error),
    }
}

async fn sync_field(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &LinearClient,
    change: &PlanningChange,
) -> OpsResult<()> {
    let (external, content, revision) = observe(store, repo, owner, client).await?;
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
    observe(store, repo, owner, client).await?;
    if pending(store, owner)?.iter().any(|c| c.id == change.id) {
        return Err(message("Field write remains uncertain; receipt retained"));
    }
    Ok(())
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
            ingest_task(
                store,
                repo,
                &PmTaskRecord {
                    item,
                    project,
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                },
            )
            .await?;
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

// Exact detail can establish the saved Project's association without claiming a
// complete inventory refresh. Export and later field reads use this same path.
pub(super) async fn ingest_task(
    store: &Store,
    repo: &Path,
    record: &PmTaskRecord,
) -> OpsResult<()> {
    let mut confirmed = None;
    if let Some(project) = &record.project {
        if let Some(local) = store
            .get_project_by_project(&project.id)
            .await
            .map_err(message)?
            .filter(|p| {
                p.plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == project.id)
            })
        {
            let wave = store
                .get_wave(&local.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project Wave is missing"))?;
            if let Some(initiative) = super::pm::read_initiative(repo, wave.slug()) {
                confirmed = Some((local.wave_id, initiative));
            }
        }
    }
    store
        .sqlite
        .put_pm_task(
            &repo.to_string_lossy(),
            "linear",
            record,
            confirmed
                .as_ref()
                .map(|(wave, initiative)| (wave, initiative.as_str())),
        )
        .map_err(message)
}
