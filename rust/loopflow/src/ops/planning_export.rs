//! Export saved planning identities on the foreground connection's lifetime.
use std::path::Path;
use std::time::Duration;

use serde_json::json;

use crate::durable::WorkRef;
use crate::store::sqlite::planning_changes::PlanningChanges;
use crate::store::sqlite::planning_export::PlanningExport;
use crate::store::Store;

use super::planning_client::{PlanningClient, StateScope};
use super::{OpsError, OpsResult};

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub(crate) async fn sync_repository_exports(store: &Store, repo: &str) -> OpsResult<()> {
    if !super::planning_client::connected(repo) {
        return Ok(());
    }
    store.sqlite.recover_task_filings(repo).map_err(message)?;
    // Projects precede Tasks; each failure is bounded and leaves other exports eligible.
    for work in store.sqlite.planning_export_owners(repo).map_err(message)? {
        if let Err(error) = sync_export(store, Path::new(repo), &work).await {
            tracing::debug!(%error, "planning export pending");
        }
    }
    if super::planning_client::is_linear(repo) {
        sync_retained_relations(store, Path::new(repo)).await?;
    }
    Ok(())
}

pub(crate) async fn sync_export(store: &Store, repo: &Path, work: &WorkRef) -> OpsResult<()> {
    // Creations spend the reserved Linear budget so they never fail behind sync.
    crate::linear::urgent(export(store, repo, work, false)).await
}

pub(crate) async fn retry_task_export(
    store: &Store,
    repo: &Path,
    task: &crate::durable::TaskId,
) -> OpsResult<()> {
    crate::linear::urgent(export(store, repo, &WorkRef::Task(task.clone()), true)).await
}

async fn export(store: &Store, repo: &Path, work: &WorkRef, retry_creation: bool) -> OpsResult<()> {
    if !super::planning_client::connected(&repo.to_string_lossy()) {
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
                (task.wave_id, task.linear_id.is_some())
            }
            PlanningChanges::Project(id) => {
                let project = store
                    .get_project(id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Project is missing"))?;
                (Some(project.wave_id), project.linear_id.is_some())
            }
        };
        if mapped {
            return Ok(());
        }
        let linear = super::planning_client::is_linear(&repo.to_string_lossy());
        let initiative = match &wave_id {
            Some(id) => {
                let wave = store
                    .get_wave(id)
                    .await
                    .map_err(message)?
                    .ok_or_else(|| message("Wave is missing"))?;
                Some(
                    super::planning_client::wave_scope(repo, wave.slug())?.ok_or_else(|| {
                        message("Wave has no Linear Initiative; export remains pending")
                    })?,
                )
            }
            None => None,
        };
        let team = super::planning_client::repository_scope(repo)?;
        let export = store
            .sqlite
            .prepare_planning_export(owner, &team, initiative.as_deref())
            .map_err(message)?;
        let client = PlanningClient::for_repository(repo).await?;
        if observe(store, repo, owner, &client, &export, wave_id.as_ref()).await? {
            return Ok(());
        }
        let (attempted, _) = store
            .sqlite
            .planning_export_attempts(owner)
            .map_err(message)?;
        if attempted && !retry_creation {
            return Err(message(
                "Creation remains uncertain; no repeated create issued",
            ));
        }
        let mut input = export.input.clone();
        let project = matches!(owner, PlanningChanges::Project(_));
        if project && linear {
            input["statusId"] = client
                .linear()?
                .project_field_input(
                    &crate::work::edit::WorkValue::ProjectStatus(
                        export.project_status().map_err(message)?,
                    ),
                    "",
                )
                .await
                .map_err(message)?["statusId"]
                .clone();
        } else if !project {
            input["stateId"] = json!(client
                .state_id(
                    StateScope::Team(
                        input["teamId"]
                            .as_str()
                            .ok_or_else(|| message("creation Team is missing"))?,
                    ),
                    export.task_state.as_deref().unwrap_or("unstarted")
                )
                .await
                .map_err(message)?);
        }
        if !store
            .sqlite
            .attempt_planning_export(owner, &input, false, retry_creation)
            .map_err(message)?
        {
            return Ok(());
        }
        let creation = match (project, linear) {
            (true, true) => {
                client
                    .linear()?
                    .deliver_planning_creation(true, input)
                    .await
            }
            (true, false) => match client.create_project(&input).await {
                Ok(revision) => Ok(crate::linear::PlanningResponse::Project {
                    project: Box::new(export.mirrored_project(&team, revision).map_err(message)?),
                    content: String::new(),
                }),
                Err(error) => Err(error),
            },
            (false, _) => client.create_task(input).await,
        };
        let response = match creation {
            Ok(response) => response,
            Err(error) => {
                // A rejected retry cannot disprove the earlier uncertain attempt.
                if !attempted && matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                    store
                        .sqlite
                        .reject_planning_export(owner, false)
                        .map_err(message)?;
                }
                return Err(message(error));
            }
        };
        match response {
            crate::linear::PlanningResponse::Project { project, .. } => {
                attach_project(
                    store,
                    owner,
                    &client,
                    &export,
                    wave_id
                        .as_ref()
                        .ok_or_else(|| message("Project export has no Wave"))?,
                    *project,
                )
                .await?;
            }
            response => {
                super::planning_delivery::apply_response(store, repo, owner, response).await?
            }
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
    client: &PlanningClient,
    export: &PlanningExport,
    wave: Option<&crate::id::WaveId>,
) -> OpsResult<bool> {
    let (attempted, _) = store
        .sqlite
        .planning_export_attempts(owner)
        .map_err(message)?;
    match owner {
        PlanningChanges::Project(_) => {
            let found = match client {
                PlanningClient::Linear(linear) => {
                    linear.find_project(&export.id).await.map_err(message)?
                }
                mirror => match mirror.project_revision(&export.id).await.map_err(message)? {
                    Some(revision) => Some(
                        export
                            .mirrored_project(
                                export
                                    .initiative
                                    .as_deref()
                                    .ok_or_else(|| message("Project export has no team"))?,
                                revision,
                            )
                            .map_err(message)?,
                    ),
                    None => None,
                },
            };
            let Some(project) = found else {
                return Ok(false);
            };
            if project.id != export.id || !attempted {
                return Err(message("Project identity already exists without an attempted creation; receipt retained"));
            }
            attach_project(
                store,
                owner,
                client,
                export,
                wave.ok_or_else(|| message("Project export has no Wave"))?,
                project,
            )
            .await?;
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
                &item,
                project.as_ref(),
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await?;
        }
    }
    Ok(true)
}

async fn attach_project(
    store: &Store,
    owner: PlanningChanges<'_>,
    client: &PlanningClient,
    export: &PlanningExport,
    wave: &crate::id::WaveId,
    mut project: crate::linear::LinearProject,
) -> OpsResult<()> {
    let initiative = export
        .initiative
        .as_deref()
        .ok_or_else(|| message("Project export has no Initiative"))?;
    if project.initiative_ids.is_empty() {
        let (_, attempted) = store
            .sqlite
            .planning_export_attempts(owner)
            .map_err(message)?;
        if attempted {
            return Err(message(
                "Project attachment remains uncertain; no repeated attachment issued",
            ));
        }
        if !store
            .sqlite
            .attempt_planning_export(owner, &export.input, true, false)
            .map_err(message)?
        {
            return Ok(());
        }
        project = match client
            .linear()?
            .deliver_project_attachment(&export.link_id, &export.id, initiative)
            .await
        {
            Ok(project) => project,
            Err(error) => {
                if matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                    store
                        .sqlite
                        .reject_planning_export(owner, true)
                        .map_err(message)?;
                }
                return Err(message(error));
            }
        };
    }
    if project.id != export.id || project.initiative_ids.as_slice() != [initiative] {
        return Err(message(
            "Created Project has conflicting Initiative membership; receipt retained",
        ));
    }
    crate::linear::acceptance::accept_linear_project_in(
        &store.sqlite,
        wave,
        initiative,
        &project,
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )
    .map_err(message)?;
    Ok(())
}

// Local filing settles independently. Exporting either endpoint makes its retained
// relation eligible, including after the source Task has already completed.
async fn sync_retained_relations(store: &Store, repo: &Path) -> OpsResult<()> {
    for relation in store
        .sqlite
        .pending_retained_relations(&repo.to_string_lossy())
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
            super::linear::issue_client()
                .await?
                .for_repository(repo)
                .ensure_follow_up_relation(
                    &relation.source_issue_id,
                    &relation.target_issue_id,
                    &relation.relation_id,
                )
                .await
                .map_err(message)?;
            store
                .sqlite
                .confirm_retained_relation(&relation)
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
