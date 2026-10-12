//! First-start requests go to one immutable coordinator, never a mergeable ref.
use anyhow::{anyhow, ensure, Result};

use crate::durable::{MachineId, TaskAdmission, TaskId};
use crate::store::sqlite::SqliteStore;

pub async fn observe(
    store: &SqliteStore,
    task: &TaskId,
    claim: Option<&MachineId>,
    request: &str,
) -> Result<Option<TaskAdmission>> {
    let Some(saved) = store.task_admission(task, request)? else {
        return Ok(None);
    };
    // A positive decision is permanent. Reusing it needs no lease renewal or
    // inference from process liveness, and cannot authorize another Machine.
    if saved.owner.is_some() {
        return Ok(Some(saved));
    }
    if saved.authority.machine_id == store.local_machine()?.id {
        return Ok(Some(match claim {
            Some(candidate) => store.claim_task_admission(task, candidate, request)?,
            None => saved,
        }));
    }
    let machine = store.machine_by_id(&saved.authority.machine_id)?
        .filter(|machine| machine.label.is_some())
        .ok_or_else(|| anyhow!("Task first-start authority {} is unavailable; add its Machine connection, then retry", saved.authority.machine_id))?;
    let mut args = vec![
        "--repository".into(),
        saved.authority.repository_id.to_string(),
        "task".into(),
        "admission".into(),
        task.to_string(),
        "--request".into(),
        request.into(),
        "--json".into(),
    ];
    if let Some(candidate) = claim {
        args.extend(["--claim".into(), candidate.to_string()]);
    }
    let bytes = crate::lf::commands::ssh::request(&machine, &args).await?;
    let reply: TaskAdmission = serde_json::from_slice(&bytes)?;
    ensure!(
        reply.request == request && reply.task_id == *task && reply.authority == saved.authority,
        "stale or mismatched Task admission reply; no execution admitted"
    );
    Ok(Some(reply))
}

pub async fn ensure_local(store: &SqliteStore, task: &TaskId) -> Result<()> {
    let local = store.local_machine()?.id;
    let Some(reply) = observe(store, task, Some(&local), &uuid::Uuid::new_v4().to_string()).await?
    else {
        // Historical unshared Tasks retain their existing placement boundary.
        // Shared Tasks without an established authority still refuse there.
        return Ok(());
    };
    ensure!(
        reply.owner.as_ref() == Some(&local),
        "Task first execution is reserved on {}; retry there, no local checkout was allocated",
        reply
            .owner
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "an unconfirmed Machine".into())
    );
    store.retain_task_admission(&reply)?;
    Ok(())
}

pub fn command(
    selector: &str,
    claim: Option<&MachineId>,
    request: Option<&str>,
    json: bool,
) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = if claim.is_some() {
            crate::store::open_store(&crate::store::storage_config_from_env()?).await?
        } else {
            crate::store::read_existing_registry()?.ok_or_else(|| anyhow!("Task admission registry is unavailable"))?
        };
        let repo = crate::repository::CanonicalRepo::current()?.ok_or_else(|| anyhow!("select the Task repository"))?;
        let task = store.sqlite.resolve_task_id(selector, Some(&repo.to_string()))?
            .ok_or_else(|| anyhow!("Task {selector} is not recorded in this repository"))?;
        let request = request.map(str::to_owned).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let reply = observe(&store.sqlite, &task, claim, &request).await?
            .ok_or_else(|| anyhow!("Task has no established first-start authority; historical absence cannot admit execution"))?;
        if claim.is_some() && reply.owner.is_some() {
            store.sqlite.retain_task_admission(&reply)?;
        }
        if json { println!("{}", serde_json::to_string(&reply)?); }
        else { println!("{}: authority {}, execution {}", reply.task_id, reply.authority.machine_id,
            reply.owner.as_ref().map(ToString::to_string).unwrap_or_else(|| "unclaimed".into())); }
        Ok(())
    })
}
