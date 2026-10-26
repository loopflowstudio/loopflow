//! `lf ship`: someone approves a Task once, and its remaining graph runs to
//! `end` with no one at hand. The approval is stored; the Task reads shipping
//! while this Machine's ship process carries it there, and idle when that
//! process exited. At each stage a headless stand-in runs the stage's skill and
//! decides the route. Defer holds it, Stop withdraws the approval, and a ship
//! process that died is started again.

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::id::LfProcessId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::error::OpsResult;
use crate::ops::pr::PrReadFreshness;
use crate::ops::workflow::{Workflow, WorkflowPosition};
use crate::pr_landing::{PrLanding, PrLandingState};
use crate::store::sqlite::TaskShipping;
use crate::work::task::{PrPhase, Task};
use crate::workflow::{END, START};

use super::{block_on_task, task_error, task_store, SharedStore, TaskRunSelection};

/// What `lf ship` found before running anything.
#[derive(Debug)]
pub enum ShipStart {
    /// This process now ships the Task.
    Begun(Box<Task>),
    /// A live process already ships it.
    AlreadyShipping {
        issue: String,
        process: LfProcessId,
    },
    Deferred {
        issue: String,
    },
    /// The Task already stands at `end`.
    AtEnd {
        issue: String,
    },
}

/// The next thing the ship process runs.
#[derive(Debug, PartialEq, Eq)]
pub enum ShipStep {
    /// The Task stands at a stage where a person takes part: run its skill
    /// headless, standing in for them. The stand-in's decision chooses
    /// `advance`, or `repeat` when the stage has a loop behind it.
    StandIn {
        stage: String,
        skill: String,
        advance: String,
        repeat: Option<String>,
    },
    /// Take this edge, or run it again when it stopped.
    Edge(String),
    Deferred,
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
    Deferred,
}

async fn task_by_issue(store: &SharedStore, issue: &str) -> OpsResult<Task> {
    store
        .get_task_by_issue(issue)
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))
}

/// The configured participant's name, or `fallback` when none is set.
fn participant_or(fallback: &str) -> String {
    crate::config::participant_name()
        .ok()
        .flatten()
        .unwrap_or_else(|| fallback.into())
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

/// The Task's shipping, when it is shipping. `alive` is the ship process's
/// liveness and `failure` when it failed and why.
pub(crate) fn shipping_snapshot(
    workflow: Option<&Workflow>,
    shipping: &TaskShipping,
    status: TaskState,
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
    if status == TaskState::Deferred {
        return shipping(
            ShippingState::Deferred,
            "Deferred; resume it to keep shipping".into(),
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
    let reason = match workflow {
        None => "taking up its graph".to_string(),
        Some(workflow) => match &workflow.position {
            WorkflowPosition::Edge { edge, .. } => {
                let edge = &workflow.definition.edges[*edge as usize];
                format!("{} → {}", edge.name, edge.to)
            }
            WorkflowPosition::Node { node } if workflow.definition.node(node).is_some() => {
                format!("{node} (stand-in)")
            }
            WorkflowPosition::Node { node } => format!("from {node}"),
        },
    };
    shipping(ShippingState::Building, reason)
}

/// Read a Task's shipping from the store: its handoff, its ship process and its landing.
pub(crate) fn read_shipping(
    sqlite: &crate::store::sqlite::SqliteStore,
    task: &crate::work::task::TaskId,
    workflow: Option<&Workflow>,
    status: TaskState,
) -> crate::store::StoreResult<Option<ShippingSnapshot>> {
    let Some(shipping) = sqlite.task_shipping(task)? else {
        return Ok(None);
    };
    let alive = carrier_evidence(sqlite, &shipping);
    let failure = carrier_failure(sqlite, &shipping)?;
    let landing = sqlite.pending_task_landing(task)?;
    Ok(shipping_snapshot(
        workflow,
        &shipping,
        status,
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
            TaskState::Deferred => return Ok(ShipStart::Deferred { issue }),
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
        let current = store.sqlite.workflow(&task.id).map_err(task_error)?;
        let workflow = match (current, graph) {
            (Some(workflow), None) => workflow,
            (current, graph) => {
                let TaskRunSelection { take_up, .. } =
                    super::select_task_run(&store, &task, task.worktree()?, graph)?;
                match take_up {
                    Some(graph) => {
                        store
                            .sqlite
                            .take_up_workflow(&task.id, &graph, &process, None)
                            .map_err(task_error)?;
                        store
                            .sqlite
                            .workflow(&task.id)
                            .map_err(task_error)?
                            .expect("the graph was just taken up")
                    }
                    // The graph named is the one the Task is already on.
                    None => current.expect("a Task that takes nothing up keeps its graph"),
                }
            }
        };
        if matches!(&workflow.position, WorkflowPosition::Node { node } if node == END) {
            return Ok(ShipStart::AtEnd { issue });
        }
        let by = participant_or("someone");
        let mut handoff = format!("{by} shipped it");
        if let Some(graph) = graph {
            handoff.push_str(&format!(" with {graph}"));
        }
        if let Some(note) = note {
            handoff.push_str(&format!(": {note}"));
        }
        store
            .sqlite
            .ship_task(&task.id, &process, &handoff)
            .map_err(task_error)?;
        Ok(ShipStart::Begun(Box::new(task)))
    })
}

/// Withdraw a Task's Auto-Ship approval: stop its ship process and take back any
/// merge request Loopflow armed on GitHub. Returns false when it was not shipping.
pub fn ship_stop(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let (task, stopped) = block_on_task(async {
        let store = task_store().await?;
        let task = task_by_issue(&store, issue).await?;
        let by = participant_or("someone");
        let note = match note {
            Some(note) => format!("{by}: {note}"),
            None => by,
        };
        let stopped = store
            .sqlite
            .stop_shipping(&task.id, &note)
            .map_err(task_error)?;
        if let Some(process) = stopped
            .as_ref()
            .and_then(|shipping| shipping.process.as_ref())
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
    let task = super::task_create(repo, wave, Some(title), None)?;
    super::prepare_task(
        repo,
        &task.identifier,
        super::TaskProcessOptions {
            wave: wave.map(str::to_owned),
            branch: Some(branch),
            ..Default::default()
        },
        None,
    )
}

/// What a merged PR did to a shipping Task.
#[derive(Debug, PartialEq, Eq)]
pub enum ShipMerge {
    /// No merge to act on.
    Open,
    /// Its PR merged; the Task now stands at the stage that checks merged work.
    Verifying(String),
    /// Its PR merged and the Task is done.
    Done,
}

/// The stage that checks merged work: the one whose route into `end` runs nothing.
fn post_merge_stage(workflow: &Workflow) -> Option<&str> {
    workflow
        .definition
        .edges
        .iter()
        .zip(&workflow.routes)
        .find(|(edge, route)| edge.to == END && route.start.is_none())
        .map(|(edge, _)| edge.from.as_str())
        .filter(|stage| workflow.definition.node(stage).is_some())
}

/// Whether the Task has stood at `stage` on its current graph. A merge sends
/// it to the post-merge stage only once, so a fix forward is not pulled back.
fn reached(workflow: &Workflow, stage: &str) -> bool {
    workflow
        .history
        .iter()
        .any(|step| step.workflow == workflow.definition.name && step.to == stage)
}

/// Act on a shipping Task's merged PR, whoever merged it, so no further
/// build step runs on delivered work: send it to the stage that checks merged
/// work, or complete it when its graph has none.
pub fn ship_settle_merge(issue: &str) -> OpsResult<ShipMerge> {
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| task_error("lf ship requires a registered Process"))?;
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
        let workflow = store.sqlite.workflow(&task.id).map_err(task_error)?;
        if let Some(workflow) = &workflow {
            if let Some(stage) = post_merge_stage(workflow) {
                if reached(workflow, stage) {
                    return Ok(ShipMerge::Open);
                }
                store
                    .sqlite
                    .set_workflow_node(
                        &task.id,
                        stage,
                        &process,
                        Some("its PR merged while shipping"),
                    )
                    .map_err(task_error)?;
                return Ok(ShipMerge::Verifying(stage.to_owned()));
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
        match store.sqlite.task_state(&task.id).map_err(task_error)? {
            TaskState::Deferred => return Ok(ShipStep::Deferred),
            TaskState::Done | TaskState::Abandoned => return Ok(ShipStep::Finished),
            _ => {}
        }
        if store
            .sqlite
            .task_shipping(&task.id)
            .map_err(task_error)?
            .is_none()
        {
            return Ok(ShipStep::Stopped);
        }
        let workflow = store
            .sqlite
            .workflow(&task.id)
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("Task {issue} has no Workflow to ship")))?;
        next_step(&workflow, &task.identifier)
    })
}

fn next_step(workflow: &Workflow, issue: &str) -> OpsResult<ShipStep> {
    let definition = &workflow.definition;
    match &workflow.position {
        WorkflowPosition::Node { node } if node == END => Ok(ShipStep::Finished),
        WorkflowPosition::Edge {
            edge,
            running: true,
            ..
        } => Err(task_error(format!(
            "Task {issue} is on {}, carried by another run; read `lf task status {issue}`",
            definition.edges[*edge as usize].name
        ))),
        WorkflowPosition::Edge { edge, .. } => Ok(ShipStep::Edge(
            definition.edges[*edge as usize].name.clone(),
        )),
        WorkflowPosition::Node { node } => {
            let (advance, repeat) = routes_from(workflow, node).ok_or_else(|| {
                task_error(format!(
                    "Task {issue} is at {node} of graph {}, and no route leaves it for another stage",
                    definition.name
                ))
            })?;
            Ok(match definition.node(node) {
                Some(stage) if node != START => ShipStep::StandIn {
                    stage: node.clone(),
                    skill: stage.skill.clone(),
                    advance,
                    repeat,
                },
                _ => ShipStep::Edge(advance),
            })
        }
    }
}

/// The edges a stand-in chooses between at `node`: the one that advances
/// (toward `end` when one goes there, else the one to a later stage) and the
/// one that repeats, back to this stage or an earlier one.
fn routes_from(workflow: &Workflow, node: &str) -> Option<(String, Option<String>)> {
    let definition = &workflow.definition;
    // `start` comes before every stage and `end` after.
    let order = |stage: &str| match stage {
        START => 0,
        _ => definition
            .nodes
            .iter()
            .position(|node| node.name == stage)
            .map_or(usize::MAX, |index| index + 1),
    };
    let (repeat, onward): (Vec<_>, Vec<_>) = definition
        .outgoing(node)
        .map(|(_, edge)| edge)
        .partition(|edge| order(&edge.to) <= order(node));
    let advance = onward
        .iter()
        .find(|edge| edge.to == END)
        .or(onward.first())?;
    Some((
        advance.name.clone(),
        repeat.first().map(|edge| edge.name.clone()),
    ))
}

/// How long a stand-in waits before repeating its stage after `consecutive`
/// blocked decisions in a row: the landing's repair backoff, 1 minute up to 30.
pub fn blocked_backoff(consecutive: u32) -> std::time::Duration {
    let seconds = crate::ops::pr_landing::repair_backoff(consecutive.saturating_add(1));
    std::time::Duration::from_secs(u64::try_from(seconds).expect("backoff is positive"))
}

/// The instruction a headless stand-in receives at a stage. With a loop behind
/// the stage it answers advance or iterate; blocked repeats with the blocker.
pub fn stand_in_message(stage: &str, repeats: bool) -> String {
    let person = participant_or("the person who shipped this Task");
    let mut message = format!(
        "No one is available. You are standing in for {person} at {stage}. Decide as they would, record why in your final message, and continue."
    );
    if repeats {
        message.push_str(" Advance when the work before this stage holds; iterate only for a concrete, repairable gap, naming it; there is no one to hand a blocker to.");
    }
    message
}

/// A ship step completed, so a later failure restarts after the shortest wait.
pub fn ship_progressed(task: &crate::work::task::TaskId) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        store.sqlite.reset_ship_restarts(task).map_err(task_error)
    })
}

/// Start `lf --task ISSUE ship` again for each shipping Task checked out on
/// this Machine whose ship process has exited. Deferred and finished Tasks wait.
/// After a failure the restart waits, longer each time in a row; a process
/// that ended without failing (killed, interrupted) restarts at once.
pub(crate) fn restart_ships() -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let now = crate::store::rows::now_unix();
        for (id, shipping) in store.sqlite.local_shipping_tasks().map_err(task_error)? {
            let state = store.sqlite.task_state(&id).map_err(task_error)?;
            if state.is_terminal()
                || state == TaskState::Deferred
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
    use crate::ops::workflow::{Workflow, WorkflowPosition};
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::store::sqlite::TaskShipping;
    use crate::workflow::CapturedGraph;

    #[test]
    fn a_repeatedly_blocked_stand_in_waits_longer_up_to_half_an_hour() {
        let minutes = |n| super::blocked_backoff(n).as_secs() / 60;
        assert_eq!([1, 2, 3, 6, 20].map(minutes), [1, 2, 4, 30, 30]);
    }

    #[test]
    fn a_failed_ship_restarts_later_each_time_and_says_why() {
        let mut handoff = TaskShipping {
            shipped_at: 5,
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
                TaskState::Shipping,
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
    fn a_stand_in_runs_each_stage_and_chooses_between_its_routes() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let at = |node: &str| {
            let workflow = Workflow::new(
                definition.clone(),
                routes.clone(),
                WorkflowPosition::Node { node: node.into() },
                Vec::new(),
            );
            next_step(&workflow, "INF-1").unwrap()
        };
        assert_eq!(at("start"), ShipStep::Edge("kickoff".into()));
        assert_eq!(
            at("explore"),
            ShipStep::StandIn {
                stage: "explore".into(),
                skill: "explore".into(),
                advance: "pursue".into(),
                repeat: Some("kickoff".into()),
            }
        );
        // From demo the onward edge is the one into `end`, not the round back.
        assert_eq!(
            at("demo"),
            ShipStep::StandIn {
                stage: "demo".into(),
                skill: "demo".into(),
                advance: "deliver".into(),
                repeat: Some("pursue".into()),
            }
        );
        // After merge, verify finishes into `end` or fixes forward through implement.
        assert_eq!(
            at("verify"),
            ShipStep::StandIn {
                stage: "verify".into(),
                skill: "verify".into(),
                advance: "end".into(),
                repeat: Some("pursue".into()),
            }
        );
        let workflow = Workflow::new(
            definition.clone(),
            routes.clone(),
            WorkflowPosition::Node {
                node: "demo".into(),
            },
            Vec::new(),
        );
        assert_eq!(super::post_merge_stage(&workflow), Some("verify"));
        assert!(!super::reached(&workflow, "verify"));
        assert_eq!(at("end"), ShipStep::Finished);
        // A stopped edge runs again.
        let stopped = Workflow::new(
            definition.clone(),
            routes.clone(),
            WorkflowPosition::Edge {
                edge: 4,
                lf_process_id: "gone".into(),
                running: false,
            },
            Vec::new(),
        );
        assert_eq!(
            next_step(&stopped, "INF-1").unwrap(),
            ShipStep::Edge("deliver".into())
        );
    }

    #[test]
    fn shipping_reads_what_happens_now_and_never_waits_on_a_person() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let workflow = Workflow::new(
            definition,
            routes,
            WorkflowPosition::Node {
                node: "demo".into(),
            },
            Vec::new(),
        );
        let handoff = TaskShipping {
            shipped_at: 5,
            process: Some(LfProcessId::new()),
            restarts: 0,
        };
        let read = |status, alive, landing: Option<&PrLanding>| {
            shipping_snapshot(Some(&workflow), &handoff, status, alive, None, landing).unwrap()
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
            read(TaskState::Deferred, ProcessIdentityEvidence::Dead, None).state,
            ShippingState::Deferred
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
            Some(&workflow),
            &handoff,
            TaskState::Done,
            ProcessIdentityEvidence::Live,
            None,
            None
        )
        .is_none());
    }
}
