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

/// Select by the shared binding, independently of names and other Project statuses.
pub(crate) fn select_project<'a>(
    store: &Store,
    wave: &Wave,
    projects: &'a [PmProject],
) -> OpsResult<&'a PmProject> {
    let selected = read_project_binding(&store.sqlite, wave.id())
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

pub(crate) fn current_project(store: &Store, wave: &Wave) -> OpsResult<PmProject> {
    if store
        .sqlite
        .personal_wave_definition(wave.id())
        .map_err(project_error)?
        .is_some()
    {
        let projects = store
            .sqlite
            .list_projects(Some(wave.id()))
            .map_err(project_error)?
            .into_iter()
            .map(super::task::local_project_item)
            .collect::<OpsResult<Vec<_>>>()?;
        return select_project(store, wave, &projects).cloned();
    }
    let projects = store
        .sqlite
        .accepted_projects(wave.id())
        .map_err(project_error)?;
    select_project(store, wave, &projects).cloned()
}

pub(crate) async fn resolve_project_for_task(
    store: &Store,
    wave: &Wave,
    project_id: &str,
) -> OpsResult<Project> {
    let current = current_project(store, wave)?;
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
        return super::task::local_project_item(
            store
                .sqlite
                .ensure_personal_project(&repo.to_string(), name)
                .map_err(project_error)?,
        );
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
        return super::task::local_project_item(
            store
                .sqlite
                .ensure_personal_project(wave.repo(), name)
                .map_err(project_error)?,
        );
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

pub fn update_plan(repo: &Path, wave: Option<&str>, content: ProjectContent) -> OpsResult<()> {
    let wave = crate::work::wave::context::resolve_managed_wave_sync(Some(repo), wave)
        .map_err(project_error)?;
    tokio::runtime::Runtime::new()
        .map_err(project_error)?
        .block_on(write_plan(repo, &wave, content))
}

pub(crate) async fn write_plan(repo: &Path, wave: &Wave, content: ProjectContent) -> OpsResult<()> {
    let store = super::pm::pm_store().await?;
    let acquisition = super::pm::lock_wave_planning(wave).await?;
    if store
        .sqlite
        .personal_wave_definition(wave.id())
        .map_err(project_error)?
        .is_some()
    {
        let project = current_project(&store, wave)?;
        return store
            .sqlite
            .update_local_project_content(
                &crate::durable::ProjectId::parse(&project.id).map_err(project_error)?,
                &content,
                None,
            )
            .map_err(project_error);
    }
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    let projects = super::pm::checked_projects_with_store(repo, &ctx, wave.slug(), &store).await?;
    let project = select_project(&store, wave, &projects)?;
    let provider = ctx
        .client
        .project_ownership(&project.id)
        .await
        .map_err(project_error)?;
    content.validate().map_err(project_error)?;
    super::metrics::validate_chapter_targets(wave, &content.metric_targets)
        .map_err(project_error)?;
    ctx.client
        .update_project(&provider.id, &provider.name, &content)
        .await
        .map_err(project_error)?;
    super::pm::refresh_pm_snapshot_locked(repo, wave, &ctx, &store, acquisition).await?;
    Ok(())
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
/// A read uses accepted planning; a write refreshes provider facts under its lock.
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
            .personal_workflows(&project.wave_id)
            .map_err(project_error)?
        {
            let name = format!("personal:{name}");
            let (workflow, unavailable) =
                match crate::engine::workflow::parse_workflow(&name, &content, repo) {
                    Ok(workflow) => (Some(workflow), None),
                    Err(error) => (None, Some(error)),
                };
            entries.push(crate::engine::workflow::WorkflowCatalogEntry {
                name,
                source: Some("personal".into()),
                workflow,
                unavailable,
            });
        }
    }
    Ok(entries)
}

pub async fn workflow_source(repo: &Path, selector: &str, name: &str) -> OpsResult<String> {
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    let content = if let Some(name) = name.strip_prefix("personal:") {
        store
            .sqlite
            .personal_workflow(&project.wave_id, name)
            .map_err(project_error)?
    } else {
        crate::engine::workflow::workflow_source(name, repo).map_err(project_error)?
    };
    content.ok_or_else(|| project_error(format!("Workflow {name:?} is unavailable")))
}

pub async fn edit(
    repo: &Path,
    selector: &str,
    name: Option<&str>,
    summary: Option<&str>,
) -> OpsResult<()> {
    if name.is_none() && summary.is_none() {
        return Err(project_error("project edit requires --name or --summary"));
    }
    if name.is_some_and(|name| name.trim().is_empty()) {
        return Err(project_error("Project name cannot be empty"));
    }
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    let wave = store
        .get_wave(&project.wave_id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project Wave is unavailable"))?;
    let acquisition = super::pm::lock_wave_planning(&wave).await?;
    if store
        .sqlite
        .project_planning_authority(&project.id)
        .map_err(project_error)?
        == crate::planning::PlanningAuthority::Local
    {
        return store
            .sqlite
            .edit_local_project(&project.id, name, summary)
            .map_err(project_error);
    }
    let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
    let id = project.plan.linear_id()?.as_str();
    let current = require_project(&ctx, id).await?;
    accept_project(
        &store,
        &wave,
        &ctx,
        current,
        time::OffsetDateTime::now_utc().unix_timestamp(),
        &acquisition,
    )
    .await?;
    ctx.client
        .edit_project(id, name, summary)
        .await
        .map_err(project_error)?;
    let edited = require_project(&ctx, id).await?;
    if name.is_some_and(|name| name != edited.name)
        || summary.is_some_and(|summary| summary != edited.summary)
    {
        return Err(project_error("Project edit was sent but readback differs; inspect its current values before retrying"));
    }
    accept_project(
        &store,
        &wave,
        &ctx,
        edited,
        time::OffsetDateTime::now_utc().unix_timestamp(),
        &acquisition,
    )
    .await?;
    Ok(())
}

pub async fn workflow(
    repo: &Path,
    selector: &str,
    selection: Option<&str>,
    file: Option<&Path>,
) -> OpsResult<PmProject> {
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    let wave = store
        .get_wave(&project.wave_id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project Wave is unavailable"))?;
    if store
        .sqlite
        .project_planning_authority(&project.id)
        .map_err(project_error)?
        == crate::planning::PlanningAuthority::Local
    {
        let _guard = super::pm::lock_wave_planning(&wave).await?;
        let mut current = super::task::local_project_item(
            store
                .sqlite
                .project(&project.id)
                .map_err(project_error)?
                .ok_or_else(|| project_error("Project disappeared"))?,
        )?;
        if let Some(name) = selection {
            let definition = file
                .map(std::fs::read_to_string)
                .transpose()
                .map_err(project_error)?;
            if let Some(content) = definition.as_deref() {
                name.strip_prefix("personal:")
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| project_error("a stored Workflow uses personal:<name>"))?;
                crate::engine::workflow::parse_workflow(name, content, repo)
                    .map_err(project_error)?;
            } else {
                load_workflow(&store, wave.id(), name, repo)?
                    .ok_or_else(|| project_error(format!("Workflow {name:?} not found")))?;
            }
            store
                .sqlite
                .update_local_project_content(
                    &project.id,
                    &ProjectContent {
                        workflow: name.into(),
                        krs: current.krs.clone(),
                        metric_targets: current.metric_targets.clone(),
                    },
                    definition.as_deref(),
                )
                .map_err(project_error)?;
            current.workflow = name.into();
        }
        return Ok(current);
    }
    if file.is_some() || selection.is_some_and(|name| name.starts_with("personal:")) {
        return Err(project_error(
            "personal Workflow definitions belong to a personal Wave",
        ));
    }
    if let Some(name) = selection {
        crate::engine::workflow::load_workflow(name, repo)
            .map_err(project_error)?
            .ok_or_else(|| project_error(format!("Workflow {name:?} not found")))?;
        super::pm::require_planning_home(&store, &wave).await?;
        let acquisition = super::pm::lock_wave_planning(&wave).await?;
        let ctx = super::pm::resolve_context(repo, wave.slug()).await?;
        let provider = require_project(&ctx, project.plan.linear_id()?.as_str()).await?;
        let content = ProjectContent {
            workflow: name.to_string(),
            metric_targets: provider.metric_targets.clone(),
            krs: provider.krs.clone(),
        };
        ctx.client
            .update_project(&provider.id, &provider.name, &content)
            .await
            .map_err(project_error)?;
        super::pm::refresh_pm_snapshot_locked(repo, &wave, &ctx, &store, acquisition).await?;
    }
    store
        .sqlite
        .accepted_projects(wave.id())
        .map_err(project_error)?
        .into_iter()
        .find(|p| {
            project
                .plan
                .linear_id
                .as_ref()
                .is_some_and(|id| id.as_str() == p.id)
        })
        .ok_or_else(|| project_error("Project planning is unavailable; sync its Wave"))
}

pub(crate) fn load_workflow(
    store: &crate::store::Store,
    wave: &crate::id::WaveId,
    name: &str,
    repo: &Path,
) -> OpsResult<Option<crate::engine::workflow::WorkflowDefinition>> {
    if let Some(private) = name.strip_prefix("personal:") {
        return store
            .sqlite
            .personal_workflow(wave, private)
            .map_err(project_error)?
            .map(|content| {
                crate::engine::workflow::parse_workflow(name, &content, repo).map_err(project_error)
            })
            .transpose();
    }
    crate::engine::workflow::load_workflow(name, repo).map_err(project_error)
}
