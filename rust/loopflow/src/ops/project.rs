use std::path::Path;
use std::sync::Arc;

use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmProject, ProjectContent, ProjectStatus};
use crate::store::project_transitions::ProjectTransition;
use crate::store::sqlite::project_selection::{read_project_binding, write_project_binding};
use crate::store::{PlanningLocks, Store};
use crate::work::project::Project;
use crate::work::wave::Wave;

fn project_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

pub(crate) fn current_project(store: &Store, wave: &Wave) -> OpsResult<PmProject> {
    let project = store
        .sqlite
        .selected_planning_project(wave.id())
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Wave {} has no configured Project", wave.slug())))?;
    if matches!(
        project.status,
        ProjectStatus::Completed | ProjectStatus::Canceled
    ) {
        return Err(project_error(format!(
            "configured Project {} is terminal; its history is unchanged",
            project.id
        )));
    }
    Ok(project)
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
    let acquisition = super::pm::lock_wave_planning(&wave).await?;
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    import_binding(&store, &wave, &ctx, &acquisition).await?;
    let selected = read_project_binding(&store.sqlite, wave.id()).map_err(project_error)?;
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
        &store.sqlite,
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
    if let Some(name) = name.strip_prefix("personal:") {
        let repo = crate::repository::CanonicalRepo::discover(repo).map_err(project_error)?;
        let project = store
            .sqlite
            .ensure_personal_project(&repo.to_string(), name)
            .map_err(project_error)?;
        return store
            .sqlite
            .planning_project(&project.id)
            .map_err(project_error);
    }
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        Some(name),
        None,
    )
    .await
    .map_err(project_error)?;
    if let Some(name) = wave.slug().strip_prefix("personal:") {
        let project = store
            .sqlite
            .ensure_personal_project(wave.repo(), name)
            .map_err(project_error)?;
        return store
            .sqlite
            .planning_project(&project.id)
            .map_err(project_error);
    }
    super::pm::require_planning_home(&store, &wave).await?;
    store
        .sqlite
        .record_project_activation(wave.id(), crate::journal::current_process_lfid().as_ref())
        .map_err(project_error)?;
    let acquisition = super::pm::lock_wave_planning(&wave).await?;
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    import_binding(&store, &wave, &ctx, &acquisition).await?;
    let selected = read_project_binding(&store.sqlite, wave.id()).map_err(project_error)?;
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
            create_successor: None,
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
                workflow: String::new(),
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
    if creation.is_some() {
        store
            .sqlite
            .finish_project_creation(wave.id(), selected.as_deref(), id, &acquisition)
            .map_err(project_error)?;
    } else {
        write_project_binding(
            &store.sqlite,
            wave.id(),
            selected.as_deref(),
            id,
            &acquisition,
        )
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
pub(crate) async fn accept_project(
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
            "linear",
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

pub async fn update_plan(
    repo: &Path,
    wave: Option<&str>,
    content: ProjectContent,
) -> OpsResult<ProjectPlanning> {
    let store = super::pm::pm_store().await?;
    let wave =
        crate::work::wave::context::resolve_managed_wave(Some(&store), Some(repo), wave, None)
            .await
            .map_err(project_error)?;
    let _acquisition = super::pm::lock_wave_planning(&wave).await?;
    let selected = current_project(&store, &wave)?;
    let project = resolve_project(&store, repo, &selected.id).await?;
    super::metrics::validate_chapter_targets(&wave, &content.metric_targets)
        .map_err(project_error)?;
    store
        .sqlite
        .update_project_content(&project.id, &content, None)
        .map_err(project_error)?;
    planning(&store, &project, repo)
}

/// One-time supported import at an explicit mutation boundary. A failed read is
/// never permission to create; the original bytes remain durable evidence.
pub(crate) async fn import_binding(
    store: &Store,
    wave: &Wave,
    ctx: &super::pm::PmContext,
    guard: &Arc<PlanningLocks>,
) -> OpsResult<()> {
    if store
        .sqlite
        .project_binding_imported(wave.id())
        .map_err(project_error)?
    {
        return Ok(());
    }
    let path = store
        .sqlite
        .home_dir()
        .map_err(project_error)?
        .join("waves")
        .join(wave.id().as_str())
        .join("config.yaml");
    let original = match std::fs::read_to_string(&path) {
        Ok(content) => Some(content),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(project_error(format!(
                "Project binding import at {}: {error}",
                path.display()
            )))
        }
    };
    #[derive(serde::Deserialize)]
    struct Config {
        pm: Option<Binding>,
    }
    #[derive(serde::Deserialize)]
    struct Binding {
        linear_project: Option<String>,
    }
    let selected = original
        .as_deref()
        .map(serde_yaml_ng::from_str::<Config>)
        .transpose()
        .map_err(project_error)?
        .and_then(|config| config.pm)
        .and_then(|pm| pm.linear_project);
    if let Some(id) = &selected {
        uuid::Uuid::parse_str(id).map_err(project_error)?;
        let acquired = time::OffsetDateTime::now_utc().unix_timestamp();
        let project = require_project(ctx, id).await?;
        accept_project(store, wave, ctx, project, acquired, guard).await?;
    }
    store
        .sqlite
        .import_project_binding(wave.id(), original.as_deref(), selected.as_deref(), guard)
        .map_err(project_error)
}

/// Address a retained Project by durable ID, provider ID, or unique slug/name.
async fn resolve_project(store: &Store, repo: &Path, selector: &str) -> OpsResult<Project> {
    let repo = crate::repository::CanonicalRepo::discover(repo)
        .map_err(project_error)?
        .to_string();
    let waves = store.list_waves(None).await.map_err(project_error)?;
    let projects = store.list_projects(None).await.map_err(project_error)?;
    let mut matches = projects.into_iter().filter(|project| {
        waves
            .iter()
            .any(|wave| wave.id() == &project.wave_id && wave.repo() == repo)
            && (project.id.as_str() == selector
                || project
                    .plan
                    .linear_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == selector)
                || project.plan.slug == selector
                || project.plan.name == selector)
    });
    let project = matches
        .next()
        .ok_or_else(|| project_error(format!("Project {selector:?} not found")))?;
    if matches.next().is_some() {
        return Err(project_error("Project name is ambiguous; use its ID"));
    }
    Ok(project)
}

pub async fn workflow_catalog(
    repo: &Path,
    selector: Option<&str>,
) -> OpsResult<Vec<crate::engine::workflow::WorkflowCatalogEntry>> {
    let mut entries = crate::engine::workflow::workflow_catalog(repo).map_err(project_error)?;
    if let Some(selector) = selector {
        let store = super::pm::pm_store().await?;
        let project = resolve_project(&store, repo, selector).await?;
        for (name, content) in store
            .sqlite
            .wave_workflows(&project.wave_id)
            .map_err(project_error)?
        {
            let (workflow, unavailable) =
                match crate::engine::workflow::parse_workflow(&name, &content, repo) {
                    Ok(workflow) => (Some(workflow), None),
                    Err(error) => (None, Some(error)),
                };
            entries.retain(|entry| entry.name != name);
            entries.push(crate::engine::workflow::WorkflowCatalogEntry {
                name,
                source: Some("stored".into()),
                workflow,
                unavailable,
            });
        }
    }
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub async fn workflow_source(repo: &Path, selector: &str, name: &str) -> OpsResult<String> {
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    read_workflow_source(&store, &project.wave_id, name, repo)?
        .ok_or_else(|| project_error(format!("Workflow {name:?} is unavailable")))
}

pub async fn edit(
    repo: &Path,
    selector: &str,
    name: Option<&str>,
    summary: Option<&str>,
) -> OpsResult<ProjectPlanning> {
    if name.is_none() && summary.is_none() {
        return Err(project_error("project edit requires --name or --summary"));
    }
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    let wave = store
        .get_wave(&project.wave_id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project Wave is unavailable"))?;
    let _acquisition = super::pm::lock_wave_planning(&wave).await?;
    store
        .sqlite
        .edit_project(&project.id, name, summary)
        .map_err(project_error)?;
    planning(&store, &project, repo)
}

pub async fn workflow(
    repo: &Path,
    selector: &str,
    selection: Option<&str>,
    file: Option<&Path>,
) -> OpsResult<ProjectPlanning> {
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    if let Some(name) = selection {
        let definition = match file {
            Some(path) => std::fs::read_to_string(path).map_err(project_error)?,
            None => read_workflow_source(&store, &project.wave_id, name, repo)?
                .ok_or_else(|| project_error(format!("Workflow {name:?} not found")))?,
        };
        crate::engine::workflow::parse_workflow(name, &definition, repo).map_err(project_error)?;
        let wave = store
            .get_wave(&project.wave_id)
            .await
            .map_err(project_error)?
            .ok_or_else(|| project_error("Project Wave is unavailable"))?;
        let _acquisition = super::pm::lock_wave_planning(&wave).await?;
        store
            .sqlite
            .select_project_workflow(&project.id, name, &definition)
            .map_err(project_error)?;
    }
    planning(&store, &project, repo)
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectPlanning {
    #[serde(flatten)]
    pub project: PmProject,
    pub sync_enabled: bool,
    pub pending_changes: Vec<crate::planning::PlanningChange>,
}

fn planning(store: &Store, project: &Project, repo: &Path) -> OpsResult<ProjectPlanning> {
    let (project, pending_changes) = store
        .sqlite
        .project_with_changes(&project.id)
        .map_err(project_error)?;
    Ok(ProjectPlanning {
        project,
        sync_enabled: crate::engine::config::load_config_or_default(Some(repo))
            .pm
            .and_then(|pm| pm.linear_team)
            .is_some(),
        pending_changes,
    })
}

pub(crate) fn load_workflow(
    store: &crate::store::Store,
    wave: &crate::id::WaveId,
    name: &str,
    repo: &Path,
) -> OpsResult<Option<crate::engine::workflow::WorkflowDefinition>> {
    read_workflow_source(store, wave, name, repo)?
        .map(|content| {
            crate::engine::workflow::parse_workflow(name, &content, repo).map_err(project_error)
        })
        .transpose()
}

fn read_workflow_source(
    store: &Store,
    wave: &crate::id::WaveId,
    name: &str,
    repo: &Path,
) -> OpsResult<Option<String>> {
    match store
        .sqlite
        .wave_workflow(wave, name)
        .map_err(project_error)?
    {
        Some(content) => Ok(Some(content)),
        None => crate::engine::workflow::workflow_source(name, repo).map_err(project_error),
    }
}
