//! `lf --machine M resume <session>` moves the conversation before resuming
//! it there, and `lf task move X --machine M` moves a Task with its
//! conversation. `send` runs where the work lives and `receive` on M, each
//! locally when that side is this Machine, otherwise over one SSH request, so
//! laptop → mini, rental → laptop and rental → mini are the same relay.

use anyhow::{anyhow, Context, Result};

use crate::durable::{Machine, MachineId, RepositoryId, TaskOwnership};
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
    let moved = match store.sqlite.session_moved_to(&id)? {
        Some(machine) if machine == target.id => return Ok(()),
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
    let repository = moved
        .checkout
        .as_ref()
        .map(|checkout| checkout.repository_id.clone())
        .ok_or_else(|| anyhow!("Session {id} moved without its checkout"))?;
    receive(store, &moved, &repository, target)
        .await
        .map_err(|error| retry(error, &format!("Session {id}"), &id, target))?;
    eprintln!("Moved {} to {}", session.title, label(target));
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
    let repository = route.repository_id.clone();
    let name = &task.identifier;
    let moved: Move = if route.machine_id == to {
        anyhow::bail!("{name} already runs on {}", describe(target.as_ref()));
    } else if route.machine_id == local {
        moves::send_task(store, task.id.as_str(), &to).await?
    } else {
        let source = super::ssh::resolve_target(route.machine_id.as_str(), false).await?;
        let bytes = super::ssh::request(
            &source,
            &[
                "--repository".into(),
                repository.to_string(),
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
    match &target {
        Some(target) => receive(store, &moved, &repository, target)
            .await
            .map_err(|error| retry(error, name, &format!("--task {}", task.id), target))?,
        None => {
            let repo = super::work_route::repository_path(&repository)?;
            moves::receive(store, &repo, moved.clone()).await?;
        }
    }
    if moved.checkout.is_some() {
        eprintln!(
            "Committed and pushed {} on {}",
            name,
            moved.checkout.as_ref().expect("checked above").branch
        );
    }
    let place = match moved_task.workflow.as_ref() {
        Some(workflow) => format!(" (Workflow at `{}`)", workflow.node),
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

/// Receive on `target` over SSH.
async fn receive(
    store: &SharedStore,
    moved: &Move,
    repository: &RepositoryId,
    target: &Machine,
) -> Result<()> {
    if target.id == store.sqlite.local_machine()?.id {
        let repo = super::work_route::repository_path(repository)?;
        moves::receive(store, &repo, moved.clone()).await?;
        return Ok(());
    }
    super::ssh::request(
        target,
        &[
            "--repository".into(),
            repository.to_string(),
            "session".into(),
            "receive".into(),
        ],
        Some(&serde_json::to_vec(moved)?),
    )
    .await?;
    Ok(())
}

fn retry(error: anyhow::Error, what: &str, selector: &str, target: &Machine) -> anyhow::Error {
    anyhow!(
        "{error:#}\n{what} was sent but did not arrive on {}; where it lived, retry with\n  lf session send {selector} --to {} | lf --machine {} session receive",
        label(target),
        target.id,
        label(target),
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
        Some(machine) => label(machine),
        None => format!(
            "{} (this Machine)",
            crate::store::sqlite::task_ownership::local_label()
        ),
    }
}

fn label(machine: &Machine) -> String {
    machine
        .label
        .clone()
        .unwrap_or_else(|| machine.id.to_string())
}
