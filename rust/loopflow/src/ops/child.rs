//! Shared recovery and execution helpers for Project and Task Work.

use crate::durable::WorkRef;
use crate::store::SharedStore;

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
