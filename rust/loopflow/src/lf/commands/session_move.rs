//! `lf --machine M resume <session>` moves the conversation before resuming
//! it there; `lf task move X --machine M` and `lf --machine M resume --task X`
//! move a Task with its conversation. `send` runs where the work lives and
//! `receive` on M, each locally when that side is this Machine, otherwise over one SSH request, so
//! laptop → mini, rental → laptop and rental → mini are the same relay.

use anyhow::{anyhow, Context, Result};

use crate::durable::{Machine, MachineId, RepositoryId, TaskOwnership};
use crate::ops::human_session::moves::{self, Move};
use crate::session::LfSession;
use crate::store::SharedStore;
use crate::work::task::Task;

/// Move `session`, which this Machine knows, to `target` unless it is
/// already there.
pub(crate) async fn relay(
    store: &SharedStore,
    session: &LfSession,
    target: &Machine,
) -> Result<()> {
    let id = session.id.to_string();
    let moved_to = store.sqlite.session_moved_to(&id)?;
    if moved_to.as_ref() == Some(&target.id) {
        return Ok(());
    }
    ready(store, target, None, &format!("Session {}", session.title)).await?;
    let moved = match moved_to {
        Some(machine) => {
            let source = registered(store, &machine)?;
            let bytes = super::ssh::request(
                &source,
                &[
                    "session".into(),
                    "send".into(),
                    id.clone(),
                    "--to".into(),
                    target.id.to_string(),
                ],
                None,
            )
            .await?;
            serde_json::from_slice(&bytes).context("read the moved Session")?
        }
        None => moves::send(store, &id, &target.id).await?,
    };
    receive(store, &moved, Some(target))
        .await
        .map_err(|error| retry(error, &format!("Session {id}"), &id, target))?;
    eprintln!("Moved {} to {}", session.title, target.name());
    Ok(())
}

/// `lf task move X --machine M`. A Graph the move stopped continues where the
/// Task arrives.
pub fn move_task(issue: &str, target: &str) -> Result<TaskOwnership> {
    let (ownership, stopped) = tokio::runtime::Runtime::new()?.block_on(async {
        let store: SharedStore = std::sync::Arc::new(
            crate::store::open_existing_store()
                .await?
                .ok_or_else(|| anyhow!("no Loopflow registry on this machine; run `lf doctor`"))?,
        );
        relay_task(&store, issue, target).await
    })?;
    if let Some(stopped) = stopped {
        stopped.resume();
    }
    Ok(ownership)
}

/// A Task whose running Graph a move stopped, now owned where it arrived.
struct StoppedRun {
    identifier: String,
    /// The step it was running, which runs again on arrival.
    from: Option<String>,
    task: crate::durable::TaskId,
    /// Where it arrived, as the stage lines name it.
    place: String,
    /// This Machine's checkout of the Task's repository; the run is addressed
    /// from it and starts on the Task's owner.
    repository: Result<std::path::PathBuf, String>,
}

impl StoppedRun {
    /// Start the Task's run on its new owner. The move already happened, so a
    /// run that does not start is reported, never an error.
    fn resume(&self) {
        let name = &self.identifier;
        let started = self.repository.clone().and_then(|repository| {
            crate::engine::task::run(
                &repository,
                self.task.as_str(),
                self.from.as_deref(),
                None,
                None,
                false,
            )
            .map_err(|error| error.message)
        });
        match started {
            Ok(crate::engine::task::TaskRun::Running {
                graph_process,
                ships,
                ..
            }) => {
                eprintln!("Running on {} as Graph process {graph_process}", self.place);
                eprintln!("  follow: lf task status {name}");
                if ships {
                    eprintln!("  ships:  no interactive step is left, so this run may merge the PR; `lf task hold {name}` withdraws");
                }
            }
            Ok(run) => {
                if let Some(text) = run.render() {
                    eprintln!("{text}");
                }
            }
            Err(error) => {
                let from = self
                    .from
                    .as_ref()
                    .map(|step| format!(" --from '{step}'"))
                    .unwrap_or_default();
                eprintln!(
                    "{name} moved, but its Graph did not continue: {error}\n  lf task run {name}{from}"
                )
            }
        }
    }
}

/// Send the Task where it is owned, receive it on `target`, and learn the
/// handoff here too so this Machine routes it there.
async fn relay_task(
    store: &SharedStore,
    issue: &str,
    target: &str,
) -> Result<(TaskOwnership, Option<StoppedRun>)> {
    let task = store
        .get_task_by_issue(issue)
        .await?
        .ok_or_else(|| anyhow!("no Task {issue} on this Machine"))?;
    let local = store.sqlite.local_machine()?.id;
    let target = crate::ops::task_ownership::resolve_machine(&store.sqlite, target)?;
    let target = if target == local {
        None
    } else {
        Some(super::ssh::resolve_target(target.as_str(), false).await?)
    };
    let to = target
        .as_ref()
        .map_or(local.clone(), |machine| machine.id.clone());
    let route = store.task_execution_route(&task.id).await?;
    let name = &task.identifier;
    if route.machine_id == to {
        anyhow::bail!("{name} already runs on {}", describe(target.as_ref()));
    }
    let place = describe(target.as_ref());
    if let Some(target) = &target {
        ready(store, target, Some((&task, &route.repository_id)), name).await?;
        eprintln!("Checked {place}: it can take {name}");
    }
    let moved: Move = if route.machine_id == local {
        moves::send_task(store, task.id.as_str(), &to).await?
    } else {
        let source = super::ssh::resolve_target(route.machine_id.as_str(), false).await?;
        let bytes = super::ssh::request(
            &source,
            &[
                "--repo-id".into(),
                route.repository_id.to_string(),
                "session".into(),
                "send".into(),
                "--task".into(),
                task.id.to_string(),
                "--to".into(),
                to.to_string(),
            ],
            None,
        )
        .await?;
        serde_json::from_slice(&bytes).context("read the moved Task")?
    };
    let moved_task = moved
        .task
        .as_ref()
        .ok_or_else(|| anyhow!("{name} was sent as a Session, not a Task"))?;
    // A stopped Graph continues at the step it was running; one that was
    // resting stays at its step.
    let from = moved_task
        .stopped
        .as_ref()
        .and_then(|stopped| stopped.step.clone());
    let step = from
        .clone()
        .or_else(|| moved_task.graph.as_ref().map(|graph| graph.step.clone()));
    if moved_task.stopped.is_some() {
        match &from {
            Some(step) => eprintln!("Stopped {name} at `{step}`"),
            None => eprintln!("Stopped {name}'s Graph"),
        }
    }
    if let Some(checkout) = &moved.checkout {
        eprintln!("Committed and pushed {name} on {}", checkout.branch);
    }
    store
        .sqlite
        .ingest_ownership_marker(&task.id, &moved_task.ownership_marker)?;
    receive(store, &moved, target.as_ref())
        .await
        .map_err(|error| match &target {
            Some(target) => retry(error, name, &format!("--task {}", task.id), target),
            None => error,
        })?;
    let at = step
        .map(|step| format!(" (Graph at `{step}`)"))
        .unwrap_or_default();
    let conversation = if moved.session.is_some() {
        "; its Session moved with it"
    } else {
        ""
    };
    eprintln!("Arrived on {place}{at}{conversation}");
    let stopped = moved_task.stopped.is_some().then(|| StoppedRun {
        identifier: name.clone(),
        from,
        task: task.id.clone(),
        place: place.clone(),
        repository: crate::ops::task::repository_for_task(store, &task)
            .map_err(|error| error.to_string()),
    });
    Ok((store.sqlite.task_ownership(&task.id)?, stopped))
}

/// Before anything is committed, pushed, stopped or handed off, `target`
/// answers that it can receive the move and, for a Task, takes it in from
/// planning when it has the repository.
async fn ready(
    store: &SharedStore,
    target: &Machine,
    task: Option<(&Task, &RepositoryId)>,
    what: &str,
) -> Result<()> {
    if target.id == store.sqlite.local_machine()?.id {
        return Ok(());
    }
    let mut args = vec!["session".into(), "receive".into(), "--check".into()];
    if let Some((task, repository)) = task {
        args.extend([
            "--task".into(),
            task.id.to_string(),
            "--issue".into(),
            task.identifier.clone(),
            "--repository".into(),
            repository.to_string(),
        ]);
    }
    super::ssh::request(target, &args, None)
        .await
        .map(drop)
        .map_err(|error| {
            anyhow!(
                "{what} did not move: {} cannot receive it; nothing was sent\n{error:#}",
                target.name()
            )
        })
}

/// Receive on `target`, over SSH unless it is this Machine (`None`).
async fn receive(store: &SharedStore, moved: &Move, target: Option<&Machine>) -> Result<()> {
    match target {
        Some(target) if target.id != store.sqlite.local_machine()?.id => {
            super::ssh::request(
                target,
                &["session".into(), "receive".into()],
                Some(&serde_json::to_vec(moved)?),
            )
            .await?;
        }
        _ => {
            moves::receive(store, moved.clone()).await?;
        }
    }
    Ok(())
}

fn retry(error: anyhow::Error, what: &str, selector: &str, target: &Machine) -> anyhow::Error {
    anyhow!(
        "{error:#}\n{what} was sent but did not arrive on {}; where it lived, retry with\n  lf session send {selector} --to {} | lf --machine {} session receive",
        target.name(),
        target.id,
        target.name(),
    )
}

fn registered(store: &SharedStore, machine: &MachineId) -> Result<Machine> {
    store
        .sqlite
        .machine_by_id(machine)?
        .ok_or_else(|| anyhow!("Machine {machine} is not registered here"))
}

fn describe(machine: Option<&Machine>) -> String {
    match machine {
        Some(machine) => machine.name(),
        None => format!(
            "{} (this Machine)",
            crate::store::sqlite::task_ownership::local_label()
        ),
    }
}
