use std::path::Path;

use crate::ops::{OpsError, OpsResult};
use crate::store::planning_sync::PlanningSyncStatus;
use crate::store::{PlanningLocks, Store};
use crate::work::project::Project;
use crate::work::project::ProjectStatus;
use crate::work::wave::Wave;

fn project_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

pub(crate) fn current_project(store: &Store, wave: &Wave) -> OpsResult<Project> {
    let project = store
        .sqlite
        .selected_project(wave.id())
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
pub async fn bind_project(repo: &Path, name: &str, project_id: &str) -> OpsResult<Project> {
    let store = super::planning_sync::planning_store().await?;
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        Some(name),
        None,
    )
    .await
    .map_err(project_error)?;
    let acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    import_binding(&store, &wave, &acquisition)?;
    store
        .sqlite
        .bind_project(wave.id(), project_id)
        .map_err(project_error)
}

/// Explicit activation; ordinary planning reads never call this operation.
pub async fn ensure(repo: &Path, name: &str) -> OpsResult<Project> {
    let store = super::planning_sync::planning_store().await?;
    let wave = crate::work::wave::ensure_wave_row(&store, repo, name)
        .await
        .map_err(project_error)?;
    store
        .sqlite
        .record_project_activation(wave.id(), crate::journal::current_lf_process_id().as_ref())
        .map_err(project_error)?;
    let acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    import_binding(&store, &wave, &acquisition)?;
    let id = store
        .sqlite
        .ensure_project(wave.id(), wave.slug())
        .map_err(project_error)?;
    store
        .sqlite
        .project(&id)
        .map_err(project_error)?
        .ok_or_else(|| project_error("selected Project is missing"))
}

pub async fn update_plan(
    repo: &Path,
    wave: Option<&str>,
    krs: &[crate::work::project::ProjectKR],
    metric_targets: &[crate::work::wave::metrics::MetricTarget],
) -> OpsResult<(Project, PlanningSyncStatus)> {
    let store = super::planning_sync::planning_store().await?;
    let ambient = std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok();
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(&store),
        Some(repo),
        wave,
        ambient.as_deref(),
    )
    .await
    .map_err(project_error)?;
    let _acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    let project = current_project(&store, &wave)?;
    super::metrics::validate_chapter_targets(&wave, metric_targets).map_err(project_error)?;
    store
        .sqlite
        .update_project_content(&project.id, krs, metric_targets)
        .map_err(project_error)?;
    drop(_acquisition);
    super::planning_sync::sync_after_save(&store, wave.repo()).await;
    read_saved(&store, &project.id)
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
    if let Ok(id) = crate::durable::ProjectId::parse(selector) {
        let project = store
            .get_project_by_project(id.as_str())
            .await
            .map_err(project_error)?
            .ok_or_else(|| project_error(format!("Project {selector:?} not found")))?;
        let wave = store
            .get_wave(&project.wave_id)
            .await
            .map_err(project_error)?;
        return if wave.is_some_and(|wave| wave.repo() == repo) {
            Ok(project)
        } else {
            Err(project_error(format!(
                "Project {selector:?} not found in this repository"
            )))
        };
    }
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
        project
            .linear_id
            .as_ref()
            .is_some_and(|id| id.as_str() == crate::id::canonical_uuid(selector))
    }) {
        return Ok(project.clone());
    }
    let mut matches = projects
        .into_iter()
        .filter(|project| project.slug == selector || project.name == selector);
    let project = matches
        .next()
        .ok_or_else(|| project_error(format!("Project {selector:?} not found")))?;
    if matches.next().is_some() {
        return Err(project_error("Project name is ambiguous; use its ID"));
    }
    Ok(project)
}

/// Current chapter selection and exact historical lookup are distinct addresses.
#[derive(Debug, Clone, Copy)]
pub enum PlanTarget<'a> {
    Wave(Option<&'a str>),
    Project(&'a str),
}

/// Resolve on the operation's store; never ensure a Project or start work.
async fn resolve_plan(store: &Store, repo: &Path, target: PlanTarget<'_>) -> OpsResult<Project> {
    let selector = match target {
        PlanTarget::Project(project) => return resolve_project(store, repo, project).await,
        PlanTarget::Wave(wave) => wave,
    };
    let wave = crate::work::wave::context::resolve_managed_wave(
        Some(store),
        Some(repo),
        selector,
        std::env::var("LF_WAVE_ID").ok().as_deref(),
    )
    .await
    .map_err(project_error)?;
    let selected = store
        .sqlite
        .selected_project(wave.id())
        .map_err(project_error)?
        .ok_or_else(|| project_error(format!("Wave {} has no configured Project", wave.slug())))?;
    resolve_project(store, repo, selected.id.as_str()).await
}

fn read_store() -> OpsResult<Store> {
    #[cfg(test)]
    if let Ok(store) = super::linear::LINEAR_TEST_CONTEXT
        .try_with(|ctx| Store::from_sqlite_for_test(ctx.store.sqlite.clone()))
    {
        return Ok(store);
    }
    crate::store::read_existing_registry()
        .map_err(project_error)?
        .ok_or_else(|| project_error("Planning unavailable: local registry is absent"))
}

async fn graph_wave(store: &Store, repo: &Path, wave: Option<&str>) -> OpsResult<Wave> {
    crate::work::wave::context::resolve_managed_wave(
        Some(store),
        Some(repo),
        wave,
        std::env::var("LF_WAVE_ID").ok().as_deref(),
    )
    .await
    .map_err(project_error)
}

pub async fn edit(
    repo: &Path,
    target: PlanTarget<'_>,
    name: Option<&str>,
    summary: Option<&str>,
    description: Option<&str>,
) -> OpsResult<(Project, PlanningSyncStatus)> {
    if name.is_none() && summary.is_none() && description.is_none() {
        return Err(project_error(
            "wave edit-plan requires --name, --summary or --description",
        ));
    }
    let store = super::planning_sync::planning_store().await?;
    let project = resolve_plan(&store, repo, target).await?;
    let wave = store
        .get_wave(&project.wave_id)
        .await
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project Wave is unavailable"))?;
    let _acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    store
        .sqlite
        .edit_project(&project.id, name, summary, description)
        .map_err(project_error)?;
    drop(_acquisition);
    super::planning_sync::sync_after_save(&store, wave.repo()).await;
    read_saved(&store, &project.id)
}

/// The graph a scope's new Tasks take up: the Wave's own selection, if any,
/// and the default resolved through Wave, repository `graph:` and `code`.
/// The repository scope has no Wave.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WaveGraph {
    pub wave_id: Option<crate::id::WaveId>,
    pub wave: Option<String>,
    pub graph: Option<String>,
    pub default: GraphDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GraphDefault {
    pub name: String,
    /// `wave`, `repository` or `builtin`
    pub source: String,
}

/// `selector: None` reads the ambient Wave; `in_wave: false` the repository.
pub async fn wave_graph(
    repo: &Path,
    selector: Option<&str>,
    in_wave: bool,
) -> OpsResult<WaveGraph> {
    let store = read_store()?;
    let wave = if in_wave {
        Some(graph_wave(&store, repo, selector).await?)
    } else {
        None
    };
    let main = crate::git::worktrees::main_repo_root(repo).map_err(project_error)?;
    let (name, source) =
        super::task::task_graph_default(&store, wave.as_ref().map(|wave| wave.id()), &main)?;
    Ok(WaveGraph {
        graph: wave
            .as_ref()
            .map(|wave| store.sqlite.selected_wave_graph(wave.id()))
            .transpose()
            .map_err(project_error)?
            .flatten(),
        wave_id: wave.as_ref().map(|wave| wave.id().clone()),
        wave: wave.as_ref().map(|wave| wave.slug().into()),
        default: GraphDefault {
            name,
            source: source.into(),
        },
    })
}

pub async fn set_wave_graph(repo: &Path, selector: Option<&str>, name: &str) -> OpsResult<String> {
    let store = super::planning_sync::planning_store().await?;
    let wave = graph_wave(&store, repo, selector).await?;
    if crate::task_graph::CapturedGraph::load(name, repo)
        .map_err(project_error)?
        .is_none()
    {
        return Err(project_error(crate::task_graph::missing_graph(name)));
    }
    let _acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    store
        .sqlite
        .select_wave_graph(wave.id(), name)
        .map_err(project_error)?;
    Ok(wave.slug().into())
}

fn read_saved(
    store: &Store,
    id: &crate::durable::ProjectId,
) -> OpsResult<(Project, PlanningSyncStatus)> {
    let project = store
        .sqlite
        .project(id)
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project is unavailable"))?;
    let sync = store
        .sqlite
        .project_planning_sync(id)
        .map_err(project_error)?;
    Ok((project, sync))
}
