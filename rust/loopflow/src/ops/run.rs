use std::path::{Path, PathBuf};

use crate::durable::{render_steers, Steer, WorkRef};
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
        project_id = project.linear_id.as_ref().map(|id| id.as_str()).unwrap_or("local"),
        project_context = project.prompt_context,
        direction = render_steers(steers),
        task_snapshot_synced_at = task.plan.pm_snapshot_synced_at.map(|at| at.to_string()).unwrap_or_else(|| "not observed".into()),
        project_snapshot_synced_at = project.pm_snapshot_synced_at.map(|at| at.to_string()).unwrap_or_else(|| "not observed".into()),
        wave = wave_name,
        task_id = task.id,
        worktree = task.worktree.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "unplaced".into()),
        pr_sequence = pr.sequence,
        pr_branch = pr.branch,
        base_commit = pr.base_commit,
        placement = placement,
    )
}

fn render_wave_context(repo: &Path, wave: &str, metric_context: &str) -> String {
    // Authored goals and memory enter once through the prompt document gatherer.
    let flows = match crate::engine::available_flow_names(repo) {
        Ok(names) => names
            .iter()
            .map(|flow| format!("- {flow}"))
            .collect::<Vec<_>>()
            .join("\n"),
        Err(error) => format!("Flow catalog unavailable: {error}"),
    };
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
        let task = crate::ops::task::resolve_task(store, repo, value).await?;
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
        let mut context = render_task_context(&task, &project.plan, &pr, wave.slug(), &steers);
        if let Ok(Some(workflow)) = store.sqlite.workflow(&task.id) {
            context.push_str(&format!("\n\n{}", workflow.guidance(&task.plan.identifier)));
        }
        let cwd = if crate::engine::git::current_branch(repo)
            .ok()
            .flatten()
            .as_deref()
            == Some(pr.branch.as_str())
        {
            crate::engine::git::worktree_root(repo).unwrap_or_else(|_| repo.to_path_buf())
        } else {
            task.worktree()?.clone()
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
            plan: ProjectPlan {
                summary: String::new(),
                workflow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
                linear_id: Some(LinearProjectId::new(planning_id).unwrap()),
                slug: slug.to_string(),
                name: slug.to_string(),
                prompt_context: "Ship the requested behavior.".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
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
                revision: 0,
                linear_id: Some(LinearIssueId::new("runtime-research").unwrap()),
                identifier: "LOO-267".to_string(),
                title: "Research the runtime".to_string(),
                description: "Compare independent findings.".to_string(),
                pm_snapshot_synced_at: Some(now.unix_timestamp()),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: Some(worktree),
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
        crate::store::sqlite::project_selection::write_project_binding(
            &store.sqlite,
            wave.id(),
            None,
            project.plan.linear_id.as_ref().unwrap().as_str(),
            &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
        )
        .unwrap();
        store.seed_task(&task, &pr).await.unwrap();
        task
    }

    #[test]
    fn context_delivery_supplies_one_goal_for_direct_and_wave_launches() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::flow::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        let tmp = loopflow_test_support::TestRepo::new();
        std::fs::create_dir_all(tmp.path().join("wave/release")).unwrap();
        let goal = "---\ncrons: []\n---\n## Objective\nShip a reliable release.\n\n## Bounds\nKeep rollback available.\n";
        std::fs::write(tmp.path().join("wave/release/GOAL.md"), goal).unwrap();
        store
            .ensure_wave(
                &crate::repository::CanonicalRepo::discover(tmp.path())
                    .unwrap()
                    .to_string(),
                "release",
            )
            .unwrap();
        let seed = super::render_wave_context(tmp.path(), "release", "");
        for message in [None, Some(seed)] {
            let prepared = crate::engine::process_prompt::prepare_process_prompt(
                &crate::engine::config::Config {
                    diff_files: false,
                    diff: false,
                    paste: false,
                    ..Default::default()
                },
                crate::engine::process_prompt::ProcessPromptInput {
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
            assert!(prepared.config.system_prompt.contains(goal));
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
    #[allow(clippy::await_holding_lock)] // Prompt assembly reads the selected fixture Machine.
    async fn release_task_prompt_follows_parent_rename_and_reparenting() {
        let _lock = crate::journal::test_env_lock();
        let (home, store) = test_store().await;
        let _home =
            crate::lf::commands::flow::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
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
                crate::work::wave::relocate::relocate_wave(
                    &store,
                    parent.id(),
                    repo.path(),
                    None,
                    Some("infra"),
                )
                .await
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
            let prompt = crate::engine::format_prompt(&components);
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
            .join("wave/infrastructure/release/GOAL.md")
            .exists());
        assert!(!repo.path().join("wave/infra").exists());
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
            .append_steer(
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
        assert!(super::resolve_checkout_binding(&store, repo.path())
            .await
            .unwrap()
            .is_some());
        rusqlite::Connection::open(directory.path().join("loopflow.db"))
            .unwrap()
            .execute(
                "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,1)",
                rusqlite::params![wave.id().as_str(), task.plan.linear_id.as_ref().unwrap().as_str(), task.plan.identifier],
            )
            .unwrap();

        for selector in [
            task.id.as_str(),
            task.plan.linear_id.as_ref().unwrap().as_str(),
            &task.plan.identifier,
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
                &history,
            );
            assert!(resumed.unwrap_err().to_string().contains("was deleted"));
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
        project.plan.slug = "desktop-renamed".into();
        store.create_project(&project).await.unwrap();
        let task = task(&store, &wave, &project, directory.path().join("workspace")).await;
        let session = |task_id: Option<TaskId>, caller: Option<String>| {
            let inherited = caller.is_some();
            crate::session::LfSession {
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
                flow_process_lfid: None,
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
                Some(worker.artifact_key.clone()),
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
            task.plan.identifier.as_str(),
            task.id.as_str(),
            task.plan.linear_id.as_ref().unwrap().as_str(),
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
                .create_session(session(
                    Some(task.id.clone()),
                    Some(worker.artifact_key.clone()),
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
                workflow: "feature".into(),
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
            .put_pm_snapshot(
                PmSnapshotRow {
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
