use std::collections::BTreeMap;
use std::path::Path;

use crate::ops::{OpsError, OpsResult};
use crate::pm::{PmProject, ProjectContent, ProjectStatus};
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
    import_binding(&store, &wave, &acquisition)?;
    store
        .sqlite
        .bind_project(wave.id(), project_id)
        .map_err(project_error)
}

/// Explicit activation; ordinary planning reads never call this operation.
pub async fn ensure(repo: &Path, name: &str) -> OpsResult<PmProject> {
    let store = super::pm::pm_store().await?;
    let wave = crate::work::wave::ensure_wave_row(&store, repo, name)
        .await
        .map_err(project_error)?;
    store
        .sqlite
        .record_project_activation(wave.id(), crate::journal::current_lf_process_id().as_ref())
        .map_err(project_error)?;
    let acquisition = super::pm::lock_wave_planning(&wave).await?;
    import_binding(&store, &wave, &acquisition)?;
    let id = store
        .sqlite
        .ensure_project(wave.id(), wave.slug())
        .map_err(project_error)?;
    store.sqlite.planning_project(&id).map_err(project_error)
}

pub async fn update_plan(
    repo: &Path,
    wave: Option<&str>,
    content: ProjectContent,
) -> OpsResult<ProjectPlanning> {
    let store = super::pm::pm_store().await?;
    let ambient = std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok();
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        wave,
        ambient.as_deref(),
    )
    .await
    .map_err(project_error)?;
    let _acquisition = super::pm::lock_wave_planning(&wave).await?;
    let selected = current_project(&store, &wave)?;
    let project = resolve_project(&store, repo, &selected.id).await?;
    super::metrics::validate_chapter_targets(&wave, &content.metric_targets)
        .map_err(project_error)?;
    store
        .sqlite
        .update_project_content(&project.id, &content)
        .map_err(project_error)?;
    planning(&store, &project)
}

/// One-time supported import at an explicit mutation boundary. A failed read is
/// never permission to create; the original bytes remain durable evidence.
pub(crate) fn import_binding(store: &Store, wave: &Wave, guard: &PlanningLocks) -> OpsResult<()> {
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
    let projects: Vec<_> = projects
        .into_iter()
        .filter(|project| {
            waves
                .iter()
                .any(|wave| wave.id() == &project.wave_id && wave.repo() == repo)
        })
        .collect();
    if let Some(project) = projects.iter().find(|project| {
        project.id.as_str() == selector
            || project
                .plan
                .linear_id
                .as_ref()
                .is_some_and(|id| id.as_str() == selector)
    }) {
        return Ok(project.clone());
    }
    let mut matches = projects
        .into_iter()
        .filter(|project| project.plan.slug == selector || project.plan.name == selector);
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
    let Some(selector) = selector else {
        return crate::engine::workflow::workflow_catalog(repo).map_err(project_error);
    };
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    let stored: BTreeMap<_, _> = store
        .sqlite
        .wave_workflows(&project.wave_id)
        .map_err(project_error)?
        .into_iter()
        .collect();
    let mut names =
        crate::engine::workflow::available_workflow_names(repo).map_err(project_error)?;
    names.extend(stored.keys().cloned());
    names.sort();
    names.dedup();
    names
        .into_iter()
        .map(|name| {
            let content = stored
                .get(&name)
                .map(String::as_str)
                .or_else(|| crate::engine::workflow::builtin_workflow(&name));
            let (workflow, unavailable) = match content {
                Some(content) => {
                    match crate::engine::workflow::parse_workflow(&name, content, repo) {
                        Ok(workflow) => (Some(workflow), None),
                        Err(error) => (None, Some(error)),
                    }
                }
                None => (
                    None,
                    Some("Import the repository definition with lf wave ensure".into()),
                ),
            };
            Ok(crate::engine::workflow::WorkflowCatalogEntry {
                source: stored.contains_key(&name).then(|| "stored".into()),
                name,
                workflow,
                unavailable,
            })
        })
        .collect()
}

pub async fn workflow_source(repo: &Path, selector: &str, name: &str) -> OpsResult<String> {
    let store = super::pm::pm_store().await?;
    let project = resolve_project(&store, repo, selector).await?;
    read_workflow_source(&store, &project.wave_id, name)?
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
    planning(&store, &project)
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
            None => read_workflow_source(&store, &project.wave_id, name)?
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
    planning(&store, &project)
}

#[derive(Debug, serde::Serialize)]
pub struct ProjectPlanning {
    pub sync: crate::planning::PlanningSyncStatus,
    #[serde(flatten)]
    pub project: PmProject,
}

fn planning(store: &Store, project: &Project) -> OpsResult<ProjectPlanning> {
    let sync = store
        .sqlite
        .project_planning_sync(&project.id)
        .map_err(project_error)?;
    let project = store
        .sqlite
        .planning_project(&project.id)
        .map_err(project_error)?;
    Ok(ProjectPlanning { project, sync })
}

pub(crate) fn load_workflow(
    store: &crate::store::Store,
    wave: &crate::id::WaveId,
    name: &str,
    repo: &Path,
) -> OpsResult<Option<crate::engine::workflow::WorkflowDefinition>> {
    read_workflow_source(store, wave, name)?
        .map(|content| {
            crate::engine::workflow::parse_workflow(name, &content, repo).map_err(project_error)
        })
        .transpose()
}

fn read_workflow_source(
    store: &Store,
    wave: &crate::id::WaveId,
    name: &str,
) -> OpsResult<Option<String>> {
    match store
        .sqlite
        .wave_workflow(wave, name)
        .map_err(project_error)?
    {
        Some(content) => Ok(Some(content)),
        None => Ok(crate::engine::workflow::builtin_workflow(name).map(str::to_string)),
    }
}
