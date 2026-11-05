//! The completion and landing callers share this durable retirement operation.
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
#[cfg(not(test))]
use std::os::unix::process::CommandExt;
use std::path::Path;
#[cfg(not(test))]
use std::process::{Command, Stdio};
use std::time::Duration;

use super::{block_on_task, task_error, task_store};
use crate::durable::{TaskId, WorkStatus};
use crate::id::LfProcessId;
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
pub(super) fn lock(task: &TaskId, executor: bool) -> OpsResult<File> {
    let directory = crate::store::lf_home_dir().join("workspace-locks");
    std::fs::create_dir_all(&directory)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!(
            "{}{}.lock",
            task,
            if executor { "-executor" } else { "" }
        )))?;
    if executor {
        file.try_lock_exclusive()?;
    } else {
        file.lock_exclusive()?;
    }
    Ok(file)
}

const VIEWED: &str = "Retained by an open Task Session viewer";
const UNKNOWN: &str = "Session viewer liveness unknown; cleanup pending";

/// Why an LF viewer still retains this checkout. Only owners' own viewer
/// records count; an ordinary shell in the checkout never does. An owner that
/// died without recording its viewers leaves their fate unknown.
fn viewer_hold(store: &SqliteStore, task: &TaskId) -> OpsResult<Option<&'static str>> {
    let mut unknown = false;
    for owner in store.task_viewer_holds(task).map_err(task_error)? {
        let live = !owner.ended
            && crate::journal::process_evidence(store, &owner.owner)
                != ProcessIdentityEvidence::Dead;
        if live && owner.hold == ViewerHold::Viewed {
            return Ok(Some(VIEWED));
        }
        unknown = true;
    }
    Ok(unknown.then_some(UNKNOWN))
}

fn reconcile(store: &SqliteStore, task: &TaskId) -> OpsResult<Option<TaskRetirement>> {
    let hold = viewer_hold(store, task)?;
    store
        .reconcile_task_retirement(task, now(), hold)
        .map_err(task_error)
}

pub(crate) async fn schedule(store: &SharedStore, task: &Task) -> OpsResult<()> {
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(());
    }
    let _guard = lock(&task.id, false)?;
    let intent = reconcile(&store.sqlite, &task.id)?;
    if !needs_executor(intent.as_ref()) {
        return Ok(());
    }
    drop(_guard);
    start(&task.id)
}

/// The executor runs only toward a deadline. While a viewer holds the
/// checkout it exits; that owner wakes it again when its viewers release.
fn needs_executor(intent: Option<&TaskRetirement>) -> bool {
    intent.is_some_and(|intent| intent.phase != "retired" && intent.due_at.is_some())
}

/// An owner whose viewers released restarts cleanup for its waiting Tasks.
pub(crate) fn wake_released(store: &SqliteStore, owner: &LfProcessId) -> OpsResult<()> {
    for task in store.owner_waiting_retirements(owner).map_err(task_error)? {
        start(&task)?;
    }
    Ok(())
}

fn start(task: &TaskId) -> OpsResult<()> {
    #[cfg(test)]
    {
        let _ = task;
        Ok(())
    }
    #[cfg(not(test))]
    {
        // A running executor already watches the durable intent; repeated
        // releases must not launch another process for the same Task.
        let reservation = match lock(task, true) {
            Ok(lock) => lock,
            Err(OpsError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Ok(())
            }
            Err(error) => return Err(error),
        };
        let directory = crate::store::lf_home_dir().join("logs");
        std::fs::create_dir_all(&directory)?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("workspace-retirement.log"))?;
        let mut command = Command::new(std::env::current_exe()?);
        // The executor is not part of the completing turn's lineage: final work
        // in that turn remains a blocker, including a silent tool call.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("LF_") && key != "LF_HOME" && key != "LF_BIN" {
                command.env_remove(key);
            }
        }
        command
            .args(["task", "retire", task.as_str()])
            .current_dir(crate::store::lf_home_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log);
        // SAFETY: setsid is async-signal-safe and only changes the new child.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        drop(reservation);
        let mut child = command.spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

pub fn run(issue: &str) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let id = store
            .sqlite
            .resolve_task_id(issue, None)
            .map_err(task_error)?
            .ok_or_else(|| task_error("Task is unknown"))?;
        let _executor = match lock(&id, true) {
            Ok(lock) => lock,
            Err(OpsError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => {
                return Ok(())
            }
            Err(error) => return Err(error),
        };
        loop {
            if !crate::store::lf_home_dir().exists() {
                return Ok(());
            }
            let Some(task) = store.get_task(&id).await.map_err(task_error)? else {
                return Ok(());
            };
            if super::task_work_status(&store, &task).await? != WorkStatus::Done {
                return Ok(());
            }
            let result = {
                let _guard = lock(&id, false)?;
                advance(&store, &task).await
            };
            match result {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(error) => {
                    if let Some(intent) = store.sqlite.task_retirement(&id).map_err(task_error)? {
                        store
                            .sqlite
                            .retirement_progress(&intent, &intent.phase, Some(&error.to_string()))
                            .map_err(task_error)?;
                    }
                    tracing::warn!(task=%id,%error,"Task cleanup pending");
                    return Err(error);
                }
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    })
}

async fn advance(store: &SharedStore, task: &Task) -> OpsResult<bool> {
    let Some(intent) = reconcile(&store.sqlite, &task.id)? else {
        return Ok(true);
    };
    if !needs_executor(Some(&intent)) {
        return Ok(true);
    }
    if intent.due_at.is_none_or(|due| due > now()) {
        return Ok(false);
    }
    if super::task_work_status(store, task).await? != WorkStatus::Done {
        return Ok(true);
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
        return Ok(false);
    }
    if !store
        .sqlite
        .retirement_progress(&intent, "closing", None)
        .map_err(task_error)?
    {
        return Ok(false);
    }
    store
        .sqlite
        .close_task_shells(&task.id)
        .map_err(task_error)?;
    // The existing owner policy sees the closing intent and rechecks active
    // work and queued input in the serving loop before ending the provider.
    let blockers = crate::ops::task_execution::task_execution_blockers(&store.sqlite, &task.id)?;
    if !blockers.is_empty() {
        store
            .sqlite
            .retirement_progress(&intent, "closing", Some(&blockers.join("; ")))
            .map_err(task_error)?;
        return Ok(false);
    }
    if !store
        .sqlite
        .task_shell_jobs_settled(&task.id)
        .map_err(task_error)?
    {
        return Err(task_error(
            "Shell owner ended without confirmed job cleanup; retained checkout",
        ));
    }
    // A reopen revokes the pinned intent in the same write as the Task change.
    let Some(current) = store.sqlite.task_retirement(&task.id).map_err(task_error)? else {
        return Ok(true);
    };
    if current.completion != intent.completion || current.due_at.is_none() {
        return Ok(false);
    }
    // A viewer that attached during closing keeps the checkout.
    if reconcile(&store.sqlite, &task.id)?.is_none_or(|current| current.due_at.is_none()) {
        return Ok(false);
    }
    crate::ops::wt::apply_delete(deletion, &NullProgress)?;
    store
        .sqlite
        .retirement_progress(&intent, "retired", None)
        .map_err(task_error)?;
    Ok(true)
}

fn require_settled_work(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let shells = store
        .sqlite
        .task_shell_owners(&task.id)
        .map_err(task_error)?;
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
            if crate::journal::process_evidence(&store.sqlite, &owner)
                == ProcessIdentityEvidence::Dead
            {
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
        let mut current = Some(process.id.clone());
        let mut visited = HashSet::new();
        let mut owned = false;
        while let Some(id) = current.filter(|id| visited.insert(id.clone())) {
            if allowed.contains(&id) {
                owned = true;
                break;
            }
            current = store
                .sqlite
                .process(&id)
                .map_err(task_error)?
                .and_then(|p| p.parent_lf_process_id);
        }
        if !owned {
            return Err(task_error(format!(
                "Process {} still has live or unresolved work",
                process.id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
