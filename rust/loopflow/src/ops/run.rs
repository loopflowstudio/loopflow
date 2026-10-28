use std::path::{Path, PathBuf};

use crate::durable::WorkRef;
use crate::id::WaveId;
use crate::store::SharedStore;
use crate::work::project::Project;

use crate::work::task::{Task, TaskPr};
use crate::work::wave::Wave;

use super::{OpsError, OpsResult};

#[derive(Debug, Clone)]
pub struct WorkBinding {
    pub source: crate::session::WorkSource,
    pub work: WorkRef,
    pub wave_id: Option<WaveId>,
    pub wave_name: Option<String>,
    pub subjects: Vec<String>,
    pub cwd: PathBuf,
    pub context: String,
    pub agent: Option<String>,
}

pub(crate) fn render_task_context(
    task: &Task,
    project: Option<&Project>,
    pr: Option<&TaskPr>,
    wave_name: Option<&str>,
) -> String {
    let mut context = format!(
        "Task {}: {}\n\n{}\n\nRepository: {}\nTask Work: {}",
        task.identifier, task.title, task.description, task.repository_id, task.id
    );
    if let Some(project) = project {
        context.push_str(&format!(
            "\n\nChapter plan: {} (source {})\n{}",
            project.name,
            project.id,
            render_project_context(project)
        ));
    }
    if let Some(wave) = wave_name {
        context.push_str(&format!("\nWave: {wave}"));
    }
    context.push_str(&format!(
        "\nTask directive snapshot synced at: {}\nWorktree: {}\nBranch: {}\nBase commit: {}",
        task.pm_snapshot_synced_at
            .map(|at| at.to_string())
            .unwrap_or_else(|| "not observed".into()),
        task.worktree
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "not placed".into()),
        task.branch,
        task.base_commit
    ));
    if let Some(parent) = &task.parent_pr_id {
        context.push_str(&format!(
            "\nStack parent PR: {parent} (land the parent first)"
        ));
    }
    if let Some(pr) = pr.and_then(TaskPr::github) {
        context.push_str(&format!("\nPull request: {}", pr.url));
    }
    context
}

fn render_project_context(project: &crate::work::project::Project) -> String {
    let targets = serde_json::to_string_pretty(&project.metric_targets)
        .expect("validated metric targets serialize");
    let mut content = format!(
        "{}\n## Metric targets\n\n```json\n{targets}\n```",
        project.description
    );
    content.push_str("\n\n## KRs");
    for kr in &project.krs {
        let marker = if kr.holds { "x" } else { " " };
        content.push_str(&format!("\n\n- [{marker}] {}", kr.text.trim()));
    }
    content.push('\n');
    content
}

fn render_wave_context(repo: &Path, wave: &str, metric_context: &str) -> String {
    // Authored goals and memory enter once through the prompt document gatherer.
    let graphs = match crate::graph::available_graph_names(repo) {
        Ok(names) => names
            .iter()
            .map(|graph| format!("- {graph}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(error) => format!("Graph catalog unavailable: {error}"),
    };
    format!(
        "Drive the '{wave}' Wave's goal forward using its checkout-local files.\n\n<lf:goal-context>\nAvailable graphs:\n{graphs}\n</lf:goal-context>\n\n{metric_context}\n\n<lf:wave-executive-loop>\n1. What is most important?\n2. What signals are arriving?\n3. What works?\n4. What does not?\n5. What is the current strategy?\n6. How should strategy adjust?\n\nTreat metrics as evidence, never as automatic KR completion or a composite Wave score.\n</lf:wave-executive-loop>"
    )
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkSelection<'a> {
    pub task: Option<&'a str>,
    pub wave: Option<&'a str>,
}

impl<'a> WorkSelection<'a> {
    fn parse(selector: &'a str) -> OpsResult<Self> {
        let (kind, value) = selector.split_once(':').ok_or_else(|| {
            run_error(format!(
                "invalid Work selector {selector:?}; expected task:<selector> or wave:<selector>"
            ))
        })?;
        match kind {
            "task" => Ok(Self {
                task: Some(value),
                wave: None,
            }),
            "wave" => Ok(Self {
                task: None,
                wave: Some(value),
            }),
            _ => Err(run_error(format!(
                "invalid Work selector kind {kind:?}; expected task or wave"
            ))),
        }
    }
}

/// Planning identity is readable before a Task has a checkout, PR, or Graph.
/// Launch binding adds those requirements only after selecting the same records.
#[derive(Debug)]
pub(crate) struct SelectedWork {
    pub source: crate::session::WorkSource,
    pub wave: Option<Wave>,
    pub task: Option<Task>,
}

pub(crate) async fn select_work(
    store: &SharedStore,
    cwd: &Path,
    selection: WorkSelection<'_>,
) -> OpsResult<Option<SelectedWork>> {
    for (kind, value) in [("task", selection.task), ("wave", selection.wave)] {
        if value.is_some_and(|value| value.trim().is_empty()) {
            return Err(run_error(format!("{kind} selector cannot be empty")));
        }
    }
    let selected_wave = match selection.wave {
        Some(value) => Some(resolve_wave(store, cwd, value.trim()).await?),
        None => None,
    };
    let (task, source) = if let Some(value) = selection.task {
        let id = store
            .sqlite
            .resolve_task_id(value.trim(), None)
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Task {value:?} is not registered")))?;
        let task = store
            .get_task(&id)
            .await
            .map_err(run_error)?
            .ok_or_else(|| run_error(format!("Task {id} is not registered")))?;
        (Some(task), crate::session::WorkSource::Declared)
    } else if selected_wave.is_some() {
        (None, crate::session::WorkSource::Declared)
    } else {
        let task = super::task::task_for_checkout(store, cwd).await?;
        (task, crate::session::WorkSource::Checkout)
    };
    let wave =
        match &task {
            Some(task) => {
                let wave =
                    match &task.wave_id {
                        Some(id) => Some(store.get_wave(id).await.map_err(run_error)?.ok_or_else(
                            || run_error(format!("Task {} Wave is missing", task.id)),
                        )?),
                        None => None,
                    };
                if let Some(selected) = &selected_wave {
                    if wave.as_ref().map(Wave::id) != Some(selected.id()) {
                        return Err(run_error("selected Wave does not own Task"));
                    }
                }
                wave
            }
            None if selected_wave.is_none() => return Ok(None),
            None => selected_wave,
        };
    Ok(Some(SelectedWork { source, wave, task }))
}

pub async fn resolve_work_binding(
    store: &SharedStore,
    repo: &Path,
    selector: &str,
) -> OpsResult<WorkBinding> {
    let selection = WorkSelection::parse(selector)?;
    resolve_work_selection(store, repo, selection).await
}

pub async fn resolve_work_selection(
    store: &SharedStore,
    repo: &Path,
    selection: WorkSelection<'_>,
) -> OpsResult<WorkBinding> {
    let selected = select_work(store, repo, selection)
        .await?
        .ok_or_else(|| run_error("select a Task or Wave"))?;
    bind_selected_work(store, repo, selected).await
}

async fn bind_selected_work(
    store: &SharedStore,
    repo: &Path,
    selected: SelectedWork,
) -> OpsResult<WorkBinding> {
    let wave = selected.wave;
    if let Some(task) = selected.task {
        let project = match &task.project_id {
            Some(id) => Some(
                store
                    .get_project(id)
                    .await
                    .map_err(run_error)?
                    .ok_or_else(|| run_error("Task Project is missing"))?,
            ),
            None => None,
        };
        let work = WorkRef::Task(task.id.clone());
        let pr = store.active_task_pr(&task.id).await.map_err(run_error)?;
        let mut context = render_task_context(
            &task,
            project.as_ref(),
            pr.as_ref(),
            wave.as_ref().map(Wave::slug),
        );
        if let Ok(Some(graph)) = store.sqlite.task_graph(&task.id) {
            context.push_str(&format!("\n\n{}", graph.guidance(&task.identifier)));
        }
        let cwd = task.worktree()?.clone();
        return Ok(WorkBinding {
            source: selected.source,
            subjects: wave
                .as_ref()
                .map(|w| format!("wave:{}", w.slug()))
                .into_iter()
                .chain(project.as_ref().map(|p| format!("project:{}", p.slug)))
                .chain(std::iter::once(format!("task:{}", task.identifier)))
                .collect(),
            work,
            wave_id: task.wave_id,
            wave_name: wave.as_ref().map(|w| w.slug().to_owned()),
            cwd,
            context,
            agent: task.agent,
        });
    }

    let wave = wave.expect("a selection without a Task has a Wave");
    let metric_context = crate::ops::metrics::metric_prompt_section(
        "metric-portfolio",
        crate::ops::metrics::stored_wave_metric_portfolio(
            store,
            &wave,
            time::OffsetDateTime::now_utc(),
        )
        .await,
    );
    let cwd = if crate::repository::CanonicalRepo::discover(Path::new(wave.repo()))
        .is_ok_and(|canonical| canonical.contains(repo))
    {
        crate::git::worktree_root(repo).unwrap_or_else(|_| repo.to_path_buf())
    } else {
        PathBuf::from(wave.repo())
    };
    let context = render_wave_context(&cwd, wave.slug(), &metric_context);
    Ok(WorkBinding {
        source: selected.source,
        subjects: vec![format!("wave:{}", wave.slug())],
        work: WorkRef::Wave(wave.id().clone()),
        wave_id: Some(wave.id().clone()),
        wave_name: Some(wave.slug().to_string()),
        cwd,
        context,
        agent: None,
    })
}

/// Resolve checkout ownership before an ancestor's explicit declaration.
/// This command's Task selector is resolved by the CLI before reaching this fallback.
pub async fn resolve_execution_binding(
    store: &SharedStore,
    cwd: &Path,
) -> OpsResult<Option<WorkBinding>> {
    let Some(selected) = select_execution_work(store, cwd).await? else {
        return Ok(None);
    };
    let checkout = selected.source == crate::session::WorkSource::Checkout;
    let mut binding = bind_selected_work(store, cwd, selected).await?;
    binding.cwd = if checkout {
        crate::git::worktree_root(cwd).unwrap_or_else(|_| cwd.to_path_buf())
    } else {
        cwd.to_path_buf()
    };
    Ok(Some(binding))
}

/// Shared implicit selection for launch and explanation; neither prepares Work.
pub(crate) async fn select_execution_work(
    store: &SharedStore,
    cwd: &Path,
) -> OpsResult<Option<SelectedWork>> {
    if crate::repo::discover_repo_root(cwd)
        .map_err(run_error)?
        .is_some()
    {
        if let Some(selected) = select_work(store, cwd, WorkSelection::default()).await? {
            return Ok(Some(selected));
        }
    }
    match std::env::var(crate::lf::WORK_DECLARATION_ENV) {
        Ok(selector) => select_work(store, cwd, WorkSelection::parse(&selector)?).await,
        Err(_) => Ok(None),
    }
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

fn run_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[cfg(test)]
mod tests {
    use crate::test_linear_input::{InventoryFixture, SeedLinear};
    use std::sync::Arc;

    use time::OffsetDateTime;

    use super::{resolve_work_binding, resolve_work_selection, WorkSelection};
    use crate::durable::{ProjectId, TaskId, WorkRef};
    use crate::id::WaveId;
    use crate::linear::LinearProject;
    use crate::linear::{LinearIssueId, LinearProjectId};
    use crate::store::SharedStore;
    use crate::store::StorageConfig;
    use crate::test_linear_input::Inventory;
    use crate::work::project::Project;
    use crate::work::project::ProjectKR;
    use crate::work::task::{LinearWritebackState, Observation, Task, TaskPr, TaskPrId};
    use crate::work::wave::Wave;
    use std::path::PathBuf;

    async fn test_store() -> (tempfile::TempDir, SharedStore) {
        let directory = tempfile::tempdir().unwrap();
        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            directory.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        (directory, Arc::new(store))
    }

    fn project(wave: &Wave, slug: &str, planning_id: &str) -> Project {
        let now = OffsetDateTime::now_utc();
        Project {
            id: ProjectId::new(),

            summary: String::new(),
            status: crate::work::project::ProjectStatus::Started,
            linear_id: Some(LinearProjectId::new(planning_id).unwrap()),
            slug: slug.to_string(),
            name: slug.to_string(),
            description: "Ship the requested behavior.".to_string(),
            krs: Vec::new(),
            metric_targets: Vec::new(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),

            wave_id: wave.id().clone(),
            created_at: now,
            updated_at: now,
        }
    }

    async fn task(store: &SharedStore, wave: &Wave, project: &Project, worktree: PathBuf) -> Task {
        let now = OffsetDateTime::now_utc();
        let task = Task {
            repository_id: store.sqlite.ensure_repository(wave.repo()).unwrap(),
            id: TaskId::new(),

            revision: 0,
            linear_id: Some(LinearIssueId::new("runtime-research").unwrap()),
            identifier: "LOO-267".to_string(),
            title: "Research the runtime".to_string(),
            description: "Compare independent findings.".to_string(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),

            pm_writeback: LinearWritebackState::Current,
            wave_id: Some(wave.id().clone()),
            project_id: Some(project.id.clone()),
            worktree: Some(worktree),
            workspace_slug: "runtime-research".to_string(),
            branch: "jack/runtime-research".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            agent: None,
            created_at: now,
            updated_at: now,
            rank: 0,
            assignee: None,
            due_date: None,
            state: None,
            completed_at: None,
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
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            project.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        store.seed_task(&task, &pr).await.unwrap();
        task
    }

    #[tokio::test]
    async fn explanation_reads_unstarted_planning_without_allocating_work() {
        use crate::ops::context::{explain_context, ContextFact};
        let (directory, store) = test_store().await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = project(&wave, "runtime", "project-runtime");
        store.create_project(&project).await.unwrap();
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            project.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        let task = store
            .create_task(
                &crate::work::task::NewTask {
                    repository_id: store.sqlite.ensure_repository(wave.repo()).unwrap(),
                    due_date: None,
                    id: TaskId::new(),
                    project_id: Some(project.id),
                    title: "Inspect before starting".into(),
                    description: String::new(),
                },
                Some(std::sync::Arc::new(crate::store::PlanningLocks::new(
                    tempfile::tempfile().unwrap(),
                ))),
            )
            .await
            .unwrap();
        let report = explain_context(
            &store,
            directory.path(),
            WorkSelection {
                task: Some(task.id.as_str()),
                wave: None,
            },
            None,
            None,
        )
        .await;
        assert_eq!(
            report.task,
            ContextFact::Bound {
                value: task.id.to_string(),
                source: "explicit".into()
            }
        );
        assert_eq!(report.checkout, ContextFact::Unbound);
        assert!(matches!(
            report.execution_machine,
            ContextFact::Unavailable { .. }
        ));
        assert!(store.task_prs(&task.id).await.unwrap().is_empty());
        assert!(store.task_checkout(&task.id).await.unwrap().is_none());
        assert!(store.sqlite.task_graph(&task.id).unwrap().is_none());
        assert!(report.render().contains("Checkout: unbound"));
        assert!(report
            .render()
            .contains("peer execution has not been observed"));
    }

    #[tokio::test]
    async fn explanation_and_launch_share_checkout_identity_not_branch_names() {
        use crate::ops::context::{explain_context, ContextFact};
        let (_directory, store) = test_store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let other = loopflow_test_support::TestRepo::new();
        repo.create_branch("jack/runtime-research");
        other.create_branch("jack/runtime-research");
        let wave = Wave::new(
            WaveId::new(),
            "runtime".into(),
            repo.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = project(&wave, "runtime", "project-runtime");
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, repo.path().to_path_buf()).await;
        let inferred =
            explain_context(&store, repo.path(), WorkSelection::default(), None, None).await;
        assert_eq!(
            inferred.task,
            ContextFact::Bound {
                value: task.id.to_string(),
                source: "checkout".into()
            }
        );
        let declared = explain_context(
            &store,
            other.path(),
            WorkSelection {
                task: Some(task.identifier.as_str()),
                wave: None,
            },
            None,
            None,
        )
        .await;
        assert_eq!(declared.checkout, inferred.checkout);
        assert_eq!(declared.repository, inferred.repository);
        let launch = resolve_work_binding(&store, other.path(), "task:LOO-267")
            .await
            .unwrap();
        assert_eq!(launch.cwd, *task.worktree.as_ref().unwrap());
        assert!(super::resolve_execution_binding(&store, other.path())
            .await
            .unwrap()
            .is_none());
        assert!(crate::ops::task::task_for_checkout(&store, other.path())
            .await
            .unwrap()
            .is_none());
        // Checkout identity survives branch changes, including detached HEAD.
        assert!(std::process::Command::new("git")
            .args(["checkout", "--detach"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        let inferred_launch = super::resolve_execution_binding(&store, repo.path())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(inferred_launch.work, launch.work);
        assert_eq!(inferred_launch.source, crate::session::WorkSource::Checkout);
        assert_eq!(inferred_launch.context, launch.context);
        assert_eq!(
            crate::ops::task::task_for_checkout(&store, repo.path())
                .await
                .unwrap()
                .unwrap()
                .id,
            task.id
        );
        let unbound =
            explain_context(&store, other.path(), WorkSelection::default(), None, None).await;
        assert_eq!(unbound.task, ContextFact::Unbound);
        // Explaining a plain clone does not mint a plan or associate by remote.
        assert_eq!(unbound.repository, ContextFact::Unbound);
    }

    #[tokio::test]
    async fn explanation_preserves_remote_and_unknown_location_without_local_access() {
        use crate::ops::context::{explain_context, ContextFact};
        let (directory, store) = test_store().await;
        let wave = Wave::new(
            WaveId::new(),
            "runtime".into(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = project(&wave, "runtime", "project-runtime");
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, "/not/a/local/checkout".into()).await;
        let peer = crate::durable::MachineId::new();
        store.add_machine(&peer, "peer", "peer").await.unwrap();
        let conn = rusqlite::Connection::open(directory.path().join("loopflow.db")).unwrap();
        for machine in [Some(peer.as_str()), None] {
            conn.execute(
                "UPDATE tasks SET checkout_machine_id=?2 WHERE id=?1",
                rusqlite::params![task.id.as_str(), machine],
            )
            .unwrap();
            let report = explain_context(
                &store,
                directory.path(),
                WorkSelection {
                    task: Some(task.id.as_str()),
                    wave: None,
                },
                None,
                None,
            )
            .await;
            assert_eq!(
                report.checkout,
                ContextFact::Bound {
                    value: "/not/a/local/checkout".into(),
                    source: "recorded_checkout".into()
                }
            );
            match machine {
                Some(id) => assert_eq!(
                    report.execution_machine,
                    ContextFact::Bound {
                        value: id.into(),
                        source: "recorded_checkout".into()
                    }
                ),
                None => assert!(matches!(
                    report.execution_machine,
                    ContextFact::Unavailable { .. }
                )),
            }
        }
    }

    #[tokio::test]
    async fn explanation_missing_subjects_are_unavailable_not_unbound() {
        use crate::ops::context::{explain_context, ContextFact};
        let (directory, store) = test_store().await;
        for (task, session, process) in [
            (Some("absent"), None, None),
            (None, Some("absent"), None),
            (None, None, Some("absent")),
        ] {
            let report = explain_context(
                &store,
                directory.path(),
                WorkSelection { task, wave: None },
                session,
                process,
            )
            .await;
            assert!(matches!(report.task, ContextFact::Unavailable { .. }));
            if session.is_some() {
                assert!(matches!(report.session, ContextFact::Unavailable { .. }));
            }
            if process.is_some() {
                assert!(matches!(report.process, ContextFact::Unavailable { .. }));
            }
        }
    }

    #[test]
    fn context_delivery_supplies_one_goal_for_direct_and_wave_launches() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let tmp = loopflow_test_support::TestRepo::new();
        std::fs::create_dir_all(tmp.path().join(".lf/wave/release")).unwrap();
        let goal = "---\ncrons: []\n---\n## Objective\nShip a reliable release.\n\n## Bounds\nKeep rollback available.\n";
        std::fs::write(tmp.path().join(".lf/wave/release/GOAL.md"), goal).unwrap();
        let seed = super::render_wave_context(tmp.path(), "release", "");
        for (message, explicit) in [
            (None, false),
            (Some(seed.clone()), false),
            (None, true),
            (Some(seed), true),
        ] {
            let prepared = crate::prompt::process::prepare_process_prompt(
                &crate::config::Config {
                    diff_files: false,
                    diff: false,
                    paste: false,
                    ..Default::default()
                },
                crate::prompt::process::ProcessPromptInput {
                    repo_root: tmp.path().to_path_buf(),
                    wave: Some("release".into()),
                    docs: if explicit {
                        vec![".lf/wave/release/GOAL.md".into()]
                    } else {
                        vec![]
                    },
                    message,
                    ..Default::default()
                },
            )
            .unwrap();
            let block = prepared
                .config
                .conversation_context
                .as_ref()
                .unwrap()
                .block(crate::context_block::ContextMoment::Start)
                .unwrap()
                .text;
            for content in ["Ship a reliable release.", "Keep rollback available."] {
                assert_eq!(block.matches(content).count(), usize::from(!explicit));
                assert_eq!(
                    prepared.config.task_prompt.matches(content).count(),
                    usize::from(explicit)
                );
            }
            assert!(!prepared.config.system_prompt.contains(goal));
            let context = crate::lf::commands::run::attributed_context(
                &prepared.components,
                &prepared.config.system_prompt,
                &prepared.config.task_prompt,
                &prepared.deduplication_decisions,
                None,
            );
            assert_eq!(
                context
                    .system
                    .as_ref()
                    .unwrap()
                    .assets
                    .iter()
                    .filter(|asset| {
                        asset.source_path.as_deref() == Some(".lf/wave/release/GOAL.md")
                    })
                    .count(),
                0
            );
            assert_eq!(
                context
                    .task
                    .assets
                    .iter()
                    .filter(|asset| {
                        asset.source_path.as_deref() == Some(".lf/wave/release/GOAL.md")
                    })
                    .count(),
                usize::from(explicit)
            );
            if explicit {
                assert!(context.decisions.iter().any(|decision| {
                    decision.source_path.as_deref() == Some(".lf/wave/release/GOAL.md")
                        && decision.decision == crate::trace::ContextDecisionKind::Deduplicated
                }));
            }
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Prompt assembly reads the selected fixture Machine.
    async fn release_task_prompt_follows_parent_rename_and_reparenting() {
        let _lock = crate::journal::test_env_lock();
        let (home, store) = test_store().await;
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        for (name, memory) in [
            ("infrastructure", "Parent memory"),
            ("infrastructure/release", "Release memory"),
            ("product", "Product memory"),
            ("infrastructure/auth", "Excluded auth memory"),
        ] {
            let dir = repo.path().join(".lf/wave").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("GOAL.md"),
                format!("## Objective\n\n{name} objective\n"),
            )
            .unwrap();
            std::fs::write(dir.join("MEMORY.md"), memory).unwrap();
        }
        let release =
            crate::work::wave::ensure_wave_row(&store, repo.path(), "infrastructure/release")
                .await
                .unwrap();
        assert_eq!(release.name(), "release");
        assert_eq!(release.slug(), "infrastructure/release");
        let parent = store
            .get_wave(release.parent_wave_id().unwrap())
            .await
            .unwrap()
            .unwrap();
        let plan = project(&release, "release-plan", "release-project");
        store.create_project(&plan).await.unwrap();
        let task = task(&store, &release, &plan, repo.path().to_path_buf()).await;
        let child_row = serde_json::to_value(&release).unwrap();
        for address in ["infrastructure/release", "infra/release", "product/release"] {
            if address == "infra/release" {
                crate::work::wave::relocate::relocate_wave(
                    &store,
                    parent.id(),
                    repo.path(),
                    None,
                    Some("infra"),
                )
                .await
                .unwrap();
                // Registry relocation preserves bytes; authoring moves the directory.
                assert!(repo
                    .path()
                    .join(".lf/wave/infrastructure/release/GOAL.md")
                    .is_file());
                assert!(!repo.path().join(".lf/wave/infra").exists());
                std::fs::rename(
                    repo.path().join(".lf/wave/infrastructure"),
                    repo.path().join(".lf/wave/infra"),
                )
                .unwrap();
            } else if address == "product/release" {
                crate::work::wave::ensure_wave_row(&store, repo.path(), "product")
                    .await
                    .unwrap();
                crate::work::wave::relocate::relocate_wave(
                    &store,
                    release.id(),
                    repo.path(),
                    None,
                    Some(address),
                )
                .await
                .unwrap();
                assert!(repo.path().join(".lf/wave/infra/release/GOAL.md").is_file());
                assert!(!repo.path().join(".lf/wave/product/release").exists());
                std::fs::rename(
                    repo.path().join(".lf/wave/infra/release"),
                    repo.path().join(".lf/wave/product/release"),
                )
                .unwrap();
            }
            let discovered = store.get_wave(release.id()).await.unwrap().unwrap();
            assert_eq!(discovered.id(), release.id());
            if address == "infra/release" {
                assert_eq!(
                    serde_json::to_value(&discovered).unwrap(),
                    child_row,
                    "renaming a parent leaves the child row unchanged"
                );
                assert_eq!(
                    store.get_wave(parent.id()).await.unwrap().unwrap().name(),
                    "infra"
                );
            }
            let binding = resolve_work_binding(&store, repo.path(), "task:LOO-267")
                .await
                .unwrap();
            assert_eq!(binding.wave_name.as_deref(), Some(address));
            assert_eq!(
                store.get_task(&task.id).await.unwrap().unwrap().wave_id,
                Some(release.id().clone())
            );
            let components = crate::prompt::gather_context(&crate::prompt::GatherContextOpts {
                repo_root: binding.cwd,
                wave: Some(binding.wave_name.clone().unwrap()),
                include_diff: false,
                include_diff_files: false,
                ..Default::default()
            })
            .unwrap();
            // Source discovery preserves ancestry; diagnostics no longer inline bodies.
            let prompt = components
                .docs
                .iter()
                .map(|doc| doc.content.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let ancestor = if address.starts_with("product/") {
                "Product memory"
            } else {
                "Parent memory"
            };
            assert_eq!(prompt.matches("Release memory").count(), 1);
            assert_eq!(prompt.matches(ancestor).count(), 1);
            assert!(prompt.find(ancestor).unwrap() < prompt.find("Release memory").unwrap());
            assert!(!prompt.contains("Excluded auth memory"));
            assert!(!prompt.contains(if ancestor == "Parent memory" {
                "Product memory"
            } else {
                "Parent memory"
            }));
        }
        assert!(repo
            .path()
            .join(".lf/wave/product/release/GOAL.md")
            .is_file());
        assert!(!repo.path().join(".lf/wave/infra/release").exists());
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
        std::fs::create_dir_all(&repo).unwrap();
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
            .save_steer(
                &WorkRef::Task(task.id.clone()),
                crate::durable::Author::User,
                "ADVANCER ONLY",
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
    #[allow(clippy::await_holding_lock)] // Isolates the native Session environment.
    async fn deleted_task_retains_context_and_run_attribution() {
        let _environment = crate::journal::test_env_lock();
        let (directory, store) = test_store().await;
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_branch("jack/runtime-research");
        let wave = Wave::new(
            WaveId::new(),
            "runtime".into(),
            repo.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = project(&wave, "runtime", "project-runtime");
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, repo.path().to_path_buf()).await;
        let task = store.get_task(&task.id).await.unwrap().unwrap();
        let work = WorkRef::Task(task.id.clone());
        let pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
        std::fs::write(repo.path().join("authored.txt"), "keep this work").unwrap();

        // Context and attribution survive retirement and provider deletion.
        store.abandon(&work, "fixture retirement").await.unwrap();
        let task = store.get_task(&task.id).await.unwrap().unwrap();
        assert!(super::resolve_execution_binding(&store, repo.path())
            .await
            .unwrap()
            .is_some());
        rusqlite::Connection::open(directory.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,1)",
                rusqlite::params![wave.id().as_str(), task.linear_id.as_ref().unwrap().as_str(), task.identifier],
            )
            .unwrap();

        for selector in [
            task.id.as_str(),
            task.linear_id.as_ref().unwrap().as_str(),
            &task.identifier,
        ] {
            let binding = resolve_work_binding(&store, repo.path(), &format!("task:{selector}"))
                .await
                .unwrap();
            assert_eq!(binding.work, work);
            let capture = crate::session_record::CaptureHandle::begin_at(
                directory.path(),
                crate::session_record::SessionCaptureSpec {
                    harness: "codex".into(),
                    model: None,
                    surface: "headless".into(),
                    cwd: repo.path().to_path_buf(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: vec![crate::session_record::SubjectAttribution::declared(
                        format!("task:{selector}"),
                    )],
                    graph: crate::session_record::SessionGraphMembership::Independent,
                    work: Some(crate::session::SessionWork {
                        task_id: Some(task.id.clone()),
                        wave_id: Some(wave.id().clone()),
                        source: crate::session::WorkSource::Declared,
                    }),
                },
            )
            .unwrap();
            assert_eq!(
                store
                    .session_for_artifact(&capture.artifact_key())
                    .await
                    .unwrap()
                    .unwrap()
                    .task_id,
                Some(task.id.clone())
            );
            let previous = [
                "LF_HOME",
                "LF_RUN_DIR",
                "LF_CAPTURE_KEY",
                crate::lf::WORK_DECLARATION_ENV,
            ]
            .map(|name| (name, std::env::var_os(name)));
            std::env::set_var("LF_HOME", directory.path());
            std::env::set_var("LF_RUN_DIR", capture.artifact_dir());
            std::env::set_var("LF_CAPTURE_KEY", capture.artifact_key().as_str());
            std::env::set_var(crate::lf::WORK_DECLARATION_ENV, format!("task:{selector}"));
            crate::session_record::write_provider_session(
                &capture.artifact_dir(),
                &"saved-session".into(),
                None,
            )
            .unwrap();
            let history = crate::session_record::read_provider_session(&capture.artifact_dir())
                .unwrap()
                .unwrap();
            // The resuming invocation and its capture share this fixture's store.
            let resumed = crate::journal::with_test_ledger(
                crate::store::database_path_from_env().unwrap(),
                || crate::lf::commands::util::resume_session(&capture.artifact_key(), &history),
            );
            let error = resumed.unwrap_err().to_string();
            assert!(error.contains("was deleted"), "{error}");
            assert!(
                crate::session_record::read_provider_session(&capture.artifact_dir())
                    .unwrap()
                    .is_some()
            );
            for (name, value) in previous {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
        assert!(super::resolve_execution_binding(&store, repo.path())
            .await
            .unwrap()
            .is_some());
        assert_eq!(store.get_task(&task.id).await.unwrap(), Some(task.clone()));
        assert_eq!(store.active_task_pr(&task.id).await.unwrap(), Some(pr));
        assert_eq!(
            std::fs::read_to_string(repo.path().join("authored.txt")).unwrap(),
            "keep this work"
        );
    }

    #[tokio::test]
    async fn task_run_drill_selects_rows_by_any_name_of_the_task() {
        let (directory, store) = test_store().await;
        let repo = loopflow_test_support::TestRepo::new();
        let wave =
            crate::work::wave::ensure_wave_row(&store, repo.path(), "infrastructure/release")
                .await
                .unwrap();
        let mut project = project(&wave, "desktop", "project-desktop");
        project.slug = "desktop-renamed".into();
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, directory.path().join("workspace")).await;
        let session = |task_id: Option<TaskId>, caller: Option<String>| {
            let inherited = caller.is_some();
            crate::session::LfSession {
                captured: None,
                id: uuid::Uuid::new_v4().to_string().into(),
                artifact_key: Some(crate::session_record::new_artifact_key()),
                caller_artifact_key: caller,
                input_published: true,
                cwd: directory.path().to_path_buf(),
                skill: Some("implement".into()),
                provider: Some("codex".into()),
                model: None,
                node: None,
                iterations: None,
                work_source: task_id.as_ref().map(|_| {
                    if inherited {
                        crate::session::WorkSource::Inherited
                    } else {
                        crate::session::WorkSource::Declared
                    }
                }),
                task_id,
                wave_id: None,
                graph_lf_process_id: None,
                bound_at: None,
                interactive: false,
                repo: None,
                title: "Investigation".into(),
                title_source: crate::session::TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            }
        };
        let worker = store
            .create_session(session(Some(task.id.clone()), None))
            .await
            .unwrap();
        // Reader fixture: inherited admission is separately proved through public
        // child commands. A causal input reference does not itself assign Work.
        let helper = store
            .create_session(session(
                Some(task.id.clone()),
                Some(worker.input_key().unwrap().clone()),
            ))
            .await
            .unwrap();
        assert_eq!(helper.wave_id, Some(wave.id().clone()));
        assert_eq!(
            helper.work_source,
            Some(crate::session::WorkSource::Inherited)
        );
        store.create_session(session(None, None)).await.unwrap();
        for selector in [
            task.identifier.as_str(),
            task.id.as_str(),
            task.linear_id.as_ref().unwrap().as_str(),
        ] {
            let rows = store
                .sqlite
                .conversation_history(
                    Some(wave.slug()),
                    Some(project.id.as_str()),
                    Some(selector),
                    None,
                    0,
                    false,
                )
                .unwrap();
            assert_eq!(rows.len(), 2);
            for snapshot in &rows {
                assert_eq!(snapshot.task_id.as_ref(), Some(&task.id));
            }
            assert!(store
                .sqlite
                .conversation_history(None, Some("other"), Some(selector), None, 0, false)
                .unwrap()
                .is_empty());
        }
        let children = store
            .sqlite
            .conversation_history(
                None,
                None,
                None,
                Some(worker.input_key().unwrap().as_str()),
                0,
                false,
            )
            .unwrap();
        assert_eq!(
            children[0].artifact_key.as_deref(),
            Some(helper.input_key().unwrap().as_str())
        );
        for _ in 0..55 {
            store
                .create_session(session(
                    Some(task.id.clone()),
                    Some(worker.input_key().unwrap().clone()),
                ))
                .await
                .unwrap();
        }
        let children = store
            .sqlite
            .conversation_history(
                None,
                None,
                None,
                Some(worker.input_key().unwrap().as_str()),
                0,
                false,
            )
            .unwrap();
        assert_eq!(
            children.len(),
            56,
            "exact parent history has no presentation cap"
        );
        assert!(children
            .iter()
            .all(|snapshot| snapshot.caller_artifact_key.as_deref()
                == Some(worker.input_key().unwrap().as_str())
                && snapshot.task_id.as_ref() == Some(&task.id)));
        store.sqlite.assert_no_historical_runs();
    }

    #[tokio::test]
    async fn hierarchical_work_selectors_must_match() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
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
        let other_project = project(&other_wave, "other", "project-other");
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
        assert!(wave_error
            .to_string()
            .contains("selected Wave does not own Task"));
    }

    #[tokio::test]
    async fn direct_wave_binding_carries_graphs_and_shared_metric_context() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join(".lf/wave/runtime")).unwrap();
        std::fs::create_dir_all(repo.join(".lf/graphs")).unwrap();
        std::fs::write(repo.join(".lf/graphs/local-delivery.yaml"), "- implement\n").unwrap();
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
        let snapshot = Inventory {
            projects: vec![LinearProject {
                description: None,
                revision: None,
                id: "project-api".to_string(),
                slug: "loopflow-api".to_string(),
                name: "Loopflow API".to_string(),
                summary: String::new(),

                metric_targets: Vec::new(),
                status: crate::work::project::ProjectStatus::Started,
                krs: vec![ProjectKR {
                    text: "One model everywhere".to_string(),
                    holds: false,
                }],
                initiative_ids: vec!["initiative-1".to_string()],
                team_ids: vec!["team-1".to_string()],
            }],
            items: Vec::new(),
        };
        store
            .seed_inventory(
                InventoryFixture {
                    wave_id: wave.id().clone(),
                    provider: "linear".to_string(),
                    initiative: "initiative-1".to_string(),
                    synced_at: OffsetDateTime::now_utc().unix_timestamp(),
                    snapshot,
                },
                None,
            )
            .await
            .unwrap();

        let binding = resolve_work_binding(&store, &repo, "wave:runtime")
            .await
            .unwrap();
        assert_eq!(binding.work, WorkRef::Wave(wave.id().clone()));
        assert!(binding
            .context
            .contains("Drive the 'runtime' Wave's goal forward"));
        assert!(binding.context.contains("- local-delivery\n"));
        assert!(binding.context.contains("- pursue\n"));
        assert!(binding.context.contains("metric-portfolio"));
        assert!(resolve_work_binding(&store, &repo, "project:loopflow-api")
            .await
            .is_err());
    }
}
