use std::collections::VecDeque;
use std::io::BufRead;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::child_control::{
    absorb_run_control, apply_input as apply_child_input, send_outstanding_steers,
    take_current_input as take_child_input, ChildTarget, CommandStop, PendingInput,
};
use crate::child_session::{
    project_write_lease_from_env, ChildBodyHandoffRequest, ChildBodyOutcome, ChildLeaseState,
    ChildRef, ChildWriteLease,
};
use crate::durable::{Basis, BoundarySeed};
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

#[derive(Debug)]
struct PreparedProjectStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    basis: Basis,
}

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
    let target = ChildRef::Project(session.id.clone());
    let work = store.work_for_child(&target).await?;
    let _run_lease = store.run_lease_for_child(&target, lease).await?;
    let run = store
        .current_run(&work)
        .await?
        .ok_or_else(|| anyhow!("Project Work {} has no active Run", work.id()))?;
    let process = session
        .latest_process
        .as_ref()
        .ok_or_else(|| anyhow!("Project Session {} has no process containment", session.id))?;
    let run_control = crate::trace::ControlLaunch {
        run_id: run.id,
        home_id: run.home_id,
        account_id: None,
        containment: crate::durable::Containment::Tmux {
            name: process.tmux_name.clone(),
        },
        resume_token: session.provider_session_id.clone(),
        opaque_basis: None,
    };
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
    harness.start(&prepared.turn.config).await?;
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
    let capture = flow.current().and_then(|step| {
        let context = crate::journal::trace_capture_context(
            Path::new(wave.repo()),
            Some(step.flow.clone()),
            Some(step.step.clone()),
        )?;
        match crate::trace::CaptureHandle::begin(
            context,
            prepared.turn.context.clone(),
            crate::trace::CaptureStart {
                provider: prepared.turn.harness.clone(),
                model: prepared.turn.model.clone(),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: prepared.turn.context_gather_ms,
                render_ms: prepared.turn.context_render_ms,
                raw_provider: true,
                basis: Some(prepared.basis.clone()),
                control: Some(run_control.clone()),
            },
        ) {
            Ok(capture) => Some(capture),
            Err(error) => {
                tracing::warn!(%error, "failed to establish Project trace capture");
                None
            }
        }
    });
    if let Some(capture) = &capture {
        capture.set_provider_session_id(session.provider_session_id.clone());
    }
    let mut active_basis = prepared.basis.clone();

    let mut pending = VecDeque::new();
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
            None,
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
        "project {}> attached; /status, /interrupt, /detach, or type an instruction",
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
                let active_turn_id = flow_turn_active
                    .then(|| capture.as_ref().map(|capture| capture.current_turn_id()))
                    .flatten();
                if let Some(stop) = absorb_run_control(
                    &store,
                    ChildTarget::Project(&session.id, lease),
                    harness.as_mut(),
                    flow_turn_active,
                    active_turn_id.as_deref(),
                ).await? {
                    return finish_command_stop(&store, &mut session, lease, harness.as_mut(), stop).await;
                }
                if flow_turn_active {
                    if let Some(capture) = &capture {
                        send_outstanding_steers(
                            &store,
                            ChildTarget::Project(&session.id, lease),
                            harness.as_mut(),
                            &capture.current_turn_id(),
                            &active_basis,
                        )
                        .await?;
                    }
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
                if let Some(capture) = &capture {
                    capture.record_conversation(event.clone());
                }
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
                        if let Some(capture) = &capture {
                            let outcome = match status {
                                Lifecycle::Completed => "completed",
                                Lifecycle::Interrupted => "interrupted",
                                Lifecycle::Failed => "failed",
                                _ => "failed",
                            };
                            capture.finish_turn(outcome)?;
                        }
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
                            active_basis = prepared.basis.clone();
                            start_project_flow_turn(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                capture.as_ref(),
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
                        let mut outcome = inspect_outcome(&store, &session, &wave).await?;
                        if status == Lifecycle::Interrupted {
                            outcome.status = ProjectSessionStatus::Waiting;
                            outcome.reason = "Project flow step interrupted; waiting for resume or another instruction".to_string();
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
                            active_basis = prepared.basis.clone();
                            start_project_flow_turn(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                capture.as_ref(),
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            continue;
                        }
                        session.last_state_fingerprint = Some(outcome.fingerprint);
                        store.update_project_session_for_lease(&session, lease).await?;
                        if outcome.status == ProjectSessionStatus::Completed {
                            let work = store
                                .work_for_child(&ChildRef::Project(session.id.clone()))
                                .await?;
                            store.validate_completion_basis(&work, &active_basis).await?;
                        }
                        let stopped = store
                            .stop_project_for_lease(
                                &session.id,
                                lease,
                                outcome.status,
                                outcome.reason.clone(),
                            )
                            .await?;
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
) -> Result<PreparedProjectStep> {
    let work = store
        .work_for_child(&ChildRef::Project(session.id.clone()))
        .await?;
    let boundary = store.boundary_seed(&work).await?;
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
    let seed = project_seed(session, wave.name(), &boundary, observations);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave.name(), None)?;
    prepared.config.agent = Some(session.agent.clone());
    Ok(PreparedProjectStep {
        turn: prepared,
        basis: boundary.basis,
    })
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
    capture: Option<&crate::trace::CaptureHandle>,
    prepared: PreparedProjectStep,
) -> Result<()> {
    let wave = owning_wave(store, session).await?;
    open_project_flow_body(flow, wave.repo())?;
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    apply_input(
        store,
        session,
        lease,
        harness,
        PendingInput::system(prepared.turn.input),
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
    store
        .validate_child_write_lease(&ChildRef::Project(session.id.clone()), lease)
        .await?;
    let target = ChildRef::Project(session.id.clone());
    if line == "/interrupt" {
        let work = store.work_for_child(&target).await?;
        let run = store
            .current_run(&work)
            .await?
            .ok_or_else(|| anyhow!("Project Work {} has no active Run", work.id()))?;
        let request = crate::durable::AuthenticatedRequest::cli();
        let receipt = store
            .interrupt(&crate::durable::ControlCtx::User(&request), &work, &run.id)
            .await?;
        println!("interrupted {}", receipt.run_id);
    } else {
        let work = store.work_for_child(&target).await?;
        let receipt = store
            .append_steer(&work, crate::durable::Author::User, line, None)
            .await?;
        println!("queued {}", receipt.steer.id);
    }
    Ok(())
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
    boundary: &BoundarySeed,
    observations: &[String],
) -> String {
    let observations = if observations.is_empty() {
        "none".to_string()
    } else {
        observations.join("\n")
    };
    let direction = boundary.render();
    format!(
        "Advance Linear Project {name} ({project_id}) in wave/{wave}.\n\n{context}\n\n{direction}\n\nProject Session: {session_id}\nIteration: {iteration}\nPM snapshot synced at: {synced_at}\nSupervised Task observations:\n{observations}\n\nThe runner plays clarify, pursue, and mutate through this same provider session before it checks authoritative Project and Task state. Read and update only this Linear Project through `lf pm`. Create or select concrete Linear tasks, run file-writing work with `lf task run <issue-id>`, and supervise those Task Sessions. Do not edit repository files from the Wave home. Return concise phase evidence; the runner decides complete, wait, repeat, or block after the whole flow.",
        name = session.launch.project.name,
        project_id = session.launch.project.id.as_str(),
        wave = wave_name,
        context = session.launch.project.prompt_context,
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
    #[test]
    fn project_summary_is_bounded() {
        assert_eq!(
            super::bounded_summary(&"x".repeat(2_500)).chars().count(),
            2_000
        );
    }
}
