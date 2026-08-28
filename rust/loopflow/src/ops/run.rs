use std::path::{Path, PathBuf};

use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::engine::process::{
    current_home_execution_context, pin_control_binary, start_lf_session_with_env,
};
use crate::id::WaveId;
use crate::store::SharedStore;
use crate::work::project::Project;
use crate::work::wave::Wave;

use super::{OpsError, OpsResult};

pub(crate) const TASK_ACCOUNT_ID_ENV: &str = "LF_TASK_ACCOUNT_ID";
pub(crate) const TASK_RESUME_TOKEN_ENV: &str = "LF_TASK_RESUME_TOKEN";

#[derive(Debug, Clone)]
pub struct WorkBinding {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub wave_name: String,
    pub cwd: PathBuf,
    pub context: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkSelection<'a> {
    pub task: Option<&'a str>,
    pub project: Option<&'a str>,
    pub wave: Option<&'a str>,
}

pub async fn resolve_work_binding(
    store: &SharedStore,
    repo: &Path,
    selector: &str,
) -> OpsResult<WorkBinding> {
    let (kind, value) = selector.split_once(':').ok_or_else(|| {
        run_error(format!(
            "invalid Work selector {selector:?}; expected task:<selector>, project:<selector>, or wave:<selector>"
        ))
    })?;
    let selection = match kind {
        "task" => WorkSelection {
            task: Some(value),
            ..WorkSelection::default()
        },
        "project" => WorkSelection {
            project: Some(value),
            ..WorkSelection::default()
        },
        "wave" => WorkSelection {
            wave: Some(value),
            ..WorkSelection::default()
        },
        _ => {
            return Err(run_error(format!(
                "invalid Work selector kind {kind:?}; expected task, project, or wave"
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
    for (kind, value) in [
        ("task", selection.task),
        ("project", selection.project),
        ("wave", selection.wave),
    ] {
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
        if let Some(project_selector) = selection.project {
            require_project_match(
                &project,
                project_selector.trim(),
                &format!("Task {}", task.plan.identifier),
            )?;
        }
        if let Some(selected_wave) = &selected_wave {
            require_wave_match(
                &wave,
                selected_wave,
                &format!("Task {}", task.plan.identifier),
            )?;
        }
        return Ok(WorkBinding {
            work: WorkRef::Task(task.id.clone()),
            wave_id: task.wave_id,
            wave_name: wave.name().to_string(),
            cwd: task.worktree,
            context: format!(
                "Task {}: {}\n\n{}\n\nProject {}:\n{}",
                task.plan.identifier,
                task.plan.title,
                task.plan.description,
                project.plan.slug,
                project.plan.prompt_context,
            ),
        });
    }

    if let Some(value) = selection.project {
        let value = value.trim();
        let project = resolve_project(store, value, selected_wave.as_ref()).await?;
        let wave = store
            .get_wave(&project.wave_id)
            .await
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Project {} has no owning Wave", project.id)))?;
        if let Some(selected_wave) = &selected_wave {
            require_wave_match(
                &wave,
                selected_wave,
                &format!("Project {}", project.plan.slug),
            )?;
        }
        let metric_context = crate::ops::metrics::metric_prompt_section(
            "project-owned-metrics",
            crate::ops::metrics::stored_project_metric_portfolio(
                store,
                &wave,
                project.plan.id.as_str(),
                time::OffsetDateTime::now_utc(),
            )
            .await,
        );
        return Ok(WorkBinding {
            work: WorkRef::Project(project.id.clone()),
            wave_id: project.wave_id,
            wave_name: wave.name().to_string(),
            cwd: PathBuf::from(wave.repo()),
            context: format!(
                "Project {}: {}\n\n{}\n\n{}\n\nOnly metrics owned by this Project appear above. Cross-owned evidence appears only when the Wave routes it through durable direction. Metrics inform KR judgment; they never check a KR automatically.",
                project.plan.slug,
                project.plan.name,
                project.plan.prompt_context,
                metric_context,
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
        return Ok(WorkBinding {
            work: WorkRef::Wave(wave.id().clone()),
            wave_id: wave.id().clone(),
            wave_name: wave.name().to_string(),
            cwd: PathBuf::from(wave.repo()),
            context: format!(
                "Wave {}\n\n{}\n\nAnswer the executive loop from the objective, Project portfolio, Work state, and evidence:\n1. What is most important?\n2. What signals are arriving?\n3. What works?\n4. What does not?\n5. What is the current strategy?\n6. How should strategy adjust?\n\nMetrics are evidence, never automatic KR completion or a composite Wave score.",
                wave.name(),
                metric_context,
            ),
        });
    }

    Err(run_error("select a Task, Project, or Wave"))
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

async fn resolve_project(
    store: &SharedStore,
    value: &str,
    wave: Option<&Wave>,
) -> OpsResult<Project> {
    let project = if let Ok(id) = ProjectId::parse(value) {
        store.get_project(&id).await.map_err(run_error)?
    } else {
        let matches = store
            .list_projects(wave.map(Wave::id))
            .await
            .map_err(run_error)?
            .into_iter()
            .filter(|project| project_matches(project, value))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [project] => Some(project.clone()),
            [] => None,
            _ => {
                return Err(run_error(format!(
                    "Project selector {value:?} is ambiguous; qualify it with --wave or use its durable or planning-system id"
                )))
            }
        }
    };
    project.ok_or_else(|| run_error(format!("Project {value:?} is not registered")))
}

fn project_matches(project: &Project, value: &str) -> bool {
    ProjectId::parse(value).map_or_else(
        |_| project.plan.id.as_str() == value || project.plan.slug == value,
        |id| project.id == id,
    )
}

fn require_project_match(project: &Project, requested: &str, subject: &str) -> OpsResult<()> {
    if project_matches(project, requested) {
        return Ok(());
    }
    Err(run_error(format!(
        "{subject} belongs to Project {}, not {requested}",
        project.plan.slug
    )))
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
pub(crate) struct WorkLaunch {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub environment: Vec<(String, String)>,
}

pub(crate) async fn launch_work(request: WorkLaunch) -> OpsResult<()> {
    let environment = request.environment.clone();
    start_work_session(&request, environment).await
}

async fn start_work_session(
    request: &WorkLaunch,
    mut environment: Vec<(String, String)>,
) -> OpsResult<()> {
    let execution = current_home_execution_context()
        .map_err(|error| OpsError::Message(format!("cannot resolve current lf binary: {error}")))?;
    let control_bin = pin_control_binary(&execution.lf_bin)
        .to_string_lossy()
        .to_string();
    let argv = vec![
        control_bin.clone(),
        "__work".to_string(),
        request.work.kind().to_string(),
        request.work.id().to_string(),
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
        .map_err(|error| {
            OpsError::Message(format!(
                "failed to launch {} body: {error}",
                request.work.kind()
            ))
        })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::OffsetDateTime;

    use super::*;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::pm::{PmKr, PmProject, PmSnapshot, ProjectFlowPlan};
    use crate::store::{open_store, PmSnapshotRow, StorageConfig};
    use crate::work::project::Project;
    use crate::work::task::{Observation, PmWritebackState, Task, TaskPr, TaskPrId};
    use crate::work::wave::Wave;

    async fn test_store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
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
    async fn project_slug_selector_requires_one_exact_match() {
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
            .expect_err("ambiguous slug must not infer identity");

        assert!(error.to_string().contains("ambiguous"));
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

        let (runtime, prompts) = tokio::join!(
            resolve_work_binding(&store, &repo, "task:LOO-267"),
            resolve_work_binding(&store, &repo, "task:LOO-267")
        );

        assert_eq!(runtime.unwrap().cwd, worktree);
        assert_eq!(prompts.unwrap().work, WorkRef::Task(task.id.clone()));
        assert!(store
            .task_controller_state(&task.id)
            .await
            .unwrap()
            .is_none());
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
                project: Some("loopflow-api"),
                wave: Some(wave.id().as_str()),
            },
        )
        .await
        .unwrap();
        assert_eq!(binding.work, WorkRef::Task(task.id));

        let project_error = resolve_work_selection(
            &store,
            &repo,
            WorkSelection {
                task: Some("LOO-267"),
                project: Some("other"),
                wave: None,
            },
        )
        .await
        .unwrap_err();
        assert!(project_error.to_string().contains("belongs to Project"));

        let wave_error = resolve_work_selection(
            &store,
            &repo,
            WorkSelection {
                task: None,
                project: Some(primary_project.id.as_str()),
                wave: Some(other_wave.id().as_str()),
            },
        )
        .await
        .unwrap_err();
        assert!(wave_error.to_string().contains("belongs to Wave"));
    }

    #[tokio::test]
    async fn direct_wave_and_project_bindings_carry_the_shared_metric_context() {
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
                definition: "Keep one product model.".to_string(),
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

        let project_binding = resolve_work_binding(&store, &repo, "project:project-api")
            .await
            .unwrap();
        assert!(project_binding
            .context
            .contains("<lf:project-owned-metrics>"));
        assert!(project_binding.context.contains("\"metrics\":[]"));
        assert!(!project_binding.context.contains("<lf:metric-portfolio>"));

        let wave_binding =
            resolve_work_binding(&store, &repo, &format!("wave:{}", wave.id().as_str()))
                .await
                .unwrap();
        assert!(wave_binding.context.contains("<lf:metric-portfolio>"));
        assert!(wave_binding.context.contains("What signals are arriving?"));
        assert!(!wave_binding.context.contains("<lf:project-owned-metrics>"));
    }
}
