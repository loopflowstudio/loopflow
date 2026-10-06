use std::path::Path;
use std::sync::Arc;

use crate::ops::{OpsError, OpsResult};
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::store::{open_existing_store, SharedStore};
use crate::work::project::Project;

fn project_error(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

async fn project_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        project_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

pub(crate) async fn resolve_project_for_task(
    repo: &Path,
    wave_name: &str,
    project_id: &str,
) -> OpsResult<Project> {
    let locator = crate::work::wave::WaveLocator::discover(repo, wave_name)
        .map_err(|error| project_error(error.to_string()))?;
    let store = project_store().await?;
    let wave = store
        .get_wave_at(&locator)
        .await
        .map_err(|cause| project_error(cause.to_string()))?
        .ok_or_else(|| project_error("owning Wave is not initialized"))?;
    let current = crate::ops::chapter::current_project(&store, &wave).await?;
    if current.id != project_id {
        return Err(project_error(
            "new Tasks require the Wave's In Progress Project",
        ));
    }
    store
        .get_project_by_project(&current.id)
        .await
        .map_err(|cause| project_error(cause.to_string()))?
        .ok_or_else(|| project_error("current Project is unavailable; sync the Wave"))
}

pub(crate) fn project_plan(
    project: &crate::pm::PmProject,
    pm_snapshot_synced_at: i64,
) -> OpsResult<ProjectPlan> {
    Ok(ProjectPlan {
        id: LinearProjectId::new(project.id.clone())
            .map_err(|error| project_error(error.to_string()))?,
        slug: project.slug.clone(),
        name: project.name.clone(),
        prompt_context: crate::ops::task::project_context(project),
        pm_snapshot_synced_at,
        workflow: project.workflow.clone(),
        status: project.status,
    })
}
