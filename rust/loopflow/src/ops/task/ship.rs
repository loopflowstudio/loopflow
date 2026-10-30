//! `lf ship`: someone approves a Task once, and its remaining graph runs to
//! `end` with no one at hand, as `lf -b task run` runs it: interactive steps
//! headless, standing in for the person. The approval is stored; the Task reads
//! shipping while this Machine's ship process carries it there, and idle when
//! that process exited. Unassigning the Task stops the ship, Stop withdraws the
//! approval until the next `lf ship` (no run into `end` approves it meanwhile),
//! and a ship process that died is started again.

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::id::LfProcessId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::error::OpsResult;
use crate::ops::pr::PrReadFreshness;
use crate::pr_landing::{PrLanding, PrLandingState};
use crate::store::sqlite::TaskShipping;
use crate::task_graph::{TaskGraph, TaskGraphPosition, END};
use crate::work::task::{PrPhase, Task};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSnapshot {
    /// When the Task was handed off.
    pub shipped_at: i64,
    pub state: ShippingState,
    /// One line saying what is happening.
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Who started `process`, read from its ancestry; no process leaves
/// attribution unresolved.
pub(crate) fn starter(
    sqlite: &crate::store::sqlite::SqliteStore,
    process: Option<&LfProcessId>,
) -> crate::store::StoreResult<crate::process::Starter> {
    let Some(process) = process else {
        return Ok(crate::process::Starter::Person { name: None });
    };
    let person = crate::config::participant_name().ok().flatten();
    sqlite.process_starter(process, person.as_deref())
}

/// Whether the process carrying a shipping Task has exited. Unknown evidence
/// is never read as a dead process; no carrier at all is.
fn carrier_evidence(
    sqlite: &crate::store::sqlite::SqliteStore,
    shipping: &TaskShipping,
) -> ProcessIdentityEvidence {
    match &shipping.process {
        Some(process) => crate::journal::process_evidence(sqlite, process),
        None => ProcessIdentityEvidence::Dead,
    }
}

/// When a ship process that failed at `failed_at` starts again: the landing's
/// repair backoff, 1 minute up to 30, counted over restarts in a row.
fn restart_due(shipping: &TaskShipping, failed_at: i64) -> i64 {
    failed_at + crate::ops::pr_landing::repair_backoff(shipping.restarts.saturating_add(1))
}

/// When the ship process failed, and the error that caused it.
fn carrier_failure(
    sqlite: &crate::store::sqlite::SqliteStore,
    shipping: &TaskShipping,
) -> crate::store::StoreResult<Option<(i64, String)>> {
    match &shipping.process {
        Some(process) => sqlite.failure_cause(process),
        None => Ok(None),
    }
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
                "Ship process exited; it restarts on the next read of this Machine's Tasks".into(),
            );
        };
        let error = error
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .trim_end_matches('.');
        let wait = due - crate::store::rows::now_unix();
        let when = match wait {
            ..=0 => "retrying on the next read of this Machine's Tasks".to_string(),
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
            (TaskGraphPosition::Run { step, .. }, _) => format!("running from {step}"),
            (_, Some(step)) => format!("{} (stand-in)", step.name),
            (TaskGraphPosition::At { step }, None) => format!("from {step}"),
        },
    };
    shipping(ShippingState::Building, reason)
}

/// Read a Task's shipping from the store: its handoff, its ship process and its landing.
pub(crate) fn read_shipping(
    sqlite: &crate::store::sqlite::SqliteStore,
    task: &crate::work::task::TaskId,
    graph: Option<&TaskGraph>,
    status: TaskState,
) -> crate::store::StoreResult<Option<ShippingSnapshot>> {
    let Some(shipping) = sqlite.task_shipping(task)? else {
        return Ok(None);
    };
    let alive = carrier_evidence(sqlite, &shipping);
    let failure = carrier_failure(sqlite, &shipping)?;
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
            .and_then(|shipping| shipping.process)
        {
            if carrier != process
                && crate::journal::process_evidence(&store.sqlite, &carrier)
                    != ProcessIdentityEvidence::Dead
            {
                return Ok(ShipStart::AlreadyShipping {
                    issue,
                    process: carrier,
                });
            }
        }
        let current = store.sqlite.task_graph(&task.id).map_err(task_error)?;
        if current.is_none() || graph.is_some() {
            let TaskRunSelection {
                graph: selected,
                take_up,
                ..
            } = super::select_task_run(&store, &task, task.worktree()?, graph, Some("start"))?;
            if take_up {
                store
                    .sqlite
                    .take_up_task_graph(&task.id, &selected)
                    .map_err(task_error)?;
            }
        } else if current.as_ref().is_some_and(
            |graph| matches!(&graph.position, TaskGraphPosition::At { step } if step == END),
        ) {
            return Ok(ShipStart::AtEnd { issue });
        }
        let by = starter(&store.sqlite, Some(&process)).map_err(task_error)?;
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
            store.sqlite.claim_task(&task.id).map_err(task_error)?;
        }
        store
            .sqlite
            .ship_task(&task.id, &process, &handoff)
            .map_err(task_error)?;
        Ok(ShipStart::Begun(Box::new(task)))
    })
}

/// Withdraw a Task's Auto-Ship approval: stop its ship process and take back any
/// merge request Loopflow armed on GitHub. The stop stands until `lf ship`: a
/// run reaching `end` no longer approves it. Returns false when it was not shipping.
pub fn ship_stop(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| task_error("lf ship --stop requires a registered Process"))?;
    withdraw(issue, Withdrawal::Stop { process, note })
}

/// Withdraw the approval from inside the ship process when every attempt of its
/// run failed with one error: restarting it would only repeat the failure. This
/// is no person's stop, so a relaunch that reaches `end` may approve it again.
pub fn ship_halt(issue: &str, cause: &str) -> OpsResult<bool> {
    withdraw(issue, Withdrawal::Halt { cause })
}

enum Withdrawal<'a> {
    Stop {
        process: LfProcessId,
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
                let by = starter(&store.sqlite, Some(process)).map_err(task_error)?;
                let note = match note {
                    Some(note) => format!("{by}: {note}"),
                    None => by.to_string(),
                };
                (note, Some(process))
            }
            Withdrawal::Halt { cause } => (format!("Loopflow: {cause}"), None),
        };
        let stopped = store
            .sqlite
            .stop_shipping(&task.id, &note, stopped_by)
            .map_err(task_error)?;
        if let Some(process) = stopped
            .as_ref()
            .and_then(|shipping| shipping.process.as_ref())
            .filter(|process| current.as_ref() != Some(*process))
        {
            if crate::journal::process_evidence(&store.sqlite, process)
                == ProcessIdentityEvidence::Live
            {
                if let Some(pid) = store
                    .sqlite
                    .process(process)
                    .ok()
                    .flatten()
                    .and_then(|record| record.pid)
                {
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
    let task = super::task_create(repo, wave, Some(title), None, None)?;
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
        let row = store.sqlite.task_graph_row(&task.id).map_err(task_error)?;
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
        let graph = store
            .sqlite
            .task_graph(&task.id)
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
pub fn ship_progressed(task: &crate::work::task::TaskId) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        store.sqlite.reset_ship_restarts(task).map_err(task_error)
    })
}

/// Start `lf --task ISSUE ship` again for each shipping Task checked out on
/// this Machine whose ship process has exited. Unassigned and finished Tasks wait.
/// After a failure the restart waits, longer each time in a row; a process
/// that ended without failing (killed, interrupted) restarts at once.
pub(crate) fn restart_ships() -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let now = crate::store::rows::now_unix();
        for (id, shipping) in store.sqlite.local_shipping_tasks().map_err(task_error)? {
            let state = store.sqlite.task_state(&id).map_err(task_error)?;
            if state.is_terminal()
                || !store.sqlite.task_owned(&id).map_err(task_error)?
                || carrier_evidence(&store.sqlite, &shipping) != ProcessIdentityEvidence::Dead
            {
                continue;
            }
            let failure = carrier_failure(&store.sqlite, &shipping).map_err(task_error)?;
            if let Some((failed_at, _)) = &failure {
                if restart_due(&shipping, *failed_at) > now {
                    continue;
                }
            }
            let Some(task) = store.get_task(&id).await.map_err(task_error)? else {
                continue;
            };
            let Some(worktree) = task.worktree.as_ref().filter(|path| path.is_dir()) else {
                continue;
            };
            let context = crate::os_process::execution_context().map_err(task_error)?;
            let argv = [
                context.lf_bin.to_string_lossy().to_string(),
                "--task".into(),
                task.identifier.clone(),
                "ship".into(),
            ];
            if failure.is_some() {
                store.sqlite.count_ship_restart(&id).map_err(task_error)?;
            }
            crate::os_process::start_lf_session(worktree, &argv, &[], &|_| {})
                .map_err(task_error)?;
            tracing::info!("restarted the ship process for {}", task.identifier);
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{next_step, shipping_snapshot, ShipStep, ShippingState};
    use crate::durable::TaskState;
    use crate::id::LfProcessId;
    use crate::journal::ProcessIdentityEvidence;
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::sqlite::TaskShipping;
    use crate::task_graph::{CapturedGraph, TaskGraph, TaskGraphPosition};

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
    fn a_failed_ship_restarts_later_each_time_and_says_why() {
        let mut handoff = TaskShipping {
            shipped_at: 5,
            approved_by: None,
            process: Some(LfProcessId::new()),
            restarts: 0,
        };
        let minutes = |handoff: &TaskShipping| super::restart_due(handoff, 0) / 60;
        assert_eq!(minutes(&handoff), 1);
        handoff.restarts = 3;
        assert_eq!(minutes(&handoff), 4);
        handoff.restarts = 20;
        assert_eq!(minutes(&handoff), 30);

        let failed_at = crate::store::rows::now_unix();
        let failure = (
            failed_at,
            "no eligible managed claude account.\ncaused by".into(),
        );
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
            "Ship failed: no eligible managed claude account; retrying in 30 min"
        );
        // A process that ended without failing restarts on the next read.
        assert!(read(None).reason.starts_with("Ship process exited"));
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
            at("demo"),
            ShipStep::Run {
                from: Some("demo".into())
            }
        );
        assert_eq!(at("end"), ShipStep::Finished);
        let stopped = feature_at(TaskGraphPosition::Run {
            step: "demo".into(),
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
            step: "demo".into(),
        });
        let handoff = TaskShipping {
            shipped_at: 5,
            approved_by: None,
            process: Some(LfProcessId::new()),
            restarts: 0,
        };
        let read = |status, alive, landing: Option<&PrLanding>| {
            shipping_snapshot(Some(&graph), &handoff, (status, true), alive, None, landing).unwrap()
        };
        let building = read(TaskState::Shipping, ProcessIdentityEvidence::Live, None);
        assert_eq!(building.shipped_at, 5);
        assert_eq!(building.state, ShippingState::Building);
        assert_eq!(building.reason, "demo (stand-in)");
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
