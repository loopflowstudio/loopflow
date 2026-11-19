//! A Task's auto setting. `lf ship` is an `lf task run` whose request asks
//! for auto. Auto is one mark on `task_graphs`; the run reads it at each
//! interactive step, standing in for the person while it is on, and at each
//! step that merges, waiting there while it is off. `lf task auto off` turns
//! it off; `lf ship`, spelled `lf task auto on` too, turns it on again.

use crate::durable::GraphProcessFilter;
use crate::ops::error::OpsResult;
use crate::session::GraphProcessSummaryState;
use loopflow_model::id::LfProcessId;
use loopflow_storage::sqlite::{Auto, AutoState};
use loopflow_storage::work::task::Task;

use super::{block_on_task, task_error, task_store, SharedStore};

/// Who asks a Task run for auto.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AutoAsk {
    #[default]
    Nobody,
    /// `lf -b task run`: on, unless someone turned it off.
    Batch,
    /// `lf ship`: on, lifting an off, with the person's note.
    Person { note: Option<String> },
}

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

/// Save `state` as this caller's mark, naming who is asking as it is written;
/// with `keeps_off`, a standing off stays. Returns whether auto was on before.
fn set(
    store: &SharedStore,
    task: &Task,
    state: AutoState,
    keeps_off: bool,
    note: Option<&str>,
) -> OpsResult<bool> {
    let repo = super::repository_for_task(store, task)?;
    let mark = Auto {
        state,
        at: loopflow_storage::rows::now_unix(),
        by: starter(
            &store.sqlite,
            &repo,
            crate::journal::caller_session_id().as_ref(),
        )
        .map_err(task_error)?,
    };
    let before = store
        .sqlite
        .set_task_auto(&task.id, &mark, keeps_off, note)
        .map_err(task_error)?;
    Ok(before.is_some_and(|before| before.state == AutoState::On))
}

/// Write what the run's request asks of auto, once the Task has its Graph.
pub(super) fn ask(store: &SharedStore, task: &Task, ask: &AutoAsk) -> OpsResult<()> {
    match ask {
        AutoAsk::Nobody => Ok(()),
        AutoAsk::Batch => set(store, task, AutoState::On, true, None).map(drop),
        AutoAsk::Person { note } => {
            set(store, task, AutoState::On, false, note.as_deref()).map(drop)
        }
    }
}

/// How long `lf ship` waits for a run that just set out to register its Graph.
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
/// interactive step or merging step and waits there, until auto is on again.
/// Returns whether auto was on.
pub fn off(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let (task, was_on) = block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let was_on = set(&store, &task, AutoState::Off, false, note)?;
        Ok((task, was_on))
    })?;
    if was_on {
        if let Ok(worktree) = task.worktree() {
            super::clear_task_pr_merge_before_head_mutation(worktree, true, &|_| {})?;
        }
    }
    Ok(was_on)
}

/// Create a Task for the checkout's branch so it can ship: it takes the branch,
/// its checkout and any PR as its own, and is titled from the branch.
pub fn ship_branch_task(repo: &std::path::Path, wave: Option<&str>) -> OpsResult<Task> {
    let branch = crate::git::current_branch(repo)
        .map_err(task_error)?
        .ok_or_else(|| task_error("lf ship needs a branch; this checkout is detached"))?;
    if crate::git::get_default_branch(repo).map_err(task_error)? == branch {
        return Err(task_error(format!(
            "{branch} is the default branch; ship from a Task or a branch of its own"
        )));
    }
    let title = branch
        .rsplit('/')
        .next()
        .unwrap_or(&branch)
        .replace(['-', '_'], " ");
    let task = super::task_create(repo, wave, Some(title), None, Default::default(), None)?;
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
