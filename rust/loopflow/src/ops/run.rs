use std::path::{Path, PathBuf};

use crate::durable::{render_steers, Steer, TaskId, WorkRef};
use crate::engine::process::{execution_context, pin_control_binary, start_lf_session_with_env};
use crate::id::WaveId;
use crate::planning::ProjectPlan;
use crate::store::SharedStore;

use crate::work::task::{Task, TaskPr};
use crate::work::wave::Wave;

use super::{OpsError, OpsResult};

#[derive(Debug, Clone)]
pub struct WorkBinding {
    pub source: crate::session::WorkSource,
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

fn render_wave_context(repo: &Path, wave: &str, metric_context: &str) -> String {
    // Authored goals and memory enter once through the prompt document gatherer.
    let flows = crate::engine::available_flow_names(repo)
        .iter()
        .map(|flow| format!("- {flow}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Drive the '{wave}' Wave's goal forward using its checkout-local files.\n\n<lf:goal-context>\nAvailable flows:\n{flows}\n</lf:goal-context>\n\n{metric_context}\n\n<lf:wave-executive-loop>\n1. What is most important?\n2. What signals are arriving?\n3. What works?\n4. What does not?\n5. What is the current strategy?\n6. How should strategy adjust?\n\nTreat metrics as evidence, never as automatic KR completion or a composite Wave score.\n</lf:wave-executive-loop>"
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
            .task_prs(&task.id)
            .await
            .map_err(run_error)?
            .pop()
            .ok_or_else(|| run_error(format!("Task {} has no recorded PR", task.id)))?;
        let context = render_task_context(&task, &project.plan, &pr, wave.slug(), &steers);
        let cwd = if crate::engine::git::current_branch(repo)
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
            source: crate::session::WorkSource::Declared,
            subjects: vec![
                format!("wave:{}", wave.slug()),
                format!("project:{}", project.plan.slug),
                format!("task:{}", task.plan.identifier),
            ],
            work,
            wave_id: task.wave_id,
            wave_name: wave.slug().to_string(),
            cwd,
            context,
            agent: task.agent,
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
        let cwd = if crate::repository::CanonicalRepo::discover(Path::new(wave.repo()))
            .is_ok_and(|canonical| canonical.contains(repo))
        {
            crate::engine::git::worktree_root(repo).unwrap_or_else(|_| repo.to_path_buf())
        } else {
            PathBuf::from(wave.repo())
        };
        let context = render_wave_context(&cwd, wave.slug(), &metric_context);
        return Ok(WorkBinding {
            source: crate::session::WorkSource::Declared,
            subjects: vec![format!("wave:{}", wave.slug())],
            work: WorkRef::Wave(wave.id().clone()),
            wave_id: wave.id().clone(),
            wave_name: wave.slug().to_string(),
            cwd,
            context,
            agent: None,
        });
    }

    Err(run_error("select a Task or Wave"))
}

/// Resolve checkout ownership before an ancestor's explicit declaration.
/// This command's Task selector is resolved by the CLI before reaching this fallback.
pub async fn resolve_execution_binding(
    store: &SharedStore,
    cwd: &Path,
) -> OpsResult<Option<WorkBinding>> {
    if crate::repo::discover_repo_root(cwd)
        .map_err(run_error)?
        .is_some()
    {
        if let Some(binding) = resolve_checkout_binding(store, cwd).await? {
            return Ok(Some(binding));
        }
    }
    if let Ok(selector) = std::env::var(crate::lf::WORK_DECLARATION_ENV) {
        let mut binding = resolve_work_binding(store, cwd, &selector).await?;
        binding.cwd = cwd.to_path_buf();
        return Ok(Some(binding));
    }
    Ok(None)
}

/// Resolve checkout attribution independently of Task or PR execution eligibility.
pub async fn resolve_checkout_binding(
    store: &SharedStore,
    repo: &Path,
) -> OpsResult<Option<WorkBinding>> {
    let Some(task) = crate::ops::task::task_for_checkout(store, repo).await? else {
        return Ok(None);
    };
    let id = task.id.to_string();
    let mut binding = resolve_work_selection(
        store,
        repo,
        WorkSelection {
            task: Some(&id),
            wave: None,
        },
    )
    .await?;
    binding.source = crate::session::WorkSource::Checkout;
    binding.cwd = crate::engine::git::worktree_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    Ok(Some(binding))
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
        actual.slug(),
        requested.slug()
    )))
}

fn run_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

#[derive(Debug)]
pub(crate) struct TaskWorkerExec {
    pub task_id: TaskId,
    pub wave_id: WaveId,
    pub cwd: PathBuf,
    pub tmux_name: String,
    pub environment: Vec<(String, String)>,
}

pub(crate) async fn exec_task_worker(request: TaskWorkerExec) -> OpsResult<()> {
    let mut environment = request.environment;
    let execution = execution_context()
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
            crate::lf::WORK_DECLARATION_ENV.to_string(),
            format!("task:{}", request.task_id),
        ),
        (
            crate::work::wave::context::WAVE_ID_ENV.to_string(),
            request.wave_id.as_str().to_string(),
        ),
        ("LF_BIN".to_string(), control_bin),
        (
            "LF_DB_PATH".to_string(),
            execution.db_path.to_string_lossy().to_string(),
        ),
        (
            "LF_HOME".to_string(),
            execution.lf_home.to_string_lossy().to_string(),
        ),
        (
            "LF_DB_PATH".to_string(),
            execution.db_path.to_string_lossy().to_string(),
        ),
        (
            "LF_HOME".to_string(),
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
    environment.push((
        crate::engine::config::USER_NAME_ENV.to_string(),
        crate::engine::config::participant_name()
            .map_err(run_error)?
            .unwrap_or_default(),
    ));
    if let Ok(options) = std::env::var(crate::lf::TASK_SKILL_OPTIONS_ENV) {
        environment.push((crate::lf::TASK_SKILL_OPTIONS_ENV.to_string(), options));
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
    use crate::pm::{PmKr, PmProject, PmSnapshot};
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
                flow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
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
            agent: None,
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

    #[test]
    fn context_delivery_supplies_one_goal_for_direct_and_wave_launches() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("wave/release")).unwrap();
        let goal = "---\ncrons: []\n---\n## Objective\nShip a reliable release.\n\n## Bounds\nKeep rollback available.\n";
        std::fs::write(tmp.path().join("wave/release/GOAL.md"), goal).unwrap();
        let seed = super::render_wave_context(tmp.path(), "release", "");
        for message in [None, Some(seed)] {
            let prepared = crate::engine::exec::prepare_exec_prompt(
                &crate::engine::config::Config {
                    diff_files: false,
                    diff: false,
                    paste: false,
                    ..Default::default()
                },
                crate::engine::exec::ExecPromptInput {
                    repo_root: tmp.path().to_path_buf(),
                    wave: Some("release".into()),
                    docs: vec!["wave/release/GOAL.md".into()],
                    message,
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                prepared.prompt.matches("Ship a reliable release.").count(),
                1
            );
            assert_eq!(
                prepared.prompt.matches("Keep rollback available.").count(),
                1
            );
            assert!(prepared.config.task_prompt.contains(goal));
            let context = crate::lf::commands::run::attributed_context(
                &prepared.components,
                &prepared.config.system_prompt,
                &prepared.config.task_prompt,
                &prepared.deduplication_decisions,
            );
            assert_eq!(
                context
                    .task
                    .assets
                    .iter()
                    .filter(|asset| {
                        asset.source_path.as_deref() == Some("wave/release/GOAL.md")
                    })
                    .count(),
                1
            );
            assert!(context.decisions.iter().any(|decision| {
                decision.source_path.as_deref() == Some("wave/release/GOAL.md")
                    && decision.decision == crate::trace::ContextDecisionKind::Deduplicated
            }));
        }
    }

    #[tokio::test]
    async fn release_task_prompt_follows_parent_rename_and_reparenting() {
        let (_home, store) = test_store().await;
        let repo = loopflow_test_support::TestRepo::new();
        for (name, memory) in [
            ("infrastructure", "Parent memory"),
            ("infrastructure/release", "Release memory"),
            ("product", "Product memory"),
            ("infrastructure/auth", "Excluded auth memory"),
        ] {
            let dir = repo.path().join("wave").join(name);
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
                std::fs::rename(
                    repo.path().join("wave/infrastructure"),
                    repo.path().join("wave/infra"),
                )
                .unwrap();
            } else if address == "product/release" {
                std::fs::rename(
                    repo.path().join("wave/infra/release"),
                    repo.path().join("wave/product/release"),
                )
                .unwrap();
            }
            let discovered = crate::work::wave::ensure_wave_row(&store, repo.path(), address)
                .await
                .unwrap();
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
            assert_eq!(binding.wave_name, address);
            assert_eq!(
                store.get_task(&task.id).await.unwrap().unwrap().wave_id,
                *release.id()
            );
            let components = crate::engine::gather_context(&crate::engine::GatherContextOpts {
                repo_root: binding.cwd,
                wave: Some(binding.wave_name),
                include_diff: false,
                include_diff_files: false,
                ..Default::default()
            })
            .unwrap();
            let prompt =
                crate::engine::format_prompt(crate::engine::PromptFormatMode::Full, &components);
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
        // A new Home reconstructs the same identity from the authored files.
        let (_fresh_home, fresh) = test_store().await;
        let recovered = crate::work::wave::ensure_wave_row(&fresh, repo.path(), "product/release")
            .await
            .unwrap();
        assert_eq!(recovered.id(), release.id());
        assert_eq!(recovered.slug(), "product/release");
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
    #[allow(clippy::await_holding_lock)] // Isolates the native Session and Ask environment.
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
        assert!(super::resolve_checkout_binding(&store, repo.path())
            .await
            .unwrap()
            .is_some());
        rusqlite::Connection::open(directory.path().join("registry.db"))
            .unwrap()
            .execute(
                "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,1)",
                rusqlite::params![wave.id().as_str(), task.plan.id.as_str(), task.plan.identifier],
            )
            .unwrap();

        for selector in [
            task.id.as_str(),
            task.plan.id.as_str(),
            &task.plan.identifier,
        ] {
            let binding = resolve_work_binding(&store, repo.path(), &format!("task:{selector}"))
                .await
                .unwrap();
            assert_eq!(binding.work, work);
            let previous_db = std::env::var_os("LF_DB_PATH");
            std::env::set_var("LF_DB_PATH", directory.path().join("registry.db"));
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
                    flow: crate::session_record::SessionFlowMembership::Independent,
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
                "LF_CONTROL_DB_PATH",
                "LF_CONTROL_HOME",
                "LF_HOME",
                "LF_RUN_DIR",
                "LF_RUN_ID",
                crate::lf::WORK_DECLARATION_ENV,
            ]
            .map(|name| (name, std::env::var_os(name)));
            std::env::set_var("LF_CONTROL_DB_PATH", directory.path().join("registry.db"));
            std::env::set_var("LF_CONTROL_HOME", directory.path());
            std::env::set_var("LF_HOME", directory.path());
            std::env::set_var("LF_RUN_DIR", capture.artifact_dir());
            std::env::set_var("LF_RUN_ID", capture.artifact_key().as_str());
            std::env::set_var(crate::lf::WORK_DECLARATION_ENV, format!("task:{selector}"));
            crate::session_record::write_provider_session(
                &capture.artifact_dir(),
                "saved-session",
                None,
            )
            .unwrap();
            let history = crate::session_record::read_provider_session(&capture.artifact_dir())
                .unwrap()
                .unwrap();
            let resumed = crate::lf::commands::util::resume_session(
                "codex",
                None,
                repo.path(),
                &capture.artifact_key(),
                &capture.artifact_dir(),
                &history,
            );
            let asked = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                crate::ops::human_session::ask(&store, "Continue removed work", None),
            )
            .await;
            assert!(resumed.unwrap_err().to_string().contains("was deleted"));
            assert!(asked
                .expect("Ask must refuse before waiting")
                .unwrap_err()
                .to_string()
                .contains("was deleted"));
            assert!(
                crate::session_record::read_provider_clients(&capture.artifact_dir())
                    .unwrap()
                    .is_empty()
            );
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
            match previous_db {
                Some(value) => std::env::set_var("LF_DB_PATH", value),
                None => std::env::remove_var("LF_DB_PATH"),
            }
        }
        assert!(super::resolve_checkout_binding(&store, repo.path())
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
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, directory.path().join("workspace")).await;
        // Input ancestry names its Task by id, so Project renaming preserves history.
        project.plan.slug = "desktop-renamed".into();
        store.update_project(&project).await.unwrap();
        let session = |task_id: Option<TaskId>, caller: Option<String>| {
            let inherited = caller.is_some();
            crate::session::AgentSession {
                captured: None,
                id: uuid::Uuid::new_v4().to_string(),
                artifact_key: crate::session_record::new_artifact_key(),
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
                flow_session_id: None,
                bound_at: None,
                kind: crate::session::SessionKind::Conversation,
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
            .create_session(session(Some(task.id.clone()), None), None)
            .await
            .unwrap();
        // Reader fixture: inherited admission is separately proved through public
        // child commands. A causal input reference does not itself assign Work.
        let helper = store
            .create_session(
                session(Some(task.id.clone()), Some(worker.artifact_key.clone())),
                None,
            )
            .await
            .unwrap();
        assert_eq!(helper.wave_id, Some(wave.id().clone()));
        assert_eq!(
            helper.work_source,
            Some(crate::session::WorkSource::Inherited)
        );
        store
            .create_session(session(None, None), None)
            .await
            .unwrap();
        for selector in [
            task.plan.identifier.as_str(),
            task.id.as_str(),
            task.plan.id.as_str(),
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
                Some(worker.artifact_key.as_str()),
                0,
                false,
            )
            .unwrap();
        assert_eq!(
            children[0].artifact_key.as_deref(),
            Some(helper.artifact_key.as_str())
        );
        for _ in 0..55 {
            store
                .create_session(
                    session(Some(task.id.clone()), Some(worker.artifact_key.clone())),
                    None,
                )
                .await
                .unwrap();
        }
        let children = store
            .sqlite
            .conversation_history(
                None,
                None,
                None,
                Some(worker.artifact_key.as_str()),
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
                == Some(worker.artifact_key.as_str())
                && snapshot.task_id.as_ref() == Some(&task.id)));
        store.sqlite.assert_no_historical_runs();
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
        assert!(wave_error.to_string().contains("belongs to Wave"));
    }

    #[tokio::test]
    async fn direct_wave_binding_carries_flows_and_shared_metric_context() {
        let (directory, store) = test_store().await;
        let repo = directory.path().join("repo");
        std::fs::create_dir_all(repo.join("wave/runtime")).unwrap();
        std::fs::create_dir_all(repo.join(".lf/flows")).unwrap();
        std::fs::write(repo.join(".lf/flows/local-delivery.yaml"), "- implement\n").unwrap();
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
                revision: None,
                id: "project-api".to_string(),
                slug: "loopflow-api".to_string(),
                name: "Loopflow API".to_string(),
                summary: String::new(),

                metric_targets: Vec::new(),
                flow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
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
                snapshot,
            })
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
        assert!(binding.context.contains("- feature\n"));
        assert!(binding.context.contains("metric-portfolio"));
        assert!(resolve_work_binding(&store, &repo, "project:loopflow-api")
            .await
            .is_err());
    }
}
