use std::path::{Path, PathBuf};

use crate::durable::{
    AdvanceReceipt, AgentInvocation, BoundaryState, Containment, InvocationRoute, ProjectId,
    RunAdvance, RunContext, RunTrigger, TaskId, WorkRef,
};
use crate::engine::process::{
    current_home_execution_context, current_process_group_id, pin_control_binary,
    start_lf_session_with_env,
};
use crate::id::WaveId;
use crate::store::SharedStore;

use super::{OpsError, OpsResult};

#[derive(Debug, Clone)]
pub struct WorkBinding {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub wave_name: String,
    pub cwd: PathBuf,
    pub context: String,
}

impl WorkBinding {
    pub fn matches_run(&self, context: &RunContext) -> bool {
        context.work == self.work
    }
}

#[derive(Debug)]
pub(crate) struct DirectRun {
    store: SharedStore,
    context: RunContext,
    invocation: AgentInvocation,
    finished: bool,
}

impl DirectRun {
    pub(crate) async fn start(
        store: SharedStore,
        binding: &WorkBinding,
        route: InvocationRoute,
        surface: &str,
    ) -> OpsResult<Self> {
        let (_, context) = store
            .reserve_run(&binding.work, RunTrigger::User)
            .await
            .map_err(run_error)?;
        let process_group = match current_process_group_id() {
            Some(process_group) => process_group,
            None => {
                let _ = store.stop_run_on_interrupt(&context);
                return Err(run_error("direct lf invocation has no process group"));
            }
        };
        if let Err(error) = store
            .advance_run(
                &context,
                RunAdvance::RunStarting {
                    containment: Containment::ProcessGroup {
                        id: i64::from(process_group),
                    },
                    cwd: binding.cwd.clone(),
                },
            )
            .await
        {
            let _ = store.stop_run_on_interrupt(&context);
            return Err(run_error(error));
        }
        let invocation = match store
            .advance_run(
                &context,
                RunAdvance::InvocationStarting {
                    route,
                    surface: surface.to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
        {
            Ok(AdvanceReceipt::Invocation(invocation)) => invocation,
            Ok(_) => unreachable!("InvocationStarting returns an Invocation receipt"),
            Err(error) => {
                let _ = store.stop_run_on_interrupt(&context);
                return Err(run_error(error));
            }
        };

        let cleanup_store = store.clone();
        let cleanup_context = context.clone();
        crate::engine::agent::register_interrupt_cleanup(move || {
            let _ = cleanup_store.stop_run_on_interrupt(&cleanup_context);
        });

        Ok(Self {
            store,
            context,
            invocation,
            finished: false,
        })
    }

    pub(crate) fn context(&self) -> &RunContext {
        &self.context
    }

    pub(crate) fn invocation(&self) -> &AgentInvocation {
        &self.invocation
    }

    pub(crate) async fn finish(&mut self, outcome: BoundaryState) -> OpsResult<()> {
        let invocation_result = self
            .store
            .advance_run(
                &self.context,
                RunAdvance::InvocationEnded {
                    invocation_id: self.invocation.id.clone(),
                    outcome,
                },
            )
            .await;
        let run_result = self.store.finish_run(&self.context, outcome).await;
        if run_result.is_ok() {
            self.finished = true;
        }
        invocation_result.map_err(run_error)?;
        run_result.map_err(run_error)
    }
}

impl Drop for DirectRun {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.store.stop_run_on_interrupt(&self.context);
        }
    }
}

pub async fn resolve_work_binding(
    store: &SharedStore,
    repo: &Path,
    selector: &str,
) -> OpsResult<WorkBinding> {
    let (kind, value) = selector.split_once(':').ok_or_else(|| {
        run_error(format!(
            "invalid --as {selector:?}; use task:<selector>, project:<selector>, or wave:<selector>"
        ))
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(run_error(format!("{kind} selector cannot be empty")));
    }

    match kind {
        "task" => {
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
            Ok(WorkBinding {
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
            })
        }
        "project" => {
            let project = if let Ok(id) = ProjectId::parse(value) {
                store.get_project(&id).await.map_err(run_error)?
            } else {
                let matches = store
                    .list_projects(None)
                    .await
                    .map_err(run_error)?
                    .into_iter()
                    .filter(|project| {
                        project.plan.id.as_str() == value || project.plan.slug == value
                    })
                    .collect::<Vec<_>>();
                match matches.as_slice() {
                    [project] => Some(project.clone()),
                    [] => None,
                    _ => {
                        return Err(run_error(format!(
                            "Project selector {value:?} is ambiguous; use its durable or planning-system id"
                        )));
                    }
                }
            }
            .ok_or_else(|| run_error(format!("Project {value:?} is not registered")))?;
            let wave = store
                .get_wave(&project.wave_id)
                .await
                .map_err(run_error)?
                .ok_or_else(|| run_error(format!("Project {} has no owning Wave", project.id)))?;
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
            Ok(WorkBinding {
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
            })
        }
        "wave" => {
            let wave = if let Ok(id) = WaveId::parse(value) {
                store.get_wave(&id).await.map_err(run_error)?
            } else {
                let locator = crate::wave::WaveLocator::discover(repo, value).map_err(run_error)?;
                store.get_wave_at(&locator).await.map_err(run_error)?
            }
            .ok_or_else(|| run_error(format!("Wave {value:?} is not registered")))?;
            let metric_context = crate::ops::metrics::metric_prompt_section(
                "metric-portfolio",
                crate::ops::metrics::stored_wave_metric_portfolio(
                    store,
                    &wave,
                    time::OffsetDateTime::now_utc(),
                )
                .await,
            );
            Ok(WorkBinding {
                work: WorkRef::Wave(wave.id().clone()),
                wave_id: wave.id().clone(),
                wave_name: wave.name().to_string(),
                cwd: PathBuf::from(wave.repo()),
                context: format!(
                    "Wave {}\n\n{}\n\nAnswer the executive loop from the objective, Project portfolio, Work state, and evidence:\n1. What is most important?\n2. What signals are arriving?\n3. What works?\n4. What does not?\n5. What is the current strategy?\n6. How should strategy adjust?\n\nMetrics are evidence, never automatic KR completion or a composite Wave score.",
                    wave.name(),
                    metric_context,
                ),
            })
        }
        _ => Err(run_error(format!(
            "invalid --as kind {kind:?}; expected task, project, or wave"
        ))),
    }
}

fn run_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[derive(Debug)]
pub(crate) struct RunLaunch {
    pub work: WorkRef,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub agent: String,
    pub account_id: Option<crate::store::ProviderAccountId>,
    pub resume_token: Option<String>,
}

pub(crate) async fn launch_in_run(
    store: &SharedStore,
    lease: &RunContext,
    request: RunLaunch,
) -> OpsResult<AgentInvocation> {
    let execution = current_home_execution_context()
        .map_err(|error| OpsError::Message(format!("cannot resolve current lf binary: {error}")))?;
    let (provider, model) = crate::engine::config::parse_agent(&request.agent);
    let tmux_name = request.tmux_name.clone();
    let run = store
        .advance_run(
            lease,
            RunAdvance::RunStarting {
                containment: Containment::Tmux {
                    name: tmux_name.clone(),
                },
                cwd: request.cwd.clone(),
            },
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let AdvanceReceipt::Run(_) = run else {
        unreachable!("RunStarting returns a Run receipt")
    };
    let receipt = store
        .advance_run(
            lease,
            RunAdvance::InvocationStarting {
                route: InvocationRoute {
                    provider,
                    model,
                    account_id: request
                        .account_id
                        .as_ref()
                        .map(|account_id| account_id.as_str().to_string()),
                },
                surface: "headless".to_string(),
                resume_token: request.resume_token,
                answer_ask_id: None,
            },
        )
        .await
        .map_err(|error| OpsError::Message(error.to_string()))?;
    let AdvanceReceipt::Invocation(invocation) = receipt else {
        unreachable!("InvocationStarting returns an Invocation receipt")
    };
    let control_bin = pin_control_binary(&execution.lf_bin)
        .to_string_lossy()
        .to_string();
    let argv = vec![
        control_bin.clone(),
        "__work".to_string(),
        request.work.kind().to_string(),
        request.work.id().to_string(),
    ];
    let run_context = lease.run_id.to_string();
    let invocation_id = invocation.id.as_str().to_string();
    let db_path = execution.db_path.to_string_lossy().to_string();
    let lf_home = execution.lf_home.to_string_lossy().to_string();
    let mut environment = vec![
        (
            crate::engine::wave_context::WAVE_ID_ENV.to_string(),
            request.wave_id.as_str().to_string(),
        ),
        (
            crate::durable::RUN_CONTEXT_ENV.to_string(),
            "agent".to_string(),
        ),
        (crate::durable::RUN_ID_ENV.to_string(), run_context),
        (
            crate::durable::AGENT_INVOCATION_ENV.to_string(),
            invocation_id,
        ),
        (crate::store::CONTROL_BIN_ENV.to_string(), control_bin),
        (crate::store::CONTROL_DB_PATH_ENV.to_string(), db_path),
        (crate::store::CONTROL_HOME_ENV.to_string(), lf_home),
    ];
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
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, &request.cwd, &argv, &environment).await
    {
        let _ = store
            .advance_run(
                lease,
                RunAdvance::InvocationEnded {
                    invocation_id: invocation.id.clone(),
                    outcome: BoundaryState::Failed,
                },
            )
            .await;
        let _ = store
            .stop_run(
                lease,
                crate::durable::StopCause::Recovery,
                crate::durable::ContainmentObservation::Absent,
            )
            .await;
        return Err(OpsError::Message(format!(
            "failed to launch {} body: {error}",
            request.work.kind()
        )));
    }
    Ok(invocation)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use time::OffsetDateTime;

    use super::*;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::pm::{PmKr, PmProject, PmSnapshot, ProjectFlowPlan};
    use crate::project::Project;
    use crate::store::{open_store, PmSnapshotRow, StorageConfig};
    use crate::wave::Wave;

    async fn test_store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
            .await
            .unwrap();
        (directory, Arc::new(store))
    }

    fn binding(wave: &Wave, cwd: &Path) -> WorkBinding {
        WorkBinding {
            work: WorkRef::Wave(wave.id().clone()),
            wave_id: wave.id().clone(),
            wave_name: wave.name().to_string(),
            cwd: cwd.to_path_buf(),
            context: format!("Wave {}", wave.name()),
        }
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
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        }
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

    #[tokio::test]
    async fn work_binding_matches_only_its_run_identity() {
        let (directory, store) = test_store().await;
        let selected = Wave::new(
            WaveId::new(),
            "selected".to_string(),
            directory.path().display().to_string(),
        );
        let other = Wave::new(
            WaveId::new(),
            "other".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&selected).await.unwrap();
        store.create_wave(&other).await.unwrap();
        let selected_binding = binding(&selected, directory.path());
        let (_, selected_lease) = store
            .reserve_run(&selected_binding.work, RunTrigger::User)
            .await
            .unwrap();
        let (_, other_lease) = store
            .reserve_run(&WorkRef::Wave(other.id().clone()), RunTrigger::User)
            .await
            .unwrap();

        assert!(selected_binding.matches_run(&selected_lease));
        assert!(!selected_binding.matches_run(&other_lease));
    }

    #[tokio::test]
    async fn direct_run_fences_overlap_and_releases_work_after_finish() {
        let (directory, store) = test_store().await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let binding = binding(&wave, directory.path());
        let route = InvocationRoute {
            provider: "codex".to_string(),
            model: None,
            account_id: None,
        };
        let mut direct = DirectRun::start(store.clone(), &binding, route.clone(), "headless")
            .await
            .unwrap();

        let active_run = store.current_run(&binding.work).await.unwrap().unwrap();
        DirectRun::start(store.clone(), &binding, route, "headless")
            .await
            .expect_err("active direct Run must fence overlap");
        assert_eq!(
            store.current_run(&binding.work).await.unwrap().unwrap().id,
            active_run.id
        );

        direct.finish(BoundaryState::Succeeded).await.unwrap();
        assert!(store.current_run(&binding.work).await.unwrap().is_none());
        let invocation = store
            .invocations_for_run(&direct.context().run_id)
            .await
            .unwrap()
            .pop()
            .expect("direct invocation");
        assert!(invocation.ended_at.is_some());
    }
}
