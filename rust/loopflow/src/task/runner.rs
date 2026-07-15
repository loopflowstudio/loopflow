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
    reconcile_stale_deliveries, ChildTarget, CommandStop, DecisionResolution, PendingInput,
};
use crate::child_session::{
    task_write_lease_from_env, unincorporated_directive_version, BoundaryResult, ChildBodyOutcome,
    ChildCommand, ChildCommandEffect, ChildCommandId, ChildCommandKind, ChildCommandState,
    ChildDirective, ChildLeaseState, ChildRef, ChildWriteLease,
};
use crate::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::store::{open_existing_store, SharedStore};
use crate::task::{PrPhase, TaskEventKind, TaskSession, TaskSessionId, TaskSessionStatus};
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

pub async fn run_task_session(session_id: TaskSessionId, generation: u32) -> Result<()> {
    let lease = task_write_lease_from_env().map_err(|error| anyhow!(error))?;
    if lease.generation != generation {
        anyhow::bail!(
            "Task generation {generation} does not match its ambient write lease generation {}",
            lease.generation
        );
    }
    let result = run_task_session_inner(session_id.clone(), &lease).await;
    if let Err(error) = &result {
        record_unhandled_failure(&session_id, &lease, error).await;
    }
    result
}

async fn owning_wave(store: &SharedStore, session: &TaskSession) -> Result<Wave> {
    store
        .get_wave(&session.wave_id)
        .await?
        .ok_or_else(|| anyhow!("owning Wave {} is not registered", session.wave_id))
}

async fn run_task_session_inner(session_id: TaskSessionId, lease: &ChildWriteLease) -> Result<()> {
    let generation = lease.generation;
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let mut session = store
        .get_task_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {session_id} not found"))?;
    let wave = owning_wave(&store, &session).await?;
    let recorded_generation = session
        .latest_process
        .as_ref()
        .map(|process| process.generation);
    if recorded_generation != Some(generation) {
        anyhow::bail!(
            "Task Session {session_id} generation mismatch: expected {:?}, got {generation}",
            recorded_generation
        );
    }
    if let Some(process) = &mut session.latest_process {
        process.pid = Some(std::process::id());
        process.process_group_id = crate::engine::process::current_process_group_id();
        process.state = ChildLeaseState::Active;
    }
    let from = session.status;
    session.set_status(TaskSessionStatus::Running, "provider turn is active");
    store.activate_task_process(&session, lease).await?;
    store
        .append_task_event_for_lease(
            &session.id,
            lease,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Running,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    store
        .append_task_event_for_lease(&session.id, lease, &TaskEventKind::Started)
        .await?;
    reconcile_stale_deliveries(&store, ChildTarget::Task(&session.id, lease)).await?;

    let (mut flow, _) = Playhead::resume_root(
        QueuedInvocation::load(&session.worktree, &session.resolved_flow)?,
        session.flow_cursor,
        session.flow_iteration,
    )?;
    let prepared = prepare_task_flow_step(&store, &mut session, lease, wave.name(), &flow).await?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    store
        .validate_child_write_lease(&ChildRef::Task(session.id.clone()), lease)
        .await?;
    harness.start(&prepared.config).await?;
    session.provider = harness_name;
    session.provider_session_id = harness.provider_session_id();
    if let Some(process) = &mut session.latest_process {
        process.observe_provider(
            &session.provider,
            session.provider_session_id.clone(),
            harness.process_group_id(),
        );
    }
    if let Err(error) = store.update_task_session_for_lease(&session, lease).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    let mut state_fingerprint = task_state_fingerprint(&session)?;

    let mut pending = VecDeque::new();
    let mut seen_commands = HashSet::new();
    let commands = claim_commands(&store, &session, lease, &mut seen_commands).await?;
    if let Some(stop) = absorb_commands(
        &store,
        &session,
        lease,
        commands,
        harness.as_mut(),
        false,
        &mut pending,
    )
    .await?
    {
        return finish_command_stop(&store, &mut session, lease, harness.as_mut(), stop).await;
    }
    let mut flow_turn_active = false;
    let mut sent_pending = false;
    while let Some(input) = pending.pop_front() {
        if !pending_input_is_current(&store, &session, lease, &input).await? {
            continue;
        }
        let command = input.command_id.map(|id| (id, input.effect));
        apply_input(
            &store,
            &session,
            lease,
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
        start_task_flow_turn(
            &store,
            &mut session,
            lease,
            harness.as_mut(),
            &mut flow,
            prepared,
        )
        .await?;
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
        session.launch.issue.identifier
    );
    let mut command_poll = tokio::time::interval(Duration::from_millis(200));
    command_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    'runner: loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &session, lease, line).await?;
                }
            }
            _ = command_poll.tick() => {
                let commands = claim_commands(
                    &store,
                    &session,
                    lease,
                    &mut seen_commands,
                ).await?;
                if let Some(stop) = absorb_commands(
                    &store,
                    &session,
                    lease,
                    commands,
                    harness.as_mut(),
                    true,
                    &mut pending,
                ).await? {
                    return finish_command_stop(&store, &mut session, lease, harness.as_mut(), stop).await;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut session,
                        lease,
                        harness.as_mut(),
                        "provider event stream closed",
                    ).await;
                };
                let provider_session_id = harness.provider_session_id();
                if provider_session_id != session.provider_session_id {
                    session.provider_session_id = provider_session_id;
                    if let Some(process) = &mut session.latest_process {
                        process.observe_provider(
                            &session.provider,
                            session.provider_session_id.clone(),
                            harness.process_group_id(),
                        );
                    }
                    store.update_task_session_for_lease(&session, lease).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            return finish_failed(
                                &store,
                                &mut session,
                                lease,
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
                        if flow_turn_active {
                            record_task_flow_position(&mut session, &flow)?;
                            store.update_task_session_for_lease(&session, lease).await?;
                        }
                        flow_turn_active = false;
                        loop {
                            while let Some(input) = pending.pop_front() {
                                if !pending_input_is_current(&store, &session, lease, &input).await? {
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
                                    lease,
                                    harness.as_mut(),
                                    &input.text,
                                    command,
                                    input.decision,
                                ).await?;
                                continue 'runner;
                            }
                            if !flow_iteration_completed && status != Lifecycle::Interrupted {
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    wave.name(),
                                    &flow,
                                )
                                .await?;
                                start_task_flow_turn(
                                    &store,
                                    &mut session,
                                    lease,
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
                            if latest.status.is_terminal() {
                                let _ = harness.stop().await;
                                session = latest;
                                if !summary.is_empty() {
                                    store
                                        .append_task_event_for_lease(
                                            &session.id,
                                            lease,
                                            &TaskEventKind::Progress {
                                                summary: summary.clone(),
                                            },
                                        )
                                        .await?;
                                }
                                if let Some(process) = &mut session.latest_process {
                                    process.state = ChildLeaseState::Finished;
                                    process.outcome = Some(ChildBodyOutcome::Completed);
                                }
                                store.finish_task_process(&session, lease).await?;
                                return Ok(());
                            }
                            session.current_directive_version = latest.current_directive_version;
                            session.incorporated_directive_version =
                                latest.incorporated_directive_version;
                            let pending_directive = unincorporated_directive_version(
                                session.current_directive_version,
                                session.incorporated_directive_version,
                            );
                            let observed_pr = crate::ops::task::reconcile_task_pr_for_lease(
                                &store,
                                &mut session,
                                lease,
                            )
                            .await
                            .map_err(|error| anyhow!(error.to_string()))?;
                            if session.status == TaskSessionStatus::Completed {
                                let _ = harness.stop().await;
                                if !summary.is_empty() {
                                    store
                                        .append_task_event_for_lease(
                                            &session.id,
                                            lease,
                                            &TaskEventKind::Progress {
                                                summary: summary.clone(),
                                            },
                                        )
                                        .await?;
                                }
                                if let Some(process) = &mut session.latest_process {
                                    process.state = ChildLeaseState::Finished;
                                    process.outcome = Some(ChildBodyOutcome::Completed);
                                }
                                store.finish_task_process(&session, lease).await?;
                                return Ok(());
                            }
                            let needs_rotation = if observed_pr
                                .as_ref()
                                .is_some_and(|pr| pr.is_settled())
                            {
                                true
                            } else if observed_pr.is_none() {
                                store.active_task_pr(&session.id).await?.is_none()
                            } else {
                                false
                            };
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
                            } else if needs_rotation {
                                crate::ops::task::ensure_working_pr_for_lease(
                                    &store,
                                    &mut session,
                                    lease,
                                )
                                .await
                                .map_err(|error| anyhow!(error.to_string()))?;
                                session.status_reason =
                                    "Task PR settled; starting the next PR".to_string();
                                store.update_task_session_for_lease(&session, lease).await?;
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    wave.name(),
                                    &flow,
                                )
                                .await?;
                                start_task_flow_turn(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    prepared,
                                )
                                .await?;
                                flow_turn_active = true;
                                last_text.clear();
                                continue 'runner;
                            } else if let Some(pr) = observed_pr
                                .as_ref()
                                .filter(|pr| pr.phase() == PrPhase::Open)
                            {
                                let number = pr
                                    .github()
                                    .expect("open Task PR requires a GitHub receipt")
                                    .number;
                                (
                                    TaskSessionStatus::Waiting,
                                    format!("pull request #{number} is open for review"),
                                )
                            } else {
                                let next_fingerprint = task_state_fingerprint(&session)?;
                                if next_fingerprint != state_fingerprint {
                                    state_fingerprint = next_fingerprint;
                                    session.status_reason =
                                        "Task flow changed the worktree; starting another iteration"
                                            .to_string();
                                    store.update_task_session_for_lease(&session, lease).await?;
                                    let prepared = prepare_task_flow_step(
                                        &store,
                                        &mut session,
                                        lease,
                                        wave.name(),
                                        &flow,
                                    )
                                    .await?;
                                    start_task_flow_turn(
                                        &store,
                                        &mut session,
                                        lease,
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
                            store.update_task_session_for_lease(&session, lease).await?;
                            let boundary = store
                                .claim_task_commands_or_stop_for_lease(
                                    &session.id,
                                    lease,
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
                                    session = stopped;
                                    if !summary.is_empty() {
                                        store.append_task_event_for_lease(
                                            &session.id,
                                            lease,
                                            &TaskEventKind::Progress {
                                                summary: summary.clone(),
                                            },
                                        ).await?;
                                    }
                                    if session.status == TaskSessionStatus::Completed {
                                        store.append_task_event_for_lease(
                                            &session.id,
                                            lease,
                                            &TaskEventKind::Completed { summary },
                                        ).await?;
                                    }
                                    store.append_task_event_for_lease(
                                        &session.id,
                                        lease,
                                        &TaskEventKind::StatusChanged {
                                            from,
                                            to: session.status,
                                            reason: session.status_reason.clone(),
                                        },
                                    ).await?;
                                    if let Some(process) = &mut session.latest_process {
                                        process.state = ChildLeaseState::Finished;
                                        process.outcome = Some(if session.status == TaskSessionStatus::Completed {
                                            ChildBodyOutcome::Completed
                                        } else {
                                            ChildBodyOutcome::Interrupted {
                                                reason: session.status_reason.clone(),
                                            }
                                        });
                                    }
                                    store.finish_task_process(&session, lease).await?;
                                    return Ok(());
                                }
                            };
                            let resume_requested = boundary_commands.iter().any(|command| {
                                matches!(&command.kind, ChildCommandKind::Resume { .. })
                            });
                            if let Some(stop) = absorb_commands(
                                &store,
                                &session,
                                lease,
                                boundary_commands,
                                harness.as_mut(),
                                false,
                                &mut pending,
                            ).await? {
                                return finish_command_stop(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    stop,
                                )
                                .await;
                            }
                            if resume_requested && pending.is_empty() {
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    wave.name(),
                                    &flow,
                                )
                                .await?;
                                start_task_flow_turn(
                                    &store,
                                    &mut session,
                                    lease,
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
                            lease,
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
    lease: &ChildWriteLease,
    wave_name: &str,
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
    store.update_task_session_for_lease(session, lease).await?;
    let pr = store
        .active_task_pr(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {} has no active PR", session.id))?;
    let seed = task_seed(session, &pr, wave_name, directive);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave_name, None)?;
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
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    open_task_flow_body(flow, session)?;
    apply_input(store, session, lease, harness, &prepared.input, None, None).await?;
    store
        .mark_child_directive_applied_for_lease(
            &ChildRef::Task(session.id.clone()),
            lease,
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

fn record_task_flow_position(session: &mut TaskSession, flow: &Playhead) -> Result<()> {
    let root = flow
        .stack
        .first()
        .ok_or_else(|| anyhow!("Task flow has no root invocation"))?;
    if root.flow != session.resolved_flow {
        anyhow::bail!(
            "Task Session {} resolved flow {:?}, but its playhead is {:?}",
            session.id,
            session.resolved_flow,
            root.flow
        );
    }
    session.flow_cursor = root.cursor;
    session.flow_iteration = root.iteration;
    session.updated_at = time::OffsetDateTime::now_utc();
    Ok(())
}

fn task_state_fingerprint(session: &TaskSession) -> Result<String> {
    let state = crate::engine::git::worktree_state(Path::new(&session.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

async fn pending_input_is_current(
    store: &SharedStore,
    session: &TaskSession,
    lease: &ChildWriteLease,
    input: &PendingInput,
) -> Result<bool> {
    input_is_current(store, ChildTarget::Task(&session.id, lease), input).await
}

async fn handle_attachment(
    store: &SharedStore,
    session: &TaskSession,
    lease: &ChildWriteLease,
    line: String,
) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        println!(
            "{}  {}  {}",
            session.launch.issue.identifier,
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
        ChildCommandKind::Interrupt {
            replacement: (!message.is_empty()).then(|| message.to_string()),
        }
    } else {
        ChildCommandKind::Steer {
            text: line.to_string(),
        }
    };
    let command = ChildCommand::new(
        ChildRef::Task(session.id.clone()),
        crate::child_session::ChildCommandSource::Attachment,
        kind,
    );
    let replacement = match &command.kind {
        ChildCommandKind::Steer { text } => Some(text.clone()),
        ChildCommandKind::Interrupt {
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
            .create_child_command_with_directive(&command, &directive)
            .await?;
        (
            superseded,
            Some((directive.id, directive.version, directive.kind)),
        )
    } else if matches!(&command.kind, ChildCommandKind::Interrupt { .. }) {
        (
            store.supersede_and_create_child_command(&command).await?,
            None,
        )
    } else {
        store.create_child_command(&command).await?;
        (Vec::new(), None)
    };
    for command_id in superseded {
        store
            .append_task_event_for_lease(
                &session.id,
                lease,
                &TaskEventKind::CommandChanged {
                    command_id,
                    state: ChildCommandState::Superseded,
                    effect: None,
                    error: None,
                },
            )
            .await?;
    }
    if let Some((directive_id, version, directive_kind)) = directive_event {
        store
            .append_task_event_for_lease(
                &session.id,
                lease,
                &TaskEventKind::DirectiveChanged {
                    directive_id,
                    version,
                    directive_kind,
                },
            )
            .await?;
    }
    store
        .append_task_event_for_lease(
            &session.id,
            lease,
            &TaskEventKind::CommandChanged {
                command_id: command.id.clone(),
                state: ChildCommandState::Persisted,
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
    lease: &ChildWriteLease,
    commands: Vec<ChildCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>> {
    absorb_child_commands(
        store,
        ChildTarget::Task(&session.id, lease),
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
    lease: &ChildWriteLease,
    seen: &mut HashSet<ChildCommandId>,
) -> Result<Vec<ChildCommand>> {
    let commands = store
        .claim_child_commands_for_lease(&ChildRef::Task(session.id.clone()), lease)
        .await?;
    Ok(filter_new_commands(commands, seen))
}

fn filter_new_commands(
    commands: Vec<ChildCommand>,
    seen: &mut HashSet<ChildCommandId>,
) -> Vec<ChildCommand> {
    commands
        .into_iter()
        .filter(|command| seen.insert(command.id.clone()))
        .collect()
}

async fn record_unhandled_failure(
    session_id: &TaskSessionId,
    lease: &ChildWriteLease,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_task_session(session_id).await else {
        return;
    };
    if !session.status.is_process_active()
        || session
            .latest_process
            .as_ref()
            .map(|process| process.generation)
            != Some(lease.generation)
    {
        return;
    }
    let from = session.status;
    let message = format!("task process failed: {error}");
    session.set_status(TaskSessionStatus::Failed, &message);
    if store
        .update_task_session_for_lease(&session, lease)
        .await
        .is_err()
    {
        return;
    }
    let _ = store
        .append_task_event_for_lease(
            &session.id,
            lease,
            &TaskEventKind::StatusChanged {
                from,
                to: TaskSessionStatus::Failed,
                reason: message.clone(),
            },
        )
        .await;
    let _ = store
        .append_task_event_for_lease(
            &session.id,
            lease,
            &TaskEventKind::Failed {
                error: message.clone(),
                resumable: true,
            },
        )
        .await;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Failed { reason: message });
    }
    let _ = store.finish_task_process(&session, lease).await;
}

/// Send `text` to the harness and record the driving command's fate: accepted on
/// success, failed (with the error propagated) otherwise. `command` is `None`
/// for the task seed, which has no command to reconcile.
async fn apply_input(
    store: &SharedStore,
    session: &TaskSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    text: &str,
    command: Option<(ChildCommandId, ChildCommandEffect)>,
    decision: Option<DecisionResolution>,
) -> Result<()> {
    let (command_id, effect) = command
        .map(|(command_id, effect)| (Some(command_id), effect))
        .unwrap_or((None, ChildCommandEffect::NextTurn));
    apply_child_input(
        store,
        ChildTarget::Task(&session.id, lease),
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
    lease: &ChildWriteLease,
    status: TaskSessionStatus,
    reason: impl Into<String>,
) -> Result<()> {
    let from = session.status;
    session.set_status(status, reason);
    store.update_task_session_for_lease(session, lease).await?;
    store
        .append_task_event_for_lease(
            &session.id,
            lease,
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
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    error: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    set_and_record_status(store, session, lease, TaskSessionStatus::Failed, error).await?;
    store
        .append_task_event_for_lease(
            &session.id,
            lease,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Failed {
            reason: error.to_string(),
        });
    }
    store.finish_task_process(session, lease).await?;
    anyhow::bail!(error.to_string())
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    reason: String,
) -> Result<()> {
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    set_and_record_status(
        store,
        session,
        lease,
        TaskSessionStatus::Abandoned,
        format!("Task Session explicitly abandoned: {reason}"),
    )
    .await?;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Interrupted { reason });
    }
    store.finish_task_process(session, lease).await?;
    Ok(())
}

async fn finish_command_stop(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    stop: CommandStop,
) -> Result<()> {
    match stop {
        CommandStop::Interrupted => {
            let _ = harness.stop().await;
            set_and_record_status(
                store,
                session,
                lease,
                TaskSessionStatus::Waiting,
                "Task turn interrupted; waiting for resume or another instruction",
            )
            .await?;
            if let Some(process) = &mut session.latest_process {
                process.state = ChildLeaseState::Finished;
                process.outcome = Some(ChildBodyOutcome::Interrupted {
                    reason: "Task turn interrupted".to_string(),
                });
            }
            store.finish_task_process(session, lease).await?;
            Ok(())
        }
        CommandStop::Abandoned(reason) => {
            finish_abandoned(store, session, lease, harness, reason).await
        }
    }
}

fn task_seed(
    session: &TaskSession,
    pr: &crate::task::TaskPr,
    wave_name: &str,
    directive: &ChildDirective,
) -> String {
    let placement = pr
        .parent_pr_id
        .as_ref()
        .map(|parent| format!("Stack parent PR: {parent} (land the parent first)"))
        .unwrap_or_else(|| "Stack parent PR: none (rooted on main)".to_string());
    format!(
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\nCurrent directive v{directive_version} ({directive_kind}):\n{directive_text}\n\nAcknowledge this direction before continuing with `lf task acknowledge {identifier} --directive {directive_version} --summary \"<how the plan changed>\"`.\n\nPM snapshot synced at: {snapshot_synced_at}\nWave: {wave}\nTask Session: {session_id}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}\n\nThis PR owns one serial branch. Bare `lf pr land --next <slug>` ships it and keeps the Task open; `lf pr land -c` completes the Task after merge. `lf pr abandon` discards only this PR. `lf task complete {identifier} --summary \"...\"` completes clean work that needs no PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying your uncommitted edits forward. The runner owns branch rotation between PRs.",
        identifier = session.launch.issue.identifier,
        title = session.launch.issue.title,
        description = session.launch.issue.description,
        project = session.launch.project.name,
        project_id = session.launch.project.id.as_str(),
        project_context = session.launch.project.prompt_context,
        directive_version = directive.version,
        directive_kind = directive.kind.as_str(),
        directive_text = directive.text,
        snapshot_synced_at = session.launch.pm_snapshot_synced_at,
        wave = wave_name,
        session_id = session.id,
        worktree = session.worktree.display(),
        pr_sequence = pr.sequence,
        pr_branch = pr.branch,
        base_commit = pr.base_commit,
        placement = placement,
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

    use super::{
        absorb_commands, apply_input, handle_attachment, progress_summary, task_seed, CommandStop,
    };
    use crate::child_session::{
        ChildBodyHandoffRequest, ChildCommand, ChildCommandEffect, ChildCommandKind,
        ChildCommandSource, ChildCommandState, ChildDecisionId, ChildDirective, ChildRef,
        ChildWriteLease,
    };
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Capabilities, Harness};
    use crate::id::WaveId;
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        PmWritebackState, TaskEventKind, TaskPr, TaskPrId, TaskSession, TaskSessionId,
        TaskSessionStatus,
    };
    use crate::wave::Wave;

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

    async fn conformance_session(provider: &str) -> (SharedStore, TaskSession, ChildWriteLease) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep().join("registry.db");
        let store = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        let wave = Wave::new(
            WaveId::new(),
            format!("wave-{provider}"),
            "/repo".to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .unwrap();
        let project_snapshot = LinearProjectSnapshot {
            id: LinearProjectId::new(format!("project-{provider}")).unwrap(),
            slug: "control".to_string(),
            name: "Control".to_string(),
            prompt_context: "Provider-neutral control".to_string(),
        };
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: project_snapshot.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: ProjectSessionStatus::Created,
            status_reason: "reserved".to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: provider.to_string(),
            provider: provider.to_string(),
            provider_session_id: None,
            latest_process: None,
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&project).await.unwrap();
        let mut session = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new(format!("issue-{provider}")).unwrap(),
                    identifier: format!("{provider}-123"),
                    title: "Conformance".to_string(),
                    description: "Exercise provider-neutral control".to_string(),
                },
                project: project_snapshot,
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id,
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Waiting,
            status_reason: "ready for provider".to_string(),
            status_at: now,
            worktree: PathBuf::from(format!("/repo.{provider}")),
            workspace_slug: format!("test-{provider}"),
            resolved_flow: "task".to_string(),
            interaction_policy: crate::engine::InteractionPolicy::Require,
            flow_cursor: 0,
            flow_iteration: 0,
            agent: provider.to_string(),
            provider: provider.to_string(),
            provider_session_id: Some("provider-session".to_string()),
            latest_process: None,
            execution: Some(crate::child_session::ChildExecutionContext::for_tests()),
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: format!("test/{provider}"),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
        };
        store.create_task_session(&session, &pr).await.unwrap();
        session.begin_generation(format!("task-{provider}"));
        let lease = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut session.latest_process {
            process.state = crate::child_session::ChildLeaseState::Active;
        }
        session.set_status(TaskSessionStatus::Running, "provider active");
        store.activate_task_process(&session, &lease).await.unwrap();
        (store, session, lease)
    }

    #[test]
    fn progress_summary_bounds_wave_visible_text() {
        let summary = progress_summary(&"x".repeat(2_500));
        assert_eq!(summary.chars().count(), 2_000);
        assert!(summary.ends_with('…'));
    }

    #[tokio::test]
    async fn failed_claude_task_hands_off_to_codex_with_directive_and_active_pr() {
        let (store, mut failed, _lease) = conformance_session("claude").await;
        let session_id = failed.id.clone();
        let worktree = failed.worktree.clone();
        failed.set_status(
            TaskSessionStatus::Failed,
            "Claude quota exhausted after preserving durable state",
        );
        store.update_task_session(&failed).await.unwrap();

        let command = ChildCommand::new(
            ChildRef::Task(session_id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "Keep the existing directive and continue PR2".to_string(),
            },
        );
        let directive = ChildDirective::replacement(
            ChildRef::Task(session_id.clone()),
            1,
            "Keep the existing directive and continue PR2".to_string(),
            command.source.clone(),
            command.id.clone(),
        );
        store
            .create_child_command_with_directive(&command, &directive)
            .await
            .unwrap();
        let active_pr_before = store.active_task_pr(&session_id).await.unwrap().unwrap();

        let request = ChildBodyHandoffRequest {
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            reason: "Claude quota exhausted".to_string(),
        };
        let mut resumed = store
            .handoff_task_body(&session_id, &request)
            .await
            .unwrap();
        let active_pr_after = store.active_task_pr(&session_id).await.unwrap().unwrap();
        let current_directive = store
            .child_directives(&ChildRef::Task(session_id.clone()))
            .await
            .unwrap()
            .into_iter()
            .find(|candidate| candidate.version == resumed.current_directive_version)
            .expect("current directive survives provider death");
        let seed = task_seed(
            &resumed,
            &active_pr_after,
            "wave-claude",
            &current_directive,
        );

        assert_eq!(resumed.id, session_id);
        assert_eq!(resumed.worktree, worktree);
        assert_eq!(resumed.agent, "codex");
        assert_eq!(resumed.provider, "codex");
        assert_eq!(resumed.provider_session_id, None);
        assert_eq!(active_pr_after.id, active_pr_before.id);
        assert_eq!(active_pr_after.branch, active_pr_before.branch);
        assert!(seed.contains("Keep the existing directive and continue PR2"));
        assert!(seed.contains(&active_pr_before.branch));
        assert!(seed.contains(session_id.as_str()));

        assert_eq!(resumed.begin_generation("lf-task-codex".to_string()), 2);
        assert_eq!(resumed.latest_process.unwrap().generation, 2);
        let events = store.task_events_after(&session_id, 0).await.unwrap();
        assert!(matches!(
            events.last().map(|event| &event.kind),
            Some(TaskEventKind::BodyHandedOff { handoff })
                if handoff.from_agent == "claude"
                    && handoff.to_agent == "codex"
                    && handoff.from_provider == "claude"
                    && handoff.to_provider == "codex"
                    && handoff.reason == "Claude quota exhausted"
        ));
    }

    #[tokio::test]
    async fn attached_task_direction_is_versioned_before_provider_input() {
        let (store, session, lease) = conformance_session("codex").await;

        handle_attachment(&store, &session, &lease, "fix the parser first".to_string())
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
        assert_eq!(directives[0].source, ChildCommandSource::Attachment);
        assert!(directives[0].command_id.is_some());
    }

    #[tokio::test]
    async fn provider_control_conformance_reports_honest_steer_effects() {
        for (provider, supports_steer, expected_effect) in [
            ("codex", true, ChildCommandEffect::LiveSteer),
            ("claude", false, ChildCommandEffect::Replacement),
            ("opencode", false, ChildCommandEffect::Replacement),
        ] {
            let (store, session, lease) = conformance_session(provider).await;
            let command = ChildCommand::new(
                ChildRef::Task(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::Steer {
                    text: "change direction".to_string(),
                },
            );
            store.create_child_command(&command).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(supports_steer);
            let mut pending = VecDeque::new();

            absorb_commands(
                &store,
                &session,
                &lease,
                commands,
                &mut harness,
                true,
                &mut pending,
            )
            .await
            .unwrap();
            if let Some(input) = pending.pop_front() {
                apply_input(
                    &store,
                    &session,
                    &lease,
                    &mut harness,
                    &input.text,
                    input.command_id.map(|id| (id, input.effect)),
                    input.decision,
                )
                .await
                .unwrap();
            }

            let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
            assert_eq!(receipt.state, ChildCommandState::Accepted, "{provider}");
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
            let (store, session, lease) = conformance_session(provider).await;
            let first = ChildCommand::new(
                ChildRef::Task(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::FollowUp {
                    text: "first".to_string(),
                },
            );
            let second = ChildCommand::new(
                ChildRef::Task(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::FollowUp {
                    text: "second".to_string(),
                },
            );
            store.create_child_command(&first).await.unwrap();
            store.create_child_command(&second).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(provider == "codex");
            let mut pending = VecDeque::new();

            absorb_commands(
                &store,
                &session,
                &lease,
                commands,
                &mut harness,
                true,
                &mut pending,
            )
            .await
            .unwrap();

            assert_eq!(harness.interrupts, 0, "{provider}");
            assert!(harness.sent.is_empty(), "{provider}");
            for expected in ["first", "second"] {
                let input = pending.pop_front().expect("queued follow-up");
                apply_input(
                    &store,
                    &session,
                    &lease,
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
                    .get_child_command(&first.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .effect,
                Some(ChildCommandEffect::NextTurn),
                "{provider}"
            );
        }
    }

    #[tokio::test]
    async fn task_replacement_supersedes_queued_input() {
        let (store, session, lease) = conformance_session("claude").await;
        let first = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "A".to_string(),
            },
        );
        let second = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "B".to_string(),
            },
        );
        store.create_child_command(&first).await.unwrap();
        store.create_child_command(&second).await.unwrap();
        let mut harness = ScriptedHarness::new(false);
        let mut pending = VecDeque::new();
        let commands = store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
            .await
            .unwrap();
        absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            true,
            &mut pending,
        )
        .await
        .unwrap();

        let replacement = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Interrupt {
                replacement: Some("C".to_string()),
            },
        );
        store
            .supersede_and_create_child_command(&replacement)
            .await
            .unwrap();
        let commands = store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
            .await
            .unwrap();
        absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            true,
            &mut pending,
        )
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
                    .get_child_command(&superseded.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .state,
                ChildCommandState::Superseded
            );
        }
    }

    #[tokio::test]
    async fn bare_task_interrupt_stops_one_turn_without_abandoning_the_session() {
        let (store, session, lease) = conformance_session("codex").await;
        let command = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Interrupt { replacement: None },
        );
        store.create_child_command(&command).await.unwrap();
        let commands = store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
            .await
            .unwrap();
        let mut harness = ScriptedHarness::new(true);
        let mut pending = VecDeque::new();

        let stop = absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            true,
            &mut pending,
        )
        .await
        .unwrap();

        assert_eq!(stop, Some(CommandStop::Interrupted));
        assert_eq!(harness.interrupts, 1);
        assert!(pending.is_empty());
        assert_eq!(
            store
                .get_child_command(&command.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            ChildCommandState::Accepted
        );
        assert!(!session.status.is_terminal());
    }

    #[tokio::test]
    async fn task_decisions_resume_every_provider_without_losing_lineage() {
        for (provider, supports_steer) in [("codex", true), ("claude", false), ("opencode", false)]
        {
            let (store, session, lease) = conformance_session(provider).await;
            let decision_id = ChildDecisionId::new();
            let command = ChildCommand::new(
                ChildRef::Task(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::Decide {
                    decision_id: decision_id.clone(),
                    choice: "revise".to_string(),
                    message: Some("cover the boundary".to_string()),
                },
            );
            store.create_child_command(&command).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(supports_steer);
            let mut pending = VecDeque::new();

            absorb_commands(
                &store,
                &session,
                &lease,
                commands,
                &mut harness,
                true,
                &mut pending,
            )
            .await
            .unwrap();
            if let Some(input) = pending.pop_front() {
                apply_input(
                    &store,
                    &session,
                    &lease,
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
                    .get_child_command(&command.id)
                    .await
                    .unwrap()
                    .unwrap()
                    .effect,
                Some(ChildCommandEffect::Decision),
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
        let (store, session, lease) = conformance_session("claude").await;
        let command = ChildCommand::new(
            ChildRef::Task(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "change direction".to_string(),
            },
        );
        store.create_child_command(&command).await.unwrap();
        let commands = store
            .claim_child_commands(&ChildRef::Task(session.id.clone()), 1)
            .await
            .unwrap();
        let mut harness = ScriptedHarness::new(false);
        harness.fail_interrupt = true;

        let error = absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            true,
            &mut VecDeque::new(),
        )
        .await
        .expect_err("interrupt failure should fail control");
        assert!(error.to_string().contains("scripted interrupt failed"));
        let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
        assert_eq!(receipt.state, ChildCommandState::Failed);
        assert_eq!(receipt.effect, Some(ChildCommandEffect::Replacement));
        assert!(receipt
            .error
            .as_deref()
            .is_some_and(|error| error.contains("scripted interrupt failed")));
    }
}
