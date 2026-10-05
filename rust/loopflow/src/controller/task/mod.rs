use crate::durable::{FlowSession, TaskWorkerClaim};
use crate::engine::invocation::QueuedInvocation;
use crate::store::SharedStore;
use crate::work::task::{Task, TaskId};
use anyhow::{anyhow, Result};
use clap::Parser;

async fn load_task(store: &SharedStore, task_id: &TaskId) -> Result<Task> {
    store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} not found"))
}

pub(crate) async fn run(store: SharedStore, task_id: TaskId) -> Result<()> {
    let launch_claim =
        take_task_exec_env(crate::durable::TASK_WORKER_CLAIM_ENV, "Task worker claim")?
            .map(|value| serde_json::from_str::<TaskWorkerClaim>(&value))
            .transpose()
            .map_err(|error| anyhow!("invalid Task worker claim: {error}"))?
            .ok_or_else(|| anyhow!("Task boundary launch is missing its worker claim"))?;
    let owner = crate::journal::current_process_identity()
        .ok_or_else(|| anyhow!("Task worker requires a registered Exec"))?;
    let launch_claim = store
        .sqlite
        .handoff_task_worker(&task_id, &launch_claim, &owner)?;
    drive_task(store, task_id, launch_claim).await
}

/// Drive the Task's Flow through the shared executor with this process's
/// claim. A step that ends without a result records failure and releases the position.
async fn drive_task(
    store: SharedStore,
    task_id: TaskId,
    launch_claim: TaskWorkerClaim,
) -> Result<()> {
    let flow = store
        .task_flow(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task has no active Flow"))?;
    if flow.claim.as_ref() != Some(&launch_claim) {
        anyhow::bail!("Task driver launch claim is stale");
    }
    let options: Vec<String> =
        take_task_exec_env(crate::lf::TASK_SKILL_OPTIONS_ENV, "Task skill options")?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
    let cli = crate::lf::Cli::try_parse_from(std::iter::once("lf".to_string()).chain(options))?;
    crate::lf::commands::flow::drive(store, flow, Some(launch_claim), &cli)
        .await
        .map(|_| ())
}

pub async fn run_worker(task_id: TaskId) -> Result<()> {
    let store = std::sync::Arc::new(
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    run(store, task_id).await
}

fn take_task_exec_env(key: &'static str, label: &str) -> Result<Option<String>> {
    let Some(value) = std::env::var_os(key) else {
        return Ok(None);
    };
    std::env::remove_var(key);
    value
        .into_string()
        .map(Some)
        .map_err(|_| anyhow!("{label} is not valid UTF-8"))
}

/// The Task's Flow to launch: the one it points at, or `selected_flow` when
/// the Task has none or points at a different Flow. Selecting a different
/// Flow replaces the current one in one transaction.
pub(crate) async fn ensure_flow_position(
    store: &SharedStore,
    task_id: &TaskId,
    selected_flow: Option<&str>,
) -> Result<FlowSession> {
    let task = load_task(store, task_id).await?;
    let current = match (store.task_flow(&task.id).await?, selected_flow) {
        (Some(current), Some(selected)) if current.invocation.flow != selected => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (Some(current), _) => current,
        (None, Some(selected)) => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (None, None) => anyhow::bail!(
            "Task {} has no active Flow; run `lf --task {} flow start [TEMPLATE]` to select one",
            task.plan.identifier,
            task.plan.identifier
        ),
    };
    Ok(current)
}

/// A fresh invocation of `selected_flow` for the Task, at its first step.
fn start_task_flow(task: &Task, selected_flow: &str) -> Result<FlowSession> {
    Ok(FlowSession {
        invocation: QueuedInvocation::load(&task.worktree, selected_flow)?,
        cursor: crate::engine::ExecutionCursor::default(),
        version: 0,
        task_id: Some(task.id.clone()),
        wave_id: Some(task.wave_id.clone()),
        cwd: task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
        updated_at: time::OffsetDateTime::now_utc(),
    })
}
