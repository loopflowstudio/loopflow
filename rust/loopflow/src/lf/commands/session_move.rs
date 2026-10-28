//! `lf --machine M resume <session>` moves the conversation before resuming
//! it there; `lf task move X --machine M` and `lf --machine M resume --task X`
//! move a Task with its conversation. `send` runs where the work lives and
//! `receive` on M, each locally when that side is this Machine, otherwise over one SSH request, so
//! laptop → mini, rental → laptop and rental → mini are the same relay.

use anyhow::{anyhow, Context, Result};

use crate::durable::{Machine, MachineId, TaskId, TaskOwnership};
use crate::ops::human_session::moves::{self, Move};
use crate::session::LfSession;
use crate::store::SharedStore;

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

/// `lf task move X --machine M`.
pub fn move_task(issue: &str, target: &str) -> Result<TaskOwnership> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store: SharedStore = std::sync::Arc::new(
            crate::store::open_existing_store()
                .await
                .ok_or_else(|| anyhow!("no Loopflow registry on this machine; run `lf doctor`"))?,
        );
        relay_task(&store, issue, target).await
    })
}

/// Send the Task where it is owned, receive it on `target`, and learn the
/// handoff here too so this Machine routes it there.
async fn relay_task(store: &SharedStore, issue: &str, target: &str) -> Result<TaskOwnership> {
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
    if let Some(target) = &target {
        ready(store, target, Some(&task.id), name).await?;
    }
    let moved: Move = if route.machine_id == local {
        moves::send_task(store, task.id.as_str(), &to).await?
    } else {
        let source = super::ssh::resolve_target(route.machine_id.as_str(), false).await?;
        let bytes = super::ssh::request(
            &source,
            &[
                "--repository".into(),
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
    store
        .sqlite
        .ingest_ownership_marker(&task.id, &moved_task.ownership_marker)?;
    receive(store, &moved, target.as_ref())
        .await
        .map_err(|error| match &target {
            Some(target) => retry(error, name, &format!("--task {}", task.id), target),
            None => error,
        })?;
    if let Some(checkout) = &moved.checkout {
        eprintln!("Committed and pushed {name} on {}", checkout.branch);
    }
    let place = match moved_task.graph.as_ref() {
        Some(graph) => format!(" (Graph at `{}`)", graph.step),
        None => String::new(),
    };
    let conversation = if moved.session.is_some() {
        "; its Session moved with it"
    } else {
        ""
    };
    eprintln!(
        "{name} now runs on {}{place}{conversation}",
        describe(target.as_ref())
    );
    Ok(store.sqlite.task_ownership(&task.id)?)
}

/// Before anything is committed, pushed, stopped or handed off, `target`
/// answers that it can receive the move and, for a Task, that it knows it.
async fn ready(
    store: &SharedStore,
    target: &Machine,
    task: Option<&TaskId>,
    what: &str,
) -> Result<()> {
    if target.id == store.sqlite.local_machine()?.id {
        return Ok(());
    }
    let mut args = vec!["session".into(), "receive".into(), "--check".into()];
    if let Some(task) = task {
        args.extend(["--task".into(), task.to_string()]);
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
