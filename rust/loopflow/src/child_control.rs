//! Provider-neutral control for Project and Task Sessions.
//!
//! Project and Task lifecycle policy stays in their runners. This module owns
//! the part that must not drift between them: command claiming, live steering,
//! interrupt-and-replace, decision delivery, and durable receipt settlement.

use std::collections::VecDeque;

use anyhow::Result;

use crate::child_session::{
    ChildCommand, ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandState,
    ChildDecisionId, ChildRef, ChildWriteLease,
};
use crate::harness::Harness;
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

    async fn begin_delivery(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: ChildCommandEffect,
    ) -> Result<()> {
        let command = store
            .get_child_command(&command_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("child command {command_id} disappeared"))?;
        match command.state {
            ChildCommandState::Claimed => {
                let target = self.as_ref();
                store
                    .mark_child_command_delivering_for_lease(
                        &target,
                        self.lease(),
                        &command_id,
                        effect,
                    )
                    .await?;
                self.record_command_changed(
                    store,
                    command_id,
                    ChildCommandState::Delivering,
                    Some(effect),
                    None,
                )
                .await
            }
            ChildCommandState::Delivering => Ok(()),
            state => anyhow::bail!(
                "child command {} cannot begin provider delivery from {}",
                command.id,
                state.as_str()
            ),
        }
    }

    async fn record_claimed(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> Result<()> {
        self.record_command_changed(store, command_id, ChildCommandState::Claimed, effect, None)
            .await
    }

    async fn accept_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<ChildCommandEffect>,
    ) -> Result<()> {
        let target = self.as_ref();
        store
            .accept_child_command_for_lease(&target, self.lease(), &command_id, effect)
            .await?;
        self.record_command_changed(store, command_id, ChildCommandState::Accepted, effect, None)
            .await
    }

    async fn fail_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<ChildCommandEffect>,
        error: &str,
    ) -> Result<()> {
        let error = crate::security::sanitize_operator_message(error);
        let target = self.as_ref();
        store
            .fail_child_command_for_lease(&target, self.lease(), &command_id, effect, error.clone())
            .await?;
        self.record_command_changed(
            store,
            command_id,
            ChildCommandState::Failed,
            effect,
            Some(error),
        )
        .await
    }

    async fn record_command_changed(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        state: ChildCommandState,
        effect: Option<ChildCommandEffect>,
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
                            effect,
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
                            effect,
                            error,
                        },
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn record_decision(
        self,
        store: &SharedStore,
        decision: DecisionResolution,
    ) -> Result<()> {
        match self {
            Self::Project(session_id, lease) => {
                store
                    .append_project_event_for_lease(
                        session_id,
                        lease,
                        &ProjectEventKind::DecisionResolved {
                            decision_id: decision.decision_id,
                            choice: decision.choice,
                            message: decision.message,
                        },
                    )
                    .await?;
            }
            Self::Task(session_id, lease) => {
                store
                    .append_task_event_for_lease(
                        session_id,
                        lease,
                        &TaskEventKind::DecisionResolved {
                            decision_id: decision.decision_id,
                            choice: decision.choice,
                            message: decision.message,
                        },
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct DecisionResolution {
    pub decision_id: ChildDecisionId,
    pub choice: String,
    pub message: Option<String>,
}

#[derive(Debug)]
pub(crate) struct PendingInput {
    pub command_id: Option<ChildCommandId>,
    pub text: String,
    pub effect: ChildCommandEffect,
    pub decision: Option<DecisionResolution>,
}

impl PendingInput {
    pub fn system(text: String) -> Self {
        Self {
            command_id: None,
            text,
            effect: ChildCommandEffect::NextTurn,
            decision: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandStop {
    Interrupted,
    Abandoned(String),
}

pub(crate) async fn take_current_input(
    store: &SharedStore,
    target: ChildTarget<'_>,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<PendingInput>> {
    while let Some(input) = pending.pop_front() {
        if input_is_current(store, target, &input).await? {
            return Ok(Some(input));
        }
    }
    Ok(None)
}

pub(crate) async fn input_is_current(
    store: &SharedStore,
    target: ChildTarget<'_>,
    input: &PendingInput,
) -> Result<bool> {
    match &input.command_id {
        Some(command_id) => target.command_is_deliverable(store, command_id).await,
        None => Ok(true),
    }
}

pub(crate) async fn absorb_commands(
    store: &SharedStore,
    target: ChildTarget<'_>,
    commands: impl IntoIterator<Item = ChildCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>> {
    for command in commands {
        if !target.command_is_deliverable(store, &command.id).await? {
            continue;
        }
        target
            .record_claimed(store, command.id.clone(), command.effect)
            .await?;
        match command.kind {
            ChildCommandKind::FollowUp { text } => pending.push_back(PendingInput {
                command_id: Some(command.id),
                text,
                effect: ChildCommandEffect::NextTurn,
                decision: None,
            }),
            ChildCommandKind::Steer { text }
                if turn_active && harness.capabilities().supports_steer =>
            {
                apply_input(
                    store,
                    target,
                    harness,
                    PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: ChildCommandEffect::LiveSteer,
                        decision: None,
                    },
                )
                .await?;
            }
            ChildCommandKind::Steer { text } => {
                interrupt_harness(
                    store,
                    target,
                    harness,
                    turn_active,
                    command.id.clone(),
                    Some(ChildCommandEffect::Replacement),
                )
                .await?;
                pending.push_back(PendingInput {
                    command_id: Some(command.id),
                    text,
                    effect: ChildCommandEffect::Replacement,
                    decision: None,
                });
            }
            ChildCommandKind::Interrupt { replacement } => {
                pending.clear();
                interrupt_harness(
                    store,
                    target,
                    harness,
                    turn_active,
                    command.id.clone(),
                    replacement
                        .as_ref()
                        .map(|_| ChildCommandEffect::Replacement),
                )
                .await?;
                if let Some(text) = replacement {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: ChildCommandEffect::Replacement,
                        decision: None,
                    });
                } else {
                    target.accept_command(store, command.id, None).await?;
                    return Ok(Some(CommandStop::Interrupted));
                }
            }
            ChildCommandKind::Resume { message } => {
                if let Some(text) = message {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: ChildCommandEffect::NextTurn,
                        decision: None,
                    });
                } else {
                    target.accept_command(store, command.id, None).await?;
                }
            }
            ChildCommandKind::Decide {
                decision_id,
                choice,
                message,
            } => {
                let resolution = DecisionResolution {
                    decision_id,
                    choice,
                    message,
                };
                let input = PendingInput {
                    command_id: Some(command.id.clone()),
                    text: decision_prompt(&resolution),
                    effect: ChildCommandEffect::Decision,
                    decision: Some(resolution),
                };
                if turn_active && harness.capabilities().supports_steer {
                    apply_input(store, target, harness, input).await?;
                } else {
                    interrupt_harness(
                        store,
                        target,
                        harness,
                        turn_active,
                        command.id,
                        Some(ChildCommandEffect::Decision),
                    )
                    .await?;
                    pending.push_back(input);
                }
            }
            ChildCommandKind::Abandon { reason } => {
                target.accept_command(store, command.id, None).await?;
                return Ok(Some(CommandStop::Abandoned(reason)));
            }
        }
    }
    Ok(None)
}

pub(crate) async fn apply_input(
    store: &SharedStore,
    target: ChildTarget<'_>,
    harness: &mut dyn Harness,
    input: PendingInput,
) -> Result<()> {
    target.validate_write_lease(store).await?;
    if let Some(command_id) = input.command_id.as_ref() {
        target
            .begin_delivery(store, command_id.clone(), input.effect)
            .await?;
    }
    if let Err(error) = harness.send_input(&input.text).await {
        if let Some(command_id) = input.command_id {
            target
                .fail_command(store, command_id, Some(input.effect), &error.to_string())
                .await?;
        }
        return Err(error);
    }
    if let Some(command_id) = input.command_id {
        target
            .accept_command(store, command_id, Some(input.effect))
            .await?;
    }
    if let Some(decision) = input.decision {
        target.record_decision(store, decision).await?;
    }
    Ok(())
}

async fn interrupt_harness(
    store: &SharedStore,
    target: ChildTarget<'_>,
    harness: &mut dyn Harness,
    turn_active: bool,
    command_id: ChildCommandId,
    effect: Option<ChildCommandEffect>,
) -> Result<()> {
    if !turn_active {
        return Ok(());
    }
    if let Some(effect) = effect {
        target
            .begin_delivery(store, command_id.clone(), effect)
            .await?;
    }
    target.validate_write_lease(store).await?;
    if let Err(error) = harness.interrupt().await {
        target
            .fail_command(store, command_id, effect, &error.to_string())
            .await?;
        return Err(error);
    }
    Ok(())
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
                command.effect,
                command.error,
            )
            .await?;
    }
    Ok(())
}

fn decision_prompt(resolution: &DecisionResolution) -> String {
    let message = resolution
        .message
        .as_deref()
        .map(|message| format!("\nFeedback: {message}"))
        .unwrap_or_default();
    format!(
        "Decision {} resolved: {}{}",
        resolution.decision_id, resolution.choice, message
    )
}
