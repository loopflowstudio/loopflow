use std::path::Path;
use std::sync::Arc;

use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmProject, ProjectContent, ProjectStatus};
use crate::store::project_transitions::ProjectTransition;
use crate::store::{PlanningLocks, Store};
use crate::work::project::Project;
use crate::work::wave::project_binding::{read_project_binding, write_project_binding};
use crate::work::wave::Wave;

fn project_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

/// Select by the shared binding, independently of names and other Project statuses.
pub(crate) fn select_project<'a>(
    store: &Store,
    wave: &Wave,
    projects: &'a [PmProject],
) -> OpsResult<&'a PmProject> {
    let home = store.sqlite.home_dir().map_err(project_error)?;
    let selected = read_project_binding(&home, wave.id())
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Wave {} has no configured Project", wave.slug())))?;
    let project = projects
        .iter()
        .find(|project| project.id == selected)
        .ok_or_else(|| {
            project_error(format!(
                "configured Project {selected} is unavailable; refresh the Wave"
            ))
        })?;
    if matches!(
        project.status,
        ProjectStatus::Completed | ProjectStatus::Canceled
    ) {
        return Err(project_error(format!(
            "configured Project {selected} is terminal; its history is unchanged"
        )));
    }
    Ok(project)
}

pub(crate) async fn current_project(store: &Store, wave: &Wave) -> OpsResult<PmProject> {
    let snapshot = store
        .pm_snapshot(wave.id())
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project planning is unavailable; refresh the Wave"))?;
    select_project(store, wave, &snapshot.snapshot.projects).cloned()
}

pub(crate) async fn resolve_project_for_task(
    store: &Store,
    wave: &Wave,
    project_id: &str,
) -> OpsResult<Project> {
    let current = current_project(store, wave).await?;
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
pub async fn bind_project(repo: &Path, name: &str, project_id: &str) -> OpsResult<PmProject> {
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
    let selected = read_project_binding(&home, wave.id()).map_err(project_error)?;
    if selected.as_deref().is_some_and(|id| id != project_id) {
        return Err(project_error(
            "Wave already has a different configured Project; its binding is unchanged",
        ));
    }
    if let Some(pending) = store
        .pending_project_transition(wave.id())
        .await
        .map_err(project_error)?
    {
        if pending.successor_id != project_id
            || pending.reset_name.is_some()
            || pending.predecessor_id.is_some()
        {
            return Err(project_error(
                "an unfinished Project transition owns binding setup; resume it first",
            ));
        }
    }
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    let observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let project = require_project(&ctx, project_id).await?;
    let project = accept_project(&store, &wave, &ctx, project, observed_at, &acquisition).await?;
    if matches!(
        project.status,
        ProjectStatus::Completed | ProjectStatus::Canceled
    ) {
        return Err(project_error(
            "completed Project history cannot become the Wave's current Project",
        ));
    }
    write_project_binding(
        &home,
        wave.id(),
        selected.as_deref(),
        project_id,
        &acquisition,
    )
    .map_err(project_error)?;
    Ok(project)
}

/// Explicit activation; ordinary planning reads never call this operation.
pub async fn ensure(repo: &Path, name: &str) -> OpsResult<PmProject> {
    let store = super::pm::pm_store().await?;
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        Some(name),
        None,
    )
    .await
    .map_err(project_error)?;
    super::chapter::require_chapter_home(&store, &wave).await?;
    let acquisition = super::chapter::rotation_lock(&wave).await?;
    let home = store.sqlite.home_dir().map_err(project_error)?;
    let selected = read_project_binding(&home, wave.id()).map_err(project_error)?;
    let pending = store
        .pending_project_transition(wave.id())
        .await
        .map_err(project_error)?;
    let mut creation = match pending {
        Some(transition)
            if transition.predecessor_id.is_none() && transition.reset_name.is_none() =>
        {
            Some(transition)
        }
        Some(_) if selected.is_none() => {
            return Err(project_error(
                "Project rotation is unfinished; resume the explicit reset",
            ));
        }
        _ => None,
    };
    if let Some(creation) = &creation {
        if selected
            .as_deref()
            .is_some_and(|id| id != creation.successor_id)
        {
            return Err(project_error(
                "Project selection changed during creation; reconcile the retained transition",
            ));
        }
    }
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    if selected.is_none() && creation.is_none() {
        ctx.client
            .require_initiative(&ctx.initiative)
            .await
            .map_err(project_error)?;
        let reserved = ProjectTransition {
            wave_id: wave.id().clone(),
            successor_id: uuid::Uuid::new_v4().to_string(),
            predecessor_id: None,
            reset_name: None,
            created_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            settled_at: None,
        };
        store
            .reserve_project_transition(reserved.clone(), acquisition.clone())
            .await
            .map_err(project_error)?;
        creation = Some(reserved);
    }
    let id = selected
        .as_deref()
        .or_else(|| creation.as_ref().map(|t| t.successor_id.as_str()))
        .expect("selection or creation reservation exists");
    let mut observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut project = match ctx.client.find_project(id).await.map_err(project_error)? {
        Some(project) => project,
        None if creation.is_some() && selected.is_none() => {
            ctx.client
                .require_initiative(&ctx.initiative)
                .await
                .map_err(project_error)?;
            let content = ProjectContent {
                flow: String::new(),
                krs: Vec::new(),
                metric_targets: Vec::new(),
            };
            let created = ctx
                .client
                .create_project(&ctx.initiative, wave.slug(), &content, Some(id))
                .await
                .map_err(project_error)?;
            if created != id {
                return Err(project_error(
                    "provider returned a different Project identity",
                ));
            }
            observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
            require_project(&ctx, id).await?
        }
        None => {
            return Err(project_error(format!(
                "configured Project {id} is unavailable; its binding is unchanged"
            )))
        }
    };
    require_active_candidate(&project)?;
    // Only a retained creation reservation authorizes repairing an unattached Project.
    if creation.is_some()
        && project.initiative_ids.is_empty()
        && project.team_ids == [ctx.team_id.clone()]
    {
        ctx.client
            .attach_project(&ctx.initiative, id)
            .await
            .map_err(project_error)?;
        observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        project = require_project(&ctx, id).await?;
    }
    project = accept_project(&store, &wave, &ctx, project, observed_at, &acquisition).await?;
    require_active_candidate(&project)?;
    if project.status != ProjectStatus::Started {
        ctx.client
            .set_project_status(id, ProjectStatus::Started)
            .await
            .map_err(project_error)?;
        observed_at = time::OffsetDateTime::now_utc().unix_timestamp();
        project = require_project(&ctx, id).await?;
        project = accept_project(&store, &wave, &ctx, project, observed_at, &acquisition).await?;
        if project.status != ProjectStatus::Started {
            return Err(project_error(
                "accepted Project evidence does not confirm activation",
            ));
        }
    }
    write_project_binding(&home, wave.id(), selected.as_deref(), id, &acquisition)
        .map_err(project_error)?;
    if read_project_binding(&home, wave.id())
        .map_err(project_error)?
        .as_deref()
        != Some(id)
    {
        return Err(project_error(
            "Project binding confirmation changed; retry ensure",
        ));
    }
    if creation.is_some() {
        store
            .settle_project_transition(wave.id(), id, acquisition)
            .await
            .map_err(project_error)?;
    }
    Ok(project)
}

async fn require_project(ctx: &super::pm::PmContext, id: &str) -> OpsResult<PmProject> {
    ctx.client
        .find_project(id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Project {id} is unavailable")))
}

fn require_active_candidate(project: &PmProject) -> OpsResult<()> {
    if !matches!(
        project.status,
        ProjectStatus::Backlog | ProjectStatus::Planned | ProjectStatus::Started
    ) {
        return Err(project_error(format!(
            "Project {} is {}; reconcile its status explicitly",
            project.id,
            project.status.as_str()
        )));
    }
    Ok(())
}

// Validate both the response and the accepted body: a delayed response may lose
// to newer stored facts, and only those accepted facts may authorize activation.
async fn accept_project(
    store: &Store,
    wave: &Wave,
    ctx: &super::pm::PmContext,
    project: PmProject,
    observed_at: i64,
    acquisition: &Arc<PlanningLocks>,
) -> OpsResult<PmProject> {
    let validate = |project: &PmProject| {
        crate::pm::validate_project_ownership(
            wave.slug(),
            &ctx.initiative,
            Some(&ctx.team_id),
            project,
        )
        .map_err(project_error)
    };
    validate(&project)?;
    let project = store
        .put_pm_project(
            wave.id(),
            ctx.provider.as_str(),
            &ctx.initiative,
            project,
            observed_at,
            Some(acquisition.clone()),
        )
        .await
        .map_err(project_error)?;
    validate(&project)?;
    Ok(project)
}

pub fn update_plan(repo: &Path, wave: Option<&str>, content: &ProjectContent) -> OpsResult<()> {
    content.validate().map_err(project_error)?;
    let wave = crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave)
        .map_err(project_error)?;
    tokio::runtime::Runtime::new()
        .map_err(project_error)?
        .block_on(async {
            let store = super::pm::pm_store().await?;
            let acquisition = super::chapter::rotation_lock(&wave).await?;
            super::metrics::validate_chapter_targets(&wave, &content.metric_targets)
                .map_err(project_error)?;
            let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
            let projects =
                super::pm::checked_projects_with_store(repo, &ctx, wave.slug(), &store).await?;
            let project = select_project(&store, &wave, &projects)?;
            let provider = ctx
                .client
                .project_ownership(&project.id)
                .await
                .map_err(project_error)?;
            ctx.client
                .update_project(&provider.id, &provider.name, content)
                .await
                .map_err(project_error)?;
            super::pm::refresh_pm_snapshot_locked(repo, &wave, &ctx, &store, acquisition).await?;
            Ok(())
        })
}
