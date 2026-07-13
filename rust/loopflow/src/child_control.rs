//! Provider-neutral control for Project and Task Sessions.
//!
//! Project and Task lifecycle policy stays in their runners. This module owns
//! the part that must not drift between them: command claiming, live steering,
//! interrupt-and-replace, decision delivery, and durable receipt settlement.

use std::collections::VecDeque;

use anyhow::Result;

use crate::harness::Harness;
use crate::lfdb::SharedStore;
use crate::project_session::{ProjectCommand, ProjectEventKind, ProjectSession};
use crate::task::{
    ChildCommandId, ChildCommandKind, ChildDecisionId, TaskCommand, TaskCommandEffect,
    TaskCommandState, TaskEventKind, TaskSession,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ChildTarget<'a> {
    Project(&'a ProjectSession),
    Task(&'a TaskSession),
}

impl ChildTarget<'_> {
    async fn command_is_claimed(
        self,
        store: &SharedStore,
        command_id: &ChildCommandId,
    ) -> Result<bool> {
        let claimed = match self {
            Self::Project(_) => store
                .get_project_command(command_id)
                .await?
                .is_some_and(|command| command.state == TaskCommandState::Claimed),
            Self::Task(_) => store
                .get_task_command(command_id)
                .await?
                .is_some_and(|command| command.state == TaskCommandState::Claimed),
        };
        Ok(claimed)
    }

    async fn record_claimed(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<TaskCommandEffect>,
    ) -> Result<()> {
        self.record_command_changed(store, command_id, TaskCommandState::Claimed, effect, None)
            .await
    }

    async fn accept_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<TaskCommandEffect>,
    ) -> Result<()> {
        match self {
            Self::Project(_) => store.accept_project_command(&command_id, effect).await?,
            Self::Task(_) => store.accept_task_command(&command_id, effect).await?,
        }
        self.record_command_changed(store, command_id, TaskCommandState::Accepted, effect, None)
            .await
    }

    async fn fail_command(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        effect: Option<TaskCommandEffect>,
        error: &str,
    ) -> Result<()> {
        let error = crate::lfd::redaction::sanitize_operator_message(error);
        match self {
            Self::Project(_) => {
                store
                    .fail_project_command(&command_id, effect, error.clone())
                    .await?
            }
            Self::Task(_) => {
                store
                    .fail_task_command(&command_id, effect, error.clone())
                    .await?
            }
        }
        self.record_command_changed(
            store,
            command_id,
            TaskCommandState::Failed,
            effect,
            Some(error),
        )
        .await
    }

    async fn record_command_changed(
        self,
        store: &SharedStore,
        command_id: ChildCommandId,
        state: TaskCommandState,
        effect: Option<TaskCommandEffect>,
        error: Option<String>,
    ) -> Result<()> {
        match self {
            Self::Project(session) => {
                store
                    .append_project_event(
                        &session.id,
                        &ProjectEventKind::CommandChanged {
                            command_id,
                            state,
                            effect,
                            error,
                        },
                    )
                    .await?;
            }
            Self::Task(session) => {
                store
                    .append_task_event(
                        &session.id,
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
            Self::Project(session) => {
                store
                    .append_project_event(
                        &session.id,
                        &ProjectEventKind::DecisionResolved {
                            decision_id: decision.decision_id,
                            choice: decision.choice,
                            message: decision.message,
                        },
                    )
                    .await?;
            }
            Self::Task(session) => {
                store
                    .append_task_event(
                        &session.id,
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
pub(crate) struct ChildCommand {
    id: ChildCommandId,
    kind: ChildCommandKind,
    effect: Option<TaskCommandEffect>,
}

impl From<TaskCommand> for ChildCommand {
    fn from(command: TaskCommand) -> Self {
        Self {
            id: command.id,
            kind: command.kind,
            effect: command.effect,
        }
    }
}

impl From<ProjectCommand> for ChildCommand {
    fn from(command: ProjectCommand) -> Self {
        Self {
            id: command.id,
            kind: command.kind,
            effect: command.effect,
        }
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
    pub effect: TaskCommandEffect,
    pub decision: Option<DecisionResolution>,
}

impl PendingInput {
    pub fn system(text: String) -> Self {
        Self {
            command_id: None,
            text,
            effect: TaskCommandEffect::NextTurn,
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
        Some(command_id) => target.command_is_claimed(store, command_id).await,
        None => Ok(true),
    }
}

pub(crate) async fn absorb_commands<C>(
    store: &SharedStore,
    target: ChildTarget<'_>,
    commands: impl IntoIterator<Item = C>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>>
where
    C: Into<ChildCommand>,
{
    for command in commands {
        let command = command.into();
        if !target.command_is_claimed(store, &command.id).await? {
            continue;
        }
        target
            .record_claimed(store, command.id.clone(), command.effect)
            .await?;
        match command.kind {
            ChildCommandKind::FollowUp { text } => pending.push_back(PendingInput {
                command_id: Some(command.id),
                text,
                effect: TaskCommandEffect::NextTurn,
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
                        effect: TaskCommandEffect::LiveSteer,
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
                    Some(TaskCommandEffect::Replacement),
                )
                .await?;
                pending.push_back(PendingInput {
                    command_id: Some(command.id),
                    text,
                    effect: TaskCommandEffect::Replacement,
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
                    replacement.as_ref().map(|_| TaskCommandEffect::Replacement),
                )
                .await?;
                if let Some(text) = replacement {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Replacement,
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
                        effect: TaskCommandEffect::NextTurn,
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
                    effect: TaskCommandEffect::Decision,
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
                        Some(TaskCommandEffect::Decision),
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
    effect: Option<TaskCommandEffect>,
) -> Result<()> {
    if !turn_active {
        return Ok(());
    }
    if let Err(error) = harness.interrupt().await {
        target
            .fail_command(store, command_id, effect, &error.to_string())
            .await?;
        return Err(error);
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
