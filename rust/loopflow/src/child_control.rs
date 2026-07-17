//! Provider-neutral lifecycle control and live Steer delivery for child Work.
//!
//! Runners render authored direction from the durable Work boundary and may
//! attempt a same-Turn Send as a latency optimization. Lifecycle control is a
//! projection over the active Run, Launch, Turn, and Epoch.

use std::collections::VecDeque;

use anyhow::Result;

use crate::child_session::{ChildRef, ChildWriteLease};
use crate::durable::{Basis, BoundarySeed, SendState};
use crate::harness::{Harness, SendCurrentOutcome};
use crate::project_session::ProjectSessionId;
use crate::store::SharedStore;
use crate::task::TaskSessionId;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ChildTarget<'a> {
    Project(&'a ProjectSessionId, &'a ChildWriteLease),
    Task(&'a TaskSessionId, &'a ChildWriteLease),
}

impl<'a> ChildTarget<'a> {
    fn as_ref(self) -> ChildRef {
        match self {
            Self::Project(id, _) => ChildRef::Project(id.clone()),
            Self::Task(id, _) => ChildRef::Task(id.clone()),
        }
    }

    fn lease(self) -> &'a ChildWriteLease {
        match self {
            Self::Project(_, lease) | Self::Task(_, lease) => lease,
        }
    }

    async fn validate_write_lease(self, store: &SharedStore) -> Result<()> {
        store
            .validate_child_write_lease(&self.as_ref(), self.lease())
            .await?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct PendingInput {
    pub text: String,
}

impl PendingInput {
    pub fn system(text: String) -> Self {
        Self { text }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandStop {
    Interrupted,
    Abandoned(String),
}

/// Apply direct Run/Work control to the provider boundary owned by this body.
pub(crate) async fn absorb_run_control(
    store: &SharedStore,
    target: ChildTarget<'_>,
    harness: &mut dyn Harness,
    turn_active: bool,
    active_turn_id: Option<&str>,
) -> Result<Option<CommandStop>> {
    target.validate_write_lease(store).await?;
    let run_lease = store
        .run_lease_for_child(&target.as_ref(), target.lease())
        .await?;
    match store.run_control(&run_lease, active_turn_id).await? {
        Some(crate::durable::RunControl::Interrupt) => {
            if turn_active {
                harness.interrupt().await?;
            }
            Ok(Some(CommandStop::Interrupted))
        }
        Some(crate::durable::RunControl::Abandon { reason }) => {
            Ok(Some(CommandStop::Abandoned(reason)))
        }
        None => Ok(None),
    }
}

pub(crate) async fn take_current_input(
    _store: &SharedStore,
    _target: ChildTarget<'_>,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<PendingInput>> {
    Ok(pending.pop_front())
}

pub(crate) async fn input_is_current(
    _store: &SharedStore,
    _target: ChildTarget<'_>,
    _input: &PendingInput,
) -> Result<bool> {
    Ok(true)
}

/// Attempt each outstanding Steer once against the exact observed Turn.
///
/// The Steer remains authoritative for a later boundary regardless of outcome.
/// A Send records transport evidence only; it never advances applied Basis.
pub(crate) async fn send_outstanding_steers(
    store: &SharedStore,
    target: ChildTarget<'_>,
    harness: &mut dyn Harness,
    turn_id: &str,
    active_basis: &Basis,
) -> Result<BoundarySeed> {
    target.validate_write_lease(store).await?;
    let work = store.work_for_child(&target.as_ref()).await?;
    let seed = store.boundary_seed(&work).await?;
    for steer in seed
        .steers
        .iter()
        .filter(|steer| steer.basis.revision > active_basis.revision)
    {
        let Some(send) = store.begin_live_send(&steer.id, turn_id).await? else {
            continue;
        };
        let (state, provider_turn_id, reason) = match harness.send_current(&steer.text).await {
            SendCurrentOutcome::Sent { provider_turn_id } => {
                (SendState::Sent, Some(provider_turn_id), None)
            }
            SendCurrentOutcome::NotSteerable => (
                SendState::Failed,
                None,
                Some("active Turn is not steerable".to_string()),
            ),
            SendCurrentOutcome::Failed { error } => (SendState::Failed, None, Some(error)),
            SendCurrentOutcome::Unknown {
                provider_turn_id,
                error,
            } => (SendState::Unknown, provider_turn_id, Some(error)),
        };
        store
            .finish_send(
                &send.id,
                state,
                provider_turn_id.as_deref(),
                reason.as_deref(),
            )
            .await?;
    }
    Ok(seed)
}

pub(crate) async fn apply_input(
    store: &SharedStore,
    target: ChildTarget<'_>,
    harness: &mut dyn Harness,
    input: PendingInput,
) -> Result<()> {
    target.validate_write_lease(store).await?;
    harness.send_input(&input.text).await
}
