//! Provider-neutral lifecycle control and live Steer delivery for child Work.
//!
//! Authored direction never enters the command ledger. Runners render it from
//! the durable Work boundary and may attempt a same-Turn Send as a latency
//! optimization. Interrupt, resume, abandon, and CI wake remain typed lifecycle
//! commands while the shared Run controller absorbs them.

use std::collections::VecDeque;

use anyhow::Result;

use crate::child_session::{
    ChildCommand, ChildCommandId, ChildCommandKind, ChildCommandState, ChildRef, ChildWriteLease,
};
use crate::durable::{Basis, BoundarySeed, SendState};
use crate::harness::{Harness, SendCurrentOutcome};
use crate::project_session::{ProjectEventKind, ProjectSessionId};
use crate::store::SharedStore;
use crate::task::{TaskEventKind, TaskSessionId};

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

    async fn command_is_deliverable(
        self,
        store: &SharedStore,
        command_id: &ChildCommandId,
    ) -> Result<bool> {
        let claimed = store
            .get_child_command(command_id)
            .await?
            .is_some_and(|command| {
                let targets_match = match (self, &command.target) {
                    (Self::Project(target_id, _), ChildRef::Project(command_id)) => {
                        target_id == command_id
                    }
                    (Self::Task(target_id, _), ChildRef::Task(command_id)) => {
                        target_id == command_id
                    }
                    _ => false,
                };
                targets_match
                    && matches!(
                        command.state,
                        ChildCommandState::Claimed | ChildCommandState::Delivering
                    )
            });
        Ok(claimed)
    }

    async fn record_claimed(self, store: &SharedStore, command_id: ChildCommandId) -> Result<()> {
        self.record_command_changed(store, command_id, ChildCommandState::Claimed, None)
            .await
    }

    pub(crate) async fn accept_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
    ) -> Result<()> {
        let target = self.as_ref();
        store
            .accept_child_command_for_lease(&target, self.lease(), &command_id, None)
            .await?;
        self.record_command_changed(store, command_id, ChildCommandState::Accepted, None)
            .await
    }

    pub(crate) async fn supersede_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        reason: &str,
    ) -> Result<()> {
        let target = self.as_ref();
        store
            .supersede_child_command_for_lease(&target, self.lease(), &command_id)
            .await?;
        self.record_command_changed(
            store,
            command_id,
            ChildCommandState::Superseded,
            Some(crate::security::sanitize_operator_message(reason)),
        )
        .await
    }

    pub(crate) async fn fail_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        error: &str,
    ) -> Result<()> {
        let error = crate::security::sanitize_operator_message(error);
        let target = self.as_ref();
        store
            .fail_child_command_for_lease(&target, self.lease(), &command_id, None, error.clone())
            .await?;
        self.record_command_changed(store, command_id, ChildCommandState::Failed, Some(error))
            .await
    }

    async fn record_command_changed(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        state: ChildCommandState,
        error: Option<String>,
    ) -> Result<()> {
        match self {
            Self::Project(session_id, lease) => {
                store
                    .append_project_event_for_lease(
                        session_id,
                        lease,
                        &ProjectEventKind::CommandChanged {
                            command_id,
                            state,
                            effect: None,
                            error,
                        },
                    )
                    .await?;
            }
            Self::Task(session_id, lease) => {
                store
                    .append_task_event_for_lease(
                        session_id,
                        lease,
                        &TaskEventKind::CommandChanged {
                            command_id,
                            state,
                            effect: None,
                            error,
                        },
                    )
                    .await?;
            }
        }
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

pub(crate) async fn absorb_commands(
    store: &SharedStore,
    target: ChildTarget<'_>,
    commands: impl IntoIterator<Item = ChildCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    _pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>> {
    for command in commands {
        if !target.command_is_deliverable(store, &command.id).await? {
            continue;
        }
        target.record_claimed(store, command.id.clone()).await?;
        match command.kind {
            ChildCommandKind::Interrupt => {
                if turn_active {
                    target.validate_write_lease(store).await?;
                    if let Err(error) = harness.interrupt().await {
                        target
                            .fail_command(store, command.id, &error.to_string())
                            .await?;
                        return Err(error);
                    }
                }
                target.accept_command(store, command.id).await?;
                return Ok(Some(CommandStop::Interrupted));
            }
            ChildCommandKind::Resume => {
                target.accept_command(store, command.id).await?;
            }
            ChildCommandKind::CiFix { .. } => {
                target
                    .supersede_command(
                        store,
                        command.id,
                        "a live body already owns this PR; the ci-fix wake arrived too late to seed it",
                    )
                    .await?;
            }
            ChildCommandKind::Abandon { reason } => {
                target.accept_command(store, command.id).await?;
                return Ok(Some(CommandStop::Abandoned(reason)));
            }
        }
    }
    Ok(None)
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

pub(crate) async fn reconcile_stale_deliveries(
    store: &SharedStore,
    target: ChildTarget<'_>,
) -> Result<()> {
    let commands = store
        .mark_stale_child_deliveries_uncertain_for_lease(&target.as_ref(), target.lease())
        .await?;
    for command in commands {
        target
            .record_command_changed(
                store,
                command.id,
                ChildCommandState::Uncertain,
                command.error,
            )
            .await?;
    }
    Ok(())
}
