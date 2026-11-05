//! Export saved planning identities on the foreground connection's lifetime.
//! A pass costs no requests when nothing changed; `README.md` holds the policy.
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;

use crate::durable::WorkRef;
use crate::store::sqlite::planning_changes::PlanningChanges;
use crate::store::sqlite::planning_export::PlanningExport;
use crate::store::Store;

use super::planning_client::{PlanningClient, StateScope};
use super::{OpsError, OpsResult};

/// Seconds between looks at a creation that stays uncertain while the Linear
/// budget is healthy. A paced or reserved budget stretches the window to its
/// pacing interval or its reset through `delta_fresh_secs`.
const UNCERTAIN_EXPORT_WINDOW_SECS: i64 = 600;

fn message(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn uncertain_export_window_secs(linear: bool) -> i64 {
    if linear {
        UNCERTAIN_EXPORT_WINDOW_SECS.max(crate::linear::delta_fresh_secs())
    } else {
        UNCERTAIN_EXPORT_WINDOW_SECS
    }
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

/// A pass: waits out the window after an uncertain creation was looked for.
pub(crate) async fn sync_export(store: &Store, repo: &Path, work: &WorkRef) -> OpsResult<()> {
    export(store, repo, work, false).await
}

/// Deliver one pending export. `force` looks for an uncertain creation now,
/// however recently a pass looked, and re-sends it when absent (`lf task sync`).
pub(crate) async fn export(
    store: &Store,
    repo: &Path,
    work: &WorkRef,
    force: bool,
) -> OpsResult<()> {
    // Creations spend the reserved Linear budget so they never fail behind sync.
    crate::linear::urgent(deliver(store, repo, work, force)).await
}

/// What the store knows before the provider is asked.
struct PreparedExport {
    wave_id: Option<crate::id::WaveId>,
    team: String,
    export: PlanningExport,
    /// The creation was sent and its outcome is unknown.
    attempted: bool,
    /// When a pass last looked for the uncertain creation.
    observed_at: Option<i64>,
}

async fn prepare(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
) -> OpsResult<Option<PreparedExport>> {
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
        return Ok(None);
    }
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
    let attempts = store
        .sqlite
        .planning_export_attempts(owner)
        .map_err(message)?;
    Ok(Some(PreparedExport {
        wave_id,
        team,
        export,
        attempted: attempts.creation,
        observed_at: attempts.observed_at,
    }))
}

async fn deliver(store: &Store, repo: &Path, work: &WorkRef, force: bool) -> OpsResult<()> {
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
    let linear = super::planning_client::is_linear(&repo.to_string_lossy());
    let prepared = match prepare(store, repo, owner).await {
        Ok(Some(prepared)) => prepared,
        Ok(None) => return Ok(()),
        Err(error) => return record(store, owner, Err(error)),
    };
    // An uncertain creation is looked for once per window. The refusal is not
    // recorded: the receipt keeps the error that made the creation uncertain.
    if prepared.attempted && !force {
        let window = uncertain_export_window_secs(linear);
        if let Some(ago) = prepared
            .observed_at
            .map(|at| now() - at)
            .filter(|ago| *ago < window)
        {
            return Err(message(format!(
                "Creation remains uncertain; looked {ago}s ago, next look in {}s",
                window - ago
            )));
        }
    }
    // Names the step a timeout interrupts; the future must be `Send`.
    let step = Mutex::new("validating the Team");
    let set_step = |name: &'static str| *step.lock().expect("export step poisoned") = name;
    let attempt = async {
        let PreparedExport {
            wave_id,
            team,
            export,
            attempted,
            ..
        } = &prepared;
        let client = PlanningClient::for_repository(repo).await?;
        set_step("looking for the saved creation");
        if observe(
            store,
            repo,
            owner,
            &client,
            export,
            wave_id.as_ref(),
            *attempted,
        )
        .await?
        {
            return Ok(());
        }
        if *attempted {
            tracing::info!(
                id = %export.id,
                "uncertain creation is absent; re-sending it with its saved id"
            );
        }
        let mut input = export.input.clone();
        let project = matches!(owner, PlanningChanges::Project(_));
        set_step("resolving the workflow state");
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
            .attempt_planning_export(owner, &input, false)
            .map_err(message)?
        {
            return Ok(());
        }
        set_step("sending the creation");
        let creation = match (project, linear) {
            (true, true) => {
                client
                    .linear()?
                    .deliver_planning_creation(true, input)
                    .await
            }
            (true, false) => match client.create_project(&input).await {
                Ok(revision) => Ok(crate::linear::PlanningResponse::Project {
                    project: Box::new(export.mirrored_project(team, revision).map_err(message)?),
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
                if !*attempted && matches!(error, crate::linear::LinearError::RateLimited { .. }) {
                    store
                        .sqlite
                        .reject_planning_export(owner, false)
                        .map_err(message)?;
                }
                return Err(message(error));
            }
        };
        set_step("recording the created item");
        match response {
            crate::linear::PlanningResponse::Project { project, .. } => {
                attach_project(
                    store,
                    owner,
                    &client,
                    export,
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
        .map_err(|_| {
            message(format!(
                "Export timed out while {}; creation identity and uncertainty retained",
                step.lock().expect("export step poisoned")
            ))
        })
        .and_then(|result| result);
    record(store, owner, result)
}

/// Keep the receipt's error current with the outcome.
fn record(store: &Store, owner: PlanningChanges<'_>, result: OpsResult<()>) -> OpsResult<()> {
    if let Err(error) = &result {
        store
            .sqlite
            .planning_export_error(owner, &error.to_string())
            .map_err(message)?;
    }
    result
}

/// Whether the saved creation already exists in the provider. A look at an
/// uncertain creation is stamped before asking, so a slow or failed answer
/// still counts for the window.
async fn observe(
    store: &Store,
    repo: &Path,
    owner: PlanningChanges<'_>,
    client: &PlanningClient,
    export: &PlanningExport,
    wave: Option<&crate::id::WaveId>,
    attempted: bool,
) -> OpsResult<bool> {
    if attempted {
        store
            .sqlite
            .observe_planning_export(owner, now())
            .map_err(message)?;
    }
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
            super::planning_delivery::ingest_task(store, repo, &item, project.as_ref(), now())
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
        if store
            .sqlite
            .planning_export_attempts(owner)
            .map_err(message)?
            .link
        {
            return Err(message(
                "Project attachment remains uncertain; no repeated attachment issued",
            ));
        }
        if !store
            .sqlite
            .attempt_planning_export(owner, &export.input, true)
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
        now(),
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
