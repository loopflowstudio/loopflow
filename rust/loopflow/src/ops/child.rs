//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::child::ChildRef;
use crate::durable::{AbandonReceipt, WorkRef};
use crate::store::SharedStore;
use crate::work::task::Task;

use super::{OpsError, OpsResult};

pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Steer { comment_id: String },
    Interrupt { work: WorkRef },
    Resume { work: WorkRef },
    Abandon { receipt: AbandonReceipt },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Steer { comment_id } => comment_id.clone(),
            Self::Interrupt { work } => work.id().to_string(),
            Self::Resume { work } => work.id().to_string(),
            Self::Abandon { receipt } => receipt.work.id().to_string(),
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Steer { .. } => "posted to Linear",
            Self::Interrupt { .. } => "interrupted",
            Self::Resume { .. } => "resumed",
            Self::Abandon { .. } => "abandoned",
        }
    }
}

pub(crate) async fn resume_task(store: &SharedStore, mut task: Task) -> OpsResult<WorkRef> {
    let label = format!("Task {}", task.plan.identifier);
    if let Some(intent) = &task.abandon_intent {
        return Err(child_error(format!(
            "{label} is being abandoned: {}",
            intent.reason
        )));
    }
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(child_error)?;
    super::task::resume_inactive_process(store, &mut task).await?;
    Ok(work)
}

/// Inject steer comments newer than `*cursor` into the live provider turn. A
/// comment the provider takes (`Sent`) advances the cursor; one it can't take
/// right now (`NotSteerable` — no active turn, or a driver without live input)
/// stays for the next skill boundary, whose seed re-reads every steer. Steering
/// is best-effort live and durable at the boundary, so this never fails the run.
pub(crate) async fn inject_live_steers(
    store: &SharedStore,
    task_id: &crate::work::task::TaskId,
    harness: &mut dyn crate::harness::Harness,
    cursor: &mut i64,
) -> Vec<crate::durable::Steer> {
    let mut delivered = Vec::new();
    let steers = match store.task_steers(task_id).await {
        Ok(steers) => steers,
        Err(error) => {
            tracing::warn!(%error, "failed to read steers for live injection");
            return delivered;
        }
    };
    for steer in &steers {
        if steer.id <= *cursor {
            continue;
        }
        match harness.send_current(&steer.text).await {
            crate::harness::SendCurrentOutcome::Sent { .. } => {
                *cursor = steer.id;
                delivered.push(steer.clone());
            }
            crate::harness::SendCurrentOutcome::NotSteerable => break,
            crate::harness::SendCurrentOutcome::Failed { error }
            | crate::harness::SendCurrentOutcome::Unknown { error, .. } => {
                tracing::warn!(
                    %error,
                    steer = steer.id,
                    "live steer injection failed; deferring to the next boundary"
                );
                break;
            }
        }
    }
    delivered
}

/// End the current turn if an interrupt was requested after `*cursor`. The
/// interrupt is a one-time durable event, so the cursor only advances (a turn
/// boundary never resets it) — a request is acted on exactly once per run.
pub(crate) async fn observe_interrupt(
    store: &SharedStore,
    work: &WorkRef,
    harness: &mut dyn crate::harness::Harness,
    cursor: &mut i64,
) {
    match store.latest_interrupt_id(work).await {
        Ok(id) if id > *cursor => {
            *cursor = id;
            if let Err(error) = harness.interrupt().await {
                tracing::warn!(%error, "interrupt request failed to reach the provider");
            }
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "failed to read interrupt requests"),
    }
}

fn child_error(error: impl std::fmt::Display) -> OpsError {
    OpsError::Message(error.to_string())
}
