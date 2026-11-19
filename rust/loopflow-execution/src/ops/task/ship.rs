//! `lf ship`: someone approves a Task once, and its remaining graph runs to
//! `end` with no one at hand, as `lf -b task run` runs it: interactive steps
//! headless, standing in for the person. The approval is stored; the Task reads
//! shipping while this Machine's ship process carries it there, and idle when
//! that process exited. Unassigning the Task stops the ship, Stop withdraws the
//! approval until the next `lf ship` (no run into `end` approves it meanwhile),
//! and a ship process that died is started again.

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::ops::error::OpsResult;
use crate::ops::pr::PrReadFreshness;
use crate::store::ProcessIdentityEvidence;
use crate::task_graph::{TaskGraph, TaskGraphPosition, END};
use loopflow_model::id::LfProcessId;
use loopflow_storage::graph_process::{ExecutionResult, RunOwner};
use loopflow_storage::pr_landing::{PrLanding, PrLandingState};
use loopflow_storage::sqlite::TaskShipping;
use loopflow_storage::work::task::{PrPhase, Task};

use super::{block_on_task, task_error, task_store, SharedStore, TaskRunSelection};

/// What `lf ship` found before running anything.
#[derive(Debug)]
pub enum ShipStart {
    /// This process now ships the Task.
    Begun(Box<Task>),
    /// A live process already ships it.
    AlreadyShipping { issue: String, process: LfProcessId },
    /// The Task already stands at `end`.
    AtEnd { issue: String },
}

/// The next thing the ship process runs.
#[derive(Debug, PartialEq, Eq)]
pub enum ShipStep {
    /// Run the rest of the graph headless: from the interactive step the Task
    /// waits at, standing in for the person, or on from where it stands.
    Run { from: Option<String> },
    /// Someone unassigned the Task; nothing moves it until it is claimed.
    Unassigned,
    /// Someone stopped shipping; the approval is withdrawn.
    Stopped,
    /// The Task reached `end`, completed, or was abandoned.
    Finished,
}

/// What a shipping Task is doing, derived from its ship process, its state
/// and its landing. Nothing here waits on a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ShippingSnapshot {
    /// When the Task was handed off.
    pub shipped_at: i64,
    pub state: ShippingState,
    /// One line saying what is happening.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ShippingState {
    Building,
    Landing,
    Repairing,
    Restarting,
    Unassigned,
}

async fn task_by_issue(store: &SharedStore, issue: &str) -> OpsResult<Task> {
    store
        .get_task_by_issue(issue)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))
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

/// A stop names the person as it was at admission, independent of later settings.
pub fn stopper(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    stop: &loopflow_storage::sqlite::task_work::ShipStop,
) -> crate::store::StoreResult<crate::process::Starter> {
    sqlite.starter(stop.session.as_ref(), stop.person.as_deref())
}

/// Whether the run carrying a shipping Task has exited. Unknown evidence is
/// never read as a dead process; no carrier at all is.
fn ship_carrier_evidence(shipping: &TaskShipping) -> ProcessIdentityEvidence {
    match &shipping.carrier {
        Some(carrier) => crate::process::RecordedProcess::from(carrier).evidence(),
        None => ProcessIdentityEvidence::Dead,
    }
}

/// When a ship process that failed at `failed_at` starts again: the landing's
/// repair backoff, 1 minute up to 30, counted over restarts in a row.
fn restart_due(shipping: &TaskShipping, failed_at: i64) -> i64 {
    failed_at + crate::ops::pr_landing::repair_backoff(shipping.restarts.saturating_add(1))
}

/// When the ship process failed, and the error that caused it: its Task's
/// latest Graph run's cause when that run failed, else its own error.
fn carrier_failure(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    task: &loopflow_storage::work::task::TaskId,
    shipping: &TaskShipping,
) -> crate::store::StoreResult<Option<(i64, String)>> {
    let Some(carrier) = shipping.carrier.as_ref().filter(|carrier| {
        crate::process::os_process::observed_result(carrier) == ExecutionResult::Failed
    }) else {
        return Ok(None);
    };
    let at = carrier.ended_at.expect("a failed run has ended");
    let graph = match sqlite.latest_task_graphs(&[task])?.remove(task) {
        Some(graph) => sqlite.graph_failure_cause(graph.id())?,
        None => None,
    };
    let error = graph
        .map(|(_, error)| error)
        .or_else(|| carrier.error.clone())
        .unwrap_or_else(|| "it failed".into());
    Ok(Some((at, error)))
}

/// The Task's shipping, when it is shipping. `owned` is whether anyone owns
/// it, `alive` the ship process's liveness and `failure` when it failed and why.
pub(crate) fn shipping_snapshot(
    graph: Option<&TaskGraph>,
    shipping: &TaskShipping,
    (status, owned): (TaskState, bool),
    alive: ProcessIdentityEvidence,
    failure: Option<&(i64, String)>,
    landing: Option<&PrLanding>,
) -> Option<ShippingSnapshot> {
    if status.is_terminal() {
        return None;
    }
    let restart = failure.map(|(failed_at, error)| (restart_due(shipping, *failed_at), error));
    let shipping = |state, reason: String| {
        Some(ShippingSnapshot {
            shipped_at: shipping.shipped_at,
            state,
            reason,
        })
    };
    if !owned {
        return shipping(
            ShippingState::Unassigned,
            "Unassigned; `lf task claim` and `lf ship` keep it shipping".into(),
        );
    }
    if alive == ProcessIdentityEvidence::Dead {
        let Some((due, error)) = restart else {
            return shipping(
                ShippingState::Restarting,
                "Ship process exited; the server's next pass starts it again".into(),
            );
        };
        let error = error
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .trim_end_matches('.');
        let wait = due - loopflow_storage::rows::now_unix();
        let when = match wait {
            ..=0 => "retrying on the server's next pass".to_string(),
            1..=60 => "retrying within a minute".to_string(),
            _ => format!("retrying in {} min", (wait + 59) / 60),
        };
        return shipping(
            ShippingState::Restarting,
            format!("Ship failed: {error}; {when}"),
        );
    }
    if let Some(landing) = landing {
        let number = landing.pr_number;
        return match landing.state {
            PrLandingState::Repairing | PrLandingState::Blocked => shipping(
                ShippingState::Repairing,
                match &landing.blocked_reason {
                    Some(reason) => format!("Repairing #{number} · {reason}"),
                    None => format!("Repairing #{number}"),
                },
            ),
            _ => shipping(ShippingState::Landing, format!("CI running on #{number}")),
        };
    }
    let reason = match graph {
        None => "taking up its graph".to_string(),
        Some(graph) => match (&graph.position, graph.waiting_step()) {
            (TaskGraphPosition::Run { at, .. }, _) => format!("running at {at}"),
            (_, Some(step)) => format!("{} (stand-in)", step.name),
            (TaskGraphPosition::At { step }, None) => format!("from {step}"),
        },
    };
    shipping(ShippingState::Building, reason)
}

/// Read a Task's shipping from the store: its handoff, its ship process and its landing.
pub fn read_shipping(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    task: &loopflow_storage::work::task::TaskId,
    graph: Option<&TaskGraph>,
    status: TaskState,
) -> crate::store::StoreResult<Option<ShippingSnapshot>> {
    let Some(shipping) = sqlite.task_shipping(task)? else {
        return Ok(None);
    };
    let alive = ship_carrier_evidence(&shipping);
    let failure = carrier_failure(sqlite, task, &shipping)?;
    let landing = sqlite.pending_task_landing(task)?;
    Ok(shipping_snapshot(
        graph,
        &shipping,
        (status, sqlite.task_owned(task)?),
        alive,
        failure.as_ref(),
        landing.as_ref(),
    ))
}

/// Hand the Task off and claim it for this process, unless a live process
/// already ships it. A Task with no graph takes up its default first; naming
/// `graph` takes that one up in place of the remaining steps.
pub fn ship_begin(issue: &str, graph: Option<&str>, note: Option<&str>) -> OpsResult<ShipStart> {
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| task_error("lf ship requires a registered Process"))?;
    block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        let issue = task.identifier.clone();
        match store.sqlite.task_state(&task.id).map_err(task_error)? {
            TaskState::Done => return Err(task_error(format!("Task {issue} is complete"))),
            TaskState::Abandoned => {
                return Err(task_error(format!(
                    "Task {issue} is abandoned; recover it before shipping"
                )))
            }
            _ => {}
        }
        store
            .sqlite
            .require_task_launch(&task.id)
            .map_err(task_error)?;
        if let Some(carrier) = store
            .sqlite
            .task_shipping(&task.id)
            .map_err(task_error)?
            .and_then(|shipping| shipping.carrier)
        {
            if carrier.id != process
                && crate::process::RecordedProcess::from(&carrier).evidence()
                    != ProcessIdentityEvidence::Dead
            {
                return Ok(ShipStart::AlreadyShipping {
                    issue,
                    process: carrier.id,
                });
            }
        }
        let current =
            crate::store::task_work::task_graph(&store.sqlite, &task.id).map_err(task_error)?;
        if current.is_none() || graph.is_some() {
            let TaskRunSelection {
                graph: selected,
                take_up,
                ..
            } = super::select_task_run(
                &store,
                &task,
                task.worktree()?,
                graph,
                Some("start"),
                |_| false,
            )?;
            if take_up {
                crate::store::task_work::take_up_task_graph(&store.sqlite, &task.id, &selected)
                    .map_err(task_error)?;
            }
        } else if current.as_ref().is_some_and(
            |graph| matches!(&graph.position, TaskGraphPosition::At { step } if step == END),
        ) {
            return Ok(ShipStart::AtEnd { issue });
        }
        let repo = super::repository_for_task(&store, &task)?;
        let by = starter(
            &store.sqlite,
            &repo,
            crate::journal::caller_session_id().as_ref(),
        )
        .map_err(task_error)?;
        let mut handoff = format!("{by} shipped it");
        if let Some(graph) = graph {
            handoff.push_str(&format!(" with {graph}"));
        }
        if let Some(note) = note {
            handoff.push_str(&format!(": {note}"));
        }
        // Shipping claims an unowned Task, as `lf task run` would; unassigning
        // it later is what stops shipping.
        if !store.sqlite.task_owned(&task.id).map_err(task_error)? {
            crate::store::ownership::claim_task(&store.sqlite, &task.id).map_err(task_error)?;
        }
        let start = crate::journal::current_execution_start()
            .ok_or_else(|| task_error("lf ship requires a registered Process"))?;
        store
            .sqlite
            .ship_task(&task.id, (&process, &start), &handoff)
            .map_err(task_error)?;
        crate::journal::mark_run_owner(RunOwner::ShipCarrier);
        Ok(ShipStart::Begun(Box::new(task)))
    })
}

/// Withdraw a Task's Auto-Ship approval: stop its ship process and take back any
/// merge request Loopflow armed on GitHub. The stop stands until `lf ship`: a
/// run reaching `end` no longer approves it. Returns false when it was not shipping.
pub fn ship_stop(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let process = crate::journal::current_lf_process_id();
    withdraw(issue, Withdrawal::Stop { process, note })
}

/// Withdraw approval when a Task run stops on a provider refusal or repeated
/// failure. This is no person's stop, so a relaunch that reaches `end` may
/// approve it again. Returns false when it was not shipping.
pub fn ship_halt(issue: &str, cause: &str) -> OpsResult<bool> {
    withdraw(issue, Withdrawal::Halt { cause })
}

enum Withdrawal<'a> {
    Stop {
        process: Option<LfProcessId>,
        note: Option<&'a str>,
    },
    Halt {
        cause: &'a str,
    },
}

fn withdraw(issue: &str, withdrawal: Withdrawal<'_>) -> OpsResult<bool> {
    let current = crate::journal::current_lf_process_id();
    let (task, stopped) = block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        let (note, stopped_by) = match &withdrawal {
            Withdrawal::Stop { process, note } => {
                let repo = super::repository_for_task(&store, &task)?;
                let stop = loopflow_storage::sqlite::task_work::ShipStop {
                    at: loopflow_storage::rows::now_unix(),
                    by: process.clone(),
                    person: crate::config::participant_name_in(&repo).ok().flatten(),
                    session: crate::journal::caller_session_id(),
                };
                let by = stopper(&store.sqlite, &stop).map_err(task_error)?;
                let note = match note {
                    Some(note) => format!("{by}: {note}"),
                    None => by.to_string(),
                };
                (note, Some(stop))
            }
            Withdrawal::Halt { cause } => (format!("Loopflow: {cause}"), None),
        };
        let stopped = store
            .sqlite
            .stop_shipping(&task.id, &note, stopped_by)
            .map_err(task_error)?;
        if let Some(carrier) = stopped
            .as_ref()
            .and_then(|shipping| shipping.carrier.as_ref())
            .filter(|carrier| current.as_ref() != Some(&carrier.id))
        {
            if crate::process::RecordedProcess::from(carrier).evidence()
                == ProcessIdentityEvidence::Live
            {
                if let Some(pid) = carrier.pid {
                    crate::platform::kill_process(pid);
                }
            }
        }
        Ok((task, stopped.is_some()))
    })?;
    if stopped {
        if let Ok(worktree) = task.worktree() {
            super::clear_task_pr_merge_before_head_mutation(worktree, true, &|_| {})?;
        }
    }
    Ok(stopped)
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

/// What a merged PR did to a shipping Task.
#[derive(Debug, PartialEq, Eq)]
pub enum ShipMerge {
    /// No merge to act on.
    Open,
    /// Its PR merged; the Task now stands at the interactive step that checks merged work.
    Verifying(String),
    /// Its PR merged and the Task is done.
    Done,
}

/// The interactive step that checks merged work: the last step of the graph.
fn post_merge_step(graph: &crate::task_graph::CapturedGraph) -> Option<&str> {
    graph
        .interactive_steps()
        .into_iter()
        .find(|(place, _)| graph.after(place).is_none())
        .and_then(|(_, skill)| skill.interactive_step())
}

/// Act on a shipping Task's merged PR, whoever merged it, so no further
/// build step runs on delivered work: send it to the step that checks merged
/// work, or complete it when its graph has none. A PR the Task already
/// completed with is history: reopened, the Task ships its new work.
pub fn ship_settle_merge(issue: &str) -> OpsResult<ShipMerge> {
    block_on_task(async {
        let store = task_store().await?;
        let mut task = task_by_issue(&store, issue).await?;
        let Some(pr) =
            super::reconcile_task_pr_observation(&store, &mut task, PrReadFreshness::Fresh).await?
        else {
            return Ok(ShipMerge::Open);
        };
        if pr.phase() != PrPhase::Merged {
            return Ok(ShipMerge::Open);
        }
        let published_at = pr
            .publication
            .as_ref()
            .map_or(0, |publication| publication.requested_at.unix_timestamp());
        if store
            .sqlite
            .task_completed_since(&task.id, published_at)
            .map_err(task_error)?
        {
            return Ok(ShipMerge::Open);
        }
        let row =
            crate::store::task_work::task_graph_row(&store.sqlite, &task.id).map_err(task_error)?;
        // A Task already at `end` has checked the merge; it only completes.
        if let Some(row) = row.as_ref().filter(|row| row.step != END) {
            if let Some(step) = post_merge_step(&row.graph) {
                // Standing at it, or a run left from it: it already checks the merge.
                if row.step == step {
                    return Ok(ShipMerge::Open);
                }
                store
                    .sqlite
                    .set_task_graph_step(&task.id, step)
                    .map_err(task_error)?;
                return Ok(ShipMerge::Verifying(step.to_owned()));
            }
        }
        if let Some(request) = store
            .sqlite
            .request_task_completion(&task.id, Some("its PR merged while shipping"))
            .map_err(task_error)?
        {
            super::settle_completion(&store, &mut task, request).await?;
        }
        Ok(
            if store.sqlite.task_state(&task.id).map_err(task_error)? == TaskState::Done {
                ShipMerge::Done
            } else {
                ShipMerge::Open
            },
        )
    })
}

/// Read where the shipped Task stands and what runs next.
pub fn ship_next(issue: &str) -> OpsResult<ShipStep> {
    block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        if let TaskState::Done | TaskState::Abandoned =
            store.sqlite.task_state(&task.id).map_err(task_error)?
        {
            return Ok(ShipStep::Finished);
        }
        if !store.sqlite.task_owned(&task.id).map_err(task_error)? {
            return Ok(ShipStep::Unassigned);
        }
        if store
            .sqlite
            .task_shipping(&task.id)
            .map_err(task_error)?
            .is_none()
        {
            return Ok(ShipStep::Stopped);
        }
        let graph = crate::store::task_work::task_graph(&store.sqlite, &task.id)
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {issue} has no graph to ship")))?;
        next_step(&graph, &task.identifier)
    })
}

fn next_step(graph: &TaskGraph, issue: &str) -> OpsResult<ShipStep> {
    match &graph.position {
        TaskGraphPosition::At { step } if step == END => Ok(ShipStep::Finished),
        TaskGraphPosition::Run { running: true, .. } => Err(task_error(format!(
            "Task {issue} is running, carried by another `lf task run`; read `lf task status {issue}`"
        ))),
        TaskGraphPosition::Run { .. } => Ok(ShipStep::Run { from: None }),
        TaskGraphPosition::At { .. } => Ok(ShipStep::Run {
            from: graph.waiting_step().map(|step| step.name.clone()),
        }),
    }
}

/// A ship step completed, so a later failure restarts after the shortest wait.
pub fn ship_progressed(task: &loopflow_storage::work::task::TaskId) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        store
            .sqlite
            .reset_task_run_restarts(task)
            .map_err(task_error)
    })
}

/// Start `lf <args>` again in `worktree`, outliving this process, under `agent`
/// when the run had one. Recovery runs inside any read, so the reader's own
/// agent choice and settings are cleared rather than inherited.
pub(super) fn restart_lf_in(
    worktree: &std::path::Path,
    agent: Option<&str>,
    args: &[&str],
) -> OpsResult<()> {
    let context = crate::os_process::execution_context().map_err(task_error)?;
    let mut argv = vec![context.lf_bin.to_string_lossy().to_string()];
    if let Some(agent) = agent {
        argv.extend(["--agent".to_string(), agent.to_string()]);
    }
    argv.extend(args.iter().map(ToString::to_string));
    let env = [
        (crate::config::AGENT_OVERRIDE_ENV, ""),
        ("LF_SETTINGS", "{}"),
    ];
    crate::os_process::detached::start_lf(worktree, &argv, &env, &|_| {}).map_err(task_error)?;
    Ok(())
}

/// How many times in a row a run whose carrier died is carried again before
/// it waits for a person's `lf task run`.
pub const RUN_RESTARTS: u32 = 5;

/// A Task run whose carrier (`lf task run`) died before the run stopped: it
/// was killed, or ended with no exit record, rather than exiting on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiedRun {
    /// The step the run reached.
    pub at: String,
    /// The carrier that died, which the next carrier takes the run over from.
    pub carrier: LfProcessId,
    /// Starts since the run last finished a step, and when the last one was.
    pub restarts: loopflow_storage::sqlite::task_work::TaskRunRestarts,
}

impl DiedRun {
    /// When the next start is due (0: at once), or `None` once
    /// [`RUN_RESTARTS`] ran out.
    pub fn next_at(&self) -> Option<i64> {
        (self.restarts.count < RUN_RESTARTS).then(|| {
            crate::ops::recovery::next_start_at(self.restarts.count, self.restarts.at).unwrap_or(0)
        })
    }

    /// One line for the Task's run.
    pub fn reason(&self, issue: &str, now: i64) -> String {
        let at = &self.at;
        match self.next_at() {
            None => format!(
                "Run died {} times at {at}; `lf task run {issue}` starts it again",
                self.restarts.count
            ),
            Some(due) if due <= now => {
                format!("Run died at {at}; the server's next pass carries it again")
            }
            Some(due) => format!(
                "Run died at {at}; it is carried again in {} min (restart {} of {RUN_RESTARTS})",
                (due - now + 59) / 60,
                self.restarts.count + 1
            ),
        }
    }
}

/// The Task's run, when its carrier died and nothing of the run still runs.
/// Shipping Tasks answer through their shipping instead.
pub fn died_run(
    sqlite: &loopflow_storage::sqlite::SqliteStore,
    task: &loopflow_storage::work::task::TaskId,
) -> crate::store::StoreResult<Option<DiedRun>> {
    let Some(row) = crate::store::task_work::task_graph_row(sqlite, task)? else {
        return Ok(None);
    };
    let Some((cursor, carrier)) = row.run else {
        return Ok(None);
    };
    // A carrier that exited on its own (failed, interrupted, waiting on a
    // limit) cleared its OS process; one that died left it behind, until
    // retention cleared it and recorded the death.
    let died = carrier.died_at.is_some()
        || (carrier.pid.is_some() && !crate::store::task_work::carrier_may_run(&carrier));
    if !died
        || sqlite.task_shipping(task)?.is_some()
        || crate::ops::task_execution::open_graph_execution(sqlite, task)?.is_some()
    {
        return Ok(None);
    }
    Ok(Some(DiedRun {
        at: row.graph.step_name_of(&cursor).unwrap_or(row.step),
        carrier: carrier.id,
        restarts: sqlite.task_run_restarts(task)?,
    }))
}

/// Carry again each run on this Machine whose carrier is gone: a shipping
/// Task's ship process that exited (`lf --task ISSUE ship`), and a moving
/// Task's `lf task run` that died (`lf task run ISSUE`, which stops at the next
/// interactive step as before). Unassigned, moved and finished Tasks wait.
/// Returns when the soonest run still waiting its backoff comes due.
pub(crate) fn restart_runs() -> OpsResult<Option<i64>> {
    block_on_task(async {
        let store = task_store().await?;
        let now = loopflow_storage::rows::now_unix();
        let mut soonest: Option<i64> = None;
        let mut wait = |due: i64| soonest = Some(soonest.map_or(due, |at| at.min(due)));
        for (id, shipping) in store.sqlite.local_shipping_tasks().map_err(task_error)? {
            if let Some(due) = restart_ship(&store, &id, &shipping, now).await? {
                wait(due);
            }
        }
        for id in store.sqlite.local_task_runs().map_err(task_error)? {
            let Some(died) = died_run(&store.sqlite, &id).map_err(task_error)? else {
                continue;
            };
            let Some(due) = died.next_at() else {
                continue;
            };
            if due > now {
                wait(due);
                continue;
            }
            let Some((task, worktree, _)) = continuable(&store, &id, now).await? else {
                continue;
            };
            if !store
                .sqlite
                .count_task_run_restart(&id, Some(&died.carrier), died.restarts.count)
                .map_err(task_error)?
            {
                continue;
            }
            restart_lf_in(&worktree, None, &["task", "run", &task.identifier])?;
            tracing::info!(
                "carried {}'s run again at {} after its carrier died",
                task.identifier,
                died.at
            );
        }
        Ok(soonest)
    })
}

/// The Task, its checkout and the agent a usage limit that reset left it on,
/// when a run of it may start here now: open, claimed here, checked out, and
/// not waiting out a limit.
async fn continuable(
    store: &SharedStore,
    id: &loopflow_storage::work::task::TaskId,
    now: i64,
) -> OpsResult<Option<(Task, std::path::PathBuf, Option<String>)>> {
    if store
        .sqlite
        .task_state(id)
        .map_err(task_error)?
        .is_terminal()
        || !store.sqlite.task_claimed_here(id).map_err(task_error)?
    {
        return Ok(None);
    }
    let wait = super::limit_recovery::waiting_on_limit(&store.sqlite, id).map_err(task_error)?;
    if wait.as_ref().is_some_and(|wait| wait.retry_at > now) {
        return Ok(None);
    }
    let Some(task) = store.get_task(id).await.map_err(task_error)? else {
        return Ok(None);
    };
    let Some(worktree) = task.worktree.clone().filter(|path| path.is_dir()) else {
        return Ok(None);
    };
    Ok(Some((task, worktree, wait.and_then(|wait| wait.agent))))
}

/// Start `lf --task ISSUE ship` again when its ship process has exited. After
/// a failure the restart waits, longer each time in a row; a process that
/// ended without failing (killed, interrupted) restarts at once. Returns when
/// a restart still waiting its backoff comes due.
async fn restart_ship(
    store: &SharedStore,
    id: &loopflow_storage::work::task::TaskId,
    shipping: &TaskShipping,
    now: i64,
) -> OpsResult<Option<i64>> {
    if ship_carrier_evidence(shipping) != ProcessIdentityEvidence::Dead {
        return Ok(None);
    }
    // The carrier can record its exit after the sweep reads it, before the OS
    // reports it dead. Read its final result before deciding whether to back off.
    let carrier = shipping.carrier.as_ref().map(|run| &run.id);
    let Some(shipping) = store.sqlite.task_shipping(id).map_err(task_error)? else {
        return Ok(None);
    };
    if shipping.carrier.as_ref().map(|run| &run.id) != carrier {
        return Ok(None);
    }
    // A run another `lf task run` carries, such as one a move continued
    // on this Machine, ships once it stops.
    if crate::store::task_work::task_graph(&store.sqlite, id)
        .map_err(task_error)?
        .is_some_and(|graph| matches!(graph.position, TaskGraphPosition::Run { running: true, .. }))
    {
        return Ok(None);
    }
    let failure = carrier_failure(&store.sqlite, id, &shipping).map_err(task_error)?;
    if let Some(due) = failure
        .as_ref()
        .map(|(failed_at, _)| restart_due(&shipping, *failed_at))
        .filter(|due| *due > now)
    {
        return Ok(Some(due));
    }
    let Some((task, worktree, agent)) = continuable(store, id, now).await? else {
        return Ok(None);
    };
    if failure.is_some()
        && !store
            .sqlite
            .count_task_run_restart(id, None, shipping.restarts)
            .map_err(task_error)?
    {
        return Ok(None);
    }
    restart_lf_in(
        &worktree,
        agent.as_deref(),
        &["--task", &task.identifier, "ship"],
    )?;
    tracing::info!("restarted the ship process for {}", task.identifier);
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{next_step, shipping_snapshot, ShipStep, ShippingState};
    use crate::durable::TaskState;
    use crate::store::ProcessIdentityEvidence;
    use crate::task_graph::{CapturedGraph, TaskGraph, TaskGraphPosition};
    use loopflow_model::id::LfProcessId;
    use loopflow_storage::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use loopflow_storage::sqlite::TaskShipping;

    fn feature_at(position: TaskGraphPosition) -> TaskGraph {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        TaskGraph {
            name: graph.name.clone(),
            interactive_steps: graph.interactive_summary(),
            position,
        }
    }

    #[test]
    fn a_died_run_is_carried_again_at_once_then_later_then_waits_for_a_person() {
        use loopflow_storage::sqlite::task_work::TaskRunRestarts;
        let died = |count, at| super::DiedRun {
            at: "implement".into(),
            carrier: LfProcessId::new(),
            restarts: TaskRunRestarts { count, at },
        };
        assert_eq!(died(0, None).next_at(), Some(0));
        assert_eq!(
            died(0, None).reason("LOO-1", 100),
            "Run died at implement; the server's next pass carries it again"
        );
        assert_eq!(died(3, Some(100)).next_at(), Some(100 + 2 * 60));
        assert_eq!(
            died(3, Some(100)).reason("LOO-1", 100),
            "Run died at implement; it is carried again in 2 min (restart 4 of 5)"
        );
        assert_eq!(died(5, Some(100)).next_at(), None);
        assert_eq!(
            died(5, Some(100)).reason("LOO-1", 100),
            "Run died 5 times at implement; `lf task run LOO-1` starts it again"
        );
    }

    #[test]
    fn a_failed_ship_restarts_later_each_time_and_says_why() {
        let mut handoff = TaskShipping {
            shipped_at: 5,
            approved_by: None,
            approving_session: None,
            carrier: None,
            restarts: 0,
        };
        let minutes = |handoff: &TaskShipping| super::restart_due(handoff, 0) / 60;
        assert_eq!(minutes(&handoff), 1);
        handoff.restarts = 3;
        assert_eq!(minutes(&handoff), 4);
        handoff.restarts = 20;
        assert_eq!(minutes(&handoff), 30);

        let failed_at = loopflow_storage::rows::now_unix();
        let failure = (failed_at, "provider usage limit.\ncaused by".into());
        let read = |failure| {
            shipping_snapshot(
                None,
                &handoff,
                (TaskState::Shipping, true),
                ProcessIdentityEvidence::Dead,
                failure,
                None,
            )
            .unwrap()
        };
        let failed = read(Some(&failure));
        assert_eq!(failed.state, ShippingState::Restarting);
        assert_eq!(
            failed.reason,
            "Ship failed: provider usage limit; retrying in 30 min"
        );
        // A process that ended without failing restarts on the next read.
        assert!(read(None).reason.starts_with("Ship process exited"));
    }

    #[tokio::test]
    async fn a_ship_that_records_failure_after_the_sweep_reads_it_waits_to_restart() {
        use crate::store::{open_ephemeral_store, StorageConfig};
        use loopflow_model::durable::TaskId;
        use loopflow_storage::graph_process::{ExecutionStart, RunOwner};
        use loopflow_storage::work::task::NewTask;
        use std::sync::Arc;

        let home = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(home.path().join("db")))
                .await
                .unwrap(),
        );
        let task = TaskId::new();
        store
            .sqlite
            .create_task(&NewTask {
                repository_id: loopflow_storage::test_fixture::repository(&store.sqlite, "/repo"),
                id: task.clone(),
                project_id: None,
                title: "Verify the release".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        let carrier = LfProcessId::new();
        let start = ExecutionStart {
            // The old carrier's PID now belongs to this test process.
            pid: Some(std::process::id()),
            os_started_at: Some(1),
            ..ExecutionStart::reserved(1)
        };
        let before_exit = store
            .sqlite
            .ship_task(&task, (&carrier, &start), "verify")
            .unwrap();
        let now = 100;
        store
            .sqlite
            .finish_run(
                RunOwner::ShipCarrier,
                &carrier,
                now,
                Some(i32::from(crate::process::GraphHeld::EXIT)),
                Some("verification blocked"),
            )
            .unwrap();

        assert_eq!(
            super::restart_ship(&store, &task, &before_exit, now)
                .await
                .unwrap(),
            Some(now + 60),
        );
    }

    #[test]
    fn ship_stands_in_at_the_waiting_step_and_restarts_a_stopped_run() {
        let at = |step: &str| {
            next_step(
                &feature_at(TaskGraphPosition::At { step: step.into() }),
                "INF-1",
            )
            .unwrap()
        };
        assert_eq!(at("start"), ShipStep::Run { from: None });
        assert_eq!(
            at("human-review"),
            ShipStep::Run {
                from: Some("human-review".into())
            }
        );
        assert_eq!(at("end"), ShipStep::Finished);
        let stopped = feature_at(TaskGraphPosition::Run {
            step: "human-review".into(),
            at: "implement".into(),
            lf_process_id: "gone".into(),
            running: false,
        });
        assert_eq!(
            next_step(&stopped, "INF-1").unwrap(),
            ShipStep::Run { from: None }
        );
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        assert_eq!(super::post_merge_step(&graph), Some("verify"));
    }

    #[test]
    fn shipping_reads_what_happens_now_and_never_waits_on_a_person() {
        let repo = tempfile::tempdir().unwrap();
        let graph = feature_at(TaskGraphPosition::At {
            step: "human-review".into(),
        });
        let handoff = TaskShipping {
            shipped_at: 5,
            approved_by: None,
            approving_session: None,
            carrier: None,
            restarts: 0,
        };
        let read = |status, alive, landing: Option<&PrLanding>| {
            shipping_snapshot(Some(&graph), &handoff, (status, true), alive, None, landing).unwrap()
        };
        let building = read(TaskState::Shipping, ProcessIdentityEvidence::Live, None);
        assert_eq!(building.shipped_at, 5);
        assert_eq!(building.state, ShippingState::Building);
        assert_eq!(building.reason, "human-review (stand-in)");
        // Unknown liveness is not a dead process.
        assert_eq!(
            read(TaskState::Shipping, ProcessIdentityEvidence::Unknown, None).state,
            ShippingState::Building
        );
        assert_eq!(
            read(TaskState::Shipping, ProcessIdentityEvidence::Dead, None).state,
            ShippingState::Restarting
        );
        assert_eq!(
            shipping_snapshot(
                Some(&graph),
                &handoff,
                (TaskState::Shipping, false),
                ProcessIdentityEvidence::Dead,
                None,
                None
            )
            .unwrap()
            .state,
            ShippingState::Unassigned
        );

        // The handoff remains stoppable between runs and after unassignment.
        assert_eq!(
            read(TaskState::Idle, ProcessIdentityEvidence::Dead, None).state,
            ShippingState::Restarting
        );
        assert_eq!(
            shipping_snapshot(
                Some(&graph),
                &handoff,
                (TaskState::Active, false),
                ProcessIdentityEvidence::Dead,
                None,
                None
            )
            .unwrap()
            .state,
            ShippingState::Unassigned
        );

        let mut landing = PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".into(),
                pr_number: 1650,
                worktree: repo.path().to_path_buf(),
                branch: "jack/x".into(),
                task_id: None,
                requested_head_sha: "head".into(),
            },
            time::OffsetDateTime::now_utc(),
        )
        .unwrap();
        let landing_now = read(
            TaskState::Shipping,
            ProcessIdentityEvidence::Live,
            Some(&landing),
        );
        assert_eq!(landing_now.state, ShippingState::Landing);
        assert_eq!(landing_now.reason, "CI running on #1650");
        landing.state = PrLandingState::Blocked;
        landing.blocked_reason = Some("rust-lint".into());
        let repairing = read(
            TaskState::Shipping,
            ProcessIdentityEvidence::Live,
            Some(&landing),
        );
        assert_eq!(repairing.state, ShippingState::Repairing);
        assert_eq!(repairing.reason, "Repairing #1650 · rust-lint");

        // Done is not shipping.
        assert!(shipping_snapshot(
            Some(&graph),
            &handoff,
            (TaskState::Done, true),
            ProcessIdentityEvidence::Live,
            None,
            None
        )
        .is_none());
    }
}
