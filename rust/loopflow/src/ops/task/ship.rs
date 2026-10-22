//! `lf ship`: someone approves a Task once, and its remaining graph runs to
//! `end` with no one at hand. The approval is a `Shipped` move; whether a Task
//! is shipping, and by which process, is read from its moves and that
//! process's liveness, never stored. At each stage a headless stand-in runs
//! the stage's skill and decides the route. Pause holds it; reconcile restarts
//! a ship process that died.

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::id::LfProcessId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::error::OpsResult;
use crate::ops::workflow::{Workflow, WorkflowMove, WorkflowMoveKind, WorkflowPosition};
use crate::pr_landing::{PrLanding, PrLandingState};
use crate::work::task::Task;
use crate::workflow::{END, START};

use super::{block_on_task, task_error, task_store, TaskRunSelection};

/// The note a reconcile restart leaves on its `Shipped` move.
const RESTART_NOTE: &str = "ship process exited; restarted";

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
    Paused {
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
    Paused,
    /// The Task reached `end`, completed, or was abandoned.
    Finished,
}

/// The process shipping the Task: the one behind its latest `Shipped` move,
/// unless a person has since set its position or it has reached `end`.
pub(crate) fn shipping_process(workflow: &Workflow) -> Option<&LfProcessId> {
    if matches!(&workflow.position, WorkflowPosition::Node { node } if node == END) {
        return None;
    }
    shipped_moves(workflow).last()?.lf_process_id.as_ref()
}

/// What a shipped Task is doing, derived from its moves, its ship process,
/// its state and its landing; never stored. Nothing here waits on a person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShippingSnapshot {
    /// When the current shipping began: its first `Shipped` move.
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
    Paused,
}

/// The `Shipped` moves of the current shipping, oldest first: those since a
/// person last set the Task's position.
fn shipped_moves(workflow: &Workflow) -> Vec<&WorkflowMove> {
    let mut moves: Vec<_> = workflow
        .history
        .iter()
        .rev()
        .take_while(|entry| entry.kind != WorkflowMoveKind::Set)
        .filter(|entry| entry.kind == WorkflowMoveKind::Shipped)
        .collect();
    moves.reverse();
    moves
}

/// The Task's shipping, when it is shipping. `alive` is the ship process's
/// liveness; unknown evidence is never read as a dead process.
pub(crate) fn shipping_snapshot(
    workflow: &Workflow,
    status: TaskState,
    alive: ProcessIdentityEvidence,
    landing: Option<&PrLanding>,
) -> Option<ShippingSnapshot> {
    shipping_process(workflow)?;
    if matches!(status, TaskState::Done | TaskState::Abandoned) {
        return None;
    }
    let moves = shipped_moves(workflow);
    let shipped_at = moves.first()?.at;
    let restarts = moves
        .iter()
        .filter(|entry| entry.note.as_deref() == Some(RESTART_NOTE))
        .count();
    let shipping = |state, reason: String| {
        Some(ShippingSnapshot {
            shipped_at,
            state,
            reason,
        })
    };
    if status == TaskState::Paused {
        return shipping(
            ShippingState::Paused,
            "Paused; resume it to keep shipping".into(),
        );
    }
    if alive == ProcessIdentityEvidence::Dead {
        let times = if restarts > 0 {
            format!(" (restarted ×{restarts})")
        } else {
            String::new()
        };
        return shipping(
            ShippingState::Restarting,
            format!("Ship process exited; restarts at the next check{times}"),
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
    let definition = &workflow.definition;
    let reason = match &workflow.position {
        WorkflowPosition::Edge { edge, .. } => {
            let edge = &definition.edges[*edge as usize];
            format!("{} → {}", edge.name, edge.to)
        }
        WorkflowPosition::Node { node } if definition.node(node).is_some() => {
            format!("{node} (stand-in)")
        }
        WorkflowPosition::Node { node } => format!("from {node}"),
    };
    shipping(ShippingState::Building, reason)
}

/// Read a Task's shipping from the store: its moves, its ship process and its landing.
pub(crate) fn read_shipping(
    sqlite: &crate::store::sqlite::SqliteStore,
    task: &crate::work::task::TaskId,
    workflow: Option<&Workflow>,
    status: TaskState,
) -> crate::store::StoreResult<Option<ShippingSnapshot>> {
    let Some(workflow) = workflow else {
        return Ok(None);
    };
    let Some(process) = shipping_process(workflow) else {
        return Ok(None);
    };
    let alive = crate::journal::process_evidence(sqlite, process);
    let landing = sqlite.pending_task_landing(task)?;
    Ok(shipping_snapshot(workflow, status, alive, landing.as_ref()))
}

/// Approve the Task and claim it for this process, unless a live process
/// already ships it. A Task with no graph takes up its default first; naming
/// `graph` takes that one up in place of the remaining steps.
pub fn ship_begin(issue: &str, graph: Option<&str>, note: Option<&str>) -> OpsResult<ShipStart> {
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| task_error("lf ship requires a registered Process"))?;
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let issue = task.identifier.clone();
        match store.sqlite.task_state(&task.id).map_err(task_error)? {
            TaskState::Paused => return Ok(ShipStart::Paused { issue }),
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
        let shipped_note = match (graph, note) {
            (Some(graph), Some(note)) => Some(format!("shipped with {graph}: {note}")),
            (Some(graph), None) => Some(format!("shipped with {graph}")),
            (None, note) => note.map(str::to_owned),
        };
        let note = shipped_note.as_deref();
        if matches!(&workflow.position, WorkflowPosition::Node { node } if node == END) {
            return Ok(ShipStart::AtEnd { issue });
        }
        if let Some(shipping) = shipping_process(&workflow) {
            if shipping != &process
                && crate::journal::process_evidence(&store.sqlite, shipping)
                    != ProcessIdentityEvidence::Dead
            {
                return Ok(ShipStart::AlreadyShipping {
                    issue,
                    process: shipping.clone(),
                });
            }
        }
        store
            .sqlite
            .ship_workflow(&task.id, &process, note)
            .map_err(task_error)?;
        Ok(ShipStart::Begun(Box::new(task)))
    })
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

/// Read where the shipped Task stands and what runs next.
pub fn ship_next(issue: &str) -> OpsResult<ShipStep> {
    block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        match store.sqlite.task_state(&task.id).map_err(task_error)? {
            TaskState::Paused => return Ok(ShipStep::Paused),
            TaskState::Done | TaskState::Abandoned => return Ok(ShipStep::Finished),
            _ => {}
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
                    "Task {issue} is at {node} of graph {}, and no edge leaves it for another stage",
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
/// (toward `end` when one goes there, else the one edge for another stage)
/// and the one that repeats the stage's loop.
fn routes_from(workflow: &Workflow, node: &str) -> Option<(String, Option<String>)> {
    let (repeat, onward): (Vec<_>, Vec<_>) = workflow
        .definition
        .outgoing(node)
        .map(|(_, edge)| edge)
        .partition(|edge| edge.to == node);
    let advance = onward
        .iter()
        .find(|edge| edge.to == END)
        .or(onward.first())?;
    Some((
        advance.name.clone(),
        repeat.first().map(|edge| edge.name.clone()),
    ))
}

/// The instruction a headless stand-in receives at a stage. With a loop behind
/// the stage it answers advance or iterate; blocked repeats with the blocker.
pub fn stand_in_message(stage: &str, repeats: bool) -> String {
    let person = crate::config::participant_name()
        .ok()
        .flatten()
        .unwrap_or_else(|| "the person who shipped this Task".into());
    let mut message = format!(
        "No one is available. You are standing in for {person} at {stage}. Decide as they would, record why in your final message, and continue."
    );
    if repeats {
        message.push_str(" Advance when the work before this stage holds; iterate only for a concrete, repairable gap, naming it; there is no one to hand a blocker to.");
    }
    message
}

/// Start `lf --task ISSUE ship` again for each of the repository's shipped
/// Tasks whose ship process has exited. Paused and finished Tasks wait.
pub fn restart_ships(repo: &std::path::Path) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let repo_id = crate::repository::RepoId::discover(repo).map_err(task_error)?;
        for task in crate::ops::task_automation::repository_tasks(&store, &repo_id).await? {
            if !matches!(
                store.sqlite.task_state(&task.id).map_err(task_error)?,
                TaskState::Ready | TaskState::Active
            ) {
                continue;
            }
            let Some(workflow) = store.sqlite.workflow(&task.id).map_err(task_error)? else {
                continue;
            };
            let Some(process) = shipping_process(&workflow) else {
                continue;
            };
            if crate::journal::process_evidence(&store.sqlite, process)
                != ProcessIdentityEvidence::Dead
            {
                continue;
            }
            let Ok(worktree) = task.worktree() else {
                continue;
            };
            if !worktree.is_dir() {
                continue;
            }
            let context = crate::os_process::execution_context().map_err(task_error)?;
            let argv = [
                context.lf_bin.to_string_lossy().to_string(),
                "--task".into(),
                task.identifier.clone(),
                "ship".into(),
                "--reason".into(),
                RESTART_NOTE.into(),
            ];
            crate::os_process::start_lf_session(worktree, &argv, &[], &|_| {})
                .map_err(task_error)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::{
        next_step, shipping_process, shipping_snapshot, ShipStep, ShippingState, RESTART_NOTE,
    };
    use crate::durable::TaskState;
    use crate::id::LfProcessId;
    use crate::journal::ProcessIdentityEvidence;
    use crate::ops::workflow::{
        Workflow, WorkflowActor, WorkflowMove, WorkflowMoveKind, WorkflowPosition,
    };
    use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
    use crate::workflow::CapturedGraph;

    fn moved(kind: WorkflowMoveKind, process: &LfProcessId) -> WorkflowMove {
        WorkflowMove {
            workflow: "feature".into(),
            kind,
            from: "design".into(),
            to: "design".into(),
            edge: None,
            lf_process_id: Some(process.clone()),
            actor: WorkflowActor::Person,
            session_id: None,
            note: None,
            at: 0,
        }
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
            at("review-design"),
            ShipStep::StandIn {
                stage: "review-design".into(),
                skill: "review-design".into(),
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
    fn shipping_lasts_until_a_person_sets_the_task_or_it_reaches_the_end() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let on = |node: &str, history| {
            Workflow::new(
                definition.clone(),
                routes.clone(),
                WorkflowPosition::Node { node: node.into() },
                history,
            )
        };
        let restart = LfProcessId::new();
        let shipped = vec![
            moved(WorkflowMoveKind::Shipped, &LfProcessId::new()),
            moved(WorkflowMoveKind::Shipped, &restart),
            moved(WorkflowMoveKind::Arrived, &LfProcessId::new()),
        ];
        assert_eq!(
            shipping_process(&on("demo", shipped.clone())),
            Some(&restart)
        );
        assert!(shipping_process(&on("end", shipped.clone())).is_none());
        let mut set = shipped;
        set.push(moved(WorkflowMoveKind::Set, &LfProcessId::new()));
        assert!(shipping_process(&on("demo", set)).is_none());
        assert!(shipping_process(&on("demo", Vec::new())).is_none());
    }

    #[test]
    fn shipping_reads_what_happens_now_and_never_waits_on_a_person() {
        let repo = tempfile::tempdir().unwrap();
        let graph = CapturedGraph::load("feature", repo.path())
            .unwrap()
            .unwrap();
        let (definition, routes) = graph.project().unwrap();
        let mut restarted = moved(WorkflowMoveKind::Shipped, &LfProcessId::new());
        restarted.note = Some(RESTART_NOTE.into());
        restarted.at = 9;
        let mut first = moved(WorkflowMoveKind::Shipped, &LfProcessId::new());
        first.at = 5;
        let shipped = Workflow::new(
            definition.clone(),
            routes.clone(),
            WorkflowPosition::Node {
                node: "demo".into(),
            },
            vec![first, restarted],
        );
        let read = |status, alive, landing: Option<&PrLanding>| {
            shipping_snapshot(&shipped, status, alive, landing).unwrap()
        };
        let building = read(TaskState::Active, ProcessIdentityEvidence::Live, None);
        assert_eq!(building.shipped_at, 5);
        assert_eq!(building.state, ShippingState::Building);
        assert_eq!(building.reason, "demo (stand-in)");
        // Unknown liveness is not a dead process.
        assert_eq!(
            read(TaskState::Active, ProcessIdentityEvidence::Unknown, None).state,
            ShippingState::Building
        );
        let restarting = read(TaskState::Active, ProcessIdentityEvidence::Dead, None);
        assert_eq!(restarting.state, ShippingState::Restarting);
        assert!(restarting.reason.contains("×1"), "{}", restarting.reason);
        assert_eq!(
            read(TaskState::Paused, ProcessIdentityEvidence::Dead, None).state,
            ShippingState::Paused
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
            TaskState::Active,
            ProcessIdentityEvidence::Live,
            Some(&landing),
        );
        assert_eq!(landing_now.state, ShippingState::Landing);
        assert_eq!(landing_now.reason, "CI running on #1650");
        landing.state = PrLandingState::Blocked;
        landing.blocked_reason = Some("rust-lint".into());
        let repairing = read(
            TaskState::Active,
            ProcessIdentityEvidence::Live,
            Some(&landing),
        );
        assert_eq!(repairing.state, ShippingState::Repairing);
        assert_eq!(repairing.reason, "Repairing #1650 · rust-lint");

        // Done, or never shipped, is not shipping.
        assert!(shipping_snapshot(
            &shipped,
            TaskState::Done,
            ProcessIdentityEvidence::Live,
            None
        )
        .is_none());
        let unshipped = Workflow::new(
            definition,
            routes,
            WorkflowPosition::Node {
                node: "demo".into(),
            },
            Vec::new(),
        );
        assert!(shipping_snapshot(
            &unshipped,
            TaskState::Active,
            ProcessIdentityEvidence::Live,
            None
        )
        .is_none());
    }
}
