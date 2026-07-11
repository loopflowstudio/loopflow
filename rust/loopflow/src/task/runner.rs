use std::collections::{HashSet, VecDeque};
use std::io::BufRead;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::task::{
    BoundaryResult, TaskCommand, TaskCommandEffect, TaskCommandId, TaskCommandKind,
    TaskCommandState, TaskDecisionId, TaskEventKind, TaskSession, TaskSessionId, TaskSessionStatus,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecisionResolution {
    decision_id: TaskDecisionId,
    choice: String,
    message: Option<String>,
}

struct PendingInput {
    command_id: Option<TaskCommandId>,
    text: String,
    effect: TaskCommandEffect,
    decision: Option<DecisionResolution>,
}

pub async fn run_task_session(session_id: TaskSessionId, generation: u32) -> Result<()> {
    let result = run_task_session_inner(session_id.clone(), generation).await;
    if let Err(error) = &result {
        record_unhandled_failure(&session_id, generation, error).await;
    }
    result
}

async fn run_task_session_inner(session_id: TaskSessionId, generation: u32) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let mut session = store
        .get_task_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {session_id} not found"))?;
    let recorded_generation = session.process.as_ref().map(|process| process.generation);
    if recorded_generation != Some(generation) {
        anyhow::bail!(
            "Task Session {session_id} generation mismatch: expected {:?}, got {generation}",
            recorded_generation
        );
    }

    if let Some(process) = &mut session.process {
        process.pid = Some(std::process::id());
    }
    set_and_record_status(
        &store,
        &mut session,
        TaskSessionStatus::Running,
        "provider turn is active",
    )
    .await?;
    store
        .append_task_event(&session.id, &TaskEventKind::Started)
        .await?;

    let seed = task_seed(&session);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn("implement", &seed, &session.wave, None)?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let resuming = generation > 1 || session.provider_session_id.is_some();
    prepared.config.agent = Some(session.agent.clone());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    harness.start(&prepared.config).await?;
    if let Some(provider_session_id) = harness.provider_session_id() {
        session.provider_session_id = Some(provider_session_id);
    }
    session.provider = harness_name;
    store.update_task_session(&session).await?;

    let mut pending = VecDeque::new();
    let mut seen_commands = HashSet::new();
    let commands = claim_commands(&store, &session, generation, &mut seen_commands).await?;
    if let Some(reason) = absorb_commands(
        &store,
        &session,
        commands,
        harness.as_mut(),
        false,
        &mut pending,
    )
    .await?
    {
        return finish_abandoned(&store, &mut session, harness.as_mut(), reason).await;
    }
    let task_seed_input = prepared.input;
    let (mut first_input, mut first_command, mut first_decision) =
        take_first_input(task_seed_input.clone(), resuming, &mut pending);
    if let Some((command_id, _)) = &first_command {
        if !command_is_claimed(&store, command_id).await? {
            first_input = task_seed_input;
            first_command = None;
            first_decision = None;
        }
    }
    apply_input(
        &store,
        &session,
        harness.as_mut(),
        &first_input,
        first_command,
        first_decision,
    )
    .await?;

    let (attachment_tx, mut attachment_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if attachment_tx.send(line).is_err() {
                break;
            }
        }
    });
    println!(
        "task {}> attached; /status, /interrupt [message], /detach, or type an instruction",
        session.issue.identifier
    );
    let mut command_poll = tokio::time::interval(Duration::from_millis(200));
    command_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    'runner: loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &session, line).await?;
                }
            }
            _ = command_poll.tick() => {
                let commands = claim_commands(
                    &store,
                    &session,
                    generation,
                    &mut seen_commands,
                ).await?;
                if let Some(reason) = absorb_commands(
                    &store,
                    &session,
                    commands,
                    harness.as_mut(),
                    true,
                    &mut pending,
                ).await? {
                    return finish_abandoned(&store, &mut session, harness.as_mut(), reason).await;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut session,
                        harness.as_mut(),
                        "provider event stream closed",
                    ).await;
                };
                if session.provider_session_id.is_none() {
                    if let Some(provider_session_id) = harness.provider_session_id() {
                        session.provider_session_id = Some(provider_session_id);
                        store.update_task_session(&session).await?;
                    }
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            return finish_failed(
                                &store,
                                &mut session,
                                harness.as_mut(),
                                "provider turn failed",
                            ).await;
                        }
                        loop {
                            while let Some(input) = pending.pop_front() {
                                if !pending_input_is_current(&store, &input).await? {
                                    continue;
                                }
                                let command = input.command_id.map(|id| (id, input.effect));
                                apply_input(
                                    &store,
                                    &session,
                                    harness.as_mut(),
                                    &input.text,
                                    command,
                                    input.decision,
                                ).await?;
                                continue 'runner;
                            }
                            let summary = progress_summary(&last_text);
                            let observed_pr = crate::ops::current_or_merged_pr(&session.worktree)
                                .ok()
                                .flatten();
                            let (stopped_status, stopped_reason) = if let Some(pr) = observed_pr {
                                let pull_request = crate::task::PullRequestRef {
                                    number: pr.number as u32,
                                    url: pr.url.clone(),
                                };
                                session.pull_request = Some(pull_request);
                                if pr.state == "merged" {
                                    crate::ops::task::reconcile_pm_writeback(&mut session).await;
                                    (
                                        TaskSessionStatus::Merged,
                                        format!("pull request #{} merged", pr.number),
                                    )
                                } else {
                                    (
                                        TaskSessionStatus::Submitted,
                                        format!("pull request #{} is open for review", pr.number),
                                    )
                                }
                            } else {
                                (
                                    TaskSessionStatus::Waiting,
                                    "provider turn completed; Task Session is waiting for review, merge, or another instruction".to_string(),
                                )
                            };
                            // Persist non-status fields while the generation is still active.
                            // The following transaction alone chooses commands or inactivity.
                            store.update_task_session(&session).await?;
                            let boundary = store
                                .claim_task_commands_or_stop(
                                    &session.id,
                                    generation,
                                    stopped_status,
                                    &stopped_reason,
                                )
                                .await?;
                            let boundary_commands = match boundary {
                                BoundaryResult::Commands(commands) => {
                                    record_claimed_commands(
                                        &store,
                                        &session,
                                        commands,
                                        &mut seen_commands,
                                    )
                                    .await?
                                }
                                BoundaryResult::Stopped(stopped) => {
                                    let _ = harness.stop().await;
                                    let from = session.status;
                                    session = *stopped;
                                    if !summary.is_empty() {
                                        store.append_task_event(
                                            &session.id,
                                            &TaskEventKind::Progress {
                                                summary: summary.clone(),
                                            },
                                        ).await?;
                                    }
                                    if let Some(pull_request) = session.pull_request.clone() {
                                        if session.status == TaskSessionStatus::Merged {
                                            store.append_task_event(
                                                &session.id,
                                                &TaskEventKind::Completed {
                                                    pull_request,
                                                    summary,
                                                },
                                            ).await?;
                                        } else {
                                            store.append_task_event(
                                                &session.id,
                                                &TaskEventKind::PullRequestOpened {
                                                    number: pull_request.number,
                                                    url: pull_request.url,
                                                },
                                            ).await?;
                                        }
                                    }
                                    store.append_task_event(
                                        &session.id,
                                        &TaskEventKind::StatusChanged {
                                            from,
                                            to: session.status,
                                            reason: session.status_reason.clone(),
                                        },
                                    ).await?;
                                    return Ok(());
                                }
                            };
                            if let Some(reason) = absorb_commands(
                                &store,
                                &session,
                                boundary_commands,
                                harness.as_mut(),
                                false,
                                &mut pending,
                            ).await? {
                                return finish_abandoned(
                                    &store,
                                    &mut session,
                                    harness.as_mut(),
                                    reason,
                                ).await;
                            }
                        }
                    }
                    ConversationEvent::Error { code, message } => {
                        return finish_failed(
                            &store,
                            &mut session,
                            harness.as_mut(),
                            &format!("{code}: {message}"),
                        ).await;
                    }
                    ConversationEvent::TurnStarted { .. }
                    | ConversationEvent::ItemStarted { .. }
                    | ConversationEvent::ItemUpdated { .. }
                    | ConversationEvent::ItemCompleted { .. }
                    | ConversationEvent::ReasoningDelta { .. }
                    | ConversationEvent::DiffUpdated { .. }
                    | ConversationEvent::TurnUsage { .. }
                    | ConversationEvent::SuggestedActions { .. }
                    | ConversationEvent::StatusChanged { .. } => {}
                }
            }
        }
    }
}

fn take_first_input(
    task_seed: String,
    resuming: bool,
    pending: &mut VecDeque<PendingInput>,
) -> (
    String,
    Option<(TaskCommandId, TaskCommandEffect)>,
    Option<DecisionResolution>,
) {
    if !resuming {
        return (task_seed, None, None);
    }
    pending
        .pop_front()
        .map_or((task_seed, None, None), |input| {
            (
                input.text,
                input
                    .command_id
                    .map(|command_id| (command_id, input.effect)),
                input.decision,
            )
        })
}

async fn pending_input_is_current(store: &SharedStore, input: &PendingInput) -> Result<bool> {
    match &input.command_id {
        Some(command_id) => command_is_claimed(store, command_id).await,
        None => Ok(true),
    }
}

async fn handle_attachment(store: &SharedStore, session: &TaskSession, line: String) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        println!(
            "{}  {}  {}",
            session.issue.identifier,
            session.status.as_str(),
            session.status_reason
        );
        return Ok(());
    }
    if line == "/detach" {
        let _ = std::process::Command::new("tmux")
            .args(["detach-client"])
            .status();
        return Ok(());
    }
    let kind = if let Some(message) = line.strip_prefix("/interrupt") {
        let message = message.trim();
        TaskCommandKind::Interrupt {
            replacement: (!message.is_empty()).then(|| message.to_string()),
        }
    } else {
        TaskCommandKind::Steer {
            text: line.to_string(),
        }
    };
    let command = TaskCommand::new(
        session.id.clone(),
        crate::task::TaskCommandSource::Attachment,
        kind,
    );
    let superseded = if matches!(&command.kind, TaskCommandKind::Interrupt { .. }) {
        store.supersede_and_create_task_command(&command).await?
    } else {
        store.create_task_command(&command).await?;
        Vec::new()
    };
    for command_id in superseded {
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::CommandChanged {
                    command_id,
                    state: TaskCommandState::Superseded,
                    effect: None,
                    error: None,
                },
            )
            .await?;
    }
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::CommandChanged {
                command_id: command.id.clone(),
                state: TaskCommandState::Persisted,
                effect: command.effect,
                error: None,
            },
        )
        .await?;
    println!("queued {}", command.id);
    Ok(())
}

async fn absorb_commands(
    store: &SharedStore,
    session: &TaskSession,
    commands: Vec<TaskCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<String>> {
    for command in commands {
        if !command_is_claimed(store, &command.id).await? {
            continue;
        }
        match command.kind {
            TaskCommandKind::FollowUp { text } => {
                pending.push_back(PendingInput {
                    command_id: Some(command.id),
                    text,
                    effect: TaskCommandEffect::NextTurn,
                    decision: None,
                });
            }
            TaskCommandKind::Steer { text } => {
                if turn_active && harness.capabilities().supports_steer {
                    apply_input(
                        store,
                        session,
                        harness,
                        &text,
                        Some((command.id, TaskCommandEffect::LiveSteer)),
                        None,
                    )
                    .await?;
                } else {
                    if turn_active {
                        if let Err(error) = harness.interrupt().await {
                            record_command_failed(
                                store,
                                session,
                                command.id,
                                Some(TaskCommandEffect::Replacement),
                                &error.to_string(),
                            )
                            .await?;
                            return Err(error);
                        }
                    }
                    record_command_effect(
                        store,
                        session,
                        &command.id,
                        TaskCommandEffect::Replacement,
                    )
                    .await?;
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Replacement,
                        decision: None,
                    });
                }
            }
            TaskCommandKind::Resume {
                message: Some(text),
            } => {
                pending.push_back(PendingInput {
                    command_id: Some(command.id),
                    text,
                    effect: TaskCommandEffect::NextTurn,
                    decision: None,
                });
            }
            TaskCommandKind::Resume { message: None } => {
                record_command_accepted(store, session, command.id, None).await?;
            }
            TaskCommandKind::Interrupt { replacement } => {
                pending.clear();
                if turn_active {
                    if let Err(error) = harness.interrupt().await {
                        record_command_failed(
                            store,
                            session,
                            command.id,
                            replacement.as_ref().map(|_| TaskCommandEffect::Replacement),
                            &error.to_string(),
                        )
                        .await?;
                        return Err(error);
                    }
                }
                if let Some(text) = replacement {
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Replacement,
                        decision: None,
                    });
                } else {
                    record_command_accepted(store, session, command.id, None).await?;
                }
            }
            TaskCommandKind::Decide {
                decision_id,
                choice,
                message,
            } => {
                let resolution = DecisionResolution {
                    decision_id,
                    choice,
                    message,
                };
                let text = decision_prompt(&resolution);
                if turn_active && harness.capabilities().supports_steer {
                    apply_input(
                        store,
                        session,
                        harness,
                        &text,
                        Some((command.id, TaskCommandEffect::Decision)),
                        Some(resolution),
                    )
                    .await?;
                } else {
                    if turn_active {
                        if let Err(error) = harness.interrupt().await {
                            record_command_failed(
                                store,
                                session,
                                command.id,
                                Some(TaskCommandEffect::Decision),
                                &error.to_string(),
                            )
                            .await?;
                            return Err(error);
                        }
                    }
                    pending.push_back(PendingInput {
                        command_id: Some(command.id),
                        text,
                        effect: TaskCommandEffect::Decision,
                        decision: Some(resolution),
                    });
                }
            }
            TaskCommandKind::Abandon { reason } => {
                record_command_accepted(store, session, command.id, None).await?;
                return Ok(Some(reason));
            }
        }
    }
    Ok(None)
}

async fn claim_commands(
    store: &SharedStore,
    session: &TaskSession,
    generation: u32,
    seen: &mut HashSet<TaskCommandId>,
) -> Result<Vec<TaskCommand>> {
    let commands = store.claim_task_commands(&session.id, generation).await?;
    record_claimed_commands(store, session, commands, seen).await
}

async fn record_claimed_commands(
    store: &SharedStore,
    session: &TaskSession,
    commands: Vec<TaskCommand>,
    seen: &mut HashSet<TaskCommandId>,
) -> Result<Vec<TaskCommand>> {
    let commands: Vec<_> = commands
        .into_iter()
        .filter(|command| seen.insert(command.id.clone()))
        .collect();
    for command in &commands {
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::CommandChanged {
                    command_id: command.id.clone(),
                    state: TaskCommandState::Claimed,
                    effect: command.effect,
                    error: None,
                },
            )
            .await?;
    }
    Ok(commands)
}

async fn record_command_effect(
    store: &SharedStore,
    session: &TaskSession,
    command_id: &TaskCommandId,
    effect: TaskCommandEffect,
) -> Result<()> {
    store.set_task_command_effect(command_id, effect).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::CommandChanged {
                command_id: command_id.clone(),
                state: TaskCommandState::Claimed,
                effect: Some(effect),
                error: None,
            },
        )
        .await?;
    Ok(())
}

async fn record_command_failed(
    store: &SharedStore,
    session: &TaskSession,
    command_id: TaskCommandId,
    effect: Option<TaskCommandEffect>,
    error: &str,
) -> Result<()> {
    let error = crate::lfd::redaction::sanitize_operator_message(error);
    store
        .fail_task_command(&command_id, effect, error.clone())
        .await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::CommandChanged {
                command_id,
                state: TaskCommandState::Failed,
                effect,
                error: Some(error),
            },
        )
        .await?;
    Ok(())
}

async fn record_unhandled_failure(
    session_id: &TaskSessionId,
    generation: u32,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_task_session(session_id).await else {
        return;
    };
    if !session.status.is_process_active()
        || session.process.as_ref().map(|process| process.generation) != Some(generation)
    {
        return;
    }
    let from = session.status;
    let message = format!("task process failed: {error}");
    session.set_status(TaskSessionStatus::Failed, &message);
    if store.update_task_session(&session).await.is_err() {
        return;
    }
    let _ = store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: message.clone(),
            },
        )
        .await;
    let _ = store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error: message.clone(),
                resumable: true,
            },
        )
        .await;
}

async fn record_command_accepted(
    store: &SharedStore,
    session: &TaskSession,
    command_id: TaskCommandId,
    effect: Option<TaskCommandEffect>,
) -> Result<()> {
    store.accept_task_command(&command_id, effect).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::CommandChanged {
                command_id,
                state: TaskCommandState::Accepted,
                effect,
                error: None,
            },
        )
        .await?;
    Ok(())
}

/// Send `text` to the harness and record the driving command's fate: accepted on
/// success, failed (with the error propagated) otherwise. `command` is `None`
/// for the task seed, which has no command to reconcile.
async fn apply_input(
    store: &SharedStore,
    session: &TaskSession,
    harness: &mut dyn Harness,
    text: &str,
    command: Option<(TaskCommandId, TaskCommandEffect)>,
    decision: Option<DecisionResolution>,
) -> Result<()> {
    if let Err(error) = harness.send_input(text).await {
        if let Some((command_id, effect)) = command {
            record_command_failed(store, session, command_id, Some(effect), &error.to_string())
                .await?;
        }
        return Err(error);
    }
    if let Some((command_id, effect)) = command {
        record_command_accepted(store, session, command_id, Some(effect)).await?;
    }
    if let Some(decision) = decision {
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

async fn command_is_claimed(store: &SharedStore, command_id: &TaskCommandId) -> Result<bool> {
    Ok(store
        .get_task_command(command_id)
        .await?
        .is_some_and(|command| command.state == TaskCommandState::Claimed))
}

/// Apply a status transition and persist it: set the status, update the row, and
/// append the paired `StatusChanged` event.
async fn set_and_record_status(
    store: &SharedStore,
    session: &mut TaskSession,
    status: TaskSessionStatus,
    reason: impl Into<String>,
) -> Result<()> {
    let from = session.status;
    session.set_status(status, reason);
    store.update_task_session(session).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::StatusChanged {
                from,
                to: status,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    Ok(())
}

async fn finish_failed(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    error: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    set_and_record_status(store, session, TaskSessionStatus::Failed, error).await?;
    store
        .append_task_event(
            &session.id,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
    anyhow::bail!(error.to_string())
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    reason: String,
) -> Result<()> {
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    set_and_record_status(
        store,
        session,
        TaskSessionStatus::Abandoned,
        format!("Task Session explicitly abandoned: {reason}"),
    )
    .await
}

fn task_seed(session: &TaskSession) -> String {
    let snapshot_warning = session
        .pm_snapshot_warning
        .as_deref()
        .map(|warning| format!("\nPM snapshot warning: {warning}"))
        .unwrap_or_default();
    format!(
        "Implement Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\nPM snapshot synced at: {snapshot_synced_at}{snapshot_warning}\nWave: {wave}\nTask Session: {session_id}\nWorktree: {worktree}\nBase commit: {base_commit}\nDelivery: one pull request targeting main. Opening the PR submits the task; completion is merge or explicit abandonment.",
        identifier = session.issue.identifier,
        title = session.issue.title,
        description = session.issue.description,
        project = session.project.name,
        project_id = session.project.id.as_str(),
        project_context = session.project.context,
        snapshot_synced_at = session.pm_snapshot_synced_at,
        wave = session.wave,
        session_id = session.id,
        worktree = session.worktree.display(),
        base_commit = session.base_commit,
    )
}

fn progress_summary(text: &str) -> String {
    const MAX_CHARS: usize = 2_000;
    let text = text.trim();
    if text.chars().count() <= MAX_CHARS {
        return text.to_string();
    }
    let mut summary: String = text.chars().take(MAX_CHARS - 1).collect();
    summary.push('…');
    summary
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use time::OffsetDateTime;

    use super::{absorb_commands, apply_input, progress_summary, take_first_input, PendingInput};
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Capabilities, Harness};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::Wave;
    use crate::lfdb::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        LinearIssueId, LinearIssueRef, LinearProjectId, LinearProjectRef, PmWritebackState,
        TaskCommand, TaskCommandEffect, TaskCommandId, TaskCommandKind, TaskCommandSource,
        TaskCommandState, TaskProcess, TaskSession, TaskSessionId, TaskSessionStatus,
    };

    struct ScriptedHarness {
        supports_steer: bool,
        sent: Vec<String>,
        interrupts: usize,
    }

    #[async_trait]
    impl Harness for ScriptedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            self.sent.push(content.to_string());
            Ok(())
        }

        async fn interrupt(&mut self) -> Result<()> {
            self.interrupts += 1;
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: self.supports_steer,
            }
        }

        fn provider_session_id(&self) -> Option<String> {
            Some("provider-session".to_string())
        }
    }

    async fn conformance_session(provider: &str) -> (SharedStore, TaskSession) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep().join("registry.db");
        let store = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        let wave = Wave::new(
            LfdId::new(),
            format!("wave-{provider}"),
            "/repo".to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .unwrap();
        let session = TaskSession {
            id: TaskSessionId::new(),
            issue: LinearIssueRef {
                id: LinearIssueId::new(format!("issue-{provider}")).unwrap(),
                identifier: format!("{provider}-123"),
                title: "Conformance".to_string(),
                description: "Exercise provider-neutral control".to_string(),
            },
            project: LinearProjectRef {
                id: LinearProjectId::new(format!("project-{provider}")).unwrap(),
                slug: "control".to_string(),
                name: "Control".to_string(),
                context: "Provider-neutral control".to_string(),
            },
            pm_snapshot_synced_at: now.unix_timestamp(),
            pm_snapshot_warning: None,
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            wave: wave.name().clone(),
            status: TaskSessionStatus::Running,
            status_reason: "provider active".to_string(),
            status_at: now,
            worktree: PathBuf::from(format!("/repo.{provider}")),
            branch: format!("test/{provider}"),
            base_commit: "deadbeef".to_string(),
            agent: provider.to_string(),
            provider: provider.to_string(),
            provider_session_id: Some("provider-session".to_string()),
            process: Some(TaskProcess {
                generation: 1,
                pid: None,
                tmux_name: format!("task-{provider}"),
                started_at: now,
            }),
            pull_request: None,
            created_at: now,
            updated_at: now,
        };
        store.create_task_session(&session).await.unwrap();
        (store, session)
    }

    #[test]
    fn progress_summary_bounds_wave_visible_text() {
        let summary = progress_summary(&"x".repeat(2_500));
        assert_eq!(summary.chars().count(), 2_000);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn fresh_session_keeps_task_seed_ahead_of_racing_commands() {
        let command_id = TaskCommandId::new();
        let mut pending = VecDeque::from([PendingInput {
            command_id: Some(command_id.clone()),
            text: "rename the flag".to_string(),
            effect: TaskCommandEffect::Replacement,
            decision: None,
        }]);

        let first = take_first_input("implement INF-123".to_string(), false, &mut pending);

        assert_eq!(first, ("implement INF-123".to_string(), None, None));
        assert_eq!(
            pending.front().and_then(|input| input.command_id.clone()),
            Some(command_id)
        );
    }

    #[test]
    fn resumed_session_starts_with_the_oldest_queued_command() {
        let command_id = TaskCommandId::new();
        let mut pending = VecDeque::from([PendingInput {
            command_id: Some(command_id.clone()),
            text: "address review".to_string(),
            effect: TaskCommandEffect::NextTurn,
            decision: None,
        }]);

        let first = take_first_input("original seed".to_string(), true, &mut pending);

        assert_eq!(
            first,
            (
                "address review".to_string(),
                Some((command_id, TaskCommandEffect::NextTurn)),
                None,
            )
        );
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn provider_control_conformance_reports_honest_steer_effects() {
        for (provider, supports_steer, expected_effect) in [
            ("codex", true, TaskCommandEffect::LiveSteer),
            ("claude", false, TaskCommandEffect::Replacement),
            ("opencode", false, TaskCommandEffect::Replacement),
        ] {
            let (store, session) = conformance_session(provider).await;
            let command = TaskCommand::new(
                session.id.clone(),
                TaskCommandSource::Human,
                TaskCommandKind::Steer {
                    text: "change direction".to_string(),
                },
            );
            store.create_task_command(&command).await.unwrap();
            let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
            let mut harness = ScriptedHarness {
                supports_steer,
                sent: Vec::new(),
                interrupts: 0,
            };
            let mut pending = VecDeque::new();

            absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
                .await
                .unwrap();
            if let Some(input) = pending.pop_front() {
                apply_input(
                    &store,
                    &session,
                    &mut harness,
                    &input.text,
                    input.command_id.map(|id| (id, input.effect)),
                    input.decision,
                )
                .await
                .unwrap();
            }

            let receipt = store.get_task_command(&command.id).await.unwrap().unwrap();
            assert_eq!(receipt.state, TaskCommandState::Accepted, "{provider}");
            assert_eq!(receipt.effect, Some(expected_effect), "{provider}");
            assert_eq!(harness.sent, vec!["change direction"], "{provider}");
            assert_eq!(
                harness.interrupts,
                usize::from(!supports_steer),
                "{provider}"
            );
        }
    }
}
