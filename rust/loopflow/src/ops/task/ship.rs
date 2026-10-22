//! `lf ship`: someone hands a Task off once, and its remaining graph runs to
//! `end` with no one at hand. Shipping is the Task's lifecycle state between
//! working and done; this Machine's ship process carries it there. At each
//! stage a headless stand-in runs the stage's skill and decides the route.
//! Pause holds it, Stop returns it to working, and a ship process that died is
//! started again.

use serde::{Deserialize, Serialize};

use crate::durable::TaskState;
use crate::id::LfProcessId;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::error::OpsResult;
use crate::ops::workflow::{Workflow, WorkflowPosition};
use crate::pr_landing::{PrLanding, PrLandingState};
use crate::store::sqlite::TaskShipping;
use crate::work::task::Task;
use crate::workflow::{END, START};

use super::{block_on_task, task_error, task_store, TaskRunSelection};

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
    /// Someone stopped shipping; the Task is working again.
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
    Paused,
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

/// The Task's shipping, when it is shipping. `alive` is the ship process's liveness.
pub(crate) fn shipping_snapshot(
    workflow: Option<&Workflow>,
    shipping: &TaskShipping,
    status: TaskState,
    alive: ProcessIdentityEvidence,
    landing: Option<&PrLanding>,
) -> Option<ShippingSnapshot> {
    if !matches!(status, TaskState::Shipping | TaskState::Paused) {
        return None;
    }
    let shipping = |state, reason: String| {
        Some(ShippingSnapshot {
            shipped_at: shipping.shipped_at,
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
        return shipping(
            ShippingState::Restarting,
            "Ship process exited; it restarts on the next read of this Machine's Tasks".into(),
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
    let landing = sqlite.pending_task_landing(task)?;
    Ok(shipping_snapshot(
        workflow,
        &shipping,
        status,
        alive,
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
        let by = crate::config::participant_name()
            .ok()
            .flatten()
            .unwrap_or_else(|| "someone".into());
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

/// Return a shipping Task to working: stop its ship process and take back any
/// merge request Loopflow armed on GitHub. Returns false when it was not shipping.
pub fn ship_stop(issue: &str, note: Option<&str>) -> OpsResult<bool> {
    let (task, stopped) = block_on_task(async {
        let store = task_store().await?;
        let task = store
            .get_task_by_issue(issue)
            .await
            .map_err(task_error)?
            .ok_or_else(|| task_error(format!("no Task exists for {issue:?}")))?;
        let by = crate::config::participant_name()
            .ok()
            .flatten()
            .unwrap_or_else(|| "someone".into());
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
            TaskState::Shipping => {}
            TaskState::Unstarted | TaskState::Working => return Ok(ShipStep::Stopped),
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

/// Start `lf --task ISSUE ship` again for each of the repository's shipping
/// Tasks whose ship process has exited. Paused and finished Tasks wait.
pub fn restart_ships(repo: &std::path::Path) -> OpsResult<()> {
    block_on_task(async {
        let store = task_store().await?;
        let repo_id = crate::repository::RepoId::discover(repo).map_err(task_error)?;
        for task in crate::ops::task_automation::repository_tasks(&store, &repo_id).await? {
            if store.sqlite.task_state(&task.id).map_err(task_error)? != TaskState::Shipping {
                continue;
            }
            let Some(shipping) = store.sqlite.task_shipping(&task.id).map_err(task_error)? else {
                continue;
            };
            if carrier_evidence(&store.sqlite, &shipping) != ProcessIdentityEvidence::Dead {
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
            ];
            crate::os_process::start_lf_session(worktree, &argv, &[], &|_| {})
                .map_err(task_error)?;
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
        };
        let read = |status, alive, landing: Option<&PrLanding>| {
            shipping_snapshot(Some(&workflow), &handoff, status, alive, landing).unwrap()
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
            None
        )
        .is_none());
    }
}
