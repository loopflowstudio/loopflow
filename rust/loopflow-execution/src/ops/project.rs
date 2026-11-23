use std::path::Path;

use crate::ops::{OpsError, OpsResult};
use crate::store::Store;
use crate::work::wave::Wave;
use loopflow_storage::planning_sync::TrackerSync;
use loopflow_storage::work::project::Project;
use loopflow_storage::work::project::ProjectStatus;

fn project_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

pub fn current_project(store: &Store, wave: &Wave) -> OpsResult<Project> {
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

/// Select an existing non-terminal Project as the Wave's current Project.
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
    let _acquisition = super::planning_sync::lock_wave_planning(&wave).await?;
    store
        .sqlite
        .bind_project(wave.id(), project_id)
        .map_err(project_error)
}

pub async fn update_project(
    repo: &Path,
    wave: Option<&str>,
    krs: &[loopflow_storage::work::project::ProjectKR],
    metric_targets: &[crate::work::wave::metrics::MetricTarget],
) -> OpsResult<(Project, TrackerSync)> {
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
    super::metrics::validate_project_targets(&wave, metric_targets).map_err(project_error)?;
    store
        .sqlite
        .update_project_content(&project.id, krs, metric_targets)
        .map_err(project_error)?;
    drop(_acquisition);
    super::planning_sync::sync_after_save(
        &store,
        wave.repo(),
        &loopflow_storage::sqlite::tracker_actions::ActionOwner::Project(project.id.clone()),
    )
    .await;
    read_saved(&store, &project.id)
}

/// Address a retained Project by durable ID, provider ID, or unique slug/name.
async fn resolve_project(store: &Store, repo: &Path, selector: &str) -> OpsResult<Project> {
    let repo = crate::repository::discover(repo)
        .map_err(project_error)?
        .to_string();
    if let Ok(id) = loopflow_model::durable::ProjectId::parse(selector) {
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
            .tracker_id
            .as_ref()
            .is_some_and(|id| id.as_str() == loopflow_model::id::canonical_uuid(selector))
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

/// Current Project selection and exact historical lookup are distinct addresses.
#[derive(Debug, Clone, Copy)]
pub enum ProjectTarget<'a> {
    Wave(Option<&'a str>),
    Project(&'a str),
}

/// Resolve on the operation's store; never ensure a Project or start work.
async fn resolve_project_target(
    store: &Store,
    repo: &Path,
    target: ProjectTarget<'_>,
) -> OpsResult<Project> {
    let selector = match target {
        ProjectTarget::Project(project) => return resolve_project(store, repo, project).await,
        ProjectTarget::Wave(wave) => wave,
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
    #[cfg(any(test, feature = "test-support"))]
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
    if let Some(id) = wave.and_then(|value| value.trim().parse().ok()) {
        return super::task::existing_creation_wave(store, repo, &id).await;
    }
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
    target: ProjectTarget<'_>,
    name: Option<&str>,
    summary: Option<&str>,
    description: Option<&str>,
) -> OpsResult<(Project, TrackerSync)> {
    if name.is_none() && summary.is_none() && description.is_none() {
        return Err(project_error(
            "wave edit-project requires --name, --summary or --description",
        ));
    }
    let store = super::planning_sync::planning_store().await?;
    let project = resolve_project_target(&store, repo, target).await?;
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
    super::planning_sync::sync_after_save(
        &store,
        wave.repo(),
        &loopflow_storage::sqlite::tracker_actions::ActionOwner::Project(project.id.clone()),
    )
    .await;
    read_saved(&store, &project.id)
}

/// The graph a scope's new Tasks take up, resolved through your `lf set wave.graph`,
/// the Wave's and the repository's `graph:`, then `code`.
/// The repository scope has no Wave.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct WaveGraph {
    pub wave_id: Option<loopflow_model::id::WaveId>,
    pub wave: Option<String>,
    pub default: GraphDefault,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct GraphDefault {
    pub name: String,
    pub source: crate::config::ConfigSource,
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
    let (name, source) =
        crate::ops::task::scope_graph_default(repo, wave.as_ref().map(|wave| wave.slug()))?;
    Ok(WaveGraph {
        wave_id: wave.as_ref().map(|wave| wave.id().clone()),
        wave: wave.as_ref().map(|wave| wave.slug().into()),
        default: GraphDefault { name, source },
    })
}

fn read_saved(
    store: &Store,
    id: &loopflow_model::durable::ProjectId,
) -> OpsResult<(Project, TrackerSync)> {
    let project = store
        .sqlite
        .project(id)
        .map_err(project_error)?
        .ok_or_else(|| project_error("Project is unavailable"))?;
    let sync = store.project_tracker_sync(id).map_err(project_error)?;
    Ok((project, sync))
}

/// `wave`'s Project readiness. An activation whose lf died without writing
/// its end reads `unknown`, not pending.
pub fn readiness(
    store: &loopflow_storage::sqlite::SqliteStore,
    wave: &loopflow_model::id::WaveId,
) -> crate::store::StoreResult<loopflow_storage::sqlite::ProjectReadiness> {
    store.project_readiness(wave, crate::process::os_process::observed_result)
}

#[cfg(test)]
mod readiness_tests {
    use crate::work::wave::Wave;
    use loopflow_model::id::{LfProcessId, TraceId, WaveId};
    use loopflow_storage::graph_process::ExecutionStart;

    #[test]
    fn an_activation_whose_lf_died_unended_reads_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let store = loopflow_storage::test_fixture::open(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "a".into(), "/repo".into());
        loopflow_storage::test_fixture::wave(&store, &wave);
        // The journal's row for the activating lf, which `waves` still
        // references until its rebuild drops the foreign key.
        let process = LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                rusqlite::params![process, TraceId::new()],
            )
            .unwrap();
        let activate = |pid: u32| {
            let start = ExecutionStart {
                started_at: 1,
                pid: Some(pid),
                os_started_at: crate::process::os_process::process_started_at(pid).unwrap(),
                caller_session: None,
            };
            store
                .record_project_activation(wave.id(), &process, &start)
                .unwrap();
            super::readiness(&store, wave.id())
                .unwrap()
                .activation
                .unwrap()
        };
        assert_eq!(activate(std::process::id()).outcome, None);

        let mut child = std::process::Command::new("sleep")
            .arg("5")
            .spawn()
            .unwrap();
        let pid = child.id();
        let start = ExecutionStart {
            started_at: 1,
            pid: Some(pid),
            os_started_at: crate::process::os_process::process_started_at(pid).unwrap(),
            caller_session: None,
        };
        child.kill().unwrap();
        child.wait().unwrap();
        store
            .record_project_activation(wave.id(), &process, &start)
            .unwrap();
        let unknown = super::readiness(&store, wave.id())
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(unknown.completed_at, None);
        assert_eq!(unknown.outcome.as_deref(), Some("unknown"));
    }
}
