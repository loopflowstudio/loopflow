use std::path::Path;
use std::sync::Arc;

use crate::ops::{OpsError, OpsResult};
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::pm::PmProject;
use crate::store::{open_existing_store, SharedStore, Store};
use crate::work::project::{Project, ProjectId};
use crate::work::wave::Wave;

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

pub(crate) fn project_plan(
    project: &crate::pm::PmProject,
    pm_snapshot_synced_at: i64,
) -> OpsResult<ProjectPlan> {
    Ok(ProjectPlan {
        id: LinearProjectId::new(project.id.clone()).map_err(project_error)?,
        slug: project.slug.clone(),
        name: project.name.clone(),
        prompt_context: crate::ops::task::project_context(project),
        pm_snapshot_synced_at,
        flow: project.flow.clone(),
        status: project.status,
    })
}

/// Adopt provider Project facts and same-Wave issue moves without creating Task execution.
pub(crate) async fn sync_projects(
    store: &Store,
    wave: &Wave,
    snapshot: &crate::pm::PmSnapshot,
) -> OpsResult<()> {
    for plan in &snapshot.projects {
        let project = record_project(store, wave, plan).await?;
        for item in snapshot
            .items
            .iter()
            .filter(|item| item.project_id.as_deref() == Some(plan.id.as_str()))
        {
            if let Some(task) = store
                .get_task_by_issue(&item.id)
                .await
                .map_err(project_error)?
            {
                if task.project_id != project.id {
                    store
                        .move_chapter_task(&task.id, &project.id)
                        .await
                        .map_err(project_error)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn record_project(
    store: &Store,
    wave: &Wave,
    plan: &PmProject,
) -> OpsResult<Project> {
    if let Some(mut project) = store
        .get_project_by_project(&plan.id)
        .await
        .map_err(project_error)?
    {
        project.plan = project_plan(plan, time::OffsetDateTime::now_utc().unix_timestamp())?;
        store
            .update_project(&project)
            .await
            .map_err(project_error)?;
        return Ok(project);
    }
    let now = time::OffsetDateTime::now_utc();
    let project = Project {
        id: ProjectId::new(),
        plan: project_plan(plan, now.unix_timestamp())?,
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .create_project(&project)
        .await
        .map_err(project_error)?;
    Ok(project)
}
