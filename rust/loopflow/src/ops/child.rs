//! Shared recovery and execution helpers for Project and Task Work.

use std::time::Duration;

use crate::durable::WorkRef;
use crate::store::SharedStore;

pub(crate) const CHILD_STARTUP_GRACE: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WorkControlReceipt {
    Interrupt { work: WorkRef },
}

impl WorkControlReceipt {
    pub fn label(&self) -> String {
        match self {
            Self::Interrupt { work } => work.id().to_string(),
        }
    }

    pub fn action(&self) -> &'static str {
        match self {
            Self::Interrupt { .. } => "interrupted",
        }
    }
}

/// Inject steer comments newer than `*cursor` into the live provider turn. A
/// comment the provider takes (`Sent`) advances the cursor; one it can't take
/// right now (`NotSteerable` — no active turn, or a driver without live input)
/// stays for the next skill boundary, whose seed reads unconsumed steers. Steering
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
    for mut steer in steers {
        if steer.id <= *cursor {
            continue;
        }
        // Token count cannot exceed byte count, so small inputs need no lookup.
        if steer.text.len() > crate::engine::context_budget::GOAL_TOKENS {
            let result = async {
                let task = store
                    .get_task(task_id)
                    .await?
                    .ok_or(crate::store::StoreError::NotFound)?;
                crate::engine::context_budget::bound_message(&steer.text, &task.worktree)
                    .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
            }
            .await;
            match result {
                Ok(text) => steer.text = text,
                Err(error) => {
                    tracing::warn!(%error, "failed to preserve oversized live direction; deferring delivery");
                    break;
                }
            }
        }
        match harness.send_current(&steer.text).await {
            crate::harness::SendCurrentOutcome::Sent { .. } => {
                *cursor = steer.id;
                delivered.push(steer);
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
