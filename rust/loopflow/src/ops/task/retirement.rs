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
use crate::work::task::Task;
use fs2::FileExt;

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

// Completion, landing and each cleanup pass serialize against the same Task.
// A returning viewer revokes queued work before the next pass performs an effect.
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
                    && owner.owner.as_ref().is_none_or(|owner| {
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

/// Owners consume automatic closes between sweeps. The queued phase alone
/// cannot authorize ending execution after Session viewing has returned.
pub(crate) fn owner_retiring<'a>(
    store: &SqliteStore,
    owner: impl Into<crate::store::sqlite::Transport<'a>>,
) -> OpsResult<bool> {
    let now = now();
    for task in store.owner_retiring_tasks(owner).map_err(task_error)? {
        match viewer_hold(store, &task, now)? {
            Some(ViewerHold::Viewed(_)) => continue,
            Some(ViewerHold::Absent(at)) if at + crate::store::sqlite::WORKSPACE_GRACE > now => {
                continue;
            }
            _ => {}
        }
        // Read intent after viewing: a concurrent return revokes this phase,
        // including a return that has already left again.
        if store
            .task_retirement(&task)
            .map_err(task_error)?
            .is_some_and(|intent| {
                intent.phase == "closing" && intent.due_at.is_some_and(|due| due <= now)
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
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
    if tasks.is_empty() {
        return Ok(());
    }
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
    let Some(intent) = reconcile(&store.sqlite, &task.id)? else {
        return Ok(());
    };
    if intent.phase == "retired" {
        return Ok(());
    }
    if intent.due_at.is_none_or(|due| due > now()) || intent.retry_at.is_some_and(|at| at > now()) {
        return Ok(());
    }
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(());
    }
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
    let checkout = Path::new(&intent.checkout);
    let _mutation = checkout
        .is_dir()
        .then(|| super::lock_task_pr_mutation(checkout))
        .transpose()?;
    // Prepare before closing anything. This preserves primary/persistent and
    // changed committed refs while allowing uncommitted files to be discarded.
    let deletion = crate::ops::wt::prepare_task_retirement(
        &repo,
        &intent.branch,
        pr.head_sha()
            .ok_or_else(|| task_error("Merged PR has no recorded head"))?,
        checkout,
    )?;
    if let Err(error) = require_settled_work(store, task) {
        store
            .sqlite
            .retirement_progress(&intent, &intent.phase, Some(&error.to_string()))
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
    crate::desktop::shells::request_retirement(&store.sqlite, &task.id).map_err(task_error)?;
    // The existing owner policy sees the closing intent and rechecks active
    // work and queued input in the serving loop before ending the provider.
    let blockers = crate::ops::task_execution::task_execution_blockers(&store.sqlite, &task.id)?;
    if !blockers.is_empty() {
        store
            .sqlite
            .retirement_progress(&intent, "closing", Some(&blockers.join("; ")))
            .map_err(task_error)?;
        return Ok(());
    }
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

fn require_settled_work(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let shells = crate::desktop::shells::owners(&store.sqlite, &task.id).map_err(task_error)?;
    let mut allowed: HashSet<_> = shells.into_iter().collect();
    for session in store
        .sqlite
        .task_work(&task.id)
        .map_err(task_error)?
        .sessions
    {
        if session.completed_at.is_some() {
            continue;
        }
        if let Some(owner) = store
            .sqlite
            .session_transport(session.id.as_str())
            .map_err(task_error)?
        {
            if crate::journal::lf_evidence(&store.sqlite, &owner) == ProcessIdentityEvidence::Dead {
                continue;
            }
            if store
                .sqlite
                .owner_retention(&owner)
                .map_err(task_error)?
                .is_some()
            {
                return Err(task_error(
                    "Requested work, a draft or a pending request still holds this Task",
                ));
            }
            allowed.insert(owner);
        }
    }
    let processes = store
        .sqlite
        .task_open_processes(&task.id)
        .map_err(task_error)?;
    for (process, _) in crate::ops::task_execution::unresolved_processes(&store.sqlite, &processes)?
    {
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
                .sqlite
                .lf_process(&id)
                .map_err(task_error)?
                .and_then(|p| p.parent_lf_process_id);
        }
        if !owned {
            return Err(task_error(format!(
                "Process {} still has live or unresolved work",
                process.id()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
