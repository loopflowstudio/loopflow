use std::collections::{HashSet, VecDeque};
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::child_control::{
    absorb_commands as absorb_child_commands, apply_input as apply_child_input,
    reconcile_stale_deliveries, take_current_input as take_child_input, ChildTarget, CommandStop,
    PendingInput,
};
use crate::child_session::{
    project_write_lease_from_env, unincorporated_directive_version, BoundaryResult,
    ChildBodyHandoffRequest, ChildBodyOutcome, ChildCommand, ChildCommandEffect, ChildCommandId,
    ChildCommandKind, ChildCommandSource, ChildCommandState, ChildDirective, ChildLeaseState,
    ChildRef, ChildWriteLease,
};
use crate::engine::wave_config::read_wave_config;
use crate::harness::{
    classify_disconnect_recovery, default_create_harness, drain_turn_failure_reason,
    ApprovalPolicy, Harness, RecoveryDecision,
};
use crate::project_session::{
    ChildEventPayload, ProjectEventKind, ProjectSession, ProjectSessionId, ProjectSessionStatus,
};
use crate::store::{open_existing_store, SharedStore};
use crate::task::TaskSessionStatus;
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

const TASK_SUPERVISION_INTERVAL: Duration = Duration::from_secs(5);

pub async fn run_project_session(session_id: ProjectSessionId, generation: u32) -> Result<()> {
    let lease = project_write_lease_from_env().map_err(|error| anyhow!(error))?;
    if lease.generation != generation {
        anyhow::bail!(
            "Project generation {generation} does not match its ambient write lease generation {}",
            lease.generation
        );
    }
    let result = run_project_session_inner(session_id.clone(), &lease).await;
    if let Err(error) = &result {
        record_unhandled_failure(&session_id, &lease, error).await;
    }
    result
}

async fn owning_wave(store: &SharedStore, session: &ProjectSession) -> Result<Wave> {
    store
        .get_wave(&session.wave_id)
        .await?
        .ok_or_else(|| anyhow!("owning Wave {} is not registered", session.wave_id))
}

async fn run_project_session_inner(
    session_id: ProjectSessionId,
    lease: &ChildWriteLease,
) -> Result<()> {
    let generation = lease.generation;
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let mut session = store
        .get_project_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Project Session {session_id} not found"))?;
    let wave = owning_wave(&store, &session).await?;
    if session
        .latest_process
        .as_ref()
        .map(|process| process.generation)
        != Some(generation)
    {
        anyhow::bail!("Project Session {session_id} generation {generation} is not current");
    }
    if let Some(process) = &mut session.latest_process {
        process.mark_booted();
    }
    let from = session.status;
    session.set_status(
        ProjectSessionStatus::Running,
        "project pursuit turn is active",
    );
    store.activate_project_process(&session, lease).await?;
    store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::StatusChanged {
                from,
                to: ProjectSessionStatus::Running,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    store
        .append_project_event_for_lease(&session.id, lease, &ProjectEventKind::Started)
        .await?;
    reconcile_stale_deliveries(&store, ChildTarget::Project(&session.id, lease)).await?;

    let observations = consume_task_observations(&store, &mut session, lease).await?;
    let (mut flow, _) = Playhead::new(QueuedInvocation::load(Path::new(wave.repo()), "project")?);
    let prepared =
        prepare_project_flow_step(&store, &mut session, lease, &wave, &flow, &observations).await?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    store
        .validate_child_write_lease(&ChildRef::Project(session.id.clone()), lease)
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
    if let Err(error) = store
        .update_project_session_for_lease(&session, lease)
        .await
    {
        let _ = harness.stop().await;
        return Err(error.into());
    }

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
    if let Some(input) = take_current_input(&store, &session, lease, &mut pending).await? {
        apply_input(&store, &session, lease, harness.as_mut(), input).await?;
    } else {
        start_project_flow_turn(
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
        "project {}> attached; /status, /interrupt [message], /detach, or type an instruction",
        session.launch.project.slug
    );
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut task_supervision = tokio::time::interval(TASK_SUPERVISION_INTERVAL);
    task_supervision.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    let mut turn_had_durable_side_effect = false;
    loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &session, lease, line).await?;
                }
            }
            _ = poll.tick() => {
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
            _ = task_supervision.tick() => {
                if let Err(error) = crate::ops::task::supervise_project_task_bodies(
                    &store,
                    &session,
                ).await {
                    tracing::warn!(
                        project_session = %session.id,
                        error = %error,
                        "could not supervise Task progress leases"
                    );
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(&store, &mut session, lease, harness.as_mut(), "provider event stream closed").await;
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
                    store.update_project_session_for_lease(&session, lease).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnStarted { .. } => {
                        turn_had_durable_side_effect = false;
                    }
                    ConversationEvent::ItemCompleted { item, .. } => {
                        if matches!(
                            item,
                            ConversationItem::Command { .. } | ConversationItem::File { .. }
                        ) {
                            turn_had_durable_side_effect = true;
                        }
                    }
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            let reason = drain_turn_failure_reason(
                                &mut event_rx,
                                "provider turn failed",
                            );
                            return handle_body_failure(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &wave,
                                &reason,
                                turn_had_durable_side_effect,
                            )
                            .await;
                        }
                        if let Err(error) =
                            verify_control_plane_checkout(Path::new(wave.repo()))
                        {
                            return finish_failed(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &error.to_string(),
                            )
                            .await;
                        }
                        let resume_interrupted_flow =
                            flow_turn_active && status == Lifecycle::Interrupted;
                        let flow_iteration_completed = if flow_turn_active {
                            finish_project_flow_turn(&mut flow, status)?
                        } else {
                            false
                        };
                        flow_turn_active = false;
                        if let Some(input) = take_current_input(&store, &session, lease, &mut pending).await? {
                            if resume_interrupted_flow {
                                open_project_flow_body(&mut flow, wave.repo())?;
                                flow_turn_active = true;
                            }
                            apply_input(&store, &session, lease, harness.as_mut(), input).await?;
                            continue;
                        }
                        if status != Lifecycle::Interrupted {
                            let observations =
                                consume_task_observations(&store, &mut session, lease).await?;
                            if !observations.is_empty() {
                                apply_input(
                                    &store,
                                    &session,
                                    lease,
                                    harness.as_mut(),
                                    PendingInput::system(format!(
                                            "New supervised Task observations arrived. Continue the same Project iteration:\n{}",
                                            observations.join("\n")
                                        )),
                                ).await?;
                                continue;
                            }
                        }
                        if !flow_iteration_completed && status != Lifecycle::Interrupted {
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut session,
                                lease,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            start_project_flow_turn(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            continue;
                        }
                        let summary = bounded_summary(&last_text);
                        if flow_iteration_completed {
                            session.iteration += 1;
                            store.append_project_event_for_lease(
                                &session.id,
                                lease,
                                &ProjectEventKind::IterationCompleted {
                                    iteration: session.iteration,
                                    summary: summary.clone(),
                                },
                            ).await?;
                        }
                        let latest = store
                            .get_project_session(&session.id)
                            .await?
                            .ok_or_else(|| anyhow!("Project Session {} disappeared", session.id))?;
                        session.current_directive_version = latest.current_directive_version;
                        session.incorporated_directive_version =
                            latest.incorporated_directive_version;
                        let mut outcome = inspect_outcome(&store, &session, &wave).await?;
                        if status == Lifecycle::Interrupted {
                            outcome.status = ProjectSessionStatus::Waiting;
                            outcome.reason = "Project flow step interrupted; waiting for resume or another instruction".to_string();
                        }
                        if let Some(version) = unincorporated_directive_version(
                            session.current_directive_version,
                            session.incorporated_directive_version,
                        ) {
                            outcome.status = ProjectSessionStatus::Blocked;
                            outcome.reason = format!(
                                "current directive v{version} was applied but not incorporated; resume the Project flow and acknowledge it before settling"
                            );
                        }
                        if outcome.status == ProjectSessionStatus::Running {
                            session.last_state_fingerprint = Some(outcome.fingerprint);
                            session.updated_at = time::OffsetDateTime::now_utc();
                            store.update_project_session_for_lease(&session, lease).await?;
                            last_text.clear();
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut session,
                                lease,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            start_project_flow_turn(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            continue;
                        }
                        session.last_state_fingerprint = Some(outcome.fingerprint);
                        store.update_project_session_for_lease(&session, lease).await?;
                        let boundary = store
                            .claim_project_commands_or_stop_for_lease(
                                &session.id,
                                lease,
                                outcome.status,
                                outcome.reason.clone(),
                            )
                            .await?;
                        let commands = match boundary {
                            BoundaryResult::Commands(commands) => {
                                filter_new_commands(commands, &mut seen_commands)
                            }
                            BoundaryResult::Stopped(stopped) => {
                                let from = session.status;
                                session = stopped;
                                let _ = harness.stop().await;
                                store
                                    .append_project_event_for_lease(
                                        &session.id,
                                        lease,
                                        &ProjectEventKind::StatusChanged {
                                            from,
                                            to: session.status,
                                            reason: session.status_reason.clone(),
                                        },
                                    )
                                    .await?;
                                if session.status == ProjectSessionStatus::Completed {
                                    store
                                        .append_project_event_for_lease(
                                            &session.id,
                                            lease,
                                            &ProjectEventKind::Completed { summary },
                                        )
                                        .await?;
                                }
                                if let Some(process) = &mut session.latest_process {
                                    process.state = ChildLeaseState::Finished;
                                    process.outcome = Some(if session.status == ProjectSessionStatus::Completed {
                                        ChildBodyOutcome::Completed
                                    } else {
                                        ChildBodyOutcome::Interrupted {
                                            reason: session.status_reason.clone(),
                                        }
                                    });
                                }
                                store.finish_project_process(&session, lease).await?;
                                return Ok(());
                            }
                        };
                        if !commands.is_empty() {
                            if let Some(stop) = absorb_commands(
                                &store,
                                &session,
                                lease,
                                commands,
                                harness.as_mut(),
                                false,
                                &mut pending,
                            ).await? {
                                return finish_command_stop(&store, &mut session, lease, harness.as_mut(), stop).await;
                            }
                            if let Some(input) = take_current_input(&store, &session, lease, &mut pending).await? {
                                apply_input(&store, &session, lease, harness.as_mut(), input).await?;
                            } else {
                                let prepared = prepare_project_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    &wave,
                                    &flow,
                                    &[],
                                )
                                .await?;
                                start_project_flow_turn(
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
                            continue;
                        }
                        return Err(anyhow!(
                            "project boundary returned no commands without stopping"
                        ));
                    }
                    ConversationEvent::Error { code, message } => {
                        let reason = format!("{code}: {message}");
                        return handle_body_failure(
                            &store,
                            &mut session,
                            lease,
                            harness.as_mut(),
                            &wave,
                            &reason,
                            turn_had_durable_side_effect,
                        )
                        .await;
                    }
                    ConversationEvent::ItemStarted { .. }
                    | ConversationEvent::ItemUpdated { .. }
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

async fn prepare_project_flow_step(
    store: &SharedStore,
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
    wave: &Wave,
    flow: &Playhead,
    observations: &[String],
) -> Result<crate::lf::commands::run::PreparedHarnessTurn> {
    let latest = store
        .get_project_session(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Project Session {} disappeared", session.id))?;
    session.current_directive_version = latest.current_directive_version;
    session.incorporated_directive_version = latest.incorporated_directive_version;
    let directives = store
        .child_directives(&ChildRef::Project(session.id.clone()))
        .await?;
    let directive = directives
        .iter()
        .find(|directive| directive.version == session.current_directive_version)
        .ok_or_else(|| {
            anyhow!(
                "Project Session {} has no current directive v{}",
                session.id,
                session.current_directive_version
            )
        })?;
    let step = flow
        .current()
        .ok_or_else(|| anyhow!("Project flow has no current step"))?;
    if step.kind != StepKind::Skill {
        anyhow::bail!(
            "Project flow step {} is {:?}; durable Project flows currently require skills",
            step.step,
            step.kind
        );
    }
    session.status_reason = format!(
        "Project flow iteration {}, step {}/{}: {}",
        step.iteration + 1,
        step.index + 1,
        step.total,
        step.step
    );
    store
        .update_project_session_for_lease(session, lease)
        .await?;
    let seed = project_seed(session, wave.name(), directive, observations);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave.name(), None)?;
    prepared.config.agent = Some(session.agent.clone());
    Ok(prepared)
}

fn open_project_flow_body(flow: &mut Playhead, control_repo: &str) -> Result<()> {
    let step = flow
        .current()
        .ok_or_else(|| anyhow!("Project flow has no current step"))?;
    if step.kind != StepKind::Skill {
        anyhow::bail!("Project flow step {} is not a skill", step.step);
    }
    flow.start_body(BodyProvenance::for_step(&step, Path::new(control_repo)))?;
    Ok(())
}

async fn start_project_flow_turn(
    store: &SharedStore,
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    let wave = owning_wave(store, session).await?;
    open_project_flow_body(flow, wave.repo())?;
    apply_input(
        store,
        session,
        lease,
        harness,
        PendingInput {
            command_id: None,
            text: prepared.input,
            effect: ChildCommandEffect::NextTurn,
            decision: None,
        },
    )
    .await?;
    store
        .mark_child_directive_applied_for_lease(
            &ChildRef::Project(session.id.clone()),
            lease,
            session.current_directive_version,
        )
        .await?;
    Ok(())
}

fn finish_project_flow_turn(flow: &mut Playhead, status: Lifecycle) -> Result<bool> {
    let body_id = flow
        .active
        .as_ref()
        .map(|body| body.body_id.clone())
        .ok_or_else(|| anyhow!("Project flow turn completed without an active body"))?;
    let outcome = match status {
        Lifecycle::Completed => StepOutcome::Completed,
        Lifecycle::Interrupted => StepOutcome::Interrupted,
        _ => anyhow::bail!("Project flow turn ended with unexpected status {status:?}"),
    };
    let events = flow.finish_body(&body_id, outcome, status.name())?;
    Ok(events
        .iter()
        .any(|event| matches!(event, PlayheadEvent::InvocationCompleted { .. })))
}

async fn handle_attachment(
    store: &SharedStore,
    session: &ProjectSession,
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
            session.launch.project.slug,
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
        ChildRef::Project(session.id.clone()),
        ChildCommandSource::Attachment,
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
            .get_project_session(&session.id)
            .await?
            .ok_or_else(|| anyhow!("Project Session {} disappeared", session.id))?;
        let directive = ChildDirective::replacement(
            ChildRef::Project(session.id.clone()),
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
            .append_project_event_for_lease(
                &session.id,
                lease,
                &ProjectEventKind::CommandChanged {
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
            .append_project_event_for_lease(
                &session.id,
                lease,
                &ProjectEventKind::DirectiveChanged {
                    directive_id,
                    version,
                    directive_kind,
                },
            )
            .await?;
    }
    store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::CommandChanged {
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

async fn claim_commands(
    store: &SharedStore,
    session: &ProjectSession,
    lease: &ChildWriteLease,
    seen: &mut HashSet<ChildCommandId>,
) -> Result<Vec<ChildCommand>> {
    let commands = store
        .claim_child_commands_for_lease(&ChildRef::Project(session.id.clone()), lease)
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

async fn take_current_input(
    store: &SharedStore,
    session: &ProjectSession,
    lease: &ChildWriteLease,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<PendingInput>> {
    take_child_input(store, ChildTarget::Project(&session.id, lease), pending).await
}

struct ProjectOutcome {
    status: ProjectSessionStatus,
    reason: String,
    fingerprint: String,
}

async fn inspect_outcome(
    store: &SharedStore,
    session: &ProjectSession,
    wave: &Wave,
) -> Result<ProjectOutcome> {
    let repo = wave.repo().to_string();
    let project_id = session.launch.project.id.as_str().to_string();
    let resolved = tokio::task::spawn_blocking(move || {
        crate::ops::task_pm::resolve_project(
            std::path::Path::new(&repo),
            &project_id,
            crate::ops::pm::PmRefresh::Never,
        )
    })
    .await
    .map_err(|error| anyhow!(error.to_string()))??;
    let tasks = crate::ops::task::reconcile_project_tasks(store, session)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let pm_tasks = resolved
        .snapshot
        .items
        .iter()
        .filter(|item| item.project.as_deref() == Some(session.launch.project.slug.as_str()))
        .collect::<Vec<_>>();
    let fingerprint_payload = serde_json::json!({
        "project": resolved.project,
        "pm_tasks": pm_tasks,
        "tasks": tasks.iter().map(|task| (&task.id, task.status, &task.updated_at)).collect::<Vec<_>>(),
    });
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&fingerprint_payload)?));
    if !resolved.project.krs.is_empty() && resolved.project.krs.iter().all(|kr| kr.holds) {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Completed,
            reason: "every current Project KR holds".to_string(),
            fingerprint,
        });
    }
    let mut has_open_pr = false;
    for task in &tasks {
        has_open_pr |= store
            .active_task_pr(&task.id)
            .await?
            .is_some_and(|pr| pr.phase() == crate::task::PrPhase::Open);
    }
    if has_open_pr
        || tasks.iter().any(|task| {
            matches!(
                task.status,
                TaskSessionStatus::Created
                    | TaskSessionStatus::Starting
                    | TaskSessionStatus::Running
            )
        })
    {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Waiting,
            reason: "supervised Tasks are active; waiting for typed Task observations".to_string(),
            fingerprint,
        });
    }
    if session.last_state_fingerprint.as_deref() == Some(&fingerprint) {
        return Ok(ProjectOutcome {
            status: ProjectSessionStatus::Blocked,
            reason: "open KRs remain but a complete iteration changed no PM or Task state"
                .to_string(),
            fingerprint,
        });
    }
    Ok(ProjectOutcome {
        status: ProjectSessionStatus::Running,
        reason: "Project state changed; another iteration is actionable".to_string(),
        fingerprint,
    })
}

fn verify_control_plane_checkout(repo: &Path) -> Result<()> {
    crate::ops::project::ensure_clean_main(repo, "Project turn")
        .map(|_| ())
        .map_err(|error| {
            anyhow!("Project Session violated its read-only control-plane boundary: {error}")
        })
}

async fn consume_task_observations(
    store: &SharedStore,
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
) -> Result<Vec<String>> {
    // The successor consumes the whole project chain: observations addressed to
    // a terminal predecessor the Task was born under are routed here, not
    // stranded on the dead session. The outbox recipient stays the historical
    // owner; this read is the live routing key.
    let observations = store
        .pending_project_observations_for_chain(session.launch.project.id.as_str())
        .await?;
    let mut prompts = Vec::new();
    for observation in observations {
        let event = match &observation.payload {
            ChildEventPayload::Task { event } => event,
            _ => continue,
        };
        let inserted = store
            .consume_task_observation_for_project_for_lease(&session.id, &observation, lease)
            .await?;
        if inserted {
            prompts.push(serde_json::to_string(event)?);
        }
        session.observation_cursor = session.observation_cursor.max(observation.id);
    }
    store
        .update_project_session_for_lease(session, lease)
        .await?;
    Ok(prompts)
}

async fn absorb_commands(
    store: &SharedStore,
    session: &ProjectSession,
    lease: &ChildWriteLease,
    commands: Vec<ChildCommand>,
    harness: &mut dyn Harness,
    turn_active: bool,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<CommandStop>> {
    absorb_child_commands(
        store,
        ChildTarget::Project(&session.id, lease),
        commands,
        harness,
        turn_active,
        pending,
    )
    .await
}

async fn apply_input(
    store: &SharedStore,
    session: &ProjectSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    input: PendingInput,
) -> Result<()> {
    apply_child_input(
        store,
        ChildTarget::Project(&session.id, lease),
        harness,
        input,
    )
    .await
}

async fn set_and_record_status(
    store: &SharedStore,
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
    status: ProjectSessionStatus,
    reason: impl Into<String>,
) -> Result<()> {
    let from = session.status;
    session.set_status(status, reason);
    store
        .update_project_session_for_lease(session, lease)
        .await?;
    store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::StatusChanged {
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
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    error: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    set_and_record_status(store, session, lease, ProjectSessionStatus::Failed, error).await?;
    store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::Failed {
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
    store.finish_project_process(session, lease).await?;
    anyhow::bail!(error.to_string())
}

/// Handle a body failure with disconnect-class recovery: classify the failure,
/// and if it's a disconnect/hollow-body with a configured backup agent, hand
/// the next generation to the backup instead of leaving the body failed for
/// the supervisor to respawn the same flaky provider.
async fn handle_body_failure(
    store: &SharedStore,
    session: &mut ProjectSession,
    lease: &ChildWriteLease,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
) -> Result<()> {
    let wave_config = read_wave_config(Path::new(wave.repo()), wave.name());
    let backup_agent = wave_config.as_ref().and_then(|c| c.backup_agent.as_deref());
    let decision = classify_disconnect_recovery(
        reason,
        &session.agent,
        turn_had_durable_side_effect,
        backup_agent,
    );

    match decision {
        RecoveryDecision::HandoffToBackup { agent, provider } => {
            let _ = harness.stop().await;
            set_and_record_status(store, session, lease, ProjectSessionStatus::Failed, reason)
                .await?;
            store
                .append_project_event_for_lease(
                    &session.id,
                    lease,
                    &ProjectEventKind::Failed {
                        error: reason.to_string(),
                        resumable: true,
                    },
                )
                .await?;
            if let Some(process) = &mut session.latest_process {
                process.state = ChildLeaseState::Finished;
                process.outcome = Some(ChildBodyOutcome::Failed {
                    reason: reason.to_string(),
                });
            }
            store.finish_project_process(session, lease).await?;

            let request = ChildBodyHandoffRequest {
                agent: agent.clone(),
                provider: provider.clone(),
                reason: format!(
                    "disconnect-class failure; handing off from {} to {agent}",
                    session.agent
                ),
            };
            *session = store.handoff_project_body(&session.id, &request).await?;
            Ok(())
        }
        RecoveryDecision::Stop => {
            let non_convergence = format!(
                "{reason}; not replay-safe (durable side effects this turn) and no backup agent configured"
            );
            finish_failed(store, session, lease, harness, &non_convergence).await
        }
        _ => finish_failed(store, session, lease, harness, reason).await,
    }
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut ProjectSession,
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
        ProjectSessionStatus::Abandoned,
        format!("Project Session explicitly abandoned: {reason}"),
    )
    .await?;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Interrupted { reason });
    }
    store.finish_project_process(session, lease).await?;
    Ok(())
}

async fn finish_command_stop(
    store: &SharedStore,
    session: &mut ProjectSession,
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
                ProjectSessionStatus::Waiting,
                "Project turn interrupted; waiting for resume or another instruction",
            )
            .await?;
            if let Some(process) = &mut session.latest_process {
                process.state = ChildLeaseState::Finished;
                process.outcome = Some(ChildBodyOutcome::Interrupted {
                    reason: "Project turn interrupted".to_string(),
                });
            }
            store.finish_project_process(session, lease).await?;
            Ok(())
        }
        CommandStop::Abandoned(reason) => {
            finish_abandoned(store, session, lease, harness, reason).await
        }
    }
}

async fn record_unhandled_failure(
    session_id: &ProjectSessionId,
    lease: &ChildWriteLease,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_project_session(session_id).await else {
        return;
    };
    if session
        .latest_process
        .as_ref()
        .map(|process| process.generation)
        != Some(lease.generation)
        || !session.status.is_process_active()
    {
        return;
    }
    let message = format!("project runner failed: {error}");
    let from = session.status;
    session.set_status(ProjectSessionStatus::Failed, &message);
    if store
        .update_project_session_for_lease(&session, lease)
        .await
        .is_err()
    {
        return;
    }
    let _ = store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::StatusChanged {
                from,
                to: ProjectSessionStatus::Failed,
                reason: message.clone(),
            },
        )
        .await;
    let _ = store
        .append_project_event_for_lease(
            &session.id,
            lease,
            &ProjectEventKind::Failed {
                error: message.clone(),
                resumable: true,
            },
        )
        .await;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Failed { reason: message });
    }
    let _ = store.finish_project_process(&session, lease).await;
}

fn project_seed(
    session: &ProjectSession,
    wave_name: &str,
    directive: &ChildDirective,
    observations: &[String],
) -> String {
    let observations = if observations.is_empty() {
        "none".to_string()
    } else {
        observations.join("\n")
    };
    format!(
        "Advance Linear Project {name} ({project_id}) in wave/{wave}.\n\n{context}\n\nCurrent directive v{directive_version} ({directive_kind}):\n{directive_text}\n\nAcknowledge this direction before continuing with `lf project acknowledge {project_id} --directive {directive_version} --summary \"<how the plan changed>\"`.\n\nProject Session: {session_id}\nIteration: {iteration}\nPM snapshot synced at: {synced_at}\nSupervised Task observations:\n{observations}\n\nThe runner plays clarify, pursue, and mutate through this same provider session before it checks authoritative Project and Task state. Read and update only this Linear Project through `lf pm`. Create or select concrete Linear tasks, run file-writing work with `lf task run <issue-id>`, and supervise those Task Sessions. Do not edit repository files from the Wave home. Return concise phase evidence; the runner decides complete, wait, repeat, or block after the whole flow.",
        name = session.launch.project.name,
        project_id = session.launch.project.id.as_str(),
        wave = wave_name,
        context = session.launch.project.prompt_context,
        directive_version = directive.version,
        directive_kind = directive.kind.as_str(),
        directive_text = directive.text,
        session_id = session.id,
        iteration = session.iteration + 1,
        synced_at = session.launch.pm_snapshot_synced_at,
    )
}

fn bounded_summary(text: &str) -> String {
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
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use time::OffsetDateTime;

    use super::{
        absorb_commands, apply_input, claim_commands, handle_attachment, take_current_input,
        CommandStop,
    };
    use crate::child_session::{
        ChildCommand, ChildCommandEffect, ChildCommandKind, ChildCommandSource, ChildCommandState,
        ChildDecisionId, ChildRef, ChildWriteLease,
    };
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Harness, SendCurrentOutcome};
    use crate::id::WaveId;
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{LinearProjectId, LinearProjectSnapshot, ProjectLaunchReceipt};
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::wave::Wave;

    struct ScriptedHarness {
        accepts_current_send: bool,
        sent: Vec<String>,
        interrupts: usize,
        fail_send: bool,
        unknown_send: bool,
        fail_interrupt: bool,
    }

    impl ScriptedHarness {
        fn new(accepts_current_send: bool) -> Self {
            Self {
                accepts_current_send,
                sent: Vec::new(),
                interrupts: 0,
                fail_send: false,
                unknown_send: false,
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

        async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
            if self.unknown_send {
                return SendCurrentOutcome::Unknown {
                    provider_turn_id: Some("scripted-turn".to_string()),
                    error: "scripted response lost".to_string(),
                };
            }
            if !self.accepts_current_send {
                return SendCurrentOutcome::NotSteerable;
            }
            match self.send_input(content).await {
                Ok(()) => SendCurrentOutcome::Sent {
                    provider_turn_id: "scripted-turn".to_string(),
                },
                Err(error) => SendCurrentOutcome::Failed {
                    error: error.to_string(),
                },
            }
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

        fn provider_session_id(&self) -> Option<String> {
            Some("provider-session".to_string())
        }
    }

    async fn session(provider: &str) -> (SharedStore, ProjectSession, ChildWriteLease) {
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
        let mut session = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: LinearProjectSnapshot {
                    id: LinearProjectId::new(format!("project-{provider}")).unwrap(),
                    slug: "control".to_string(),
                    name: "Control".to_string(),
                    prompt_context: "Provider-neutral control".to_string(),
                },
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
            provider_session_id: Some("provider-session".to_string()),
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&session).await.unwrap();
        session.begin_generation(format!("project-{provider}"));
        let lease = store
            .reserve_project_process(&session, ProjectSessionStatus::Created)
            .await
            .unwrap()
            .unwrap();
        if let Some(process) = &mut session.latest_process {
            process.state = crate::child_session::ChildLeaseState::Active;
        }
        session.set_status(ProjectSessionStatus::Running, "provider active");
        store
            .activate_project_process(&session, &lease)
            .await
            .unwrap();
        (store, session, lease)
    }

    #[tokio::test]
    async fn project_provider_control_reports_honest_steer_effects() {
        for (provider, accepts_current_send, expected_effect) in [
            ("codex", true, ChildCommandEffect::LiveSteer),
            ("claude", false, ChildCommandEffect::NextTurn),
            ("opencode", false, ChildCommandEffect::NextTurn),
        ] {
            let (store, session, lease) = session(provider).await;
            let command = ChildCommand::new(
                ChildRef::Project(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::Steer {
                    text: "change direction".to_string(),
                },
            );
            store.create_child_command(&command).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Project(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(accepts_current_send);
            let mut pending = std::collections::VecDeque::new();

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
                apply_input(&store, &session, &lease, &mut harness, input)
                    .await
                    .unwrap();
            }

            let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
            assert_eq!(receipt.state, ChildCommandState::Accepted, "{provider}");
            assert_eq!(receipt.effect, Some(expected_effect), "{provider}");
            let expected_sends = if accepts_current_send { 2 } else { 1 };
            assert_eq!(harness.sent.len(), expected_sends, "{provider}");
            assert!(
                harness.sent.iter().all(|text| text == "change direction"),
                "{provider}"
            );
            assert_eq!(harness.interrupts, 0, "{provider}");
        }
    }

    #[tokio::test]
    async fn project_runner_delivers_each_command_once_and_skips_superseded_input() {
        let (store, session, lease) = session("codex").await;
        let first = ChildCommand::new(
            ChildRef::Project(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::FollowUp {
                text: "old context".to_string(),
            },
        );
        store.create_child_command(&first).await.unwrap();
        let mut seen = std::collections::HashSet::new();
        let mut pending = std::collections::VecDeque::new();
        let mut harness = ScriptedHarness::new(true);

        let commands = claim_commands(&store, &session, &lease, &mut seen)
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
        assert!(claim_commands(&store, &session, &lease, &mut seen)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(pending.len(), 1);

        let replacement = ChildCommand::new(
            ChildRef::Project(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Steer {
                text: "new direction".to_string(),
            },
        );
        store
            .supersede_and_create_child_command(&replacement)
            .await
            .unwrap();
        let commands = claim_commands(&store, &session, &lease, &mut seen)
            .await
            .unwrap();
        absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            false,
            &mut pending,
        )
        .await
        .unwrap();

        let input = take_current_input(&store, &session, &lease, &mut pending)
            .await
            .unwrap()
            .expect("replacement input");
        assert_eq!(input.command_id.as_ref(), Some(&replacement.id));
        assert_eq!(input.text, "new direction");
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn attached_project_direction_is_versioned_before_provider_input() {
        let (store, session, lease) = session("codex").await;

        handle_attachment(
            &store,
            &session,
            &lease,
            "pursue the parser first".to_string(),
        )
        .await
        .unwrap();

        let current = store
            .get_project_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        let directives = store
            .child_directives(&ChildRef::Project(session.id.clone()))
            .await
            .unwrap();
        assert_eq!(current.current_directive_version, 1);
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].text, "pursue the parser first");
        assert_eq!(directives[0].source, ChildCommandSource::Attachment);
        assert!(directives[0].command_id.is_some());
    }

    #[tokio::test]
    async fn bare_project_interrupt_stops_without_abandoning_history() {
        let (store, session, lease) = session("codex").await;
        let command = ChildCommand::new(
            ChildRef::Project(session.id.clone()),
            ChildCommandSource::Human,
            ChildCommandKind::Interrupt { replacement: None },
        );
        store.create_child_command(&command).await.unwrap();
        let commands = store
            .claim_child_commands(&ChildRef::Project(session.id.clone()), 1)
            .await
            .unwrap();
        let mut harness = ScriptedHarness::new(true);
        let stop = absorb_commands(
            &store,
            &session,
            &lease,
            commands,
            &mut harness,
            true,
            &mut std::collections::VecDeque::new(),
        )
        .await
        .unwrap();

        assert!(matches!(stop, Some(CommandStop::Interrupted)));
        assert_eq!(harness.interrupts, 1);
        assert_eq!(
            store
                .get_child_command(&command.id)
                .await
                .unwrap()
                .unwrap()
                .state,
            ChildCommandState::Accepted
        );
    }

    #[tokio::test]
    async fn project_decisions_resume_every_provider_without_waiting_for_the_blocked_turn() {
        for (provider, accepts_current_send) in
            [("codex", true), ("claude", false), ("opencode", false)]
        {
            let (store, session, lease) = session(provider).await;
            let decision_id = ChildDecisionId::new();
            let command = ChildCommand::new(
                ChildRef::Project(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::Decide {
                    decision_id: decision_id.clone(),
                    choice: "approve".to_string(),
                    message: None,
                },
            );
            store.create_child_command(&command).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Project(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(accepts_current_send);
            let mut pending = std::collections::VecDeque::new();

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
                apply_input(&store, &session, &lease, &mut harness, input)
                    .await
                    .unwrap();
            }

            assert_eq!(harness.interrupts, 0);
            assert_eq!(
                harness.sent,
                vec![
                    format!("Decision {decision_id} resolved: approve");
                    if accepts_current_send { 2 } else { 1 }
                ]
            );
            let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
            assert_eq!(receipt.state, ChildCommandState::Accepted);
            assert_eq!(receipt.effect, Some(ChildCommandEffect::Decision));
            assert!(store
                .project_events_after(&session.id, 0)
                .await
                .unwrap()
                .iter()
                .any(|event| matches!(
                    &event.kind,
                    crate::project_session::ProjectEventKind::DecisionResolved {
                        decision_id: resolved,
                        ..
                    } if resolved == &decision_id
                )));
        }
    }

    #[tokio::test]
    async fn project_follow_up_is_fifo_and_never_interrupts() {
        for provider in ["codex", "claude", "opencode"] {
            let (store, session, lease) = session(provider).await;
            let first = ChildCommand::new(
                ChildRef::Project(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::FollowUp {
                    text: "first".to_string(),
                },
            );
            let second = ChildCommand::new(
                ChildRef::Project(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::FollowUp {
                    text: "second".to_string(),
                },
            );
            store.create_child_command(&first).await.unwrap();
            store.create_child_command(&second).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Project(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(provider == "codex");
            let mut pending = std::collections::VecDeque::new();

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
                apply_input(&store, &session, &lease, &mut harness, input)
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
    async fn unconfirmed_live_project_send_keeps_direction_for_the_next_turn() {
        for unknown in [false, true] {
            let (store, session, lease) = session("codex").await;
            let command = ChildCommand::new(
                ChildRef::Project(session.id.clone()),
                ChildCommandSource::Human,
                ChildCommandKind::Steer {
                    text: "change direction".to_string(),
                },
            );
            store.create_child_command(&command).await.unwrap();
            let commands = store
                .claim_child_commands(&ChildRef::Project(session.id.clone()), 1)
                .await
                .unwrap();
            let mut harness = ScriptedHarness::new(true);
            harness.fail_send = !unknown;
            harness.unknown_send = unknown;
            let mut pending = std::collections::VecDeque::new();

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
            assert_eq!(
                pending.pop_front().map(|input| input.text),
                Some("change direction".to_string())
            );
            let receipt = store.get_child_command(&command.id).await.unwrap().unwrap();
            assert_eq!(receipt.state, ChildCommandState::Delivering);
            assert_eq!(receipt.effect, Some(ChildCommandEffect::LiveSteer));
            assert_eq!(harness.interrupts, 0);
        }
    }

    #[test]
    fn project_summary_is_bounded() {
        assert_eq!(
            super::bounded_summary(&"x".repeat(2_500)).chars().count(),
            2_000
        );
    }
}
