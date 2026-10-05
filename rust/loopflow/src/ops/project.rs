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
    let current = crate::ops::chapter::current_project(&store, &wave).await?;
    if current.id != project_id {
        return Err(project_error(
            "new Tasks require the Wave's In Progress Project",
        ));
    }
    store
        .get_project_by_project(&current.id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("current Project is unavailable; sync the Wave"))
}
