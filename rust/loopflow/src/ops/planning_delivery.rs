//! Foreground delivery consumes the same receipts as local planning writers.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use crate::durable::WorkRef;
use crate::linear::LinearClient;
use crate::store::planning_sync::{PlanningChange, TaskDeletion};
use crate::store::sqlite::planning_changes::PlanningChanges;
use crate::store::Store;
use crate::work::edit::WorkValue;

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(crate) async fn sync_repository_fields(store: &Store, repo: &str) -> OpsResult<()> {
    if !super::linear_observe::connected(repo) {
        return Ok(());
    }
    let owners = store.sqlite.planning_field_owners(repo).map_err(message)?;
    let results = futures_util::future::join_all(
        owners
            .iter()
            .map(|owner| sync_fields(store, Path::new(repo), owner)),
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
    deliver_fields(store, repo, work, false).await
}

pub(crate) async fn deliver_fields(
    store: &Store,
    repo: &Path,
    work: &WorkRef,
    retry: bool,
) -> OpsResult<()> {
    if !super::linear_observe::connected(&repo.to_string_lossy()) {
        return Ok(());
    }
    let owner = match work {
        WorkRef::Task(id) => PlanningChanges::Task(id),
        WorkRef::Project(id) => PlanningChanges::Project(id),
        _ => return Err(message("field delivery requires a Task or Project")),
    };
    let Some(_lock) = lock_fields(store, owner)? else {
        return Ok(());
    };
    let changes = pending(store, owner)?;
    let deletion = match owner {
        PlanningChanges::Task(id) => store.sqlite.pending_task_deletion(id).map_err(message)?,
        _ => None,
    };
    let attempt = async {
        let client = super::linear::linear_client(repo).await?;
        if let Some(change) = &deletion {
            return sync_deletion(store, repo, owner, &client, change).await;
        }
        // Also reconcile an older attempted receipt whose successor is no longer pending.
        let mut observation = observe(store, repo, owner, &client).await?;
        for change in &changes {
            if let Err(error) =
                sync_field(store, repo, owner, &client, change, &mut observation, retry).await
            {
                store
                    .sqlite
                    .planning_field_error(owner, &change.id, &error.to_string())
                    .map_err(message)?;
            }
        }
        if let PlanningChanges::Project(id) = owner {
            if store
                .sqlite
                .project_order_delivery(id)
                .map_err(message)?
                .is_some()
            {
                super::planning_order::sync_order(store, repo, id, &client).await?;
            }
        }
        Ok(())
    };
    let result = tokio::time::timeout(Duration::from_secs(5), attempt)
        .await
        .map_err(|_| message("Field delivery timed out; attempted receipts remain uncertain"))
        .and_then(|result| result);
    if let Err(error) = &result {
        if let PlanningChanges::Project(id) = owner {
            store
                .sqlite
                .project_order_error(id, &error.to_string())
                .map_err(message)?;
        }
        if let Some(change) = &deletion {
            store
                .sqlite
                .planning_field_error(owner, &change.id, &error.to_string())
                .map_err(message)?;
        }
        for change in &changes {
            store
                .sqlite
                .planning_field_error(owner, &change.id, &error.to_string())
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
    change: &TaskDeletion,
) -> OpsResult<()> {
    let PlanningChanges::Task(id) = owner else {
        return Err(message("Only Task removal has a deletion receipt"));
    };
    let task = store
        .get_task(id)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task is missing"))?;
    let external = task.linear_id()?.as_str();
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
    let revision = observe(store, repo, owner, client).await?.revision;
    if revision.as_deref() != Some(trash_revision.as_str()) {
        return Err(message(
            "Linear deletion observations disagree; saved removal retained",
        ));
    }
    store
        .sqlite
        .observe_task_deletion(id, &trash_revision)
        .map_err(message)?;
    if store
        .sqlite
        .pending_task_deletion(id)
        .map_err(message)?
        .is_none_or(|c| c.id != change.id)
    {
        return Ok(());
    }
    let baseline = change.base_revision.as_deref();
    if baseline.is_none() {
        return Err(message(
            "Deletion has no provider baseline; saved removal retained",
        ));
    }
    if !store
        .sqlite
        .attempt_task_deletion(id, change, &trash_revision)
        .map_err(message)?
    {
        return Err(message(
            "Deletion remains uncertain; no repeated mutation issued",
        ));
    }
    if let Err(error) = client.delete_issue(external).await {
        if matches!(error, crate::linear::LinearError::RateLimited { .. }) {
            store
                .sqlite
                .reject_planning_field(owner, &change.id)
                .map_err(message)?;
        }
        return Err(message(error));
    }
    store
        .sqlite
        .acknowledge_task_deletion(id, change, None)
        .map_err(message)?;
    Ok(())
}

// Creation and field delivery share one effect lock per planning object.
pub(super) fn lock_fields(store: &Store, owner: PlanningChanges<'_>) -> OpsResult<Option<File>> {
    let (kind, id) = owner.owner();
    let path = store
        .sqlite
        .home_dir()
        .map_err(message)?
        .join("locks/planning-fields")
        .join(format!("{kind}-{id}.lock"));
    Ok(lock_delivery(&path)?)
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
    observation: &mut FieldObservation,
    retry: bool,
) -> OpsResult<()> {
    let Some(current) = pending(store, owner)?
        .into_iter()
        .find(|c| c.id == change.id)
    else {
        return Ok(());
    };
    let input = match owner {
        PlanningChanges::Task(_) => task_input(store, &current)?,
        PlanningChanges::Project(_) => client
            .project_field_input(&current.value, &observation.content)
            .await
            .map_err(message)?,
    };
    if !store
        .sqlite
        .attempt_planning_field(owner, &current, observation.revision.as_deref(), retry)
        .map_err(message)?
    {
        return Ok(());
    }
    let response = match client
        .deliver_planning_field(
            &observation.external,
            matches!(owner, PlanningChanges::Project(_)),
            input,
        )
        .await
    {
        Ok(response) => response,
        Err(error) => {
            if !retry && matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                store
                    .sqlite
                    .reject_planning_field(owner, &current.id)
                    .map_err(message)?;
            }
            return Err(message(error));
        }
    };
    match &response {
        crate::linear::PlanningResponse::Task { issue, .. } => {
            observation.revision = issue.item.revision.clone()
        }
        crate::linear::PlanningResponse::Project { project, content } => {
            observation.revision = project.revision.clone();
            observation.content = content.clone();
        }
    }
    apply_response(store, repo, owner, response).await?;
    if pending(store, owner)?.iter().any(|c| c.id == change.id) {
        return Err(message("Field write remains uncertain; receipt retained"));
    }
    Ok(())
}

pub(super) async fn apply_response(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    response: crate::linear::PlanningResponse,
) -> OpsResult<()> {
    match response {
        crate::linear::PlanningResponse::Task { issue, project, .. } => {
            ingest_task(
                store,
                repo,
                &issue.item,
                project.as_deref(),
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
        }
        crate::linear::PlanningResponse::Project {
            project: observed, ..
        } => {
            let PlanningChanges::Project(id) = owner else {
                return Err(message("Project response for a Task"));
            };
            let project = store
                .get_project(id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project is missing"))?;
            let wave = store
                .get_wave(&project.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Wave is missing"))?;
            let initiative = super::linear::read_initiative(repo, wave.slug())
                .ok_or_else(|| message("Wave has no Linear Initiative"))?;
            crate::linear::acceptance::accept_linear_project_in(
                &store.sqlite,
                &project.wave_id,
                &initiative,
                &observed,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .map_err(message)?;
            Ok(())
        }
    }
}

fn pending(store: &Store, owner: PlanningChanges<'_>) -> OpsResult<Vec<PlanningChange>> {
    match owner {
        PlanningChanges::Task(id) => store.sqlite.pending_task_changes(id),
        PlanningChanges::Project(id) => store.sqlite.pending_project_changes(id),
    }
    .map_err(message)
}

struct FieldObservation {
    external: String,
    content: String,
    revision: Option<String>,
}

async fn observe(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &LinearClient,
) -> OpsResult<FieldObservation> {
    match owner {
        PlanningChanges::Task(id) => {
            let task = store
                .get_task(id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Task is missing"))?;
            let external = task.linear_id()?.as_str().to_owned();
            let (item, project) = client
                .issue_ownership(&external)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Linked Linear issue is unavailable"))?;
            let revision = item.revision.clone();
            ingest_task(
                store,
                repo,
                &item,
                project.as_ref(),
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await?;
            if !store
                .sqlite
                .accepted_task_revision(id, &external, revision.as_deref())
                .map_err(message)?
            {
                return Err(message(
                    "Newer or unresolved Linear planning retained; field delivery deferred",
                ));
            }
            Ok(FieldObservation {
                external,
                content: String::new(),
                revision,
            })
        }
        PlanningChanges::Project(id) => {
            let project = store
                .get_project(id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project is missing"))?;
            let external = project.linear_id()?.as_str().to_owned();
            let (observed, content) = client
                .project_content_observation(&external)
                .await
                .map_err(message)?;
            let wave = store
                .get_wave(&project.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project Wave is missing"))?;
            let initiative = super::linear::read_initiative(repo, wave.slug())
                .ok_or_else(|| message("Wave has no Linear Initiative"))?;
            crate::linear::acceptance::accept_linear_project_in(
                &store.sqlite,
                &project.wave_id,
                &initiative,
                &observed,
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .map_err(message)?;
            if !store
                .sqlite
                .accepted_project_revision(id, &external, observed.revision.as_deref())
                .map_err(message)?
            {
                return Err(message(
                    "Newer or unresolved Linear Project planning retained; field delivery deferred",
                ));
            }
            Ok(FieldObservation {
                external,
                content,
                revision: observed.revision,
            })
        }
    }
}

fn task_input(store: &Store, change: &PlanningChange) -> OpsResult<Value> {
    match &change.value {
        WorkValue::TaskTitle(value) => Ok(json!({"title": value})),
        WorkValue::TaskDescription(value) => Ok(json!({"description": value})),
        WorkValue::TaskAssignee(value) => Ok(json!({"assigneeId": value})),
        WorkValue::TaskProject(None) => Ok(json!({"projectId": null})),
        WorkValue::TaskProject(Some(value)) => {
            let id = crate::durable::ProjectId::parse(value).map_err(message)?;
            let project = store
                .sqlite
                .project(&id)
                .map_err(message)?
                .ok_or_else(|| message("destination Project is missing"))?;
            Ok(json!({"projectId":project.linear_id()?.as_str()}))
        }
        _ => Err(message(format!(
            "unsupported Task planning field: {}",
            change.value.field()
        ))),
    }
}

// Exact detail can establish the saved Project's association without claiming a
// complete inventory refresh. Export and later field reads use this same path.
pub(super) async fn ingest_task(
    store: &Store,
    repo: &Path,
    issue: &crate::linear::LinearIssue,
    project: Option<&crate::linear::LinearProject>,
    observed_at: i64,
) -> OpsResult<()> {
    let team = super::linear::repository_team_id(repo)?;
    if issue.team_id.as_deref() != Some(team.as_str()) {
        return Err(message(format!(
            "Task {} must belong to repository Team {team}",
            issue.identifier
        )));
    }
    let mut confirmed = None;
    if let Some(project) = project {
        if let Some(local) = store
            .get_project_by_project(&project.id)
            .await
            .map_err(message)?
            .filter(|p| {
                p.linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == project.id)
            })
        {
            let wave = store
                .get_wave(&local.wave_id)
                .await
                .map_err(message)?
                .ok_or_else(|| message("Project Wave is missing"))?;
            if let Some(initiative) = super::linear::read_initiative(repo, wave.slug()) {
                confirmed = Some((local.wave_id, initiative));
            }
        }
    }
    if confirmed.is_none() {
        if let Some(project) = project {
            for wave in store
                .list_waves(Some(&repo.to_string_lossy()))
                .await
                .map_err(message)?
            {
                if let Some(initiative) = super::linear::read_initiative(repo, wave.slug()) {
                    if project.initiative_ids.contains(&initiative) {
                        if confirmed.is_some() {
                            return Err(message("Project matches multiple Wave Initiatives"));
                        }
                        confirmed = Some((wave.id().clone(), initiative));
                    }
                }
            }
        }
    }
    crate::linear::acceptance::accept_linear_issue_in(
        &store.sqlite,
        &repo.to_string_lossy(),
        issue,
        project,
        observed_at,
        confirmed
            .as_ref()
            .map(|(wave, initiative)| (wave, initiative.as_str())),
    )
    .map_err(message)
}
