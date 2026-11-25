//! A Task's auto setting. `lf task auto on` is an `lf task run` whose request asks
//! for auto, and `auto: on` in configuration turns it on as a Task takes up
//! its Graph. Auto is a setting on the Task, one `task_auto` row, held before
//! and after it takes up a Graph. The run reads it at each breakpoint,
//! passing on the default path while it is on, and at each step that merges,
//! waiting there while it is off. `lf task auto off` turns
//! it off; `lf task auto on` turns it on again.

use crate::durable::GraphProcessFilter;
use crate::ops::error::OpsResult;
use crate::session::GraphProcessSummaryState;
use loopflow_model::id::LfProcessId;
use loopflow_storage::process::{ConfigLayer, Starter};
use loopflow_storage::sqlite::{Auto, AutoState};
use loopflow_storage::work::task::Task;

use super::{block_on_task, task_error, task_store, SharedStore};

/// Who started a run: the Session whose agent started it, with the person
/// named as in `repo`.
pub fn starter(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    repo: &std::path::Path,
    session: Option<&loopflow_model::id::LfSessionId>,
) -> crate::store::StoreResult<crate::process::Starter> {
    let person = crate::config::participant_name_in(repo).ok().flatten();
    sqlite.starter(session, person.as_deref())
}

/// Why a Task run waits at its merging `step` while auto is off, and what
/// lands it. The run prints it as it stops; `lf task status` reads it after.
pub fn landing_wait(step: &str, off: &Auto, issue: &str) -> String {
    format!(
        "Waiting at {step}: auto is off ({}). `lf task auto on {issue}` lands it.",
        off.by
    )
}

/// Who is asking: the caller's Session, with the person named as in the
/// Task's repository.
fn caller(store: &SharedStore, task: &Task) -> OpsResult<Starter> {
    let repo = super::repository_for_task(store, task)?;
    starter(
        &store.sqlite,
        &repo,
        crate::journal::caller_session_id().as_ref(),
    )
    .map_err(task_error)
}

/// Save `state` as the Task's mark, set by `by`; with `unset_only`, a standing
/// mark stays. Returns whether auto was on before.
fn set(
    store: &SharedStore,
    task: &Task,
    state: AutoState,
    by: Starter,
    unset_only: bool,
) -> OpsResult<bool> {
    let mark = Auto {
        state,
        at: loopflow_storage::rows::now_unix(),
        by,
    };
    let before = store
        .sqlite
        .set_task_auto(&task.id, &mark, unset_only)
        .map_err(task_error)?;
    Ok(before.is_some_and(|before| before.state == AutoState::On))
}

/// `lf task auto on` turns auto on, lifting an off.
pub(super) fn turn_on(store: &SharedStore, task: &Task) -> OpsResult<()> {
    set(store, task, AutoState::On, caller(store, task)?, false)?;
    Ok(())
}

/// A Task that just took up a Graph turns auto on when its configuration says
/// `auto: on` and nobody has set it.
pub(super) fn configured(store: &SharedStore, task: &Task) -> OpsResult<()> {
    let (repo, wave) = super::task_scope(store, task)?;
    let resolved = crate::config::resolved(Some(&repo), wave.as_deref()).map_err(task_error)?;
    if resolved.config.auto != Some(crate::config::AutoDefault::On) {
        return Ok(());
    }
    let layer = match resolved.settings["auto"].selected() {
        Some(chosen) if chosen.scope == crate::config::settings::SettingScope::Wave => {
            ConfigLayer::Wave
        }
        Some(chosen) if chosen.source == crate::config::ConfigSource::Repository => {
            ConfigLayer::Repository
        }
        _ => ConfigLayer::Personal,
    };
    set(store, task, AutoState::On, Starter::Config { layer }, true)?;
    Ok(())
}

/// How long `lf task auto on` waits for a run that just set out to register its Graph.
const GRAPH_REGISTERS: std::time::Duration = std::time::Duration::from_secs(30);

/// The Graph process of the run `carrier` carries, waiting for one that just
/// set out to register it. `None` once the carrier is gone or the Task left
/// its run.
pub(super) async fn running_graph(
    store: &SharedStore,
    task: &Task,
    carrier: &LfProcessId,
) -> OpsResult<Option<LfProcessId>> {
    let filter = GraphProcessFilter {
        task_id: Some(task.id.clone()),
        state: Some(GraphProcessSummaryState::Current),
        ..GraphProcessFilter::default()
    };
    let started = std::time::Instant::now();
    loop {
        let page = store
            .graph_inventory(&filter, None, std::num::NonZeroU32::MIN)
            .await
            .map_err(task_error)?;
        if let Some(entry) = page.entries.into_iter().next() {
            return LfProcessId::parse(&entry.summary.id)
                .map(Some)
                .map_err(task_error);
        }
        let carried = crate::store::task_work::task_graph_row(&store.sqlite, &task.id)
            .map_err(task_error)?
            .and_then(|row| row.run)
            .is_some_and(|(_, now)| {
                &now.id == carrier && crate::store::task_work::carrier_may_run(&now)
            });
        if !carried || started.elapsed() > GRAPH_REGISTERS {
            return Ok(None);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

/// `lf task auto off`: turn auto off and take back any merge request
/// Loopflow armed on GitHub. Nothing is killed: the run carries on to its next
/// breakpoint or merging step and waits there, until auto is on again.
/// `reason` is saved as a comment on the Task. Returns whether auto was on, or
/// `None` when no Task has that issue.
pub fn off(issue: &str, reason: Option<&str>) -> OpsResult<Option<bool>> {
    let turned = block_on_task(async {
        let store = task_store().await?;
        let Some(task) = store.get_task_by_issue(issue).await.map_err(task_error)? else {
            return Ok(None);
        };
        let was_on = set(&store, &task, AutoState::Off, caller(&store, &task)?, false)?;
        if let Some(reason) = reason.map(str::trim).filter(|reason| !reason.is_empty()) {
            super::append_task_comment_async(&store, &task, reason, false).await?;
        }
        Ok(Some((task, was_on)))
    })?;
    let Some((task, was_on)) = turned else {
        return Ok(None);
    };
    // A run with no breakpoint ahead arms a merge without a mark, so
    // the request is taken back whether or not auto was on.
    if let Ok(worktree) = task.worktree() {
        super::clear_task_pr_merge_before_head_mutation(worktree, true, &|_| {})?;
    }
    Ok(Some(was_on))
}

/// Create a Task for the checkout's branch so auto can run it: it takes the
/// branch, its checkout and any PR as its own, and is titled from the branch.
pub fn branch_task(repo: &std::path::Path, wave: Option<&str>) -> OpsResult<Task> {
    let branch = crate::git::current_branch(repo)
        .map_err(task_error)?
        .ok_or_else(|| task_error("lf task auto on needs a branch; this checkout is detached"))?;
    if crate::git::get_default_branch(repo).map_err(task_error)? == branch {
        return Err(task_error(format!(
            "{branch} is the default branch; turn auto on for a Task or a branch of its own"
        )));
    }
    let title = branch
        .rsplit('/')
        .next()
        .unwrap_or(&branch)
        .replace(['-', '_'], " ");
    let task = super::task_create(
        repo,
        wave,
        Some(title),
        None,
        Default::default(),
        Default::default(),
        None,
    )?;
    super::prepare_task(
        repo,
        &task.identifier,
        super::TaskProcessOptions {
            wave: wave.map(str::to_owned),
            branch: Some(branch),
            ..Default::default()
        },
        None,
        super::remote::source_for_task(&task.identifier)?,
    )
}
