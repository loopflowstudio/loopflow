//! Delivery observation and CI repair settings. Repository checks never resume Flows.
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::durable::WorkStatus;
use crate::journal::{exec_process_evidence, ProcessIdentityEvidence};
use crate::store::SharedStore;
use crate::work::task::Task;

use super::error::{OpsError, OpsResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAutomation {
    pub task_id: String,
    pub issue: String,
    pub enabled: Option<bool>,
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
        let _lock = store.sqlite.lock_checkout(&task.worktree).map_err(error)?;
        store
            .sqlite
            .set_task_automation(&task.id, enabled)
            .map_err(error)
    })
}

/// No pending review, unknown provider, or unrelated live Exec is exempted.
pub(crate) fn admission_blocker(
    store: &crate::store::sqlite::SqliteStore,
    task: &crate::durable::TaskId,
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
                return Ok(Some(format!("Session {} has a reserved input", session.id)));
            }
        }
        if store.session_has_pending_turn(&session.id).map_err(error)?
            && (session.completed_at.is_none() || session_engine_unresolved(store, &session.id)?)
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
            if !flow.finished && crate::ops::flow_run::driver_live(flow.id()) {
                return Ok(Some(format!(
                    "Flow {} has a live driver",
                    flow.invocation.id
                )));
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
