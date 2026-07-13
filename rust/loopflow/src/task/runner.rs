use std::collections::{HashSet, VecDeque};
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::child_control::{
    absorb_commands as absorb_child_commands, apply_input as apply_child_input, input_is_current,
    ChildTarget, CommandStop, DecisionResolution, PendingInput,
};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::task::{
    unincorporated_directive_version, BoundaryResult, ChildDirective, ChildRef, TaskCommand,
    TaskCommandEffect, TaskCommandId, TaskCommandKind, TaskCommandState, TaskEventKind,
    TaskSession, TaskSessionId, TaskSessionStatus,
};
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};

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

    let (mut flow, _) = Playhead::new(QueuedInvocation::load(&session.worktree, "task")?);
    let prepared = prepare_task_flow_step(&store, &mut session, &flow).await?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    harness.start(&prepared.config).await?;
    if let Some(provider_session_id) = harness.provider_session_id() {
        session.provider_session_id = Some(provider_session_id);
    }
    session.provider = harness_name;
    store.update_task_session(&session).await?;
    let mut state_fingerprint = task_state_fingerprint(&session)?;

    let mut pending = VecDeque::new();
    let mut seen_commands = HashSet::new();
    let commands = claim_commands(&store, &session, generation, &mut seen_commands).await?;
    if let Some(stop) = absorb_commands(
        &store,
        &session,
        commands,
        harness.as_mut(),
        false,
        &mut pending,
    )
    .await?
    {
        return finish_command_stop(&store, &mut session, harness.as_mut(), stop).await;
    }
    let mut flow_turn_active = false;
    let mut sent_pending = false;
    while let Some(input) = pending.pop_front() {
        if !pending_input_is_current(&store, &session, &input).await? {
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
        )
        .await?;
        sent_pending = true;
        break;
    }
    if !sent_pending {
        start_task_flow_turn(&store, &mut session, harness.as_mut(), &mut flow, prepared).await?;
        flow_turn_active = true;
    }

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
                if let Some(stop) = absorb_commands(
                    &store,
                    &session,
                    commands,
                    harness.as_mut(),
                    true,
                    &mut pending,
                ).await? {
                    return finish_command_stop(&store, &mut session, harness.as_mut(), stop).await;
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
                        let resume_interrupted_flow =
                            flow_turn_active && status == Lifecycle::Interrupted;
                        let flow_iteration_completed = if flow_turn_active {
                            finish_task_flow_turn(&mut flow, status)?
                        } else {
                            false
                        };
                        flow_turn_active = false;
                        loop {
                            while let Some(input) = pending.pop_front() {
                                if !pending_input_is_current(&store, &session, &input).await? {
                                    continue;
                                }
                                if resume_interrupted_flow {
                                    open_task_flow_body(&mut flow, &session)?;
                                    flow_turn_active = true;
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
                            if !flow_iteration_completed && status != Lifecycle::Interrupted {
                                let prepared =
                                    prepare_task_flow_step(&store, &mut session, &flow).await?;
                                start_task_flow_turn(
                                    &store,
                                    &mut session,
                                    harness.as_mut(),
                                    &mut flow,
                                    prepared,
                                )
                                .await?;
                                flow_turn_active = true;
                                continue 'runner;
                            }
                            let summary = progress_summary(&last_text);
                            let latest = store
                                .get_task_session(&session.id)
                                .await?
                                .ok_or_else(|| anyhow!("Task Session {} disappeared", session.id))?;
                            session.current_directive_version = latest.current_directive_version;
                            session.incorporated_directive_version =
                                latest.incorporated_directive_version;
                            let pending_directive = unincorporated_directive_version(
                                session.current_directive_version,
                                session.incorporated_directive_version,
                            );
                            let observed_pr = crate::ops::current_or_merged_pr(&session.worktree)
                                .ok()
                                .flatten();
                            let (stopped_status, stopped_reason) = if let Some(version) = pending_directive {
                                (
                                    TaskSessionStatus::Blocked,
                                    format!(
                                        "current directive v{version} was applied but not incorporated; resume the Task flow and acknowledge it before settling"
                                    ),
                                )
                            } else if status == Lifecycle::Interrupted {
                                (
                                    TaskSessionStatus::Waiting,
                                    "Task flow step interrupted; waiting for resume or another instruction".to_string(),
                                )
                            } else if let Some(pr) = observed_pr {
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
                                let next_fingerprint = task_state_fingerprint(&session)?;
                                if next_fingerprint != state_fingerprint {
                                    state_fingerprint = next_fingerprint;
                                    session.status_reason =
                                        "Task flow changed the worktree; starting another iteration"
                                            .to_string();
                                    store.update_task_session(&session).await?;
                                    let prepared = prepare_task_flow_step(
                                        &store,
                                        &mut session,
                                        &flow,
                                    )
                                    .await?;
                                    start_task_flow_turn(
                                        &store,
                                        &mut session,
                                        harness.as_mut(),
                                        &mut flow,
                                        prepared,
                                    )
                                    .await?;
                                    flow_turn_active = true;
                                    last_text.clear();
                                    continue 'runner;
                                }
                                (
                                    TaskSessionStatus::Blocked,
                                    "Task flow completed without a PR or any worktree change; another automatic iteration would spin".to_string(),
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
                                    filter_new_commands(commands, &mut seen_commands)
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
                            let resume_requested = boundary_commands.iter().any(|command| {
                                matches!(&command.kind, TaskCommandKind::Resume { .. })
                            });
                            if let Some(stop) = absorb_commands(
                                &store,
                                &session,
                                boundary_commands,
                                harness.as_mut(),
                                false,
                                &mut pending,
                            ).await? {
                                return finish_command_stop(
                                    &store,
                                    &mut session,
                                    harness.as_mut(),
                                    stop,
                                )
                                .await;
                            }
                            if resume_requested && pending.is_empty() {
                                let prepared =
                                    prepare_task_flow_step(&store, &mut session, &flow).await?;
                                start_task_flow_turn(
                                    &store,
                                    &mut session,
                                    harness.as_mut(),
                                    &mut flow,
                                    prepared,
                                )
                                .await?;
                                flow_turn_active = true;
                                continue 'runner;
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

async fn prepare_task_flow_step(
    store: &SharedStore,
    session: &mut TaskSession,
    flow: &Playhead,
) -> Result<crate::lf::commands::run::PreparedHarnessTurn> {
    let latest = store
        .get_task_session(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {} disappeared", session.id))?;
    session.current_directive_version = latest.current_directive_version;
    session.incorporated_directive_version = latest.incorporated_directive_version;
    let directives = store
        .child_directives(&ChildRef::Task(session.id.clone()))
        .await?;
    let directive = directives
        .iter()
        .find(|directive| directive.version == session.current_directive_version)
        .ok_or_else(|| {
            anyhow!(
                "Task Session {} has no current directive v{}",
                session.id,
                session.current_directive_version
            )
        })?;
    let step = flow
        .current()
        .ok_or_else(|| anyhow!("Task flow has no current step"))?;
    if step.kind != StepKind::Skill {
        anyhow::bail!(
            "Task flow step {} is {:?}; durable Task flows currently require skills",
            step.step,
            step.kind
        );
    }
    session.status_reason = format!(
        "Task flow iteration {}, step {}/{}: {}",
        step.iteration + 1,
        step.index + 1,
        step.total,
        step.step
    );
    store.update_task_session(session).await?;
    let seed = task_seed(session, directive);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, &session.wave, None)?;
    prepared.config.agent = Some(session.agent.clone());
    Ok(prepared)
}

fn open_task_flow_body(flow: &mut Playhead, session: &TaskSession) -> Result<()> {
    let step = flow
        .current()
        .ok_or_else(|| anyhow!("Task flow has no current step"))?;
    if step.kind != StepKind::Skill {
        anyhow::bail!("Task flow step {} is not a skill", step.step);
    }
    flow.start_body(BodyProvenance::for_step(&step, &session.worktree))?;
    Ok(())
}

async fn start_task_flow_turn(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    open_task_flow_body(flow, session)?;
    apply_input(store, session, harness, &prepared.input, None, None).await?;
    store
        .mark_child_directive_applied(
            &ChildRef::Task(session.id.clone()),
            session.current_directive_version,
        )
        .await?;
    Ok(())
}

fn finish_task_flow_turn(flow: &mut Playhead, status: Lifecycle) -> Result<bool> {
    let body_id = flow
        .active
        .as_ref()
        .map(|body| body.body_id.clone())
        .ok_or_else(|| anyhow!("Task flow turn completed without an active body"))?;
    let outcome = match status {
        Lifecycle::Completed => StepOutcome::Completed,
        Lifecycle::Interrupted => StepOutcome::Interrupted,
        _ => anyhow::bail!("Task flow turn ended with unexpected status {status:?}"),
    };
    let events = flow.finish_body(&body_id, outcome, status.name())?;
    Ok(events
        .iter()
        .any(|event| matches!(event, PlayheadEvent::InvocationCompleted { .. })))
}

fn task_state_fingerprint(session: &TaskSession) -> Result<String> {
    let state = crate::engine::git::worktree_state(Path::new(&session.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

async fn pending_input_is_current(
    store: &SharedStore,
    session: &TaskSession,
    input: &PendingInput,
) -> Result<bool> {
    input_is_current(store, ChildTarget::Task(session), input).await
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
    let replacement = match &command.kind {
        TaskCommandKind::Steer { text } => Some(text.clone()),
        TaskCommandKind::Interrupt {
            replacement: Some(text),
        } => Some(text.clone()),
        _ => None,
    };
    let (superseded, directive_event) = if let Some(text) = replacement {
        let latest = store
            .get_task_session(&session.id)
            .await?
            .ok_or_else(|| anyhow!("Task Session {} disappeared", session.id))?;
        let directive = ChildDirective::replacement(
            ChildRef::Task(session.id.clone()),
            latest.current_directive_version + 1,
            text,
            command.source.clone(),
            command.id.clone(),
        );
        let superseded = store
            .create_task_command_with_directive(&command, &directive)
            .await?;
        (
            superseded,
            Some((directive.id, directive.version, directive.kind)),
        )
    } else if matches!(&command.kind, TaskCommandKind::Interrupt { .. }) {
        (
            store.supersede_and_create_task_command(&command).await?,
            None,
        )
    } else {
        store.create_task_command(&command).await?;
        (Vec::new(), None)
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
    if let Some((directive_id, version, directive_kind)) = directive_event {
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::DirectiveChanged {
                    directive_id,
                    version,
                    directive_kind,
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
) -> Result<Option<CommandStop>> {
    absorb_child_commands(
        store,
        ChildTarget::Task(session),
        commands,
        harness,
        turn_active,
        pending,
    )
    .await
}

async fn claim_commands(
    store: &SharedStore,
    session: &TaskSession,
    generation: u32,
    seen: &mut HashSet<TaskCommandId>,
) -> Result<Vec<TaskCommand>> {
    let commands = store.claim_task_commands(&session.id, generation).await?;
    Ok(filter_new_commands(commands, seen))
}

fn filter_new_commands(
    commands: Vec<TaskCommand>,
    seen: &mut HashSet<TaskCommandId>,
) -> Vec<TaskCommand> {
    commands
        .into_iter()
        .filter(|command| seen.insert(command.id.clone()))
        .collect()
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
    let (command_id, effect) = command
        .map(|(command_id, effect)| (Some(command_id), effect))
        .unwrap_or((None, TaskCommandEffect::NextTurn));
    apply_child_input(
        store,
        ChildTarget::Task(session),
        harness,
        PendingInput {
            command_id,
            text: text.to_string(),
            effect,
            decision,
        },
    )
    .await
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

async fn finish_command_stop(
    store: &SharedStore,
    session: &mut TaskSession,
    harness: &mut dyn Harness,
    stop: CommandStop,
) -> Result<()> {
    match stop {
        CommandStop::Interrupted => {
            let _ = harness.stop().await;
            set_and_record_status(
                store,
                session,
                TaskSessionStatus::Waiting,
                "Task turn interrupted; waiting for resume or another instruction",
            )
            .await
        }
        CommandStop::Abandoned(reason) => finish_abandoned(store, session, harness, reason).await,
    }
}

fn task_seed(session: &TaskSession, directive: &ChildDirective) -> String {
    let snapshot_warning = session
        .pm_snapshot_warning
        .as_deref()
        .map(|warning| format!("\nPM snapshot warning: {warning}"))
        .unwrap_or_default();
    format!(
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\nCurrent directive v{directive_version} ({directive_kind}):\n{directive_text}\n\nAcknowledge this direction before continuing with `lf task acknowledge {identifier} --directive {directive_version} --summary \"<how the plan changed>\"`.\n\nPM snapshot synced at: {snapshot_synced_at}{snapshot_warning}\nWave: {wave}\nTask Session: {session_id}\nWorktree: {worktree}\nBase commit: {base_commit}\nDelivery: one pull request targeting main. The runner plays clarify, pursue, and mutate through this same provider session, then decides whether the whole Task flow repeats. Opening the PR submits the task; completion is merge or explicit abandonment.",
        identifier = session.issue.identifier,
        title = session.issue.title,
        description = session.issue.description,
        project = session.project.name,
        project_id = session.project.id.as_str(),
        project_context = session.project.context,
        directive_version = directive.version,
        directive_kind = directive.kind.as_str(),
        directive_text = directive.text,
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

    use super::{absorb_commands, apply_input, handle_attachment, progress_summary, CommandStop};
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Capabilities, Harness};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::Wave;
    use crate::lfdb::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        ChildRef, LinearIssueId, LinearIssueRef, LinearProjectId, LinearProjectRef,
        PmWritebackState, TaskCommand, TaskCommandEffect, TaskCommandKind, TaskCommandSource,
        TaskCommandState, TaskDecisionId, TaskEventKind, TaskProcess, TaskSession, TaskSessionId,
        TaskSessionStatus,
    };

    struct ScriptedHarness {
        supports_steer: bool,
        sent: Vec<String>,
        interrupts: usize,
        fail_send: bool,
        fail_interrupt: bool,
    }

    impl ScriptedHarness {
        fn new(supports_steer: bool) -> Self {
            Self {
                supports_steer,
                sent: Vec::new(),
                interrupts: 0,
                fail_send: false,
                fail_interrupt: false,
            }
        }
    }

    #[async_trait]
    impl Harness for ScriptedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> Result<()> {
            if self.fail_send {
                anyhow::bail!("scripted send failed");
            }
            self.sent.push(content.to_string());
            Ok(())
        }

        async fn interrupt(&mut self) -> Result<()> {
            self.interrupts += 1;
            if self.fail_interrupt {
                anyhow::bail!("scripted interrupt failed");
            }
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
            supervisor: crate::project_session::SessionSupervisor::Wave {
                wave_id: wave.id().clone(),
            },
            current_directive_version: 0,
            incorporated_directive_version: 0,
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

    #[tokio::test]
    async fn attached_task_direction_is_versioned_before_delivery() {
        let (store, session) = conformance_session("codex").await;

        handle_attachment(&store, &session, "fix the parser first".to_string())
            .await
            .unwrap();

        let current = store.get_task_session(&session.id).await.unwrap().unwrap();
        let directives = store
            .child_directives(&ChildRef::Task(session.id.clone()))
            .await
            .unwrap();
        assert_eq!(current.current_directive_version, 1);
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].text, "fix the parser first");
        assert_eq!(directives[0].source, TaskCommandSource::Attachment);
        assert!(directives[0].command_id.is_some());
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
            let mut harness = ScriptedHarness::new(supports_steer);
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

    #[tokio::test]
    async fn task_follow_up_is_fifo_and_never_interrupts() {
        for provider in ["codex", "claude", "opencode"] {
            let (store, session) = conformance_session(provider).await;
            let first = TaskCommand::new(
                session.id.clone(),
                TaskCommandSource::Human,
                TaskCommandKind::FollowUp {
                    text: "first".to_string(),
                },
            );
            let second = TaskCommand::new(
                session.id.clone(),
                TaskCommandSource::Human,
                TaskCommandKind::FollowUp {
                    text: "second".to_string(),
                },
            );
            store.create_task_command(&first).await.unwrap();
            store.create_task_command(&second).await.unwrap();
            let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
            let mut harness = ScriptedHarness::new(provider == "codex");
            let mut pending = VecDeque::new();

            absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
                .await
                .unwrap();

            assert_eq!(harness.interrupts, 0, "{provider}");
            assert!(harness.sent.is_empty(), "{provider}");
            for expected in ["first", "second"] {
                let input = pending.pop_front().expect("queued follow-up");
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
                assert_eq!(harness.sent.last().map(String::as_str), Some(expected));
            }
            assert_eq!(
                store
                    .get_task_command(&first.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .effect,
                Some(TaskCommandEffect::NextTurn),
                "{provider}"
            );
        }
    }

    #[tokio::test]
    async fn task_replacement_supersedes_queued_input() {
        let (store, session) = conformance_session("claude").await;
        let first = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::FollowUp {
                text: "A".to_string(),
            },
        );
        let second = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::FollowUp {
                text: "B".to_string(),
            },
        );
        store.create_task_command(&first).await.unwrap();
        store.create_task_command(&second).await.unwrap();
        let mut harness = ScriptedHarness::new(false);
        let mut pending = VecDeque::new();
        let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
        absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
            .await
            .unwrap();

        let replacement = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Interrupt {
                replacement: Some("C".to_string()),
            },
        );
        store
            .supersede_and_create_task_command(&replacement)
            .await
            .unwrap();
        let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
        absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
            .await
            .unwrap();

        let input = pending.pop_front().expect("replacement input");
        assert_eq!(input.command_id.as_ref(), Some(&replacement.id));
        assert_eq!(input.text, "C");
        assert!(pending.is_empty());
        assert_eq!(harness.interrupts, 1);
        for superseded in [&first, &second] {
            assert_eq!(
                store
                    .get_task_command(&superseded.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .state,
                TaskCommandState::Superseded
            );
        }
    }

    #[tokio::test]
    async fn bare_task_interrupt_stops_one_turn_without_abandoning_the_session() {
        let (store, session) = conformance_session("codex").await;
        let command = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Interrupt { replacement: None },
        );
        store.create_task_command(&command).await.unwrap();
        let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
        let mut harness = ScriptedHarness::new(true);
        let mut pending = VecDeque::new();

        let stop = absorb_commands(&store, &session, commands, &mut harness, true, &mut pending)
            .await
            .unwrap();

        assert_eq!(stop, Some(CommandStop::Interrupted));
        assert_eq!(harness.interrupts, 1);
        assert!(pending.is_empty());
        assert_eq!(
            store
                .get_task_command(&command.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            TaskCommandState::Accepted
        );
        assert!(!session.status.is_terminal());
    }

    #[tokio::test]
    async fn task_decisions_resume_every_provider_without_losing_lineage() {
        for (provider, supports_steer) in [("codex", true), ("claude", false), ("opencode", false)]
        {
            let (store, session) = conformance_session(provider).await;
            let decision_id = TaskDecisionId::new();
            let command = TaskCommand::new(
                session.id.clone(),
                TaskCommandSource::Human,
                TaskCommandKind::Decide {
                    decision_id: decision_id.clone(),
                    choice: "revise".to_string(),
                    message: Some("cover the boundary".to_string()),
                },
            );
            store.create_task_command(&command).await.unwrap();
            let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
            let mut harness = ScriptedHarness::new(supports_steer);
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

            assert_eq!(
                harness.interrupts,
                usize::from(!supports_steer),
                "{provider}"
            );
            assert_eq!(
                store
                    .get_task_command(&command.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .effect,
                Some(TaskCommandEffect::Decision),
                "{provider}"
            );
            assert!(
                store
                    .task_events_after(&session.id, 0)
                    .await
                    .unwrap()
                    .iter()
                    .any(|event| matches!(
                        &event.kind,
                        TaskEventKind::DecisionResolved {
                            decision_id: resolved,
                            choice,
                            message: Some(message),
                        } if resolved == &decision_id
                            && choice == "revise"
                            && message == "cover the boundary"
                    )),
                "{provider}"
            );
        }
    }

    #[tokio::test]
    async fn task_provider_control_failures_settle_the_receipt() {
        let (store, session) = conformance_session("claude").await;
        let command = TaskCommand::new(
            session.id.clone(),
            TaskCommandSource::Human,
            TaskCommandKind::Steer {
                text: "change direction".to_string(),
            },
        );
        store.create_task_command(&command).await.unwrap();
        let commands = store.claim_task_commands(&session.id, 1).await.unwrap();
        let mut harness = ScriptedHarness::new(false);
        harness.fail_interrupt = true;

        let error = absorb_commands(
            &store,
            &session,
            commands,
            &mut harness,
            true,
            &mut VecDeque::new(),
        )
        .await
        .expect_err("interrupt failure should fail control");
        assert!(error.to_string().contains("scripted interrupt failed"));
        let receipt = store.get_task_command(&command.id).await.unwrap().unwrap();
        assert_eq!(receipt.state, TaskCommandState::Failed);
        assert_eq!(receipt.effect, Some(TaskCommandEffect::Replacement));
        assert!(receipt
            .error
            .as_deref()
            .is_some_and(|error| error.contains("scripted interrupt failed")));
    }
}
