//! Delivery observation and CI repair settings. Repository checks never resume Flows.
use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::durable::WorkStatus;
use crate::journal::{process_evidence, ProcessIdentityEvidence};
use crate::process::Process;
use crate::store::sqlite::SqliteStore;
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
        let _lock = store
            .sqlite
            .lock_checkout(task.worktree()?)
            .map_err(error)?;
        store
            .sqlite
            .set_task_automation(&task.id, enabled)
            .map_err(error)
    })
}

/// Whether anything is executing is asked only of the OS, about recorded
/// Processes. Unknown evidence blocks. The caller's lineage waits on the caller,
/// so it never does; that exempts waiting and grants no authority.
pub(crate) fn execution_blockers(
    store: &SqliteStore,
    processes: &[Process],
) -> OpsResult<Vec<String>> {
    let mut lineage = HashSet::new();
    let mut next = crate::journal::current_process_lfid();
    while let Some(id) = next.filter(|id| lineage.insert(id.clone())) {
        next = store
            .process(&id)
            .map_err(error)?
            .and_then(|process| process.parent_process_lfid);
    }
    Ok(processes
        .iter()
        .filter(|process| {
            process.completed_at.is_none()
                && !lineage.contains(&process.lfid)
                && process_evidence(store, &process.lfid) != ProcessIdentityEvidence::Dead
        })
        .map(|process| {
            format!(
                "Process {} has live or unresolved execution; inspect `lf history show {}`",
                process.lfid, process.lfid
            )
        })
        .collect())
}

/// A Task's work is every Process in its checkout or bound through a Session.
pub(crate) fn task_execution_blockers(
    store: &SqliteStore,
    task: &crate::durable::TaskId,
) -> OpsResult<Vec<String>> {
    let open = store.open_processes().map_err(error)?;
    let work = store.task_open_work(task, &open).map_err(error)?;
    execution_blockers(store, &work.processes)
}

pub(crate) async fn repository_tasks(
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
        let local = store.local_machine().await.map_err(error)?;
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
