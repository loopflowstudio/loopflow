//! A finite repository check. Task and landing owners retain all execution authority.
use std::fs::OpenOptions;
use std::path::Path;
use std::time::Duration;

use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::durable::{WorkRef, WorkStatus};
use crate::id::ExecId;
use crate::journal::{exec_process_evidence, ProcessIdentityEvidence};
use crate::store::SharedStore;
use crate::work::task::Task;

use super::error::{OpsError, OpsResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAutomation {
    pub task_id: String,
    pub issue: String,
    pub enabled: Option<bool>,
    pub exec_id: Option<String>,
    pub retry_key: Option<String>,
    pub retries: u32,
    pub checked_at: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationCheck {
    pub checked_at: i64,
    pub tasks: Vec<TaskAutomation>,
    pub errors: Vec<String>,
    pub deferred: usize,
}

fn error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}

pub fn select(issue: &str, enabled: bool) -> OpsResult<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = super::pr_landing::landing_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(error)?
            .ok_or_else(|| error(format!("no Task exists for {issue:?}")))?;
        if enabled {
            let (_, flow) = super::task_execution::task_execution_and_flow(&store, &task.id)
                .await
                .map_err(error)?;
            if matches!(flow, super::task_flow::TaskFlowRecord::None) {
                let project = store
                    .get_project(&task.project_id)
                    .await
                    .map_err(error)?
                    .ok_or_else(|| error("Task Project is missing"))?;
                crate::controller::task::ensure_flow_position(
                    &store,
                    &task.id,
                    Some(&project.plan.flow),
                )
                .await
                .map_err(error)?;
            }
        }
        let _lock = store
            .sqlite
            .lock_checkout(&task.require_workspace()?.worktree)
            .map_err(error)?;
        store
            .sqlite
            .set_task_automation(&task.id, enabled, false)
            .map_err(error)
    })
}

/// No pending review, unknown provider, or unrelated live Exec is exempted.
pub(crate) fn admission_blocker(
    store: &crate::store::sqlite::SqliteStore,
    task: &crate::durable::TaskId,
    recovery: bool,
    repair_session: Option<&str>,
) -> OpsResult<Option<String>> {
    let work = store.task_work(task).map_err(error)?;
    let caller = crate::journal::current_exec_id();
    for session in &work.sessions {
        if Some(session.id.as_str()) == repair_session {
            continue;
        }
        if session.completed_at.is_none()
            && session.kind != crate::session::SessionKind::Conversation
        {
            return Ok(Some(format!("Session {} awaits completion", session.id)));
        }
        if let Some(input) = store.session(&session.id).map_err(error)? {
            if !input.interactive && !input.input_published {
                let dead_managed_reservation = if recovery && session.managed {
                    input
                        .flow_session_id
                        .as_deref()
                        .map(|flow| store.pending_flow_step_exec(flow))
                        .transpose()
                        .map_err(error)?
                        .flatten()
                        .is_some_and(|exec| {
                            exec_process_evidence(store, &exec) == ProcessIdentityEvidence::Dead
                        })
                } else {
                    false
                };
                if !dead_managed_reservation {
                    return Ok(Some(format!("Session {} has a reserved input", session.id)));
                }
            }
        }
        if store.session_has_pending_turn(&session.id).map_err(error)?
            && (!recovery || !session.managed || session_engine_unresolved(store, &session.id)?)
        {
            return Ok(Some(format!(
                "Session {} has an unresolved provider turn",
                session.id
            )));
        }
    }
    for exec in &work.execs {
        if caller.as_ref() == Some(&exec.id) {
            continue;
        }
        if exec_process_evidence(store, &exec.id) != ProcessIdentityEvidence::Dead {
            return Ok(Some(format!("Exec {} is live or unresolved", exec.id)));
        }
    }
    for flow in &work.flows {
        if let Some(flow) = store.flow(&flow.summary.id).map_err(error)? {
            if flow.is_human() && !flow.finished {
                return Ok(Some("Flow awaits review".into()));
            }
            if let Some(claim) = flow.claim {
                if Some(&claim.owner.exec_id) != caller.as_ref()
                    && crate::journal::task_worker_owner_evidence(&claim.owner)
                        != ProcessIdentityEvidence::Dead
                {
                    return Ok(Some(format!(
                        "Flow {} has a live or unresolved worker",
                        flow.invocation.id
                    )));
                }
            }
        }
    }
    Ok(None)
}

pub(crate) fn session_engine_unresolved(
    store: &crate::store::sqlite::SqliteStore,
    session: &str,
) -> OpsResult<bool> {
    if let Some((pid, started)) = store.session_provider_process(session).map_err(error)? {
        return Ok(match crate::journal::process_started_at(pid) {
            Ok(Some(actual)) => (actual - started).abs() <= 3,
            Ok(None) => false,
            Err(_) => true,
        });
    }
    Ok(store
        .session_driver(session)
        .map_err(error)?
        .is_none_or(|driver| {
            exec_process_evidence(store, &driver.provider_exec_id) != ProcessIdentityEvidence::Dead
        }))
}

async fn repository_tasks(
    store: &SharedStore,
    repo: &crate::repository::RepoId,
) -> OpsResult<Vec<Task>> {
    let mut tasks = Vec::new();
    for task in store.list_tasks(None).await.map_err(error)? {
        let Some(wave) = store.get_wave(&task.wave_id).await.map_err(error)? else {
            continue;
        };
        if crate::repository::RepoId::discover(Path::new(wave.repo()))
            .ok()
            .as_ref()
            == Some(repo)
        {
            tasks.push(task);
        }
    }
    Ok(tasks)
}

pub fn reconcile(repo: &Path) -> OpsResult<AutomationCheck> {
    let root = crate::engine::worktrees::main_repo_root(repo).map_err(error)?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(crate::engine::git::absolute_git_dir(&root)?.join("lf-task-reconcile.lock"))?;
    let mut report = AutomationCheck {
        checked_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        tasks: vec![],
        errors: vec![],
        deferred: 0,
    };
    if let Err(cause) = FileExt::try_lock_exclusive(&lock) {
        if cause.kind() != std::io::ErrorKind::WouldBlock {
            return Err(cause.into());
        }
        // The running check already covers this repository; overlap is not a failure.
        eprintln!("another repository check is running");
        return Ok(report);
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let result = runtime.block_on(async {
        let store = super::pr_landing::landing_store().await?;
        let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
        super::pr_landing::reconcile_repository_async(&root, &store, deadline, &mut report.errors)
            .await?;
        let repo_id = crate::repository::RepoId::discover(&root).map_err(error)?;
        let mut tasks = Vec::new();
        for task in repository_tasks(&store, &repo_id).await? {
            let state = store.sqlite.task_automation(&task.id).map_err(error)?;
            if state.enabled.is_some() {
                tasks.push((task, state));
            }
        }
        tasks.sort_by_key(|(_, state)| state.checked_at.unwrap_or(0));
        for (mut task, mut state) in tasks {
            if tokio::time::Instant::now() >= deadline {
                report.deferred += 1;
                report.tasks.push(state);
                continue;
            }
            let outcome = tokio::time::timeout_at(
                deadline.min(tokio::time::Instant::now() + Duration::from_secs(10)),
                reconcile_task(&store, &mut task, &mut state),
            )
            .await;
            let detail = match outcome {
                Ok(Ok(detail)) => detail,
                Ok(Err(cause)) => {
                    report
                        .errors
                        .push(format!("{}: {cause}", task.plan.identifier));
                    cause.to_string()
                }
                Err(_) => {
                    report
                        .errors
                        .push(format!("{}: check deadline exceeded", task.plan.identifier));
                    "check deadline exceeded; next check will inspect the reservation".into()
                }
            };
            state.checked_at = Some(time::OffsetDateTime::now_utc().unix_timestamp());
            state.detail = Some(detail);
            store.sqlite.record_automation(&state).map_err(error)?;
            report.tasks.push(state);
        }
        Ok(report)
    });
    // Timed-out network observations retain their effect locks until process exit.
    runtime.shutdown_background();
    result
}

async fn reconcile_task(
    store: &SharedStore,
    task: &mut Task,
    state: &mut TaskAutomation,
) -> OpsResult<String> {
    if state.enabled != Some(true) {
        return Ok("held".into());
    }
    let local = store.local_home().await.map_err(error)?;
    let placement = store
        .placement(&WorkRef::Task(task.id.clone()))
        .await
        .map_err(error)?;
    if placement.home_id != local.id {
        return Ok(format!("placed on Home {}", placement.home_id));
    }
    if super::task::task_work_status(store, task).await? != WorkStatus::Ready
        || task.abandon_intent.is_some()
    {
        return Ok("terminal Task; no new work".into());
    }
    let lock = store
        .sqlite
        .lock_checkout(&task.require_workspace()?.worktree)
        .map_err(error)?;
    // Selection is re-read after taking the reservation boundary.
    if store
        .sqlite
        .task_automation(&task.id)
        .map_err(error)?
        .enabled
        != Some(true)
    {
        return Ok("held".into());
    }
    if let Some(exec) = state
        .exec_id
        .as_deref()
        .map(ExecId::parse)
        .transpose()
        .map_err(error)?
    {
        if Some(&exec) != crate::journal::current_exec_id().as_ref()
            && exec_process_evidence(&store.sqlite, &exec) != ProcessIdentityEvidence::Dead
        {
            return Ok(format!("admission Exec {exec} is live or unresolved"));
        }
    }
    if let Some(reason) = admission_blocker(&store.sqlite, &task.id, true, None)? {
        return Ok(reason);
    }
    let Some(flow) = store.task_flow(&task.id).await.map_err(error)? else {
        return Ok("no unfinished captured Flow; select a Flow with lf task run".into());
    };
    if let Some(failure) = &flow.failure {
        return Ok(format!(
            "Flow failed: {}; inspect and explicitly retry",
            failure.reason
        ));
    }
    if flow.finished {
        return Ok("Flow finished".into());
    }
    if flow.is_human() || flow.pending_session_id.is_some() {
        return Ok("waiting for review".into());
    }
    if let Some(landing) = store.sqlite.operation_landing(flow.id()).map_err(error)? {
        if landing.state != crate::pr_landing::PrLandingState::Merged {
            return Ok(format!(
                "waiting for PR #{}: {}",
                landing.pr_number,
                landing
                    .blocked_reason
                    .as_deref()
                    .unwrap_or(landing.state.as_str())
            ));
        }
    }
    let key = format!(
        "{}:{}",
        flow.id(),
        serde_json::to_string(&flow.cursor).map_err(error)?
    );
    if state.retry_key.as_ref() != Some(&key) {
        state.retry_key = Some(key);
        state.retries = 0;
        state.exec_id = None;
    }
    let completed = flow
        .current_attempt
        .as_ref()
        .is_some_and(|attempt| attempt.completed())
        || store
            .sqlite
            .flow_operation_completed(flow.id())
            .map_err(error)?;
    let retry = !completed
        && (state.exec_id.is_some() || flow.current_attempt.is_some() || flow.claim.is_some());
    let config =
        crate::engine::config::load_config_or_default(Some(&task.require_workspace()?.worktree));
    if retry && state.retries >= config.automation.retries {
        return Ok(
            "unchanged failure exhausted automatic retries; inspect and explicitly retry".into(),
        );
    }
    state.exec_id = crate::journal::current_exec_id().map(|id| id.to_string());
    if state.exec_id.is_none() {
        return Err(error("automatic admission requires a recorded Exec"));
    }
    if retry {
        state.retries += 1;
    }
    store.sqlite.record_automation(state).map_err(error)?;
    drop(lock);
    if retry {
        crate::lf::commands::flow::prepare_native_retry(store, flow)
            .await
            .map_err(error)?;
    }
    super::task::exec_task_process(store, task, None).await?;
    Ok("Flow admitted through its saved claim".into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutomationStatus {
    pub enabled: bool,
    pub cadence_seconds: u32,
    pub coverage: String,
    pub last_success_at: Option<i64>,
    pub last_failure_at: Option<i64>,
    pub last_error: Option<String>,
    pub tasks: Vec<TaskAutomation>,
    pub deliveries: Vec<DeliveryStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryStatus {
    pub pr_number: u32,
    pub task_id: Option<String>,
    pub state: String,
    pub detail: Option<String>,
}

pub fn status(repo: &Path) -> OpsResult<AutomationStatus> {
    let root = crate::engine::worktrees::main_repo_root(repo).map_err(error)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = super::pr_landing::landing_store().await?;
        let local = store.local_home().await.map_err(error)?;
        let key = super::cron::repository_cron_key(&root, &local.id);
        let jobs = super::cron::list_crons(
            &super::cron::default_launch_agents_dir()?,
            &super::cron::SystemLaunchctl,
        )?;
        let job = jobs.iter().find(|job| {
            job.target_kind == super::cron::CronTargetKind::Repository && job.flow == key
        });
        let enabled = job.is_some_and(|job| job.loaded);
        let receipts = super::cron::list_cron_receipts(
            &super::cron::receipt_root(&crate::store::lf_home_dir()),
            "",
            Some(&key),
            35,
        )?;
        let success = receipts
            .iter()
            .find(|receipt| receipt.outcome == super::cron::CronOutcome::Succeeded);
        let failure = receipts
            .iter()
            .find(|receipt| receipt.outcome == super::cron::CronOutcome::Failed);
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let coverage = if !enabled {
            "disabled"
        } else if receipts
            .first()
            .is_some_and(|receipt| receipt.outcome == super::cron::CronOutcome::Failed)
        {
            "failed"
        } else if success.is_none() {
            "unknown"
        } else if success.is_some_and(|receipt| now - receipt.started_at > 180) {
            "overdue"
        } else {
            "checked"
        };
        let repo_id = crate::repository::RepoId::discover(&root).map_err(error)?;
        let mut tasks = Vec::new();
        for task in repository_tasks(&store, &repo_id).await? {
            // Finished Tasks get no new work; unsettled deliveries are listed below.
            if super::task::task_work_status(&store, &task).await? == WorkStatus::Ready {
                tasks.push(store.sqlite.task_automation(&task.id).map_err(error)?);
            }
        }
        let deliveries = store
            .pending_pr_landings(repo_id.as_str())
            .await
            .map_err(error)?
            .into_iter()
            .map(|landing| DeliveryStatus {
                pr_number: landing.pr_number,
                task_id: landing.task_id.map(|id| id.to_string()),
                state: landing.state.as_str().into(),
                detail: landing.blocked_reason,
            })
            .collect();
        Ok(AutomationStatus {
            enabled,
            cadence_seconds: 60,
            coverage: coverage.into(),
            last_success_at: success.map(|receipt| receipt.started_at),
            last_failure_at: failure.map(|receipt| receipt.started_at),
            last_error: failure.and_then(|receipt| receipt.error.clone()),
            tasks,
            deliveries,
        })
    })
}
