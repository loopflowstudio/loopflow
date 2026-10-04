//! Explicit restart retires the pending review, not independent Task work.
use std::time::Duration;

use crate::durable::FlowSession;
use crate::journal::ProcessIdentityEvidence;
use crate::ops::OpsResult;
use crate::store::SharedStore;

use super::task_error;

pub(super) async fn stop_review(store: &SharedStore, flow: &FlowSession) -> OpsResult<FlowSession> {
    let Some(id) = flow.pending_session_id.as_deref() else {
        return Ok(flow.clone());
    };
    // Launch precedes dispatch everywhere a review is opened. Wait for the
    // service's reserve/spawn handshake before fencing or observing its owners.
    let lock_id = id.to_owned();
    let _launch =
        tokio::task::spawn_blocking(move || crate::ops::human_session::lock_session_exec(&lock_id))
            .await
            .map_err(task_error)?
            .map_err(task_error)?;
    let current = store
        .task_flow(
            flow.task_id
                .as_ref()
                .ok_or_else(|| task_error("review has no Task"))?,
        )
        .await
        .map_err(task_error)?
        .ok_or_else(|| task_error("Task review changed while waiting for launch"))?;
    if current.id() != flow.id() || current.pending_session_id != flow.pending_session_id {
        return Err(task_error(
            "Task review changed while waiting for launch; retry restart",
        ));
    }
    let flow = &current;
    store.sqlite.retire_task_review(flow).map_err(task_error)?;
    let session = store
        .sqlite
        .session(id)
        .map_err(task_error)?
        .ok_or_else(|| task_error("pending review disappeared"))?;
    let execs = store.sqlite.review_execs(id).map_err(task_error)?;
    let provider = store
        .sqlite
        .session_provider_process(id)
        .map_err(task_error)?;
    let saved = store.sqlite.review_stop_processes(id).map_err(task_error)?;
    let mut owners = saved.clone().unwrap_or_default();
    let receipts = crate::journal::read_exec_process_receipts_at(&crate::store::lf_home_dir())
        .map_err(task_error)?;
    for exec in &execs {
        if let Some(receipt) = receipts
            .iter()
            .find(|receipt| receipt.exec_id == exec.as_str())
        {
            owners.push((receipt.pid, receipt.started_at));
        } else if saved.is_none()
            && crate::journal::exec_process_evidence(&store.sqlite, exec)
                != ProcessIdentityEvidence::Dead
        {
            return Err(task_error(format!("review {id} Exec {exec} has unresolved process identity; retry Task restart after inspection")));
        }
    }
    if let Some(owner) = provider {
        owners.push(owner);
    }
    // A matching input owns its native clients. Neither ancestry nor the Task's
    // checkout grants authority over arbitrary processes in the same group.
    let clients = if let Some(harness) = session.provider.as_deref() {
        let dir = crate::ops::human_session::local_session_run_dir(&session.artifact_key)
            .ok_or_else(|| task_error("review has an invalid capture"))?;
        let clients = crate::lf::commands::util::active_provider_clients(&dir, harness)
            .map_err(task_error)?;
        Some((dir, harness.to_owned(), clients))
    } else {
        None
    };
    if saved.is_none()
        && session.input_published
        && provider.is_none()
        && clients
            .as_ref()
            .is_none_or(|(_, _, clients)| clients.is_empty())
    {
        return Err(task_error(format!("review {id} provider ownership is unresolved; retained its history and execution for inspection")));
    }
    if let Some((_, _, clients)) = &clients {
        owners.extend(
            clients
                .iter()
                .map(|client| (client.pid, client.started_at.unix_timestamp())),
        );
    }
    owners.sort_unstable();
    owners.dedup();
    for &(pid, started) in &owners {
        if crate::journal::process_identity_evidence(pid, started)
            == ProcessIdentityEvidence::Unknown
        {
            return Err(task_error(format!(
                "review {id} process {pid} identity is unresolved"
            )));
        }
    }
    store
        .sqlite
        .record_review_stop_processes(id, &owners)
        .map_err(task_error)?;
    if let Some((dir, harness, clients)) = clients.filter(|(_, _, clients)| !clients.is_empty()) {
        tokio::task::spawn_blocking(move || {
            crate::lf::commands::util::replace_provider_clients(
                &dir,
                &harness,
                &clients,
                crate::session_record::ProviderClientStopReason::Retired,
            )
        })
        .await
        .map_err(task_error)?
        .map_err(task_error)?;
    }
    if let (Some((pid, started)), Some((endpoint, thread))) = (
        provider,
        store.sqlite.session_connection(id).map_err(task_error)?,
    ) {
        if session.provider.as_deref() == Some("codex") {
            tokio::task::spawn_blocking(move || {
                crate::harness::codex_connection::close_engine(&endpoint, &thread, pid, started)
            })
            .await
            .map_err(task_error)?
            .map_err(task_error)?;
        }
    }
    for &(pid, started) in &owners {
        if crate::journal::process_identity_evidence(pid, started) == ProcessIdentityEvidence::Live
        {
            let status = tokio::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status()
                .await
                .map_err(task_error)?;
            if !status.success() {
                return Err(task_error(format!(
                    "could not stop review {id} process {pid}; retry Task restart"
                )));
            }
        }
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        if owners.iter().all(|&(pid, started)| {
            crate::journal::process_identity_evidence(pid, started) == ProcessIdentityEvidence::Dead
        }) {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(task_error(format!(
                "review {id} execution has not exited; retry Task restart"
            )));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    store
        .sqlite
        .review_execution_stopped(id)
        .map_err(task_error)?;
    Ok(current)
}
