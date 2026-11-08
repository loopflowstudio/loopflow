//! Legacy Project adoption: Projects observed before Wave bindings existed are
//! converted in Linear and cleared locally once that write is confirmed.
use std::path::Path;

use crate::linear::LinearProject;
use crate::ops::{OpsError, OpsResult};
use crate::store::Store;
use crate::work::project::ProjectStatus;
use crate::work::wave::WaveLocator;

use super::{project_is_foreign, LinearRepository};

fn adoption_error(value: impl std::fmt::Display) -> OpsError {
    OpsError::Message(value.to_string())
}

// The migration marker scopes old-content decoding to the exact pre-upgrade
// records. New Planned Projects never enter this transition. Reads project the
// conversion; explicit sync/rotation confirms it remotely before clearing it.
pub(super) async fn adopt_legacy_projects(
    repo: &Path,
    store: &Store,
    wave_name: &str,
    ctx: &LinearRepository,
    initiative: &str,
    apply: bool,
) -> OpsResult<Vec<LinearProject>> {
    let locator = WaveLocator::discover(repo, wave_name).map_err(adoption_error)?;
    let Some(wave) = store.get_wave_at(&locator).await.map_err(adoption_error)? else {
        return Ok(Vec::new());
    };
    let acquisition = if apply {
        Some(crate::ops::planning_sync::lock_wave_planning(&wave).await?)
    } else {
        None
    };
    let mut pending = store
        .projects_pending_adoption(wave.id())
        .await
        .map_err(adoption_error)?;
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let mut projects = ctx
        .client
        .list_projects(initiative)
        .await
        .map_err(adoption_error)?;
    for (id, _) in &pending {
        if !projects.iter().any(|project| &project.id == id) {
            projects.push(
                ctx.client
                    .project_ownership(id)
                    .await
                    .map_err(adoption_error)?,
            );
        }
    }
    // Retained migration records can belong to another repository's Team.
    // Leave those Projects and their receipts untouched, just as ordinary sync does.
    projects.retain(|project| !project_is_foreign(project, &ctx.team_id));
    pending.retain(|(id, _)| projects.iter().any(|project| &project.id == id));
    let has_current = projects
        .iter()
        .any(|project| project.status == ProjectStatus::Started);
    let candidates: Vec<_> = pending
        .iter()
        .filter(|(id, evidence)| {
            *evidence != 0
                && projects.iter().any(|project| {
                    &project.id == id
                        && matches!(
                            project.status,
                            ProjectStatus::Backlog | ProjectStatus::Planned
                        )
                })
        })
        .collect();
    let recorded: Vec<_> = candidates
        .iter()
        .filter(|(_, evidence)| *evidence == 1)
        .collect();
    let promote = if has_current {
        None
    } else if let [candidate] = recorded.as_slice() {
        Some(candidate.0.as_str())
    } else if recorded.is_empty()
        && candidates.len() == 1
        && projects
            .iter()
            .filter(|project| {
                matches!(
                    project.status,
                    ProjectStatus::Backlog | ProjectStatus::Planned
                )
            })
            .count()
            == 1
    {
        Some(candidates[0].0.as_str())
    } else if candidates.is_empty() {
        None
    } else {
        return Err(adoption_error(format!("Wave {wave_name}: legacy current Project is ambiguous; set the intended Project In Progress in Linear")));
    };
    // Validate the entire Wave before performing the first conversion.
    let mut converted = Vec::new();
    for (id, _) in &pending {
        converted.push(
            ctx.client
                .adopt_project(
                    id,
                    initiative,
                    &ctx.team_id,
                    promote == Some(id.as_str()),
                    false,
                )
                .await
                .map_err(adoption_error)?,
        );
    }
    if apply {
        for project in &mut converted {
            let confirmed = ctx
                .client
                .adopt_project(
                    &project.id,
                    initiative,
                    &ctx.team_id,
                    promote == Some(project.id.as_str()),
                    true,
                )
                .await
                .map_err(adoption_error)?;
            project.revision.clone_from(&confirmed.revision);
            if confirmed != *project {
                return Err(adoption_error(format!(
                    "Project {} changed during adoption; refresh and retry",
                    project.id
                )));
            }
            store
                .finish_project_adoption(
                    wave.id(),
                    &project.id,
                    acquisition
                        .as_ref()
                        .expect("apply owns the Wave lock")
                        .clone(),
                )
                .await
                .map_err(adoption_error)?;
        }
    }
    Ok(converted)
}
