//! Export saved planning identities on the foreground connection's lifetime.
use std::path::Path;
use std::time::Duration;

use serde_json::json;

use crate::durable::WorkRef;
use crate::pm::linear::LinearClient;
use crate::store::sqlite::planning_changes::PlanningChanges;
use crate::store::sqlite::planning_export::PlanningExport;
use crate::store::{PmTaskRecord, Store};
use crate::work::task::Task;

use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(crate) async fn sync_repository_exports(store: &Store, task: &Task) -> OpsResult<()> {
    let wave = store
        .get_wave(&task.wave_id)
        .await
        .map_err(message)?
        .ok_or_else(|| message("Task Wave is missing"))?;
    if !super::linear_observe::connected(wave.repo()) {
        return Ok(());
    }
    // Projects precede Tasks; each failure is bounded and leaves other exports eligible.
    for work in store
        .sqlite
        .planning_export_owners(wave.repo())
        .map_err(message)?
    {
        if let Err(error) = sync_export(store, Path::new(wave.repo()), &work).await {
            tracing::debug!(%error, "planning export pending");
        }
    }
    sync_follow_through_relations(store, Path::new(wave.repo())).await?;
    Ok(())
}

pub(crate) async fn sync_export(store: &Store, repo: &Path, work: &WorkRef) -> OpsResult<()> {
    if !super::linear_observe::connected(&repo.to_string_lossy()) {
        return Ok(());
    }
    let owner = match work {
        WorkRef::Task(id) => PlanningChanges::Task(id),
        WorkRef::Project(id) => PlanningChanges::Project(id),
        _ => return Err(message("export requires a Task or Project")),
    };
    let Some(_lock) = super::planning_delivery::lock_fields(store, owner)? else {
        return Ok(());
    };
    let attempt = async {
        let (wave_id, mapped) = match owner {
            PlanningChanges::Task(id) => {
                let task = store
                    .get_task(id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Task is missing"))?;
                (task.wave_id, task.plan.linear_id.is_some())
            }
            PlanningChanges::Project(id) => {
                let project = store
                    .get_project(id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Project is missing"))?;
                (project.wave_id, project.plan.linear_id.is_some())
            }
        };
        if mapped {
            return Ok(());
        }
        let wave = store
            .get_wave(&wave_id)
            .await
            .map_err(message)?
            .ok_or_else(|| message("Wave is missing"))?;
        let initiative = super::pm::read_initiative(repo, wave.slug())
            .ok_or_else(|| message("Wave has no Linear Initiative; export remains pending"))?;
        let team = super::pm::repository_team_id(repo)?;
        let export = store
            .sqlite
            .prepare_planning_export(owner, &team, &initiative)
            .map_err(message)?;
        let client = super::pm::linear_client(repo).await?;
        if observe(store, repo, owner, &client, &export, &wave_id).await? {
            return Ok(());
        }
        let (attempted, _) = store
            .sqlite
            .planning_export_attempts(owner)
            .map_err(message)?;
        if attempted {
            return Err(message(
                "Creation remains uncertain; no repeated create issued",
            ));
        }
        let mut input = export.input.clone();
        let project = matches!(owner, PlanningChanges::Project(_));
        if project {
            input["statusId"] = client
                .project_field_input("status", &export.model["status"], "")
                .await
                .map_err(message)?["statusId"]
                .clone();
        } else {
            input["stateId"] = json!(client
                .team_state_id(
                    input["teamId"]
                        .as_str()
                        .ok_or_else(|| message("creation Team is missing"))?,
                    export.model["state"].as_str().unwrap_or("unstarted")
                )
                .await
                .map_err(message)?);
        }
        if !store
            .sqlite
            .attempt_planning_export(owner, &input, false)
            .map_err(message)?
        {
            return Ok(());
        }
        client
            .deliver_planning_creation(project, input)
            .await
            .map_err(message)?;
        if !observe(store, repo, owner, &client, &export, &wave_id).await? {
            return Err(message(
                "Creation acknowledgement lacks exact readback; receipt retained",
            ));
        }
        Ok(())
    };
    let result = tokio::time::timeout(Duration::from_secs(5), attempt)
        .await
        .map_err(|_| message("Export timed out; creation identity and uncertainty retained"))
        .and_then(|result| result);
    if let Err(error) = &result {
        store
            .sqlite
            .planning_export_error(owner, &error.to_string())
            .map_err(message)?;
    }
    result
}

async fn observe(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &LinearClient,
    export: &PlanningExport,
    wave: &crate::id::WaveId,
) -> OpsResult<bool> {
    let (attempted, link_attempted) = store
        .sqlite
        .planning_export_attempts(owner)
        .map_err(message)?;
    match owner {
        PlanningChanges::Project(_) => {
            let Some(mut project) = client.find_project(&export.id).await.map_err(message)? else {
                return Ok(false);
            };
            if project.id != export.id || !attempted {
                return Err(message("Project identity already exists without an attempted creation; receipt retained"));
            }
            if project.initiative_ids.is_empty() {
                if link_attempted {
                    return Err(message(
                        "Project attachment remains uncertain; no repeated attachment issued",
                    ));
                }
                if !store
                    .sqlite
                    .attempt_planning_export(owner, &export.input, true)
                    .map_err(message)?
                {
                    return Ok(false);
                }
                client
                    .deliver_project_attachment(&export.link_id, &export.id, &export.initiative)
                    .await
                    .map_err(message)?;
                project = client
                    .find_project(&export.id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Created Project is unavailable"))?;
            }
            if project.id != export.id
                || project.initiative_ids.as_slice() != [export.initiative.as_str()]
            {
                return Err(message(
                    "Created Project has conflicting Initiative membership; receipt retained",
                ));
            }
            store
                .sqlite
                .put_pm_project(
                    wave,
                    "linear",
                    &export.initiative,
                    &project,
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )
                .map_err(message)?;
        }
        PlanningChanges::Task(_) => {
            let Some((item, project)) = client
                .find_export_issue(&export.id)
                .await
                .map_err(message)?
            else {
                return Ok(false);
            };
            if item.id != export.id || !attempted {
                return Err(message(
                    "Issue identity already exists without an attempted creation; receipt retained",
                ));
            }
            super::planning_delivery::ingest_task(
                store,
                repo,
                &PmTaskRecord {
                    item,
                    project,
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                },
            )
            .await?;
        }
    }
    Ok(true)
}

// Local filing settles independently. Exporting either endpoint makes its retained
// relation eligible, including after the source Task has already completed.
async fn sync_follow_through_relations(store: &Store, repo: &Path) -> OpsResult<()> {
    for relation in store
        .sqlite
        .pending_follow_through_relations(&repo.to_string_lossy())
        .map_err(message)?
    {
        let path = store
            .sqlite
            .home_dir()
            .map_err(message)?
            .join("locks/follow-through-relations")
            .join(format!("{}.lock", relation.task_id));
        let Some(_lock) = super::planning_delivery::lock_delivery(&path)? else {
            continue;
        };
        let attempt = async {
            super::pm::issue_client(repo)
                .await?
                .ensure_follow_up_relation(
                    &relation.source_issue_id,
                    &relation.target_issue_id,
                    &relation.relation_id,
                )
                .await
                .map_err(message)?;
            store
                .sqlite
                .confirm_follow_through_relation(&relation)
                .map_err(message)
        };
        match tokio::time::timeout(Duration::from_secs(5), attempt).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::debug!(%error, task = %relation.task_id, "follow-up relation synchronization pending")
            }
            Err(_) => {
                tracing::debug!(task = %relation.task_id, "follow-up relation synchronization timed out; receipt retained")
            }
        }
    }
    Ok(())
}
