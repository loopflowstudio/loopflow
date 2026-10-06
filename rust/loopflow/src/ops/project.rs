use std::path::Path;
use std::sync::Arc;

use crate::ops::{OpsError, OpsResult};
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;

fn project_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

async fn project_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        project_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

/// Select by the shared binding, independently of names and other Project statuses.
pub(crate) fn select_project(
    store: &crate::store::Store,
    wave: &crate::work::wave::Wave,
    projects: &[crate::pm::PmProject],
) -> OpsResult<crate::pm::PmProject> {
    let home = store.sqlite.home_dir().map_err(project_error)?;
    let selected = crate::work::wave::project_binding::read_project_binding(&home, wave.id())
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Wave {} has no configured Project", wave.slug())))?;
    let project = projects
        .iter()
        .find(|project| project.id == selected)
        .cloned()
        .ok_or_else(|| {
            project_error(format!(
                "configured Project {selected} is unavailable; refresh the Wave"
            ))
        })?;
    if matches!(
        project.status,
        crate::pm::ProjectStatus::Completed | crate::pm::ProjectStatus::Canceled
    ) {
        return Err(project_error(format!(
            "configured Project {selected} is terminal; its history is unchanged"
        )));
    }
    Ok(project)
}

pub(crate) async fn current_project(
    store: &crate::store::Store,
    wave: &crate::work::wave::Wave,
) -> OpsResult<crate::pm::PmProject> {
    let snapshot = store
        .pm_snapshot(wave.id())
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project planning is unavailable; refresh the Wave"))?;
    select_project(store, wave, &snapshot.snapshot.projects)
}

pub(crate) async fn resolve_project_for_task(
    repo: &Path,
    wave_name: &str,
    project_id: &str,
) -> OpsResult<Project> {
    let locator =
        crate::work::wave::WaveLocator::discover(repo, wave_name).map_err(project_error)?;
    let store = project_store().await?;
    let wave = store
        .get_wave_at(&locator)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("owning Wave is not initialized"))?;
    let current = current_project(&store, &wave).await?;
    if current.id != project_id {
        return Err(project_error(
            "new Tasks require the Wave's configured Project",
        ));
    }
    store
        .get_project_by_project(&current.id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("current Project is unavailable; sync the Wave"))
}

/// Explicitly seed a Wave's shared selection with an existing Project UUID.
/// Switching an established binding belongs to Project rotation.
pub async fn bind_project(
    repo: &Path,
    name: &str,
    project_id: &str,
) -> OpsResult<crate::pm::PmProject> {
    uuid::Uuid::parse_str(project_id).map_err(project_error)?;
    let store = super::pm::pm_store().await?;
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        Some(name),
        None,
    )
    .await
    .map_err(project_error)?;
    let acquisition = super::chapter::rotation_lock(&wave).await?;
    let home = store.sqlite.home_dir().map_err(project_error)?;
    let selected = crate::work::wave::project_binding::read_project_binding(&home, wave.id())
        .map_err(project_error)?;
    if selected.as_deref().is_some_and(|id| id != project_id) {
        return Err(project_error(
            "Wave already has a different configured Project; its binding is unchanged",
        ));
    }
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let project = ctx
        .client
        .find_project(project_id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Project {project_id} is unavailable")))?;
    crate::pm::validate_project_ownership(
        wave.slug(),
        &ctx.initiative,
        Some(&ctx.team_id),
        &project,
    )
    .map_err(project_error)?;
    let project = store
        .put_pm_project(
            wave.id(),
            "linear",
            &ctx.initiative,
            project,
            observed_at,
            Some(acquisition.clone()),
        )
        .await
        .map_err(project_error)?;
    crate::pm::validate_project_ownership(
        wave.slug(),
        &ctx.initiative,
        Some(&ctx.team_id),
        &project,
    )
    .map_err(project_error)?;
    if matches!(
        project.status,
        crate::pm::ProjectStatus::Completed | crate::pm::ProjectStatus::Canceled
    ) {
        return Err(project_error(
            "completed Project history cannot become the Wave's current Project",
        ));
    }
    crate::work::wave::project_binding::write_project_binding(
        &home,
        wave.id(),
        selected.as_deref(),
        project_id,
        &acquisition,
    )
    .map_err(project_error)?;
    Ok(project)
}
