use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::child_session::{
    ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandSource, ChildCommandState,
    ChildDecisionId, ChildDirective, ChildProcessGeneration, ChildRef,
};
use crate::engine::config::{load_config_or_default, parse_agent};
use crate::engine::git::{get_default_branch, rev_parse};
use crate::engine::process::{start_lf_session_with_env, tmux_session_exists, tmux_session_slug};
use crate::engine::worktrees::{
    create_from_placement_plan, plan_placement, PlacementStrategy, WorktreeSegment,
};
use crate::id::WaveId;
use crate::ops::error::{OpsError, OpsResult};
use crate::session_context::{
    LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot, TaskLaunchReceipt,
};
use crate::store::{open_existing_store, SharedStore, StoreError};
use crate::task::{
    PmWritebackOperation, PmWritebackState, TaskEventKind, TaskSession, TaskSessionStatus,
};
use crate::wave::Wave;
use sha2::{Digest, Sha256};

use super::ChildReceiptUntil;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskWaitUntil {
    Submitted,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskControlResult {
    pub issue_id: String,
    pub session_id: String,
    pub command_id: String,
    pub directive_version: Option<u32>,
    pub state: ChildCommandState,
    pub effect: Option<ChildCommandEffect>,
    pub incorporated: bool,
    pub generation: Option<u32>,
    pub accepted_at: Option<time::OffsetDateTime>,
    pub incorporated_at: Option<time::OffsetDateTime>,
    pub error: Option<String>,
}

fn task_control_result(
    issue_id: String,
    result: super::child::ChildControlResult,
) -> TaskControlResult {
    TaskControlResult {
        issue_id,
        session_id: result.session_id,
        command_id: result.command_id,
        directive_version: result.directive_version,
        state: result.state,
        effect: result.effect,
        incorporated: result.incorporated,
        generation: result.generation,
        accepted_at: result.accepted_at,
        incorporated_at: result.incorporated_at,
        error: result.error,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReceiptRead {
    pub receipt: TaskControlResult,
    pub timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskDecisionResult {
    pub issue_id: String,
    pub session_id: String,
    pub decision_id: String,
    pub resolved: bool,
    pub choice: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskSessionSnapshot {
    pub issue_id: String,
    pub issue_identifier: String,
    pub session_id: String,
    pub project_id: String,
    pub project: String,
    pub pm_snapshot_synced_at: i64,
    pub pm_writeback: crate::task::PmWritebackState,
    pub wave: String,
    pub project_session_id: String,
    pub current_directive_version: u32,
    pub incorporated_directive_version: u32,
    pub status: TaskSessionStatus,
    pub status_reason: String,
    pub status_at: time::OffsetDateTime,
    pub worktree: String,
    pub branch: String,
    pub base_commit: String,
    pub agent: String,
    pub provider: String,
    pub provider_session_id: Option<String>,
    pub process_alive: bool,
    pub latest_process: Option<ChildProcessGeneration>,
    pub pull_request: Option<crate::task::PullRequestRef>,
    pub latest_event: Option<crate::task::TaskEvent>,
    pub created_at: time::OffsetDateTime,
    pub updated_at: time::OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskChangedFile {
    pub path: String,
    pub committed: bool,
    pub staged: bool,
    pub unstaged: bool,
    pub untracked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskChangesSnapshot {
    pub issue_identifier: String,
    pub session_id: String,
    pub base_commit: String,
    pub head_commit: String,
    pub files: Vec<TaskChangedFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskDiffSnapshot {
    pub issue_identifier: String,
    pub session_id: String,
    pub path: Option<String>,
    pub patch: String,
    pub binary: bool,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TaskFileSnapshot {
    pub issue_identifier: String,
    pub session_id: String,
    pub path: String,
    pub content: Option<String>,
    pub binary: bool,
    pub size_bytes: u64,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy)]
struct TaskWorkspace<'a> {
    issue_identifier: &'a str,
    session_id: &'a crate::task::TaskSessionId,
    worktree: &'a Path,
    base_commit: &'a str,
}

impl<'a> From<&'a TaskSession> for TaskWorkspace<'a> {
    fn from(session: &'a TaskSession) -> Self {
        Self {
            issue_identifier: &session.launch.issue.identifier,
            session_id: &session.id,
            worktree: &session.worktree,
            base_commit: &session.base_commit,
        }
    }
}

fn task_error(message: impl Into<String>) -> OpsError {
    OpsError::Message(message.into())
}

fn block_on_task<T>(future: impl std::future::Future<Output = OpsResult<T>>) -> OpsResult<T> {
    tokio::runtime::Runtime::new()
        .map_err(|error| task_error(format!("failed to build task runtime: {error}")))?
        .block_on(future)
}

async fn task_store() -> OpsResult<SharedStore> {
    open_existing_store().await.map(Arc::new).ok_or_else(|| {
        task_error("no Loopflow registry on this machine; start the owning Wave first")
    })
}

async fn owning_wave(store: &SharedStore, session: &TaskSession) -> OpsResult<Wave> {
    store
        .get_wave(&session.wave_id)
        .await
        .map_err(|error| task_error(format!("failed to read owning Wave: {error}")))?
        .ok_or_else(|| task_error(format!("owning Wave {} is not registered", session.wave_id)))
}

fn command_source(session: &TaskSession) -> OpsResult<ChildCommandSource> {
    match std::env::var("LF_PROJECT_SESSION_ID") {
        Ok(value) => {
            let project_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            return if session.project_session_id == project_id {
                Ok(ChildCommandSource::Project(project_id))
            } else {
                Err(task_error(format!(
                    "Project Session {project_id} cannot control Task {}; its Project Session is {}",
                    session.launch.issue.identifier, session.project_session_id
                )))
            };
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
    let ambient = match std::env::var(crate::engine::wave_context::WAVE_ID_ENV) {
        Ok(value) => Some(
            WaveId::parse(&value)
                .map_err(|error| task_error(format!("invalid ambient Wave id: {error}")))?,
        ),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(task_error("ambient Wave id is not valid UTF-8"))
        }
    };
    command_source_for_wave(ambient, &session.wave_id, &session.launch.issue.identifier)
}

fn command_source_for_wave(
    ambient: Option<WaveId>,
    owning_wave_id: &WaveId,
    issue_identifier: &str,
) -> OpsResult<ChildCommandSource> {
    match ambient {
        Some(wave_id) if &wave_id == owning_wave_id => Ok(ChildCommandSource::Wave(wave_id)),
        Some(wave_id) => Err(task_error(format!(
            "Wave {wave_id} cannot control Task {issue_identifier} owned by Wave {owning_wave_id}"
        ))),
        None => Ok(ChildCommandSource::Human),
    }
}

pub fn task_run(repo: &Path, issue: &str, directive: Option<String>) -> OpsResult<TaskSession> {
    let directive = directive
        .map(|directive| {
            let directive = directive.trim().to_string();
            if directive.is_empty() {
                Err(task_error("directive cannot be empty"))
            } else {
                Ok(directive)
            }
        })
        .transpose()?;
    if let Some(existing) = block_on_task(async {
        let store = task_store().await?;
        let existing = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?;
        if let Some(session) = &existing {
            if let Some(requested) = directive.as_deref() {
                let current = store
                    .child_directives(&ChildRef::Task(session.id.clone()))
                    .await
                    .map_err(|error| task_error(error.to_string()))?
                    .into_iter()
                    .find(|value| value.version == session.current_directive_version)
                    .ok_or_else(|| task_error("Task Session has no current directive"))?;
                if current.text != requested {
                    return Err(task_error(format!(
                        "Task {} already exists with directive v{}; use `lf task steer {} <new-direction>` to replace it",
                        session.launch.issue.identifier,
                        current.version,
                        session.launch.issue.identifier,
                    )));
                }
            }
        }
        Ok(existing)
    })? {
        return Ok(existing);
    }
    let main_repo = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let resolved =
        crate::ops::task_pm::resolve_task(&main_repo, issue, crate::ops::pm::PmRefresh::Auto)?;
    let segment = WorktreeSegment::parse(&resolved.item.identifier)
        .map_err(|error| task_error(error.to_string()))?;
    let plan = plan_placement(&main_repo, segment)
        .map_err(|error| task_error(format!("failed to plan task worktree: {error}")))?;
    if plan.strategy != PlacementStrategy::Create {
        return Err(task_error(format!(
            "task worktree or branch already exists without a Task Session: {} ({})",
            plan.worktree_path.display(),
            plan.branch
        )));
    }
    let default_branch =
        get_default_branch(&main_repo).map_err(|error| task_error(error.to_string()))?;
    let base_commit = rev_parse(&main_repo, &default_branch)
        .map_err(|error| task_error(format!("failed to resolve task base: {error}")))?;
    let project_session = crate::ops::project::ensure_project_session_for_task(
        &main_repo,
        crate::ops::task_pm::ResolvedProject {
            snapshot: resolved.snapshot.clone(),
            project: resolved.project.clone(),
        },
    )?;
    let project_session_id = task_project_session_id(&project_session)?;
    let wave_id = project_session.wave_id.clone();
    let config = load_config_or_default(Some(&main_repo));
    let agent = config.agent.as_deref().unwrap_or("claude:opus");
    let (provider, _) = parse_agent(agent);
    let agent = agent.to_string();
    let directive = directive.unwrap_or_else(|| {
        format!(
            "Complete {}: {}\n\n{}",
            resolved.item.identifier, resolved.item.name, resolved.item.description
        )
    });

    block_on_task(async move {
        let store = task_store().await?;
        if let Some(existing) = store
            .get_task_session_by_issue(&resolved.item.id)
            .await
            .map_err(|error| task_error(format!("failed to read task registry: {error}")))?
        {
            return Ok(existing);
        }
        let now = time::OffsetDateTime::now_utc();
        let mut session = TaskSession {
            id: crate::task::TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(resolved.item.id.clone())
                        .map_err(|error| task_error(error.to_string()))?,
                    identifier: resolved.item.identifier.clone(),
                    title: resolved.item.name.clone(),
                    description: resolved.item.description.clone(),
                },
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(resolved.project.id.clone())
                        .map_err(|error| task_error(error.to_string()))?,
                    slug: resolved.project.slug.clone(),
                    name: resolved.project.name.clone(),
                    prompt_context: project_context(&resolved.project),
                },
                pm_snapshot_synced_at: resolved.snapshot.synced_at,
            },
            wave_id,
            project_session_id,
            current_directive_version: 1,
            incorporated_directive_version: 0,
            pm_writeback: PmWritebackState::Current,
            status: TaskSessionStatus::Created,
            status_reason: "Linear task reserved before placement".to_string(),
            status_at: now,
            worktree: plan.worktree_path.clone(),
            branch: plan.branch.clone(),
            base_commit,
            agent,
            provider,
            provider_session_id: None,
            latest_process: None,
            pull_request: None,
            execution: Some(
                crate::engine::process::pinned_execution_context()
                    .map_err(|error| task_error(error.to_string()))?,
            ),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };

        let initial = ChildDirective::initial(
            ChildRef::Task(session.id.clone()),
            directive,
            command_source(&session)?,
        );
        match store
            .reserve_task_session_with_directive(&session, &initial)
            .await
        {
            Ok(()) => {}
            Err(StoreError::Sqlite(_)) => {
                if let Some(existing) = store
                    .get_task_session_by_issue(&resolved.item.id)
                    .await
                    .map_err(|error| {
                        task_error(format!("failed to recover task reservation: {error}"))
                    })?
                {
                    return Ok(existing);
                }
                return Err(task_error(
                    "task reservation collided with another task placement",
                ));
            }
            Err(error) => return Err(task_error(format!("failed to reserve task: {error}"))),
        }

        store
            .append_task_event(
                &session.id,
                &TaskEventKind::DirectiveChanged {
                    directive_id: initial.id,
                    version: initial.version,
                    directive_kind: initial.kind,
                },
            )
            .await
            .map_err(|error| task_error(error.to_string()))?;

        if let Err(error) = create_from_placement_plan(&main_repo, &plan) {
            record_task_failure(
                &store,
                &mut session,
                format!("worktree creation failed: {error}"),
                error.to_string(),
            )
            .await?;
            return Err(task_error(format!(
                "failed to create task worktree: {error}"
            )));
        }

        launch_task_process(&store, &mut session).await?;
        wait_until_running(&store, &session.id).await
    })
}

fn task_project_session_id(
    project: &crate::project_session::ProjectSession,
) -> OpsResult<crate::project_session::ProjectSessionId> {
    match std::env::var("LF_PROJECT_SESSION_ID") {
        Ok(value) => {
            let session_id =
                crate::project_session::ProjectSessionId::parse(&value).map_err(|error| {
                    task_error(format!("invalid ambient Project Session id: {error}"))
                })?;
            if session_id != project.id {
                return Err(task_error(format!(
                    "Project Session {session_id} cannot supervise a Task under Project Session {}",
                    project.id
                )));
            }
            Ok(session_id)
        }
        Err(std::env::VarError::NotPresent) => Ok(project.id.clone()),
        Err(std::env::VarError::NotUnicode(_)) => {
            Err(task_error("ambient Project Session id is not valid UTF-8"))
        }
    }
}

pub(crate) fn project_context(project: &crate::pm::PmProject) -> String {
    let mut context = format!("Definition:\n{}", project.definition.trim());
    if !project.krs.is_empty() {
        context.push_str("\n\nKRs:");
        for kr in &project.krs {
            let mark = if kr.holds { "x" } else { " " };
            context.push_str(&format!("\n- [{mark}] {}", kr.text));
        }
    }
    context
}

pub fn task_start(
    repo: &Path,
    title: String,
    project_id: &str,
    directive: Option<String>,
) -> OpsResult<TaskSession> {
    let main = crate::ops::project::ensure_clean_main(repo, "Task start")
        .map_err(|error| task_error(error.to_string()))?;
    let project =
        crate::ops::task_pm::resolve_project(&main, project_id, crate::ops::pm::PmRefresh::Auto)?;
    crate::ops::project::require_registered_wave(&project.snapshot.wave)
        .map_err(|error| task_error(error.to_string()))?;
    let marker = format!(
        "<!-- loopflow-task-start:{} -->",
        hex::encode(Sha256::digest(
            format!("{}\0{}", project.project.id, title).as_bytes()
        ))
    );
    let created = crate::ops::task_pm::create_and_load_task(
        &main,
        &project.snapshot.wave,
        &project.project.slug,
        &title,
        &marker,
    )?;
    task_run(&main, &created.item.id, directive)
}

/// Record a Task Session's transition into `Failed`: set the status, persist it,
/// and append the paired `StatusChanged` + `Failed` events. Callers keep their
/// own return value; this only writes the durable failure record.
async fn record_task_failure(
    store: &SharedStore,
    session: &mut TaskSession,
    reason: impl Into<String>,
    error: String,
) -> OpsResult<()> {
    let from = session.status;
    session.set_status(TaskSessionStatus::Failed, reason);
    store
        .update_task_session(session)
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: session.status_reason.clone(),
            },
        )
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error,
                resumable: true,
            },
        )
        .await
        .map_err(|store_error| task_error(store_error.to_string()))?;
    Ok(())
}

/// Atomically start a fresh process generation for an inactive Session.
pub(crate) async fn relaunch_inactive_process(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    launch_task_process(store, session).await
}

async fn launch_task_process(store: &SharedStore, session: &mut TaskSession) -> OpsResult<()> {
    // Resolve the pinned context before reserving anything: a Session that cannot
    // name its own binary must not burn a generation discovering that.
    let execution = session.execution.clone().ok_or_else(|| {
        task_error(format!(
            "Task {} predates pinned execution context and cannot be relaunched safely; \
             abandon it and run the Linear task again to create a Session that records \
             its own `lf` and database",
            session.launch.issue.identifier
        ))
    })?;
    let tmux_name = format!(
        "lf-task-{}-{}",
        tmux_session_slug(&session.launch.issue.identifier),
        &session.id.as_str()[3..11]
    );
    let from = session.status;
    let mut launch = session.clone();
    let generation = launch.begin_generation(tmux_name.clone());
    let reserved = store
        .reserve_task_process(&launch, from)
        .await
        .map_err(|error| task_error(format!("failed to reserve task process: {error}")))?;
    if !reserved {
        let current = store
            .get_task_session(&session.id)
            .await
            .map_err(|error| task_error(format!("failed to reread task process: {error}")))?
            .ok_or_else(|| task_error("Task Session disappeared during process reservation"))?;
        if current.status.is_process_active() {
            *session = current;
            return Ok(());
        }
        if current.status.is_terminal() {
            return Err(task_error(format!(
                "task {} became {}; terminal Task Sessions cannot start a process",
                current.launch.issue.identifier,
                current.status.as_str()
            )));
        }
        return Err(task_error(format!(
            "task {} changed from {} to {} during process reservation; retry the command",
            current.launch.issue.identifier,
            from.as_str(),
            current.status.as_str()
        )));
    }
    *session = launch;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Starting,
                reason: "task process is starting".to_string(),
            },
        )
        .await
        .map_err(|error| task_error(error.to_string()))?;

    // argv[0] and the store come from the Session, never from this process:
    // whoever queued the command does not get to choose the child's binary or
    // its database.
    let argv = vec![
        execution.lf_bin.to_string_lossy().to_string(),
        "__task".to_string(),
        session.id.to_string(),
        "--generation".to_string(),
        generation.to_string(),
    ];
    let generation_text = generation.to_string();
    let db_path = execution.db_path.to_string_lossy().to_string();
    let lf_home = execution.lf_home.to_string_lossy().to_string();
    let environment = [
        (
            crate::engine::wave_context::WAVE_ID_ENV,
            session.wave_id.as_str(),
        ),
        ("LF_TASK_SESSION_ID", session.id.as_str()),
        ("LF_TASK_GENERATION", generation_text.as_str()),
        ("LF_DB_PATH", db_path.as_str()),
        ("LF_HOME", lf_home.as_str()),
    ];
    if let Err(error) =
        start_lf_session_with_env(&tmux_name, &session.worktree, &argv, &environment).await
    {
        record_task_failure(
            store,
            session,
            format!("task process launch failed: {error}"),
            error.to_string(),
        )
        .await?;
        return Err(task_error(format!(
            "failed to launch task process: {error}"
        )));
    }
    Ok(())
}

async fn wait_until_running(
    store: &SharedStore,
    session_id: &crate::task::TaskSessionId,
) -> OpsResult<TaskSession> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let session = store
            .get_task_session(session_id)
            .await
            .map_err(|error| task_error(format!("failed to observe task startup: {error}")))?
            .ok_or_else(|| task_error("task session disappeared during startup"))?;
        if session.status != TaskSessionStatus::Starting {
            return if session.status == TaskSessionStatus::Running {
                Ok(session)
            } else {
                Err(task_error(format!(
                    "task {} did not start: {}",
                    session.launch.issue.identifier, session.status_reason
                )))
            };
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(task_error(format!(
                "task {} process did not report running within 10 seconds",
                session.launch.issue.identifier
            )));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn reconcile_process_liveness(
    store: &SharedStore,
    session: &mut TaskSession,
) -> OpsResult<()> {
    if !session.status.is_process_active() {
        return Ok(());
    }
    let alive = match session.latest_process.as_ref() {
        Some(process) => tmux_session_exists(&process.tmux_name)
            .await
            .map_err(|error| task_error(error.to_string()))?,
        None => false,
    };
    if alive {
        return Ok(());
    }
    let reason = "task process is missing; resume the same Task Session with `lf task resume`";
    record_task_failure(store, session, reason, reason.to_string()).await
}

pub fn task_status(issue: &str) -> OpsResult<TaskSession> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to read task status: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_process_liveness(&store, &mut session).await?;
        if session.status == TaskSessionStatus::Failed {
            return Ok(session);
        }
        if session.status == TaskSessionStatus::Merged
            && matches!(session.pm_writeback, PmWritebackState::Pending { .. })
        {
            retry_pm_writeback(&store, &mut session).await;
            store
                .update_task_session(&session)
                .await
                .map_err(|error| task_error(error.to_string()))?;
            return Ok(session);
        }
        if !session.status.is_process_active() && !session.status.is_terminal() {
            if let Some(pr) = crate::ops::current_or_merged_pr(&session.worktree)? {
                let from = session.status;
                let pull_request = crate::task::PullRequestRef {
                    number: pr.number as u32,
                    url: pr.url.clone(),
                };
                session.pull_request = Some(pull_request.clone());
                let event = if pr.state == "merged" {
                    session.set_status(
                        TaskSessionStatus::Merged,
                        format!("pull request #{} merged", pr.number),
                    );
                    reconcile_pm_writeback(&store, &mut session).await;
                    TaskEventKind::Completed {
                        pull_request,
                        summary: "merge observed by task status".to_string(),
                    }
                } else {
                    session.set_status(
                        TaskSessionStatus::Submitted,
                        format!("pull request #{} is open for review", pr.number),
                    );
                    TaskEventKind::PullRequestOpened {
                        number: pr.number as u32,
                        url: pr.url,
                    }
                };
                store
                    .update_task_session(&session)
                    .await
                    .map_err(|error| task_error(error.to_string()))?;
                if from != session.status {
                    store
                        .append_task_event(
                            &session.id,
                            &TaskEventKind::StatusChanged {
                                from,
                                to: session.status,
                                reason: session.status_reason.clone(),
                            },
                        )
                        .await
                        .map_err(|error| task_error(error.to_string()))?;
                    store
                        .append_task_event(&session.id, &event)
                        .await
                        .map_err(|error| task_error(error.to_string()))?;
                }
            }
        }
        Ok(session)
    })
}

fn writeback_state(result: OpsResult<()>) -> PmWritebackState {
    match result {
        Ok(()) => PmWritebackState::Current,
        Err(error) => PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: error.to_string(),
        },
    }
}

pub(crate) async fn reconcile_pm_writeback(store: &SharedStore, session: &mut TaskSession) {
    let Some(pull_request) = session.pull_request.as_ref() else {
        return;
    };
    let Ok(wave) = owning_wave(store, session).await else {
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", session.wave_id),
        };
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::complete_task(
            &session.worktree,
            wave.name(),
            session.launch.issue.id.as_str(),
            &pull_request.url,
        )
        .await,
    );
}

async fn retry_pm_writeback(store: &SharedStore, session: &mut TaskSession) {
    let Some(pull_request) = session.pull_request.as_ref() else {
        return;
    };
    let Ok(wave) = owning_wave(store, session).await else {
        session.pm_writeback = PmWritebackState::Pending {
            operation: PmWritebackOperation::CompleteTask,
            error: format!("owning Wave {} is not registered", session.wave_id),
        };
        return;
    };
    session.pm_writeback = writeback_state(
        crate::ops::task_pm::retry_complete_task(
            &session.worktree,
            wave.name(),
            session.launch.issue.id.as_str(),
            &pull_request.url,
        )
        .await,
    );
    session.updated_at = time::OffsetDateTime::now_utc();
}

pub fn task_snapshot(session: &TaskSession) -> OpsResult<TaskSessionSnapshot> {
    let session = session.clone();
    block_on_task(async move {
        let store = task_store().await?;
        let wave = owning_wave(&store, &session).await?;
        let process_alive = if session.status.is_process_active() {
            match session.latest_process.as_ref() {
                Some(process) => tmux_session_exists(&process.tmux_name)
                    .await
                    .map_err(|error| task_error(error.to_string()))?,
                None => false,
            }
        } else {
            false
        };
        let latest_event = store
            .task_events_after(&session.id, 0)
            .await
            .map_err(|error| task_error(format!("failed to read task events: {error}")))?
            .into_iter()
            .last();
        Ok(TaskSessionSnapshot {
            issue_id: session.launch.issue.id.as_str().to_string(),
            issue_identifier: session.launch.issue.identifier,
            session_id: session.id.to_string(),
            project_id: session.launch.project.id.as_str().to_string(),
            project: session.launch.project.slug,
            pm_snapshot_synced_at: session.launch.pm_snapshot_synced_at,
            pm_writeback: session.pm_writeback,
            wave: wave.name().to_string(),
            project_session_id: session.project_session_id.to_string(),
            current_directive_version: session.current_directive_version,
            incorporated_directive_version: session.incorporated_directive_version,
            status: session.status,
            status_reason: session.status_reason,
            status_at: session.status_at,
            worktree: session.worktree.display().to_string(),
            branch: session.branch,
            base_commit: session.base_commit,
            agent: session.agent,
            provider: session.provider,
            provider_session_id: session.provider_session_id,
            process_alive,
            latest_process: session.latest_process,
            pull_request: session.pull_request,
            latest_event,
            created_at: session.created_at,
            updated_at: session.updated_at,
        })
    })
}

pub fn task_changes(issue: &str) -> OpsResult<TaskChangesSnapshot> {
    let session = task_status(issue)?;
    changes_snapshot(TaskWorkspace::from(&session))
}

fn changes_snapshot(workspace: TaskWorkspace<'_>) -> OpsResult<TaskChangesSnapshot> {
    let mut files = BTreeMap::<String, TaskChangedFile>::new();
    record_changed_paths(
        workspace.worktree,
        &[
            "diff",
            "--name-only",
            "-z",
            &format!("{}..HEAD", workspace.base_commit),
        ],
        &mut files,
        |file| file.committed = true,
    )?;
    record_changed_paths(
        workspace.worktree,
        &["diff", "--cached", "--name-only", "-z"],
        &mut files,
        |file| file.staged = true,
    )?;
    record_changed_paths(
        workspace.worktree,
        &["diff", "--name-only", "-z"],
        &mut files,
        |file| file.unstaged = true,
    )?;
    record_changed_paths(
        workspace.worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        &mut files,
        |file| file.untracked = true,
    )?;
    let head_commit = git_output(workspace.worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    Ok(TaskChangesSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        session_id: workspace.session_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        files: files.into_values().collect(),
    })
}

pub fn task_diff(issue: &str, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    let session = task_status(issue)?;
    diff_snapshot(TaskWorkspace::from(&session), path)
}

fn diff_snapshot(workspace: TaskWorkspace<'_>, path: Option<&str>) -> OpsResult<TaskDiffSnapshot> {
    const MAX_PATCH_BYTES: usize = 1_000_000;

    let relative = path.map(validate_task_relative_path).transpose()?;
    let mut args = vec![
        "diff".to_string(),
        "--no-ext-diff".to_string(),
        "--no-color".to_string(),
        workspace.base_commit.to_string(),
        "--".to_string(),
    ];
    if let Some(path) = &relative {
        args.push(path.clone());
    }
    let mut patch = git_output_owned(workspace.worktree, &args)?;
    let untracked = untracked_paths(workspace.worktree)?;
    let include_untracked = untracked
        .into_iter()
        .filter(|candidate| relative.as_ref().is_none_or(|path| path == candidate));
    for path in include_untracked {
        let output = Command::new("git")
            .current_dir(workspace.worktree)
            .args(["diff", "--no-index", "--no-color", "--", "/dev/null", &path])
            .output()
            .map_err(|error| {
                task_error(format!("failed to diff untracked file {path}: {error}"))
            })?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(task_error(format!(
                "failed to diff untracked file {path}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        patch.extend_from_slice(&output.stdout);
    }
    let truncated = patch.len() > MAX_PATCH_BYTES;
    if truncated {
        patch.truncate(MAX_PATCH_BYTES);
    }
    let patch = String::from_utf8_lossy(&patch).into_owned();
    let binary = patch.contains("Binary files ") || patch.contains("GIT binary patch");
    Ok(TaskDiffSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        session_id: workspace.session_id.to_string(),
        path: relative,
        patch,
        binary,
        truncated,
    })
}

pub fn task_file(issue: &str, path: &str) -> OpsResult<TaskFileSnapshot> {
    let session = task_status(issue)?;
    file_snapshot(TaskWorkspace::from(&session), path)
}

fn file_snapshot(workspace: TaskWorkspace<'_>, path: &str) -> OpsResult<TaskFileSnapshot> {
    const MAX_FILE_BYTES: usize = 1_000_000;

    let relative = validate_task_relative_path(path)?;
    let root = workspace
        .worktree
        .canonicalize()
        .map_err(|error| task_error(format!("cannot resolve Task worktree: {error}")))?;
    let absolute = root
        .join(&relative)
        .canonicalize()
        .map_err(|error| task_error(format!("cannot open Task file {relative:?}: {error}")))?;
    if !absolute.starts_with(&root) || !absolute.is_file() {
        return Err(task_error(format!(
            "Task file {relative:?} does not resolve to a file inside the Task worktree"
        )));
    }
    let bytes = std::fs::read(&absolute)
        .map_err(|error| task_error(format!("cannot read Task file {relative:?}: {error}")))?;
    let size_bytes = bytes.len() as u64;
    let binary = bytes.iter().take(8_192).any(|byte| *byte == 0);
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let visible = &bytes[..bytes.len().min(MAX_FILE_BYTES)];
    let content = (!binary).then(|| String::from_utf8_lossy(visible).into_owned());
    Ok(TaskFileSnapshot {
        issue_identifier: workspace.issue_identifier.to_string(),
        session_id: workspace.session_id.to_string(),
        path: relative,
        content,
        binary,
        size_bytes,
        truncated,
    })
}

fn validate_task_relative_path(path: &str) -> OpsResult<String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(task_error(
            "Task paths must stay relative to the Task worktree",
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            _ => {
                return Err(task_error(
                    "Task paths must stay relative to the Task worktree",
                ))
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(task_error("Task paths must name a file"));
    }
    Ok(normalized.to_string_lossy().to_string())
}

fn record_changed_paths(
    worktree: &Path,
    args: &[&str],
    files: &mut BTreeMap<String, TaskChangedFile>,
    mark: impl Fn(&mut TaskChangedFile),
) -> OpsResult<()> {
    for path in nul_paths(&git_output_bytes(worktree, args)?) {
        let file = files.entry(path.clone()).or_insert(TaskChangedFile {
            path,
            committed: false,
            staged: false,
            unstaged: false,
            untracked: false,
        });
        mark(file);
    }
    Ok(())
}

fn untracked_paths(worktree: &Path) -> OpsResult<Vec<String>> {
    Ok(nul_paths(&git_output_bytes(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?))
}

fn nul_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

fn git_output(worktree: &Path, args: &[&str]) -> OpsResult<String> {
    Ok(String::from_utf8_lossy(&git_output_bytes(worktree, args)?).into_owned())
}

fn git_output_bytes(worktree: &Path, args: &[&str]) -> OpsResult<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(worktree)
        .args(args)
        .output()
        .map_err(|error| task_error(format!("failed to run git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(task_error(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn git_output_owned(worktree: &Path, args: &[String]) -> OpsResult<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(worktree)
        .args(args)
        .output()
        .map_err(|error| task_error(format!("failed to run git diff: {error}")))?;
    if !output.status.success() {
        return Err(task_error(format!(
            "git diff failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn queue_command(issue: &str, kind: ChildCommandKind) -> OpsResult<TaskControlResult> {
    block_on_task(async move {
        let store = task_store().await?;
        let mut session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        reconcile_process_liveness(&store, &mut session).await?;
        let issue_id = session.launch.issue.identifier.clone();
        let source = command_source(&session)?;
        let result = super::child::queue_command(
            &store,
            super::child::ChildSession::Task(Box::new(session)),
            source,
            kind,
        )
        .await?;
        Ok(task_control_result(issue_id, result))
    })
}

pub fn task_follow_up(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::FollowUp { text: message })
}

pub fn task_steer(issue: &str, message: String) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::Steer { text: message })
}

pub fn task_interrupt(issue: &str, replacement: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::Interrupt { replacement })
}

pub fn task_resume(issue: &str, message: Option<String>) -> OpsResult<TaskControlResult> {
    queue_command(issue, ChildCommandKind::Resume { message })
}

pub fn task_receipt(
    command_id: &str,
    until: Option<ChildReceiptUntil>,
    timeout: Duration,
) -> OpsResult<TaskReceiptRead> {
    let command_id =
        ChildCommandId::parse(command_id).map_err(|error| task_error(error.to_string()))?;
    block_on_task(async move {
        let store = task_store().await?;
        let (command, timed_out) = if let Some(until) = until {
            super::child::wait_for_receipt_condition(&store, &command_id, until, timeout).await?
        } else {
            (
                super::child::read_receipt(&store, &command_id).await?,
                false,
            )
        };
        let ChildRef::Task(session_id) = &command.target else {
            return Err(task_error(format!(
                "command {command_id} belongs to a Project Session"
            )));
        };
        let session = store
            .get_task_session(session_id)
            .await
            .map_err(|error| task_error(format!("failed to read Task Session: {error}")))?
            .ok_or_else(|| task_error(format!("Task Session {session_id} disappeared")))?;
        let result = super::child::control_result(&store, &command, command.clone()).await?;
        Ok(TaskReceiptRead {
            receipt: task_control_result(session.launch.issue.identifier, result),
            timed_out,
        })
    })
}

pub fn task_decide(
    issue: &str,
    decision_id: &str,
    choice: String,
    message: Option<String>,
) -> OpsResult<TaskControlResult> {
    let decision_id =
        ChildDecisionId::parse(decision_id).map_err(|error| task_error(error.to_string()))?;
    let choice = choice.trim().to_string();
    if choice.is_empty() {
        return Err(task_error("decision choice cannot be empty"));
    }
    let decision = block_on_task(async {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let events = store
            .task_events_after(&session.id, 0)
            .await
            .map_err(|error| task_error(format!("failed to read task decisions: {error}")))?;
        events
            .into_iter()
            .find_map(|event| match event.kind {
                TaskEventKind::DecisionRequested {
                    decision_id: requested_id,
                    options,
                    ..
                } if requested_id == decision_id => Some(options),
                _ => None,
            })
            .ok_or_else(|| task_error(format!("decision {decision_id} was not requested")))
    })?;
    if !decision.iter().any(|option| option == &choice) {
        return Err(task_error(format!(
            "choice {choice:?} is not one of: {}",
            decision.join(", ")
        )));
    }
    queue_command(
        issue,
        ChildCommandKind::Decide {
            decision_id,
            choice,
            message,
        },
    )
}

pub fn task_request_decision(
    issue: &str,
    prompt: String,
    options: Vec<String>,
    wait: bool,
    timeout: Duration,
) -> OpsResult<TaskDecisionResult> {
    let prompt = prompt.trim().to_string();
    let options: Vec<String> = options
        .into_iter()
        .map(|option| option.trim().to_string())
        .filter(|option| !option.is_empty())
        .collect();
    if prompt.is_empty() || options.len() < 2 {
        return Err(task_error(
            "a decision requires a non-empty prompt and at least two options",
        ));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let ambient = std::env::var("LF_TASK_SESSION_ID")
            .map_err(|_| task_error("decision requests must run inside the owning Task Session"))?;
        if ambient != session.id.as_str() {
            return Err(task_error(format!(
                "Task Session {ambient} cannot request a decision for {}",
                session.id
            )));
        }
        let decision_id = ChildDecisionId::new();
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::DecisionRequested {
                    decision_id: decision_id.clone(),
                    prompt,
                    options,
                },
            )
            .await
            .map_err(|error| task_error(format!("failed to persist decision request: {error}")))?;

        let deadline = Instant::now() + timeout;
        loop {
            let events = store
                .task_events_after(&session.id, 0)
                .await
                .map_err(|error| {
                    task_error(format!("failed to read decision response: {error}"))
                })?;
            if let Some((choice, message)) = events.into_iter().find_map(|event| match event.kind {
                TaskEventKind::DecisionResolved {
                    decision_id: resolved_id,
                    choice,
                    message,
                } if resolved_id == decision_id => Some((choice, message)),
                _ => None,
            }) {
                return Ok(TaskDecisionResult {
                    issue_id: session.launch.issue.identifier.clone(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: true,
                    choice: Some(choice),
                    message,
                });
            }
            if !wait || Instant::now() >= deadline {
                return Ok(TaskDecisionResult {
                    issue_id: session.launch.issue.identifier.clone(),
                    session_id: session.id.to_string(),
                    decision_id: decision_id.to_string(),
                    resolved: false,
                    choice: None,
                    message: None,
                });
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
}

pub fn task_acknowledge(issue: &str, version: u32, summary: String) -> OpsResult<ChildDirective> {
    let summary = summary.trim().to_string();
    if summary.is_empty() {
        return Err(task_error(
            "directive acknowledgement summary cannot be empty",
        ));
    }
    block_on_task(async move {
        let store = task_store().await?;
        let session = store
            .get_task_session_by_issue(issue)
            .await
            .map_err(|error| task_error(format!("failed to resolve task: {error}")))?
            .ok_or_else(|| task_error(format!("no Task Session exists for {issue:?}")))?;
        let ambient = std::env::var("LF_TASK_SESSION_ID").map_err(|_| {
            task_error("directive acknowledgements must run inside the owning Task Session")
        })?;
        if ambient != session.id.as_str() {
            return Err(task_error(format!(
                "Task Session {ambient} cannot acknowledge a directive for {}",
                session.id
            )));
        }
        let (directive, incorporated) = store
            .incorporate_child_directive(&ChildRef::Task(session.id.clone()), version, &summary)
            .await
            .map_err(|error| task_error(format!("failed to acknowledge directive: {error}")))?;
        if incorporated {
            store
                .append_task_event(
                    &session.id,
                    &TaskEventKind::DirectiveIncorporated {
                        directive_id: directive.id.clone(),
                        version,
                        summary,
                    },
                )
                .await
                .map_err(|error| task_error(error.to_string()))?;
        }
        Ok(directive)
    })
}

pub fn task_abandon(issue: &str, reason: String) -> OpsResult<TaskControlResult> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(task_error("`lf task abandon --reason` cannot be empty"));
    }
    queue_command(
        issue,
        ChildCommandKind::Abandon {
            reason: reason.to_string(),
        },
    )
}

pub fn task_wait(
    issue: &str,
    until: TaskWaitUntil,
    timeout: Option<Duration>,
) -> OpsResult<TaskSession> {
    let started = Instant::now();
    loop {
        let session = task_status(issue)?;
        let reached = match until {
            TaskWaitUntil::Submitted => {
                matches!(session.status, TaskSessionStatus::Submitted)
                    || session.status.is_terminal()
            }
            TaskWaitUntil::Terminal => session.status.is_terminal(),
        };
        if reached || timeout.is_some_and(|limit| started.elapsed() >= limit) {
            return Ok(session);
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

pub fn task_attach(issue: &str) -> OpsResult<()> {
    let session = task_status(issue)?;
    if !session.status.is_process_active() {
        return Err(task_error(format!(
            "task {} is {}; resume it before attaching",
            session.launch.issue.identifier,
            session.status.as_str()
        )));
    }
    let tmux_name = session
        .latest_process
        .as_ref()
        .map(|process| process.tmux_name.as_str())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| task_error("task has no attachable process; resume it first"))?;
    let status = std::process::Command::new("tmux")
        .args(["attach-session", "-t", tmux_name])
        .status()
        .map_err(|error| task_error(format!("failed to attach to task: {error}")))?;
    if !status.success() {
        return Err(task_error(format!("tmux attach failed for {tmux_name}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use super::{
        changes_snapshot, command_source_for_wave, diff_snapshot, file_snapshot, project_context,
        TaskControlResult, TaskWorkspace,
    };
    use crate::child_session::ChildCommandSource;
    use crate::pm::{PmKr, PmProject};
    use crate::task::TaskSessionId;

    fn git(repo: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn changed_workspace() -> (tempfile::TempDir, String, TaskSessionId) {
        let repo = tempfile::tempdir().expect("create temp repo");
        git(repo.path(), &["init", "-b", "main"]);
        git(repo.path(), &["config", "user.name", "Loopflow Test"]);
        git(
            repo.path(),
            &["config", "user.email", "loopflow@example.com"],
        );
        std::fs::write(repo.path().join("tracked.txt"), "base\n").expect("write base");
        git(repo.path(), &["add", "tracked.txt"]);
        git(repo.path(), &["commit", "-m", "base"]);
        let base = git(repo.path(), &["rev-parse", "HEAD"]);

        std::fs::write(repo.path().join("committed.txt"), "committed\n")
            .expect("write committed file");
        git(repo.path(), &["add", "committed.txt"]);
        git(repo.path(), &["commit", "-m", "task commit"]);
        std::fs::write(repo.path().join("tracked.txt"), "staged\n").expect("write staged");
        git(repo.path(), &["add", "tracked.txt"]);
        std::fs::write(repo.path().join("tracked.txt"), "unstaged\n").expect("write unstaged");
        std::fs::write(repo.path().join("untracked.txt"), "untracked\n").expect("write untracked");

        (repo, base, TaskSessionId::new())
    }

    #[test]
    fn task_context_captures_project_definition_and_kr_state() {
        let project = PmProject {
            id: "project-1".to_string(),
            slug: "delivery".to_string(),
            name: "Delivery".to_string(),
            summary: "Ship reliably".to_string(),
            definition: "Every task has one durable session.".to_string(),
            krs: vec![
                PmKr {
                    text: "Review resumes the same session".to_string(),
                    holds: true,
                },
                PmKr {
                    text: "Merge wakes the Wave".to_string(),
                    holds: false,
                },
            ],
            initiative_ids: vec!["initiative-1".to_string()],
        };

        assert_eq!(
            project_context(&project),
            "Definition:\nEvery task has one durable session.\n\nKRs:\n- [x] Review resumes the same session\n- [ ] Merge wakes the Wave"
        );
    }

    #[test]
    fn foreign_wave_cannot_be_reclassified_as_a_human_command() {
        let wave_id = crate::id::WaveId::new();

        assert!(matches!(
            command_source_for_wave(Some(wave_id.clone()), &wave_id, "INF-123").unwrap(),
            ChildCommandSource::Wave(_)
        ));
        assert!(
            command_source_for_wave(Some(crate::id::WaveId::new()), &wave_id, "INF-123").is_err()
        );
        assert_eq!(
            command_source_for_wave(None, &wave_id, "INF-123").unwrap(),
            ChildCommandSource::Human
        );
    }

    #[test]
    fn task_control_json_reports_durable_state_and_effect() {
        let result = TaskControlResult {
            issue_id: "INF-123".to_string(),
            session_id: "ts_example".to_string(),
            command_id: "cc_example".to_string(),
            directive_version: Some(2),
            state: crate::child_session::ChildCommandState::Accepted,
            effect: Some(crate::child_session::ChildCommandEffect::LiveSteer),
            incorporated: true,
            generation: Some(2),
            accepted_at: None,
            incorporated_at: None,
            error: None,
        };

        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::json!({
                "issue_id": "INF-123",
                "session_id": "ts_example",
                "command_id": "cc_example",
                "directive_version": 2,
                "state": "accepted",
                "effect": "live_steer",
                "incorporated": true,
                "generation": 2,
                "accepted_at": null,
                "incorporated_at": null,
                "error": null,
            })
        );
    }

    #[test]
    fn task_workspace_reports_committed_staged_unstaged_and_untracked_files() {
        let (repo, base, session_id) = changed_workspace();
        let workspace = TaskWorkspace {
            issue_identifier: "INF-123",
            session_id: &session_id,
            worktree: repo.path(),
            base_commit: &base,
        };

        let snapshot = changes_snapshot(workspace).expect("inspect task changes");
        let committed = snapshot
            .files
            .iter()
            .find(|file| file.path == "committed.txt")
            .expect("committed file");
        assert!(committed.committed);
        let tracked = snapshot
            .files
            .iter()
            .find(|file| file.path == "tracked.txt")
            .expect("tracked file");
        assert!(tracked.staged && tracked.unstaged);
        let untracked = snapshot
            .files
            .iter()
            .find(|file| file.path == "untracked.txt")
            .expect("untracked file");
        assert!(untracked.untracked);

        let patch = diff_snapshot(workspace, None).expect("inspect task diff");
        assert!(patch.patch.contains("committed.txt"));
        assert!(patch.patch.contains("tracked.txt"));
        assert!(patch.patch.contains("untracked.txt"));

        let untracked_patch =
            diff_snapshot(workspace, Some("./untracked.txt")).expect("inspect one file");
        assert_eq!(untracked_patch.path.as_deref(), Some("untracked.txt"));
        assert!(untracked_patch.patch.contains("+untracked"));
    }

    #[test]
    fn task_file_reads_only_files_inside_the_task_worktree() {
        let (repo, base, session_id) = changed_workspace();
        let workspace = TaskWorkspace {
            issue_identifier: "INF-123",
            session_id: &session_id,
            worktree: repo.path(),
            base_commit: &base,
        };

        let file = file_snapshot(workspace, "./tracked.txt").expect("read task file");
        assert_eq!(file.path, "tracked.txt");
        assert_eq!(file.content.as_deref(), Some("unstaged\n"));
        assert!(file_snapshot(workspace, "../outside.txt").is_err());

        #[cfg(unix)]
        {
            let outside = tempfile::NamedTempFile::new().expect("outside file");
            std::os::unix::fs::symlink(outside.path(), repo.path().join("outside-link"))
                .expect("create outside symlink");
            assert!(file_snapshot(workspace, "outside-link").is_err());
        }
    }
}
