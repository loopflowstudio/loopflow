//! The completion and landing callers share this durable retirement operation.
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::path::Path;

use super::{block_on_task, task_error, task_store};
use crate::durable::{TaskId, WorkStatus};
use crate::journal::ProcessIdentityEvidence;
use crate::ops::{NullProgress, OpsError, OpsResult};
use crate::store::{
    sqlite::{SqliteStore, TaskRetirement, ViewerHold},
    SharedStore,
};
use crate::transport::Host;
use crate::work::task::Task;
use fs2::FileExt;

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

// Completion, landing and each cleanup pass serialize against the same Task.
// A returning viewer resets the checkout deadline before the next pass.
pub(super) fn lock(task: &TaskId) -> OpsResult<File> {
    let file = lock_file(task.as_str())?;
    file.lock_exclusive()?;
    Ok(file)
}

fn lock_file(name: &str) -> OpsResult<File> {
    let directory = crate::store::lf_home_dir().join("workspace-locks");
    std::fs::create_dir_all(&directory)?;
    Ok(OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{name}.lock")))?)
}

/// Owners refresh live viewing evidence every ten seconds. A dead or stalled
/// owner cannot keep a completed checkout forever with its last cached boolean.
pub(crate) const VIEWER_OBSERVATION_GRACE: i64 = 30;

fn viewer_hold(store: &SqliteStore, task: &TaskId, now: i64) -> OpsResult<Option<ViewerHold>> {
    let mut absent_since = None;
    for owner in store.task_viewer_holds(task).map_err(task_error)? {
        let at = match owner.hold {
            ViewerHold::Viewed(at) => {
                if !owner.ended
                    && now < at + VIEWER_OBSERVATION_GRACE
                    && owner.owner.owner().is_none_or(|owner| {
                        crate::journal::lf_evidence(store, owner) != ProcessIdentityEvidence::Dead
                    })
                {
                    return Ok(Some(ViewerHold::Viewed(at)));
                }
                at + VIEWER_OBSERVATION_GRACE
            }
            ViewerHold::Absent(at) => at,
        };
        absent_since = Some(absent_since.map_or(at, |old: i64| old.max(at)));
    }
    Ok(absent_since.map(ViewerHold::Absent))
}

fn reconcile(store: &SqliteStore, task: &TaskId) -> OpsResult<Option<TaskRetirement>> {
    let now = now();
    let hold = viewer_hold(store, task, now)?;
    store
        .reconcile_task_retirement(task, now, hold)
        .map_err(task_error)
}

/// Task cleanup owns closure, including Tasks without a checkout and resumed
/// conversations whose checkout was already retired.
pub(crate) fn close_conversations(store: &SqliteStore, task: &TaskId) -> OpsResult<()> {
    for (host, abandoned) in conversation_closes(store, task)? {
        let notice = if abandoned {
            "Task abandoned"
        } else {
            "Task completed"
        };
        match &host {
            Host::Owner(owner) => end_owner(store, owner, notice)?,
            // A held terminal has no process to signal: the server ends it on
            // its notice. One the server cannot reach is a candidate next pass.
            Host::Terminal(_) => {
                if let Err(error) = crate::transport::close_notice(&host, notice) {
                    tracing::warn!(%host, %error, "held terminal close unavailable");
                }
            }
        }
    }
    Ok(())
}

fn conversation_closes(store: &SqliteStore, task: &TaskId) -> OpsResult<Vec<(Host, bool)>> {
    let now = now();
    let mut hosts = store.task_close_candidates(task, now).map_err(task_error)?;
    let Some((_, abandoned)) = hosts.first() else {
        return Ok(hosts);
    };
    if *abandoned {
        let holds = store.task_viewer_holds(task).map_err(task_error)?;
        hosts.retain(|(host, _)| {
            !holds.iter().any(|hold| &hold.owner == host
            && matches!(hold.hold, ViewerHold::Viewed(at) if now < at + VIEWER_OBSERVATION_GRACE))
        });
    } else {
        match viewer_hold(store, task, now)? {
            Some(ViewerHold::Viewed(_)) => return Ok(Vec::new()),
            Some(ViewerHold::Absent(at)) if at + crate::store::sqlite::WORKSPACE_GRACE > now => {
                return Ok(Vec::new())
            }
            _ => {}
        }
        if unsettled_work(store, task)?.is_some() {
            return Ok(Vec::new());
        }
    }
    Ok(hosts)
}

fn end_owner(store: &SqliteStore, owner: &crate::id::LfProcessId, notice: &str) -> OpsResult<()> {
    let Some(pid) = crate::journal::live_process_pid(store, owner) else {
        return Ok(());
    };
    // Older owners may not understand the notice; the verified signal still ends them.
    if let Err(error) = crate::transport::close_notice(&Host::Owner(owner.clone()), notice) {
        tracing::debug!(%owner, %error, "Owner close notice unavailable");
    }
    crate::ops::human_session::end_owner(store, owner, pid).map_err(task_error)
}

pub(crate) async fn schedule(store: &SharedStore, task: &Task) -> OpsResult<()> {
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(());
    }
    let _guard = lock(&task.id)?;
    reconcile(&store.sqlite, &task.id)?;
    Ok(())
}

/// One finite Machine pass. The OS scheduler owns cadence; SQLite owns progress.
pub fn run(issue: Option<&str>) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let tasks = match issue {
            Some(issue) => vec![store
                .sqlite
                .resolve_task_id(issue, None)
                .map_err(task_error)?
                .ok_or_else(|| task_error("Task is unknown"))?],
            None => store.sqlite.pending_retirements().map_err(task_error)?,
        };
        sweep(&store, &tasks).await
    })
}

async fn sweep(store: &SharedStore, tasks: &[TaskId]) -> OpsResult<()> {
    let guard = lock_file("sweep")?;
    match guard.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let mut failures = Vec::new();
    for id in tasks {
        let result = async {
            let guard = lock_file(id.as_str())?;
            match guard.try_lock_exclusive() {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(OpsError::from(error)),
            }
            if let Some(task) = store.get_task(id).await.map_err(task_error)? {
                if let Err(error) = advance(store, &task).await {
                    store
                        .sqlite
                        .fail_retirement(id, now(), &error.to_string())
                        .map_err(task_error)?;
                    return Err(error);
                }
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            // Keep the failure even when another Task can finish this pass.
            tracing::warn!(task=%id,%error,"Task cleanup pending");
            failures.push(format!("{id}: {error}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(task_error(failures.join("; ")))
    }
}

async fn advance(store: &SharedStore, task: &Task) -> OpsResult<()> {
    close_conversations(&store.sqlite, &task.id)?;
    let Some(intent) = reconcile(&store.sqlite, &task.id)? else {
        return Ok(());
    };
    if intent.phase == "retired" {
        return Ok(());
    }
    if intent.due_at.is_none_or(|due| due > now()) || intent.retry_at.is_some_and(|at| at > now()) {
        return Ok(());
    }
    match super::task_work_status(store, task).await? {
        WorkStatus::Abandoned => {
            let repo = super::repository_for_task(store, task)?;
            if !super::lifecycle::cleanup_abandoned(&repo, store, task, false).await? {
                return Err(task_error("Retained checkout without unmerged delivery"));
            }
            store
                .sqlite
                .retirement_progress(&intent, "retired", None)
                .map_err(task_error)?;
            return Ok(());
        }
        WorkStatus::Done => {}
        _ => return Ok(()),
    }
    let checkout = Path::new(&intent.checkout);
    let _mutation = checkout
        .is_dir()
        .then(|| super::lock_task_pr_mutation(checkout))
        .transpose()?;
    if let Some(reason) = unsettled_work(&store.sqlite, &task.id)? {
        store
            .sqlite
            .retirement_progress(&intent, &intent.phase, Some(&reason))
            .map_err(task_error)?;
        return Ok(());
    }
    if !store
        .sqlite
        .retirement_progress(&intent, "closing", None)
        .map_err(task_error)?
    {
        return Ok(());
    }
    for owner in crate::desktop::shells::owners(&store.sqlite, &task.id).map_err(task_error)? {
        end_owner(&store.sqlite, &owner, "Task completed")?;
    }
    store
        .sqlite
        .settle_unstarted_task_shells(&task.id)
        .map_err(task_error)?;
    // A signal is not proof that all execution and shell jobs have settled.
    let blockers = crate::ops::task_execution::task_execution_blockers(&store.sqlite, &task.id)?;
    if !blockers.is_empty() {
        store
            .sqlite
            .retirement_progress(&intent, "closing", Some(&blockers.join("; ")))
            .map_err(task_error)?;
        return Ok(());
    }
    // A closed Task ends its conversations whether or not its checkout can go.
    // Check merged delivery only once execution has settled and deletion is next.
    let deletion = prepare_deletion(store, task, &intent).await?;
    if !crate::desktop::shells::jobs_settled(&store.sqlite, &task.id).map_err(task_error)? {
        return Err(task_error(
            "Shell owner ended without confirmed job cleanup; retained checkout",
        ));
    }
    // Reconcile viewing and pinned completion/placement together before deletion.
    // Reopen revokes the intent in the same write as the Task change.
    if reconcile(&store.sqlite, &task.id)?.is_none_or(|current| {
        current.completion != intent.completion
            || current.checkout != intent.checkout
            || current.branch != intent.branch
            || current.due_at.is_none_or(|due| due > now())
    }) {
        return Ok(());
    }
    crate::ops::wt::apply_delete(deletion, &NullProgress)?;
    store
        .sqlite
        .retirement_progress(&intent, "retired", None)
        .map_err(task_error)?;
    Ok(())
}

/// The checkout deletion merged delivery allows. Preparation keeps
/// primary/persistent and changed committed refs and lets uncommitted files go.
async fn prepare_deletion(
    store: &SharedStore,
    task: &Task,
    intent: &TaskRetirement,
) -> OpsResult<crate::ops::wt::BranchDeletion> {
    let Some(pr) = store.active_task_pr(&task.id).await.map_err(task_error)? else {
        return Err(task_error("Retained PR-less checkout"));
    };
    if pr.branch != intent.branch {
        return Err(task_error(
            "Task delivery branch changed; retained checkout",
        ));
    }
    if super::completion_refusal(Some(&pr), &task.identifier).is_some() {
        return Err(task_error("Delivery is unresolved"));
    }
    let repo = crate::git::worktrees::main_repo_root(&super::repository_for_task(store, task)?)?;
    crate::ops::wt::prepare_task_retirement(
        &repo,
        &intent.branch,
        pr.head_sha()
            .ok_or_else(|| task_error("Merged PR has no recorded head"))?,
        Path::new(&intent.checkout),
    )
}

fn unsettled_work(store: &SqliteStore, task: &TaskId) -> OpsResult<Option<String>> {
    let shells = crate::desktop::shells::owners(store, task).map_err(task_error)?;
    let mut allowed: HashSet<_> = shells.into_iter().collect();
    for host in store.task_session_hosts(task).map_err(task_error)? {
        if host.owner().is_some_and(|owner| {
            crate::journal::lf_evidence(store, owner) == ProcessIdentityEvidence::Dead
        }) {
            continue;
        }
        if store
            .owner_retention(host.transport())
            .map_err(task_error)?
            .is_some()
        {
            return Ok(Some(
                "Requested work or a pending request still holds this Task".into(),
            ));
        }
        // A held terminal is no process: the lf processes that started inside
        // it stand where an owner would.
        match host {
            Host::Owner(owner) => {
                allowed.extend(store.owner_session_launchers(&owner).map_err(task_error)?);
                allowed.insert(owner);
            }
            Host::Terminal(terminal) => {
                allowed.extend(
                    store
                        .held_terminal_processes(&terminal)
                        .map_err(task_error)?,
                );
            }
        }
    }
    let processes = store.task_open_processes(task).map_err(task_error)?;
    for (process, _) in crate::ops::task_execution::unresolved_processes(store, &processes)? {
        // An agent's owners are lf processes, so its walk starts at its parent.
        let mut current = match process {
            crate::process::ProcessEntry::Lf(process) => Some(process.id.clone()),
            crate::process::ProcessEntry::Agent(process) => process.parent_lf_process_id.clone(),
        };
        let mut visited = HashSet::new();
        let mut owned = false;
        while let Some(id) = current.filter(|id| visited.insert(id.clone())) {
            if allowed.contains(&id) {
                owned = true;
                break;
            }
            current = store
                .lf_process(&id)
                .map_err(task_error)?
                .and_then(|p| p.parent_lf_process_id);
        }
        if !owned {
            return Ok(Some(format!(
                "Process {} still has live or unresolved work",
                process.id()
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
