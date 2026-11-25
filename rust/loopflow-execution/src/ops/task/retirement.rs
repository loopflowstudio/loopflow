//! The completion and landing callers share this durable retirement operation.
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::path::Path;

use super::{block_on_task, task_error, task_store};
use crate::ops::{NullProgress, OpsError, OpsResult};
use crate::store::{
    sqlite::{SqliteStore, TaskRetirement, ViewerHold},
    SharedStore,
};
use fs2::FileExt;
use loopflow_model::durable::{TaskId, WorkStatus};
use loopflow_model::id::TerminalId;
use loopflow_storage::sqlite::VIEWER_OBSERVATION_GRACE;
use loopflow_storage::work::task::Task;

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

// Completion, landing and each cleanup pass serialize against the same Task.
// A returning viewer resets the checkout deadline before the next pass.
pub fn lock(task: &TaskId) -> OpsResult<File> {
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

/// The host decides whether the last view is still absent when it receives the request.
pub fn close_unviewed_chats(store: &SqliteStore) -> OpsResult<()> {
    for terminal in store.unviewed_chat_candidates(now()).map_err(task_error)? {
        if store
            .terminal_has_scoped_session(&terminal)
            .map_err(task_error)?
            || store
                .terminal_retention(&terminal)
                .map_err(task_error)?
                .is_some()
        {
            continue;
        }
        if let Err(error) = crate::transport::retire_chat(&terminal) {
            tracing::warn!(%terminal, %error, "standalone chat retirement unavailable");
        }
    }
    Ok(())
}

fn viewer_hold(store: &SqliteStore, task: &TaskId, now: i64) -> OpsResult<Option<ViewerHold>> {
    let mut absent_since = None;
    for terminal in store.task_viewer_holds(task).map_err(task_error)? {
        let at = match terminal.hold {
            ViewerHold::Viewed(at) => {
                if !terminal.ended && now < at + VIEWER_OBSERVATION_GRACE {
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
    for (terminal, abandoned) in conversation_closes(store, task)? {
        let notice = if abandoned {
            "Task abandoned"
        } else {
            "Task completed"
        };
        // A held terminal has no process to signal: the server ends it on its
        // notice. One the server cannot reach is a candidate next pass.
        if let Err(error) = crate::transport::close_notice(&terminal, notice) {
            tracing::warn!(%terminal, %error, "held terminal close unavailable");
        }
    }
    Ok(())
}

fn conversation_closes(store: &SqliteStore, task: &TaskId) -> OpsResult<Vec<(TerminalId, bool)>> {
    let now = now();
    let mut terminals = store.task_close_candidates(task, now).map_err(task_error)?;
    let Some((_, abandoned)) = terminals.first() else {
        return Ok(terminals);
    };
    if *abandoned {
        let holds = store.task_viewer_holds(task).map_err(task_error)?;
        terminals.retain(|(terminal, _)| {
            !holds.iter().any(|hold| &hold.terminal == terminal
            && matches!(hold.hold, ViewerHold::Viewed(at) if now < at + VIEWER_OBSERVATION_GRACE))
        });
    } else {
        match viewer_hold(store, task, now)? {
            Some(ViewerHold::Viewed(_)) => return Ok(Vec::new()),
            Some(ViewerHold::Absent(at))
                if at + loopflow_storage::sqlite::WORKSPACE_GRACE > now =>
            {
                return Ok(Vec::new())
            }
            _ => {}
        }
        if unsettled_work(store, task)?.is_some() {
            return Ok(Vec::new());
        }
    }
    Ok(terminals)
}

pub async fn schedule(store: &SharedStore, task: &Task) -> OpsResult<()> {
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
            Some(issue) => {
                vec![
                    crate::store::children::resolve_task_id(&store.sqlite, issue, None)
                        .map_err(task_error)?
                        .ok_or_else(|| task_error("Task is unknown"))?,
                ]
            }
            None => {
                close_unviewed_chats(&store.sqlite)?;
                store.sqlite.pending_retirements().map_err(task_error)?
            }
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
    for (terminal, _) in
        crate::desktop::shells::hosts(&store.sqlite, &task.id).map_err(task_error)?
    {
        // An ended shell has nothing to hear the notice.
        if store
            .sqlite
            .terminal_running(&terminal)
            .map_err(task_error)?
        {
            if let Err(error) = crate::transport::close_notice(&terminal, "Task completed") {
                tracing::warn!(%terminal, %error, "held shell close unavailable");
            }
        }
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
    // Work running in one of the Task's shells or conversation terminals ends
    // when that terminal does.
    let mut terminals: HashSet<_> = crate::desktop::shells::hosts(store, task)
        .map_err(task_error)?
        .into_iter()
        .map(|(terminal, _)| terminal)
        .collect();
    for terminal in store.task_session_terminals(task).map_err(task_error)? {
        if store
            .terminal_retention(&terminal)
            .map_err(task_error)?
            .is_some()
        {
            return Ok(Some(
                "Requested work or a pending request still holds this Task".into(),
            ));
        }
        terminals.insert(terminal);
    }
    let work = store.task_open_work(task).map_err(task_error)?;
    for (work, _) in crate::ops::task_execution::unresolved_work(store, &work)? {
        if !work
            .held_terminal()
            .is_some_and(|terminal| terminals.contains(terminal))
        {
            return Ok(Some(format!(
                "{} still has live or unresolved work",
                work.label()
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[path = "retirement_tests.rs"]
mod tests;
