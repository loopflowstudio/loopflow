use std::path::{Path, PathBuf};

use crate::durable::{render_steers, Steer, TaskId, WorkRef};
use crate::engine::process::{
    current_home_execution_context, pin_control_binary, start_lf_session_with_env,
};
use crate::id::WaveId;
use crate::planning::ProjectPlan;
use crate::store::SharedStore;

use crate::work::task::{Task, TaskPr};
use crate::work::wave::Wave;

use super::{OpsError, OpsResult};

pub(crate) const TASK_ACCOUNT_ID_ENV: &str = "LF_TASK_ACCOUNT_ID";

#[derive(Debug, Clone)]
pub struct WorkBinding {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub wave_name: String,
    pub subjects: Vec<String>,
    pub cwd: PathBuf,
    pub context: String,
    pub agent: Option<String>,
}

pub(crate) fn render_task_context(
    task: &Task,
    project: &ProjectPlan,
    pr: &TaskPr,
    wave_name: &str,
    steers: &[Steer],
) -> String {
    let placement = pr
        .parent_pr_id
        .as_ref()
        .map(|parent| format!("Stack parent PR: {parent} (land the parent first)"))
        .unwrap_or_else(|| "Stack parent PR: none (rooted on main)".to_string());
    format!(
        "Linear Task {identifier}: {title}\n\n{description}\n\nChapter plan: {project} (source {project_id})\n{project_context}\n\n{direction}\n\nTask directive snapshot synced at: {task_snapshot_synced_at}\nChapter plan snapshot synced at: {project_snapshot_synced_at}\nWave: {wave}\nTask Work: {task_id}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}",
        identifier = task.plan.identifier,
        title = task.plan.title,
        description = task.plan.description,
        project = project.name,
        project_id = project.id.as_str(),
        project_context = project.prompt_context,
        direction = render_steers(steers),
        task_snapshot_synced_at = task.plan.pm_snapshot_synced_at,
        project_snapshot_synced_at = project.pm_snapshot_synced_at,
        wave = wave_name,
        task_id = task.id,
        worktree = task.worktree.display(),
        pr_sequence = pr.sequence,
        pr_branch = pr.branch,
        base_commit = pr.base_commit,
        placement = placement,
    )
}

pub(crate) fn render_wave_context(
    resident_repo: &Path,
    origin_repo: &Path,
    wave: &str,
    metric_context: &str,
) -> String {
    let memory =
        crate::work::wave::context::gather_wave_memory_from(origin_repo, resident_repo, wave)
            .unwrap_or_default();
    let goal = match crate::engine::load_goal(wave, resident_repo) {
        Ok(goal) => {
            let context = crate::engine::GoalRenderContext {
                flows: crate::engine::available_flow_names(origin_repo),
                memory,
            };
            crate::engine::render_goal(&goal, &context)
        }
        Err(_) => {
            let memory = if memory.trim().is_empty() {
                "(memory is empty)".to_string()
            } else {
                memory
            };
            format!(
                "You are the agent of the '{wave}' wave. Drive the wave's goal forward.\n\nCurrent memory:\n{memory}"
            )
        }
    };
    format!(
        "{goal}\n\n{metric_context}\n\n<lf:wave-executive-loop>\n1. What is most important?\n2. What signals are arriving?\n3. What works?\n4. What does not?\n5. What is the current strategy?\n6. How should strategy adjust?\n\nTreat metrics as evidence, never as automatic KR completion or a composite Wave score.\n</lf:wave-executive-loop>"
    )
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkSelection<'a> {
    pub task: Option<&'a str>,
    pub wave: Option<&'a str>,
}

pub async fn resolve_work_binding(
    store: &SharedStore,
    repo: &Path,
    selector: &str,
) -> OpsResult<WorkBinding> {
    let (kind, value) = selector.split_once(':').ok_or_else(|| {
        run_error(format!(
            "invalid Work selector {selector:?}; expected task:<selector> or wave:<selector>"
        ))
    })?;
    let selection = match kind {
        "task" => WorkSelection {
            task: Some(value),
            ..WorkSelection::default()
        },
        "wave" => WorkSelection {
            wave: Some(value),
            ..WorkSelection::default()
        },
        _ => {
            return Err(run_error(format!(
                "invalid Work selector kind {kind:?}; expected task or wave"
            )))
        }
    };
    resolve_work_selection(store, repo, selection).await
}

pub async fn resolve_work_selection(
    store: &SharedStore,
    repo: &Path,
    selection: WorkSelection<'_>,
) -> OpsResult<WorkBinding> {
    for (kind, value) in [("task", selection.task), ("wave", selection.wave)] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(run_error(format!("{kind} selector cannot be empty")));
        }
    }

    let selected_wave = match selection.wave {
        Some(value) => Some(resolve_wave(store, repo, value.trim()).await?),
        None => None,
    };

    if let Some(value) = selection.task {
        let value = value.trim();
        let task = if let Ok(id) = TaskId::parse(value) {
            store.get_task(&id).await.map_err(run_error)?
        } else {
            store.get_task_by_issue(value).await.map_err(run_error)?
        }
        .ok_or_else(|| run_error(format!("Task {value:?} is not registered")))?;
        let wave = store
            .get_wave(&task.wave_id)
            .await
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Task {} has no owning Wave", task.id)))?;
        let project = store
            .get_project(&task.project_id)
            .await
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Task {} has no owning Project", task.id)))?;
        if let Some(selected_wave) = &selected_wave {
            require_wave_match(
                &wave,
                selected_wave,
                &format!("Task {}", task.plan.identifier),
            )?;
        }
        let work = WorkRef::Task(task.id.clone());
        let steers = Vec::new();
        let pr = store
            .active_task_pr(&task.id)
            .await
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Task {} has no active PR", task.id)))?;
        let context = render_task_context(&task, &project.plan, &pr, wave.name(), &steers);
        let cwd = if crate::engine::git::origin_branch(repo)
            .ok()
            .flatten()
            .as_deref()
            == Some(pr.branch.as_str())
        {
            crate::engine::git::worktree_root(repo).unwrap_or_else(|_| repo.to_path_buf())
        } else {
            task.worktree.clone()
        };
        return Ok(WorkBinding {
            subjects: vec![
                format!("wave:{}", wave.name()),
                format!("project:{}", project.plan.slug),
                format!("task:{}", task.plan.identifier),
            ],
            work,
            wave_id: task.wave_id,
            wave_name: wave.name().to_string(),
            cwd,
            context,
            agent: Some(
                crate::engine::config::load_config_or_default(Some(&task.worktree))
                    .agent()
                    .to_string(),
            ),
        });
    }

    if let Some(wave) = selected_wave {
        let metric_context = crate::ops::metrics::metric_prompt_section(
            "metric-portfolio",
            crate::ops::metrics::stored_wave_metric_portfolio(
                store,
                &wave,
                time::OffsetDateTime::now_utc(),
            )
            .await,
        );
        let cwd = PathBuf::from(wave.repo());
        let context = render_wave_context(&cwd, &cwd, wave.name(), &metric_context);
        return Ok(WorkBinding {
            subjects: vec![format!("wave:{}", wave.name())],
            work: WorkRef::Wave(wave.id().clone()),
            wave_id: wave.id().clone(),
            wave_name: wave.name().to_string(),
            cwd,
            context,
            agent: None,
        });
    }

    Err(run_error("select a Task or Wave"))
}

async fn resolve_wave(store: &SharedStore, repo: &Path, value: &str) -> OpsResult<Wave> {
    let wave = if let Ok(id) = WaveId::parse(value) {
        store.get_wave(&id).await.map_err(run_error)?
    } else {
        let locator = crate::work::wave::WaveLocator::discover(repo, value).map_err(run_error)?;
        store.get_wave_at(&locator).await.map_err(run_error)?
    };
    wave.ok_or_else(|| run_error(format!("Wave {value:?} is not registered")))
}

fn require_wave_match(actual: &Wave, requested: &Wave, subject: &str) -> OpsResult<()> {
    if actual.id() == requested.id() {
        return Ok(());
    }
    Err(run_error(format!(
        "{subject} belongs to Wave {}, not {}",
        actual.name(),
        requested.name()
    )))
}

fn run_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[derive(Debug)]
pub(crate) struct TaskWorkerLaunch {
    pub task_id: TaskId,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub environment: Vec<(String, String)>,
}

pub(crate) async fn launch_task_worker(request: TaskWorkerLaunch) -> OpsResult<()> {
    let environment = request.environment.clone();
    start_work_session(&request, environment).await
}

async fn start_work_session(
    request: &TaskWorkerLaunch,
    mut environment: Vec<(String, String)>,
) -> OpsResult<()> {
    let execution = current_home_execution_context()
        .map_err(|error| OpsError::Message(format!("cannot resolve current lf binary: {error}")))?;
    let control_bin = pin_control_binary(&execution.lf_bin)
        .to_string_lossy()
        .to_string();
    let argv = vec![
        control_bin.clone(),
        "task".to_string(),
        "__worker".to_string(),
        request.task_id.to_string(),
    ];
    environment.extend([
        (
            crate::work::wave::context::WAVE_ID_ENV.to_string(),
            request.wave_id.as_str().to_string(),
        ),
        (crate::store::CONTROL_BIN_ENV.to_string(), control_bin),
        (
            crate::store::CONTROL_DB_PATH_ENV.to_string(),
            execution.db_path.to_string_lossy().to_string(),
        ),
        (
            crate::store::CONTROL_HOME_ENV.to_string(),
            execution.lf_home.to_string_lossy().to_string(),
        ),
    ]);
    if let Some(switch_id) = std::env::var_os(crate::machine_install::INSTALL_SWITCH_ENV)
        .filter(|value| !value.is_empty())
    {
        environment.push((
            crate::machine_install::INSTALL_SWITCH_ENV.to_string(),
            switch_id.to_string_lossy().into_owned(),
        ));
    }
    let environment = environment
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    start_lf_session_with_env(&request.tmux_name, &request.cwd, &argv, &environment)
        .await
        .map_err(|error| OpsError::Message(format!("failed to launch Task worker: {error}")))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::OffsetDateTime;

    use super::{resolve_work_binding, resolve_work_selection, WorkSelection};
    use crate::durable::{ProjectId, TaskId, WorkRef};
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::pm::{PmKr, PmProject, PmSnapshot, ProjectFlowPlan};
    use crate::store::SharedStore;
    use crate::store::{PmSnapshotRow, StorageConfig};
    use crate::work::project::Project;
    use crate::work::task::{Observation, PmWritebackState, Task, TaskPr, TaskPrId};
    use crate::work::wave::Wave;
    use std::path::PathBuf;

    async fn test_store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("registry.db"),
        ))
        .await
        .unwrap();
        (directory, Arc::new(store))
    }

    fn project(wave: &Wave, slug: &str, planning_id: &str) -> Project {
        let now = OffsetDateTime::now_utc();
        Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new(planning_id).unwrap(),
                slug: slug.to_string(),
                name: slug.to_string(),
                prompt_context: "Ship the requested behavior.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
    }

    async fn task(store: &SharedStore, wave: &Wave, project: &Project, worktree: PathBuf) -> Task {
        let now = OffsetDateTime::now_utc();
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("runtime-research").unwrap(),
                identifier: "LOO-267".to_string(),
                title: "Research the runtime".to_string(),
                description: "Compare independent findings.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree,
            workspace_slug: "runtime-research".to_string(),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: task.workspace_slug.clone(),
            branch: "jack/runtime-research".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task(&task, &pr).await.unwrap();
        task
    }

    #[tokio::test]
    async fn project_selectors_are_not_an_execution_surface() {
        let (directory, store) = test_store().await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        store
            .create_project(&project(&wave, "shared", "project-one"))
            .await
            .unwrap();
        store
            .create_project(&project(&wave, "shared", "project-two"))
            .await
            .unwrap();

        let error = resolve_work_binding(&store, directory.path(), "project:shared")
            .await
            .expect_err("chapters are not independent operators");

        assert!(error.to_string().contains("expected task or wave"));
    }

    #[tokio::test]
    async fn controller_free_task_supports_multiple_independent_bindings() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        let worktree = directory.path().join("repo.runtime-research");
        std::fs::create_dir_all(&worktree).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = project(&wave, "loopflow-api", "project-api");
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, worktree.clone()).await;

        store
            .apply_linear_comment(
                &task.id,
                "comment-1".into(),
                "ADVANCER ONLY".into(),
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap();
        let (runtime, prompts) = tokio::join!(
            resolve_work_binding(&store, &repo, "task:LOO-267"),
            resolve_work_binding(&store, &repo, "task:LOO-267")
        );

        let runtime = runtime.unwrap();
        let prompts = prompts.unwrap();
        assert_eq!(runtime.cwd, worktree);
        assert_eq!(
            runtime.subjects,
            ["wave:runtime", "project:loopflow-api", "task:LOO-267"]
        );
        assert_eq!(prompts.work, WorkRef::Task(task.id.clone()));
        assert!(!runtime.context.contains("ADVANCER ONLY"));
        assert!(!prompts.context.contains("ADVANCER ONLY"));
        assert_eq!(store.task_steers(&task.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn task_run_drill_includes_named_workers_and_opaque_helpers() {
        let (directory, store) = test_store().await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let mut project = project(&wave, "desktop", "project-desktop");
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, directory.path().join("workspace")).await;
        // Historical Run labels remain unchanged when the Project is renamed.
        project.plan.slug = "desktop-renamed".into();
        store.update_project(&project).await.unwrap();
        let catalog = crate::lf::commands::work_catalog::WorkCatalog::load_at(
            &directory.path().join("registry.db"),
        )
        .unwrap();
        let home = tempfile::tempdir().unwrap();
        for subject in [
            format!("task:{}", task.plan.identifier),
            format!("task:{}", task.id),
            "task:LOO-999".into(),
        ] {
            let mut subjects = vec![crate::run_record::SubjectAttribution::declared(
                subject.clone(),
            )];
            if subject == format!("task:{}", task.plan.identifier) {
                subjects.extend(["wave:runtime", "project:desktop"].map(|selector| {
                    crate::run_record::SubjectAttribution::declared(selector.into())
                }));
            }
            let capture = crate::run_record::CaptureHandle::begin_at(
                home.path(),
                crate::run_record::RunSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: directory.path().to_path_buf(),
                    repo: None,
                    worktree: None,
                    skill: Some("implement".into()),
                    subjects,
                    flow: crate::run_record::RunFlowMembership::Independent,
                },
            )
            .unwrap();
            capture.finish("completed").unwrap();
        }
        for selector in [
            task.plan.identifier.as_str(),
            task.id.as_str(),
            task.plan.id.as_str(),
        ] {
            let runs = crate::lf::commands::runs::collect_runs_started_since_at(
                home.path(),
                crate::lf::commands::WorkFilter {
                    task: Some(selector),
                    project: Some(project.id.as_str()),
                    wave: Some(wave.name()),
                },
                0,
                &catalog,
            )
            .unwrap();
            assert_eq!(runs.len(), 2);
            for run in &runs {
                assert_eq!(
                    catalog.resolve_run(run).unwrap().work,
                    WorkRef::Task(task.id.clone())
                );
                assert!(!catalog.matches_run(
                    run,
                    crate::lf::commands::WorkFilter {
                        project: Some("other"),
                        ..Default::default()
                    }
                ));
            }
        }
    }

    #[tokio::test]
    async fn hierarchical_work_selectors_must_match() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        let worktree = directory.path().join("repo.runtime-research");
        std::fs::create_dir_all(&worktree).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            repo.display().to_string(),
        );
        let other_wave = Wave::new(
            WaveId::new(),
            "other".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        store.create_wave(&other_wave).await.unwrap();
        let primary_project = project(&wave, "loopflow-api", "project-api");
        let other_project = project(&wave, "other", "project-other");
        store.create_project(&primary_project).await.unwrap();
        store.create_project(&other_project).await.unwrap();
        let task = task(&store, &wave, &primary_project, worktree).await;

        let binding = resolve_work_selection(
            &store,
            &repo,
            WorkSelection {
                task: Some("LOO-267"),
                wave: Some(wave.id().as_str()),
            },
        )
        .await
        .unwrap();
        assert_eq!(binding.work, WorkRef::Task(task.id));

        let wave_error = resolve_work_selection(
            &store,
            &repo,
            WorkSelection {
                task: Some("LOO-267"),
                wave: Some(other_wave.id().as_str()),
            },
        )
        .await
        .unwrap_err();
        assert!(wave_error.to_string().contains("belongs to Wave"));
    }

    #[tokio::test]
    async fn direct_wave_binding_carries_the_shared_metric_context() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/runtime")).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            repo.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        store
            .create_project(&project(&wave, "loopflow-api", "project-api"))
            .await
            .unwrap();
        let snapshot = PmSnapshot {
            projects: vec![PmProject {
                id: "project-api".to_string(),
                slug: "loopflow-api".to_string(),
                name: "Loopflow API".to_string(),
                summary: String::new(),

                metric_targets: Vec::new(),
                flows: Some(ProjectFlowPlan::empty()),
                krs: vec![PmKr {
                    text: "One model everywhere".to_string(),
                    holds: false,
                }],
                initiative_ids: vec!["initiative-1".to_string()],
                team_ids: vec!["team-1".to_string()],
            }],
            items: Vec::new(),
        };
        store
            .put_pm_snapshot(PmSnapshotRow {
                wave_id: wave.id().clone(),
                provider: "linear".to_string(),
                initiative: "initiative-1".to_string(),
                synced_at: OffsetDateTime::now_utc().unix_timestamp(),
                payload: serde_json::to_string(&snapshot).unwrap(),
            })
            .await
            .unwrap();

        let binding = resolve_work_binding(&store, &repo, "wave:runtime")
            .await
            .unwrap();
        assert_eq!(binding.work, WorkRef::Wave(wave.id().clone()));
        assert!(binding.context.contains("metric-portfolio"));
        assert!(resolve_work_binding(&store, &repo, "project:loopflow-api")
            .await
            .is_err());
    }
}
