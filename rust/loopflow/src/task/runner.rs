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
    absorb_run_control, apply_input as apply_child_input, input_is_current,
    send_outstanding_steers, CommandStop, PendingInput,
};
use crate::child_session::{ChildBodyHandoff, ChildBodyOutcome, ChildLeaseState, ChildRef};
use crate::durable::{AttentionRoute, Basis, BoundarySeed, FlowPosition, RunLease};
use crate::engine::wave_config::read_wave_config;
use crate::engine::InteractionPolicy;
use crate::harness::{
    classify_disconnect_recovery, drain_turn_failure_reason, ApprovalPolicy, Harness,
    RecoveryDecision,
};
use crate::store::{open_existing_store, SharedStore};
use crate::task::{
    CiCheck, Observation, PrPhase, TaskEventKind, TaskGateProposal, TaskLifecyclePhase,
    TaskSession, TaskSessionId, TaskSessionStatus,
};
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

#[derive(Debug)]
struct PreparedTaskStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    attention: Option<AttentionRoute>,
    position: FlowPosition,
    basis: Basis,
}

#[derive(Debug)]
struct StartedTaskStep {
    feedback: bool,
    provider_turn_active: bool,
    basis: Option<Basis>,
}

pub async fn run_task_session(session_id: TaskSessionId) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let lease = crate::ops::required_run_lease(&store).await?;
    let result = run_task_session_with(
        store,
        session_id.clone(),
        &lease,
        Box::new(crate::harness::default_create_harness),
    )
    .await;
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

async fn spawn_failover(
    store: &SharedStore,
    session: &TaskSession,
    lease: &RunLease,
    wave: &Wave,
) -> Result<()> {
    let tmux_name = session
        .latest_process
        .as_ref()
        .map(|process| process.tmux_name.clone())
        .ok_or_else(|| anyhow!("Task failover has no reserved Launch containment"))?;
    let wave_home =
        crate::engine::wave_config::read_wave_home(Path::new(wave.repo()), wave.name()).to_string();
    crate::ops::launch_in_run(
        store,
        lease,
        crate::ops::RunLaunch {
            kind: "task",
            legacy_id: session.id.to_string(),
            wave_id: session.wave_id.clone(),
            cwd: session.worktree.clone(),
            tmux_name,
            agent: session.agent.clone(),
            resume_token: session.provider_session_id.clone(),
            wave_home,
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| anyhow!(error.to_string()))
}

async fn run_task_session_with(
    store: SharedStore,
    session_id: TaskSessionId,
    lease: &RunLease,
    create_harness: crate::harness::CreateHarness,
) -> Result<()> {
    let mut session = store
        .get_task_session(&session_id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {session_id} not found"))?;
    let wave = owning_wave(&store, &session).await?;
    if let Some(process) = &mut session.latest_process {
        process.mark_booted();
    }
    let from = session.status;
    session.set_status(TaskSessionStatus::Running, "provider turn is active");
    store.activate_task_process_for_run(&session, lease).await?;
    store
        .append_task_event_for_run(
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
        .append_task_event_for_run(&session.id, lease, &TaskEventKind::Started)
        .await?;
    let target = ChildRef::Task(session.id.clone());
    let work = store.work_for_child(&target).await?;
    if lease.work != work {
        anyhow::bail!("ambient Run lease does not own Task Work {}", work.id());
    }
    let run = store
        .current_run(&work)
        .await?
        .ok_or_else(|| anyhow!("Task Work {} has no active Run", work.id()))?;
    let launch = store
        .current_launch(lease)
        .await?
        .ok_or_else(|| anyhow!("Task Run {} has no current Launch", lease.run_id))?;
    let crate::durable::AdvanceReceipt::Launch(launch) = store
        .advance_run(
            lease,
            crate::durable::RunAdvance::LaunchLive {
                launch_id: launch.id,
            },
        )
        .await?
    else {
        unreachable!("LaunchLive returns a Launch receipt")
    };
    let run_control = crate::trace::ControlLaunch {
        run_id: run.id,
        home_id: run.home_id,
        account_id: launch.route.account_id.clone(),
        containment: launch.containment.clone(),
        resume_token: launch.resume_token.clone(),
        opaque_basis: launch.opaque_basis.clone(),
    };
    // Typed current-head evidence selects ci-fix before ordinary lifecycle work.
    // The exact Run claim is the crash/recovery fence; no command row mediates it.
    let mut ci_fix_wake = arm_ci_fix_wake(&store, &session, lease).await?;
    let mut flow = if ci_fix_wake.is_some() {
        Playhead::new(QueuedInvocation::load(&session.worktree, "ci-fix")?).0
    } else {
        resume_task_phase(&session)?
    };
    let prepared = prepare_task_flow_step(
        &store,
        &mut session,
        lease,
        wave.name(),
        &flow,
        ci_fix_wake.as_ref(),
    )
    .await?;
    let (harness_name, _) = crate::engine::config::parse_agent(&session.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(session.provider_session_id.clone());
    store.validate_run_lease(lease).await?;
    harness.start(&prepared.turn.config).await?;
    session.provider = harness_name;
    session.provider_session_id = harness.provider_session_id();
    store
        .observe_launch_provider(lease, &launch.id, session.provider_session_id.clone())
        .await?;
    if let Some(process) = &mut session.latest_process {
        process.observe_provider(
            &session.provider,
            session.provider_session_id.clone(),
            harness.process_group_id(),
        );
    }
    if let Err(error) = store.update_task_session_for_run(&session, lease).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    let mut state_fingerprint = task_state_fingerprint(&session)?;
    let mut iteration_start_head = pr_head_for_session(&store, &session).await?;
    let mut gate_fingerprint = if session.lifecycle_phase == TaskLifecyclePhase::Gate {
        Some(task_gate_fingerprint(&session)?)
    } else {
        None
    };

    let mut pending = VecDeque::new();
    let mut feedback_open = prepared.attention.is_some();
    // Record this body's turns the way `flowloop/wave.rs` does. Without it a
    // Task Session's spend reaches no store at all: the provider runs in this
    // process, so no child `lf` records on its behalf.
    let capture = flow.current().and_then(|step| {
        let context = crate::journal::trace_capture_context(
            Path::new(&session.worktree),
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
                // Spend telemetry must never take a Task body down.
                tracing::warn!(%error, "failed to establish Task trace capture");
                None
            }
        }
    });
    if let Some(capture) = &capture {
        capture.set_provider_session_id(session.provider_session_id.clone());
    }
    store
        .set_flow_position(lease, prepared.position.clone())
        .await?;
    if let Some(attention) = prepared.attention.clone() {
        let capture = capture
            .as_ref()
            .ok_or_else(|| anyhow!("interactive Task step requires an observable active Launch"))?;
        store
            .route_feedback(lease, &capture.launch_id(), attention)
            .await?;
    }
    let mut active_basis = prepared.basis.clone();
    let mut flow_turn_active = false;
    let mut provider_turn_active =
        apply_next_pending(&store, &session, lease, harness.as_mut(), &mut pending).await?;
    if !provider_turn_active {
        start_task_flow_turn(
            &store,
            &mut session,
            lease,
            harness.as_mut(),
            &mut flow,
            prepared.turn,
        )
        .await?;
        flow_turn_active = true;
        provider_turn_active = true;
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
        "task {}> attached; /status, /interrupt, /detach, or type a message/instruction",
        session.launch.issue.identifier
    );
    let mut command_poll = tokio::time::interval(Duration::from_millis(200));
    command_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_text = String::new();
    let mut turn_had_durable_side_effect = false;
    // One preempt per provider turn; cleared with `provider_turn_active`.
    let mut feedback_preempted = false;
    'runner: loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &session, lease, line).await?;
                }
            }
            _ = command_poll.tick() => {
                let active_turn_id = provider_turn_active
                    .then(|| capture.as_ref().map(|capture| capture.current_turn_id()))
                    .flatten();
                if let Some(stop) = absorb_run_control(
                    &store,
                    lease,
                    harness.as_mut(),
                    provider_turn_active,
                    active_turn_id.as_deref(),
                ).await? {
                    return finish_command_stop(
                        &store,
                        &mut session,
                        lease,
                        harness.as_mut(),
                        stop,
                        capture.as_ref(),
                    ).await;
                }
                let wake = if provider_turn_active {
                    if ci_fix_wake.is_none()
                        && !feedback_preempted
                        && feedback_open
                        && current_ci_incident_identity(&store, &session).await?.is_some()
                    {
                        harness.interrupt().await?;
                        feedback_preempted = true;
                    }
                    None
                } else if ci_fix_wake.is_none() {
                    arm_ci_fix_wake(&store, &session, lease).await?
                } else {
                    None
                };
                if let Some(wake) = wake {
                    active_basis = start_ci_fix_flow(
                        &store,
                        &mut session,
                        lease,
                        harness.as_mut(),
                        &mut flow,
                        &wake,
                        capture.as_ref(),
                    ).await?;
                    ci_fix_wake = Some(wake);
                    // The bounded repair owns this body's exit. The durable Gate
                    // feedback stays open for the next Task generation.
                    feedback_open = false;
                    flow_turn_active = true;
                    provider_turn_active = true;
                    last_text.clear();
                }
                if provider_turn_active {
                    if let Some(capture) = &capture {
                        send_outstanding_steers(
                            &store,
                            lease,
                            harness.as_mut(),
                            &capture.current_turn_id(),
                            &active_basis,
                        )
                        .await?;
                    }
                }
                if !provider_turn_active {
                    provider_turn_active = apply_next_pending(
                        &store,
                        &session,
                        lease,
                        harness.as_mut(),
                        &mut pending,
                    ).await?;
                }
                if feedback_open
                    && !provider_turn_active
                    && store.feedback(&work).await?.is_none()
                {
                    let boundary = store.boundary_seed(&work).await?;
                    let close = "Feedback continued at the current Basis. \
        Finish this step from the conversation already conducted.";
                    if let Some(capture) = &capture {
                        capture.begin_turn_at("queued", close, Some(boundary.basis.clone()))?;
                    }
                    apply_input(&store, &session, lease, harness.as_mut(), close).await?;
                    active_basis = boundary.basis;
                    provider_turn_active = true;
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
                        capture.as_ref(),
                    ).await;
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
                    store.update_task_session_for_run(&session, lease).await?;
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
                        provider_turn_active = false;
                        feedback_preempted = false;
                        if status == Lifecycle::Failed {
                            let reason = drain_turn_failure_reason(
                                &mut event_rx,
                                "provider turn failed",
                            );
                            return fail_and_maybe_relaunch(
                                &store,
                                &mut session,
                                lease,
                                harness.as_mut(),
                                &wave,
                                &reason,
                                turn_had_durable_side_effect,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        if ci_fix_wake.is_none() {
                            let wake = arm_ci_fix_wake(&store, &session, lease).await?;
                            if let Some(wake) = wake {
                                active_basis = start_ci_fix_flow(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    &wake,
                                    capture.as_ref(),
                                ).await?;
                                ci_fix_wake = Some(wake);
                                // The repair takes the just-released provider
                                // boundary before Gate or lifecycle progression.
                                // Its durable feedback remains open for recovery.
                                feedback_open = false;
                                flow_turn_active = true;
                                provider_turn_active = true;
                                last_text.clear();
                            }
                            if provider_turn_active {
                                continue 'runner;
                            }
                            provider_turn_active = apply_next_pending(
                                &store,
                                &session,
                                lease,
                                harness.as_mut(),
                                &mut pending,
                            ).await?;
                            if provider_turn_active {
                                continue 'runner;
                            }
                        }
                        let resume_interrupted_flow =
                            flow_turn_active && status == Lifecycle::Interrupted;
                        let feedback_body_completed = feedback_open
                            && store.feedback(&work).await?.is_none();
                        if feedback_open && !feedback_body_completed {
                            // The provider boundary ended, not the interactive
                            // flow interval. A later Steer starts another Turn;
                            // only continue_feedback advances the playhead.
                            flow_turn_active = false;
                            last_text.clear();
                            continue 'runner;
                        }
                        let mut flow_iteration_completed = if flow_turn_active {
                            finish_task_flow_turn(&mut flow, status)?
                        } else if feedback_body_completed {
                            feedback_open = false;
                            finish_task_flow_turn(&mut flow, Lifecycle::Completed)?
                        } else {
                            false
                        };
                        if flow_turn_active || feedback_body_completed {
                            let latest = store
                                .get_task_session(&session.id)
                                .await?
                                .ok_or_else(|| {
                                    anyhow!("Task Session {} disappeared", session.id)
                                })?;
                            sync_terminal_task_state(&mut session, &latest);
                            if ci_fix_wake.is_none() {
                                record_task_flow_position(&mut session, &flow)?;
                            }
                            store.update_task_session_for_run(&session, lease).await?;
                        }
                        if session.status == TaskSessionStatus::Abandoned {
                            let _ = harness.stop().await;
                            if let Some(process) = &mut session.latest_process {
                                process.state = ChildLeaseState::Finished;
                                process.outcome = Some(ChildBodyOutcome::Interrupted {
                                    reason: session.status_reason.clone(),
                                });
                            }
                            store.finish_task_process_for_run(&session, lease).await?;
                            return Ok(());
                        }
                        // A ci-fix body is one bounded turn, and this is its exit.
                        // Standing above the lifecycle loop rather than at its
                        // tail is the whole fix, and every path below may assume
                        // no repair body is live — `settle_ci_fix_turn` documents
                        // why, and why a new lifecycle path belongs below it.
                        if let Some(wake) = ci_fix_wake.as_ref() {
                            // The flow ended, or the turn was cut short. Anything
                            // else is a repair still mid-flow.
                            if flow_iteration_completed || status == Lifecycle::Interrupted {
                                // Settlement judges head advancement against the
                                // authoritative remote head: a `Fresh` reconcile
                                // bypasses both the store observation cache and
                                // gh's HTTP cache, so the head the repair body
                                // just pushed is what settlement reads — never a
                                // warm pre-turn observation.
                                let observed_pr =
                                    crate::ops::task::reconcile_task_pr_fresh_for_run(
                                        &store,
                                        &mut session,
                                        lease,
                                    )
                                    .await
                                    .map_err(|error| anyhow!(error.to_string()))?;
                                let _ = harness.stop().await;
                                return settle_ci_fix_turn(
                                    &store,
                                    &mut session,
                                    lease,
                                    wake,
                                    observed_pr.as_ref(),
                                    iteration_start_head.as_deref(),
                                    status,
                                    capture.as_ref(),
                                )
                                .await;
                            }
                        }
                        flow_turn_active = false;
                        if flow_iteration_completed
                                && session.lifecycle_phase == TaskLifecyclePhase::Kickoff
                            {
                                let reason =
                                    "Task kickoff closed; autonomous iteration is starting";
                                session.enter_iterate()?;
                                session.set_status(TaskSessionStatus::Running, reason);
                                store.update_task_session_for_run(&session, lease).await?;
                                flow = resume_task_phase(&session)?;
                                flow_iteration_completed = false;
                                state_fingerprint = task_state_fingerprint(&session)?;
                                gate_fingerprint = None;
                                last_text.clear();
                            }
                            while let Some(input) = pending.pop_front() {
                                if !pending_input_is_current(&store, &session, lease, &input).await? {
                                    continue;
                                }
                                if resume_interrupted_flow {
                                    open_task_flow_body(&mut flow, &session)?;
                                    flow_turn_active = true;
                                }
                                apply_input(
                                    &store,
                                    &session,
                                    lease,
                                    harness.as_mut(),
                                    &input.text,
                                ).await?;
                                provider_turn_active = true;
                                continue 'runner;
                            }
                            if feedback_open {
                                last_text.clear();
                                continue 'runner;
                            }
                            let approved_gate = if flow_iteration_completed
                                && session.lifecycle_phase == TaskLifecyclePhase::Gate
                            {
                                let next_gate_fingerprint = task_gate_fingerprint(&session)?;
                                if gate_fingerprint.as_ref() != Some(&next_gate_fingerprint) {
                                    state_fingerprint = task_state_fingerprint(&session)?;
                                    gate_fingerprint = None;
                                    session.enter_iterate()?;
                                    session.set_status(
                                        TaskSessionStatus::Running,
                                        "Task gate requested changes; returning to iteration",
                                    );
                                    store.update_task_session_for_run(&session, lease).await?;
                                    let started = start_resumed_task_phase(
                                        &store,
                                        &mut session,
                                        lease,
                                        harness.as_mut(),
                                        &mut flow,
                                        wave.name(),
                                        capture.as_ref(),
                                    )
                                    .await?;
                                    if let Some(basis) = &started.basis {
                                        active_basis = basis.clone();
                                    }
                                    feedback_open = started.feedback;
                                    flow_turn_active = true;
                                    provider_turn_active = started.provider_turn_active;
                                    last_text.clear();
                                    continue 'runner;
                                }
                                Some(session.approved_gate_proposal()?)
                            } else {
                                None
                            };
                            if !flow_iteration_completed && status != Lifecycle::Interrupted {
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    wave.name(),
                                    &flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?;
                                let started = start_prepared_task_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    capture.as_ref(),
                                    prepared,
                                )
                                .await?;
                                if let Some(basis) = &started.basis {
                                    active_basis = basis.clone();
                                }
                                feedback_open = started.feedback;
                                flow_turn_active = true;
                                provider_turn_active = started.provider_turn_active;
                                continue 'runner;
                            }
                            let summary = progress_summary(&last_text);
                            let latest = store
                                .get_task_session(&session.id)
                                .await?
                                .ok_or_else(|| anyhow!("Task Session {} disappeared", session.id))?;
                            sync_terminal_task_state(&mut session, &latest);
                            let observed_pr = crate::ops::task::reconcile_task_pr_for_run(
                                &store,
                                &mut session,
                                lease,
                            )
                            .await
                            .map_err(|error| anyhow!(error.to_string()))?;
                            // Reconcile keeps the cached PR row through a GitHub
                            // outage and names the failure on the session. For a
                            // turn that just ran, that degraded reading is an
                            // infrastructure blocker: it could not have verified
                            // a repair.
                            let github_degraded = match &session.observation {
                                Observation::Degraded { reason, .. } => Some(reason.clone()),
                                _ => None,
                            };
                            // The head before this turn is the baseline for the
                            // no-change check; the head we just observed becomes
                            // the baseline for the next turn.
                            let head_before_turn = iteration_start_head;
                            iteration_start_head = observed_pr
                                .as_ref()
                                .and_then(|pr| pr.head_sha().map(str::to_string));
                            // Merged, not merely settled: the branch below reports
                            // this PR as merged and waits on its review gate, which
                            // is false of an abandoned one.
                            let merged_completing_pr = observed_pr.as_ref().is_some_and(|pr| {
                                pr.phase() == crate::task::PrPhase::Merged
                                    && pr
                                        .publication
                                        .as_ref()
                                        .is_some_and(|publication| {
                                            publication.after_merge
                                                == crate::task::AfterMerge::CompleteTask
                                        })
                            });
                            let needs_rotation = if merged_completing_pr {
                                // A completing PR settles the Task, never rotates to a next PR.
                                false
                            } else if observed_pr
                                .as_ref()
                                .is_some_and(|pr| pr.is_settled())
                            {
                                true
                            } else if observed_pr.is_none() {
                                store.active_task_pr(&session.id).await?.is_none()
                            } else {
                                false
                            };
                            let (stopped_status, stopped_reason) = if let Some(proposal) = approved_gate {
                                (proposal.status, proposal.reason)
                            } else if session.status == TaskSessionStatus::Completed {
                                (
                                    TaskSessionStatus::Completed,
                                    session.status_reason.clone(),
                                )
                            } else if status == Lifecycle::Interrupted {
                                (
                                    TaskSessionStatus::Waiting,
                                    "Task flow step interrupted; waiting for resume or another instruction".to_string(),
                                )
                            } else if merged_completing_pr {
                                // The PR merged to complete the Task, but a required review is
                                // still open. Wait for the gate to close before completion; do
                                // not rotate to another PR.
                                let number = observed_pr
                                    .as_ref()
                                    .and_then(|pr| pr.github())
                                    .map(|github| github.number);
                                let reason = match number {
                                    Some(number) => format!(
                                        "pull request #{number} merged; awaiting required review before completion"
                                    ),
                                    None => "pull request merged; awaiting required review before completion"
                                        .to_string(),
                                };
                                (TaskSessionStatus::Waiting, reason)
                            } else if needs_rotation {
                                crate::ops::task::ensure_working_pr_for_run(
                                    &store,
                                    &mut session,
                                    lease,
                                )
                                .await
                                .map_err(|error| anyhow!(error.to_string()))?;
                                session.status_reason =
                                    "Task PR settled; starting the next PR".to_string();
                                store.update_task_session_for_run(&session, lease).await?;
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    wave.name(),
                                    &flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?;
                                let started = start_prepared_task_step(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    capture.as_ref(),
                                    prepared,
                                )
                                .await?;
                                if let Some(basis) = &started.basis {
                                    active_basis = basis.clone();
                                }
                                feedback_open = started.feedback;
                                flow_turn_active = true;
                                provider_turn_active = started.provider_turn_active;
                                last_text.clear();
                                continue 'runner;
                            } else if let Some(pr) = observed_pr
                                .as_ref()
                                .filter(|pr| pr.phase() == PrPhase::Open)
                            {
                                let head_advanced =
                                    match (head_before_turn.as_deref(), pr.head_sha()) {
                                        // No baseline (the PR was opened during
                                        // this turn): opening it is progress.
                                        (None, _) => true,
                                        (Some(start), Some(current)) => start != current,
                                        (Some(_), None) => false,
                                    };
                                {
                                    let (disposition, reason) =
                                        crate::ops::task::decide_open_pr_status(
                                            pr,
                                            github_degraded.as_deref(),
                                            head_advanced,
                                        );
                                    (session_status_for(disposition), reason)
                                }
                            } else {
                                let next_fingerprint = task_state_fingerprint(&session)?;
                                if next_fingerprint != state_fingerprint {
                                    state_fingerprint = next_fingerprint;
                                    session.status_reason =
                                        "Task flow changed the worktree; starting another iteration"
                                            .to_string();
                                    store.update_task_session_for_run(&session, lease).await?;
                                    let prepared = prepare_task_flow_step(
                                        &store,
                                        &mut session,
                                        lease,
                                        wave.name(),
                                        &flow,
                                        ci_fix_wake.as_ref(),
                                    )
                                    .await?;
                                    let started = start_prepared_task_step(
                                        &store,
                                        &mut session,
                                        lease,
                                        harness.as_mut(),
                                        &mut flow,
                                        capture.as_ref(),
                                        prepared,
                                    )
                                    .await?;
                                    if let Some(basis) = &started.basis {
                                        active_basis = basis.clone();
                                    }
                                    feedback_open = started.feedback;
                                    flow_turn_active = true;
                                    provider_turn_active = started.provider_turn_active;
                                    last_text.clear();
                                    continue 'runner;
                                }
                                (
                                    TaskSessionStatus::Blocked,
                                    "Task flow completed without a PR or any worktree change; another automatic iteration would spin".to_string(),
                                )
                            };
                            if session.lifecycle_phase == TaskLifecyclePhase::Iterate
                                && status != Lifecycle::Interrupted
                            {
                                let waiting_for_ci = observed_pr.as_ref().is_some_and(|pr| {
                                    pr.phase() == PrPhase::Open && !pr.review_ready()
                                });
                                session.enter_gate(TaskGateProposal {
                                    status: stopped_status,
                                    reason: stopped_reason,
                                })?;
                                if waiting_for_ci {
                                    let number = observed_pr
                                        .as_ref()
                                        .and_then(|pr| pr.github())
                                        .map(|github| github.number);
                                    let reason = match number {
                                        Some(number) => format!(
                                            "pull request #{number} is waiting for fresh passing required checks before Task review"
                                        ),
                                        None => "pull request is waiting for fresh passing required checks before Task review"
                                            .to_string(),
                                    };
                                    set_and_record_status(
                                        &store,
                                        &mut session,
                                        lease,
                                        TaskSessionStatus::Waiting,
                                        reason,
                                    )
                                    .await?;
                                    return finish_parked(
                                        &store,
                                        &mut session,
                                        lease,
                                        Some(harness.as_mut()),
                                        ChildBodyOutcome::Completed,
                                        capture.as_ref(),
                                    )
                                    .await;
                                }
                                session.set_status(
                                    TaskSessionStatus::Running,
                                    format!(
                                        "Task outcome is awaiting gate cycle {}",
                                        session.gate_cycle
                                    ),
                                );
                                gate_fingerprint = Some(task_gate_fingerprint(&session)?);
                                store.update_task_session_for_run(&session, lease).await?;
                                let started = start_resumed_task_phase(
                                    &store,
                                    &mut session,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    wave.name(),
                                    capture.as_ref(),
                                )
                                .await?;
                                if let Some(basis) = &started.basis {
                                    active_basis = basis.clone();
                                }
                                feedback_open = started.feedback;
                                flow_turn_active = true;
                                provider_turn_active = started.provider_turn_active;
                                last_text.clear();
                                continue 'runner;
                            }
                            // Persist non-status fields while the Run still owns authority.
                            store.update_task_session_for_run(&session, lease).await?;
                            if stopped_status == TaskSessionStatus::Completed {
                                let work = store
                                    .work_for_child(&ChildRef::Task(session.id.clone()))
                                    .await?;
                                store.validate_completion_basis(&work, &active_basis).await?;
                            }
                            let stopped = store
                                .stop_task_for_run(
                                    &session.id,
                                    lease,
                                    stopped_status,
                                    &stopped_reason,
                                )
                                .await?;
                            let _ = harness.stop().await;
                            let from = session.status;
                            session = stopped;
                            if !summary.is_empty() {
                                store.append_task_event_for_run(
                                    &session.id,
                                    lease,
                                    &TaskEventKind::Progress {
                                        summary: summary.clone(),
                                    },
                                ).await?;
                            }
                            if session.status == TaskSessionStatus::Completed {
                                store.append_task_event_for_run(
                                    &session.id,
                                    lease,
                                    &TaskEventKind::Completed { summary },
                                ).await?;
                            }
                            store.append_task_event_for_run(
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
                            store.finish_task_process_for_run(&session, lease).await?;
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message } => {
                        let reason = format!("{code}: {message}");
                        return fail_and_maybe_relaunch(
                            &store,
                            &mut session,
                            lease,
                            harness.as_mut(),
                            &wave,
                            &reason,
                            turn_had_durable_side_effect,
                            capture.as_ref(),
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

async fn prepare_task_flow_step(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    wave_name: &str,
    flow: &Playhead,
    ci_fix: Option<&CiFixWake>,
) -> Result<PreparedTaskStep> {
    let work = store
        .work_for_child(&ChildRef::Task(session.id.clone()))
        .await?;
    let boundary = store.boundary_seed(&work).await?;
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
        "Task {} cycle {}, iteration {}, step {}/{}: {}",
        session.lifecycle_phase.as_str(),
        session.lifecycle_cycle(),
        step.iteration + 1,
        step.index + 1,
        step.total,
        step.step
    );
    store.update_task_session_for_run(session, lease).await?;
    let pr = store
        .active_task_pr(&session.id)
        .await?
        .ok_or_else(|| anyhow!("Task Session {} has no active PR", session.id))?;
    // The `ci-fix` step gets the failure seed from the typed incident claimed by
    // this Run; every other Task-flow step gets the standard task seed. The flow
    // and incident are chosen together, so a `ci-fix` step without one is invalid.
    let seed = match (step.step.as_str(), ci_fix) {
        ("ci-fix", Some(wake)) => format!(
            "{}\n\n{}",
            ci_fix_seed(session, &pr, wake, wave_name),
            boundary.render()
        ),
        ("ci-fix", None) => {
            anyhow::bail!(
                "Task Session {} is running the ci-fix flow with no claimed ci-fix wake",
                session.id
            )
        }
        _ => task_seed(session, &pr, wave_name, &boundary),
    };
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave_name, None)?;
    prepared.config.agent = Some(session.agent.clone());
    let skill = crate::engine::load_skill(&step.step, Path::new(&session.worktree))?;
    let attention = if step.feedback {
        let route = match session.phase_plan().interaction_policy {
            InteractionPolicy::Require => AttentionRoute::User,
            InteractionPolicy::Defer => AttentionRoute::Parent(
                store
                    .work_for_child(&ChildRef::Project(session.project_session_id.clone()))
                    .await?,
            ),
        };
        prepared.input.push_str("\n\n");
        prepared.input.push_str(&interactive_step_protocol(
            &work,
            &step.step,
            &route,
            skill
                .content
                .as_deref()
                .unwrap_or("Follow the named skill."),
        ));
        session.status_reason = format!(
            "Task {} cycle {}, Feedback step {} routes attention to {}",
            session.lifecycle_phase.as_str(),
            session.lifecycle_cycle(),
            step.step,
            match &route {
                AttentionRoute::User => "User".to_string(),
                AttentionRoute::Parent(parent) => format!("parent {}", parent.id()),
            }
        );
        store.update_task_session_for_run(session, lease).await?;
        Some(route)
    } else {
        None
    };
    let position = FlowPosition {
        work,
        epoch_id: boundary.basis.epoch_id.clone(),
        flow: session.phase_plan().flow.clone(),
        step: step.step.clone(),
        step_index: step.index,
        iteration: step.iteration,
        feedback: attention.is_some(),
        updated_at: time::OffsetDateTime::now_utc(),
    };
    Ok(PreparedTaskStep {
        turn: prepared,
        attention,
        position,
        basis: boundary.basis,
    })
}

fn interactive_step_protocol(
    work: &crate::durable::WorkRef,
    skill: &str,
    attention: &AttentionRoute,
    instructions: &str,
) -> String {
    let route = match attention {
        AttentionRoute::User => "the authenticated User",
        AttentionRoute::Parent(_) => "the immediate parent Run",
    };
    format!(
        "Conduct the `{skill}` Feedback step in this existing Task Launch. Attention is routed \
to {route}. Conversation arrives as ordinary Steers addressed to Work `{}`. Ask bounded \
questions, respond in this same Launch, and wait when another answer is required. A current \
Basis Continue advances the flow; there is no approval or changes-requested disposition. Extra \
findings are ordinary Steers.\n\n{instructions}",
        work.id()
    )
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
    _store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    open_task_flow_body(flow, session)?;
    apply_input(_store, session, lease, harness, &prepared.input).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_prepared_task_step(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    capture: Option<&crate::trace::CaptureHandle>,
    prepared: PreparedTaskStep,
) -> Result<StartedTaskStep> {
    store
        .set_flow_position(lease, prepared.position.clone())
        .await?;
    if let Some(attention) = &prepared.attention {
        let capture = capture
            .ok_or_else(|| anyhow!("interactive Task step requires an observable active Launch"))?;
        store
            .route_feedback(lease, &capture.launch_id(), attention.clone())
            .await?;
    }
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    start_task_flow_turn(store, session, lease, harness, flow, prepared.turn).await?;
    Ok(StartedTaskStep {
        feedback: prepared.attention.is_some(),
        provider_turn_active: true,
        basis: Some(prepared.basis),
    })
}

#[allow(clippy::too_many_arguments)]
async fn start_resumed_task_phase(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    wave_name: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<StartedTaskStep> {
    *flow = resume_task_phase(session)?;
    let prepared = prepare_task_flow_step(store, session, lease, wave_name, flow, None).await?;
    start_prepared_task_step(store, session, lease, harness, flow, capture, prepared).await
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

/// End a parked body: the session status is already `Waiting` or `Blocked`, so only
/// the process is settled and the parent stays non-terminal. The caller supplies
/// `outcome` — only it knows whether the turn finished or was cut short.
/// Close the body's trace capture so its last turn's usage is persisted.
/// A capture left open leaves a `running` turn with NULL usage -- the orphan
/// row `lf doctor` reports -- so this runs on every terminal path.
fn finish_capture(capture: Option<&crate::trace::CaptureHandle>, outcome: &str) {
    let Some(capture) = capture else { return };
    if let Err(error) = capture.finish(outcome, false) {
        tracing::warn!(%error, "failed to finish Task trace capture");
    }
}

async fn finish_parked(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: Option<&mut dyn Harness>,
    outcome: ChildBodyOutcome,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "completed");
    if let Some(harness) = harness {
        let _ = harness.stop().await;
    }
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(outcome);
    }
    store.finish_task_process_for_run(session, lease).await?;
    Ok(())
}

fn record_task_flow_position(session: &mut TaskSession, flow: &Playhead) -> Result<()> {
    let root = flow
        .stack
        .first()
        .ok_or_else(|| anyhow!("Task flow has no root invocation"))?;
    if root.flow != session.phase_plan().flow {
        anyhow::bail!(
            "Task Session {} {} flow is {:?}, but its playhead is {:?}",
            session.id,
            session.lifecycle_phase.as_str(),
            session.phase_plan().flow,
            root.flow
        );
    }
    session.phase_cursor = root.cursor;
    session.phase_iteration = root.iteration;
    session.updated_at = time::OffsetDateTime::now_utc();
    Ok(())
}

fn resume_task_phase(session: &TaskSession) -> Result<Playhead> {
    let (flow, _) = Playhead::resume_root(
        QueuedInvocation::load(&session.worktree, &session.phase_plan().flow)?,
        session.phase_cursor,
        session.phase_iteration,
    )?;
    Ok(flow)
}

fn sync_terminal_task_state(session: &mut TaskSession, latest: &TaskSession) {
    if latest.status.is_terminal() {
        session.status = latest.status;
        session.status_reason = latest.status_reason.clone();
        session.status_at = latest.status_at;
        session.pm_writeback = latest.pm_writeback.clone();
    }
}

fn task_state_fingerprint(session: &TaskSession) -> Result<String> {
    let state = crate::engine::git::worktree_state(Path::new(&session.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

/// The active PR's current head SHA, or `None` when there is no active PR.
/// Captured at iteration boundaries as a GitHub-side progress baseline so the
/// runner can tell a no-change ci-fix (head unchanged) from a push (head
/// advanced) without relying on worktree churn.
async fn pr_head_for_session(store: &SharedStore, session: &TaskSession) -> Result<Option<String>> {
    Ok(store
        .active_task_pr(&session.id)
        .await?
        .and_then(|pr| pr.github().map(|g| g.head_sha.clone()))
        .flatten())
}

fn task_gate_fingerprint(session: &TaskSession) -> Result<String> {
    let state = crate::engine::git::material_worktree_state(Path::new(&session.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

async fn pending_input_is_current(
    _store: &SharedStore,
    _session: &TaskSession,
    _lease: &RunLease,
    input: &PendingInput,
) -> Result<bool> {
    input_is_current(input).await
}

async fn apply_next_pending(
    store: &SharedStore,
    session: &TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    pending: &mut VecDeque<PendingInput>,
) -> Result<bool> {
    while let Some(input) = pending.pop_front() {
        if !pending_input_is_current(store, session, lease, &input).await? {
            continue;
        }
        apply_input(store, session, lease, harness, &input.text).await?;
        return Ok(true);
    }
    Ok(false)
}

async fn handle_attachment(
    store: &SharedStore,
    session: &TaskSession,
    lease: &RunLease,
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
    store.validate_run_lease(lease).await?;
    let target = ChildRef::Task(session.id.clone());
    if line == "/interrupt" {
        let work = store.work_for_child(&target).await?;
        let run = store
            .current_run(&work)
            .await?
            .ok_or_else(|| anyhow!("Task Work {} has no active Run", work.id()))?;
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

async fn start_ci_fix_flow(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    wake: &CiFixWake,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<Basis> {
    *flow = Playhead::new(QueuedInvocation::load(&session.worktree, "ci-fix")?).0;
    let wave = owning_wave(store, session).await?;
    let prepared =
        prepare_task_flow_step(store, session, lease, wave.name(), flow, Some(wake)).await?;
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    let basis = prepared.basis;
    start_task_flow_turn(store, session, lease, harness, flow, prepared.turn).await?;
    Ok(basis)
}

async fn record_unhandled_failure(
    session_id: &TaskSessionId,
    lease: &RunLease,
    error: &anyhow::Error,
) {
    let Some(store) = open_existing_store().await.map(Arc::new) else {
        return;
    };
    let Ok(Some(mut session)) = store.get_task_session(session_id).await else {
        return;
    };
    let Ok(work) = store
        .work_for_child(&ChildRef::Task(session.id.clone()))
        .await
    else {
        return;
    };
    if !session.status.is_process_active() || lease.work != work {
        return;
    }
    let from = session.status;
    let message = format!("task process failed: {error}");
    session.set_status(TaskSessionStatus::Failed, &message);
    if store
        .update_task_session_for_run(&session, lease)
        .await
        .is_err()
    {
        return;
    }
    let _ = store
        .append_task_event_for_run(
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
        .append_task_event_for_run(
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
    let _ = store.finish_task_process_for_run(&session, lease).await;
}

async fn apply_input(
    store: &SharedStore,
    _session: &TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    text: &str,
) -> Result<()> {
    apply_child_input(
        store,
        lease,
        harness,
        PendingInput::system(text.to_string()),
    )
    .await
}

/// Project an open-PR disposition onto the legacy Session status this runner
/// still records. Every arm is a Wait; the old enum only distinguished whether
/// a human was needed. This translation lives here, at the boundary, because it
/// dies with the runner -- the disposition itself is the durable vocabulary.
fn session_status_for(disposition: crate::ops::task::OpenPrDisposition) -> TaskSessionStatus {
    use crate::ops::task::OpenPrDisposition;
    match disposition {
        OpenPrDisposition::ObservationDegraded | OpenPrDisposition::NeedsDirection => {
            TaskSessionStatus::Blocked
        }
        OpenPrDisposition::AwaitingReview => TaskSessionStatus::Waiting,
    }
}

/// Apply a status transition and persist it: set the status, update the row, and
/// append the paired `StatusChanged` event.
async fn set_and_record_status(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    status: TaskSessionStatus,
    reason: impl Into<String>,
) -> Result<()> {
    let from = session.status;
    session.set_status(status, reason);
    store.update_task_session_for_run(session, lease).await?;
    store
        .append_task_event_for_run(
            &session.id,
            lease,
            &TaskEventKind::StatusChanged {
                from,
                to: status,
                reason: session.status_reason.clone(),
            },
        )
        .await?;
    if status == TaskSessionStatus::Blocked {
        if let Some(pr) = store.active_task_pr(&session.id).await? {
            store
                .mark_ci_incidents_blocked(
                    &pr.id,
                    time::OffsetDateTime::now_utc(),
                    &session.status_reason,
                )
                .await?;
        }
    }
    Ok(())
}

async fn finish_failed(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    error: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    set_and_record_status(store, session, lease, TaskSessionStatus::Failed, error).await?;
    store
        .append_task_event_for_run(
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
    store.finish_task_process_for_run(session, lease).await?;
    anyhow::bail!(error.to_string())
}

/// The Blocked reason for an infrastructure failure, naming the failing
/// capability and the safe next action. `pr_number` keeps the attached PR
/// visible so a resume after the capability recovers picks up the same PR.
fn infra_blocked_reason(capability: &str, detail: &str, pr_number: Option<u32>) -> String {
    let pr_note = pr_number
        .map(|n| format!(" Pull request #{n} stays attached."))
        .unwrap_or_default();
    format!("ci-fix blocked by {capability}: {detail}.{pr_note}")
}

/// Stop the body and transition the Task to Blocked for an infrastructure
/// failure (provider outage, GitHub observation failure), keeping the active PR
/// attached so a resume after the capability recovers picks up the same PR.
/// Returns `Ok(())` — a clean stop, not an error — so the runner does not also
/// record an unhandled failure.
async fn finish_infra_blocked(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    capability: &str,
    detail: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    let pr_number = store
        .active_task_pr(&session.id)
        .await?
        .and_then(|pr| pr.github().map(|g| g.number));
    let reason = infra_blocked_reason(capability, detail, pr_number);
    set_and_record_status(store, session, lease, TaskSessionStatus::Blocked, &reason).await?;
    if let Some(process) = &mut session.latest_process {
        process.state = ChildLeaseState::Finished;
        process.outcome = Some(ChildBodyOutcome::Failed {
            reason: reason.clone(),
        });
    }
    store.finish_task_process_for_run(session, lease).await?;
    Ok(())
}

/// Handle a body failure with disconnect-class recovery: classify the failure,
/// and if it's a disconnect/hollow-body with a configured backup agent, hand
/// the next generation to the backup instead of leaving the body failed for
/// the supervisor to respawn the same flaky provider.
#[allow(clippy::too_many_arguments)] // capture is a terminal-path output, not a knob
async fn handle_body_failure(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<Option<RunLease>> {
    finish_capture(capture, "failed");
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
            set_and_record_status(store, session, lease, TaskSessionStatus::Failed, reason).await?;
            store
                .append_task_event_for_run(
                    &session.id,
                    lease,
                    &TaskEventKind::Failed {
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
            store.update_task_session_for_run(session, lease).await?;
            let launch = store
                .current_launch(lease)
                .await?
                .ok_or_else(|| anyhow!("Task Run {} has no Launch to hand back", lease.run_id))?;
            store
                .advance_run(
                    lease,
                    crate::durable::RunAdvance::LaunchEnded {
                        launch_id: launch.id,
                        outcome: crate::durable::BoundaryState::Failed,
                    },
                )
                .await?;
            let handoff = ChildBodyHandoff {
                from_agent: session.agent.clone(),
                to_agent: agent.clone(),
                from_provider: session.provider.clone(),
                to_provider: provider.clone(),
                reason: format!(
                    "disconnect-class failure; handing off from {} to {agent}",
                    session.agent
                ),
            };
            if session.provider != provider {
                session.provider_session_id = None;
            }
            session.agent = agent;
            session.provider = provider;
            store.update_task_session_for_run(session, lease).await?;
            store
                .append_task_event_for_run(
                    &session.id,
                    lease,
                    &TaskEventKind::BodyHandedOff { handoff },
                )
                .await?;
            let rotated = store.rotate_run_lease(lease).await?;
            let next_generation = session
                .latest_process
                .as_ref()
                .map_or(1, |process| process.generation + 1);
            let tmux_name = format!("lf-task-{}-{next_generation}", &session.id.as_str()[3..11]);
            session.begin_generation(tmux_name);
            store.update_task_session_for_run(session, &rotated).await?;
            Ok(Some(rotated))
        }
        RecoveryDecision::Stop => {
            let non_convergence = format!(
                "{reason}; not replay-safe (durable side effects this turn) and no backup agent configured"
            );
            finish_failed(store, session, lease, harness, &non_convergence, None)
                .await
                .map(|_| None)
        }
        RecoveryDecision::AllowRetry => finish_failed(store, session, lease, harness, reason, None)
            .await
            .map(|_| None),
        RecoveryDecision::Normal => {
            // Not a disconnect-class failure — a provider outage during a
            // PR/ci-fix iteration is an infrastructure blocker: keep the PR
            // attached and block actionably so a resume when the provider
            // recovers picks up the same PR. Without a PR, fall back to the
            // generic failed path.
            if store.active_task_pr(&session.id).await?.is_some() {
                return finish_infra_blocked(store, session, lease, harness, "provider", reason)
                    .await
                    .map(|_| None);
            }
            finish_failed(store, session, lease, harness, reason, None)
                .await
                .map(|_| None)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn fail_and_maybe_relaunch(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    let Some(rotated) = handle_body_failure(
        store,
        session,
        lease,
        harness,
        wave,
        reason,
        turn_had_durable_side_effect,
        capture,
    )
    .await?
    else {
        return Ok(());
    };
    spawn_failover(store, session, &rotated, wave).await
}

async fn finish_abandoned(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
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
    store.finish_task_process_for_run(session, lease).await?;
    Ok(())
}

async fn finish_command_stop(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    harness: &mut dyn Harness,
    stop: CommandStop,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(
        capture,
        match stop {
            CommandStop::Interrupted => "interrupted",
            _ => "completed",
        },
    );
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
            store.finish_task_process_for_run(session, lease).await?;
            Ok(())
        }
        CommandStop::Abandoned(reason) => {
            finish_abandoned(store, session, lease, harness, reason).await
        }
    }
}

/// Typed CI evidence selected by this exact Run.
#[derive(Debug, Clone)]
pub(crate) struct CiFixWake {
    pub incident_identity: String,
    pub pr_number: u32,
    pub head_sha: String,
    pub failing_checks: Vec<CiCheck>,
}

/// The identity of the failure this PR reads as *now*. `None` means no wake is
/// warranted: green, moved on, gone, or not `wake_legal`.
async fn current_ci_incident_identity(
    store: &SharedStore,
    session: &TaskSession,
) -> Result<Option<String>> {
    Ok(store
        .active_task_pr(&session.id)
        .await?
        .as_ref()
        .and_then(crate::ops::task::current_ci_incident)
        .map(|incident| incident.identity))
}

async fn arm_ci_fix_wake(
    store: &SharedStore,
    session: &TaskSession,
    lease: &crate::durable::RunLease,
) -> Result<Option<CiFixWake>> {
    let Some(pr) = store.active_task_pr(&session.id).await? else {
        return Ok(None);
    };
    let Some(incident) = crate::ops::task::current_ci_incident(&pr) else {
        return Ok(None);
    };
    let failing_checks = pr
        .fresh_ci()
        .map(|observation| observation.failing_checks.clone())
        .unwrap_or_default();
    if !store
        .claim_ci_incident(
            &incident.identity,
            &lease.run_id,
            time::OffsetDateTime::now_utc(),
        )
        .await?
    {
        return Ok(None);
    }
    Ok(Some(CiFixWake {
        incident_identity: incident.identity,
        pr_number: incident.pr_number,
        head_sha: incident.failed_head_sha,
        failing_checks,
    }))
}

/// End one bounded repair without entering the Task lifecycle beneath it.
#[allow(clippy::too_many_arguments)] // capture is a terminal-path output, not a knob
async fn settle_ci_fix_turn(
    store: &SharedStore,
    session: &mut TaskSession,
    lease: &RunLease,
    wake: &CiFixWake,
    observed_pr: Option<&crate::task::TaskPr>,
    head_before_turn: Option<&str>,
    status: Lifecycle,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    // The authoritative post-turn head, when it moved past the incident head. Set
    // only when the fresh reconcile proved advancement, so it names the head the
    // repair body shipped for this incident.
    let mut repaired_head: Option<String> = None;
    let (settled_status, reason) = match observed_pr
        .filter(|pr| pr.phase() == PrPhase::Open)
    {
        Some(pr) if status != Lifecycle::Interrupted => {
            let head_advanced = match (head_before_turn, pr.head_sha()) {
                // No baseline: this body never saw a head to move.
                (None, _) => false,
                (Some(start), Some(current)) => start != current,
                (Some(_), None) => false,
            };
            if head_advanced {
                repaired_head = pr.head_sha().map(str::to_string);
            }
            // Reconcile names a degraded read on the session; for a turn that just
            // ran, that reading could not have verified a repair.
            let degraded = match &session.observation {
                Observation::Degraded { reason, .. } => Some(reason.as_str()),
                _ => None,
            };
            let (disposition, reason) =
                crate::ops::task::decide_open_pr_status(pr, degraded, head_advanced);
            (session_status_for(disposition), reason)
        }
        Some(_) => (
            TaskSessionStatus::Waiting,
            format!(
                "ci-fix turn on pull request #{} was interrupted; the repair resumes on resume",
                wake.pr_number
            ),
        ),
        None => (
            TaskSessionStatus::Waiting,
            format!(
                "pull request #{} settled or is no longer attached; the ci-fix wake no longer applies",
                wake.pr_number
            ),
        ),
    };

    set_and_record_status(store, session, lease, settled_status, &reason).await?;
    // The head the body shipped is durable attribution on the incident, tied to
    // its claiming Run. First-write in the store, so a retry or a
    // later push never rewrites which head settled it.
    if let Some(head) = &repaired_head {
        store
            .mark_ci_incident_repaired(
                &wake.incident_identity,
                head,
                time::OffsetDateTime::now_utc(),
            )
            .await?;
    }
    // The body's outcome describes this turn, not the wake's verdict: a repair
    // that ran to the end Completed, whatever it found. Only a turn cut short was
    // Interrupted — and that is the one that leaves the wake reclaimable.
    let outcome = if status == Lifecycle::Interrupted {
        ChildBodyOutcome::Interrupted { reason }
    } else {
        ChildBodyOutcome::Completed
    };
    finish_parked(store, session, lease, None, outcome, capture).await
}

/// The seed for a `ci-fix` turn: the PR the skill must repair plus the failing
/// required checks (names + log URLs) so it resolves the exact failure on the
/// current head without re-deriving it.
///
/// The selected incident is immutable even after the PR's current observation
/// moves on, so the seed and settlement name the same failed head.
fn ci_fix_seed(
    session: &TaskSession,
    pr: &crate::task::TaskPr,
    wake: &CiFixWake,
    wave_name: &str,
) -> String {
    let url = pr.github().map(|github| github.url.as_str()).unwrap_or("");
    let number = wake.pr_number;
    let head = wake.head_sha.as_str();
    let failing = wake
        .failing_checks
        .iter()
        .map(|check| match &check.url {
            Some(link) => format!("- {} ({link})", check.name),
            None => format!("- {}", check.name),
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Fix the failing required CI checks on Linear task {identifier}'s open pull request.\n\n\
         Run the ci-fix skill: reproduce the latest failure on the current head, make the smallest correct fix, run targeted then proportional checks, and push the same branch. Report an infrastructure or credential blocker rather than weakening tests.\n\n\
         PR #{number}: {url}\nBranch: {branch}\nHead commit: {head}\nFailing required checks:\n{failing}\n\n\
         Wave: {wave}\nTask Session: {session_id}\nWorktree: {worktree}\n\n\
         Push fixes to the same branch; do not open a new PR or rotate the serial branch. When the push lands, the Task returns to waiting on the new head.",
        identifier = session.launch.issue.identifier,
        number = number,
        url = url,
        branch = pr.branch,
        head = head,
        failing = if failing.is_empty() { "- (none reported)".to_string() } else { failing },
        wave = wave_name,
        session_id = session.id,
        worktree = session.worktree.display(),
    )
}

fn task_seed(
    session: &TaskSession,
    pr: &crate::task::TaskPr,
    wave_name: &str,
    boundary: &BoundarySeed,
) -> String {
    let placement = pr
        .parent_pr_id
        .as_ref()
        .map(|parent| format!("Stack parent PR: {parent} (land the parent first)"))
        .unwrap_or_else(|| "Stack parent PR: none (rooted on main)".to_string());
    let gate_proposal = session
        .gate_proposal
        .as_ref()
        .map(|proposal| {
            format!(
                "Gate proposal: {} — {}",
                proposal.status.as_str(),
                proposal.reason
            )
        })
        .unwrap_or_else(|| "Gate proposal: none".to_string());
    format!(
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\n{direction}\n\nPM snapshot synced at: {snapshot_synced_at}\nWave: {wave}\nTask Session: {session_id}\nLifecycle phase: {lifecycle_phase} (epoch {phase_epoch}, gate cycle {gate_cycle})\nInteraction policy: {interaction_policy}\n{gate_proposal}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}\n\nThis PR owns one serial branch. Bare `lf pr land --next <slug>` ships it and keeps the Task open; `lf pr land -c` proposes completing the Task after merge. `lf pr abandon` discards only this PR. `lf task complete {identifier} --summary \"...\"` proposes completion for clean work that needs no PR. Gate approves settlement or returns the same Task to iteration. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward. The runner owns branch rotation between PRs.",
        identifier = session.launch.issue.identifier,
        title = session.launch.issue.title,
        description = session.launch.issue.description,
        project = session.launch.project.name,
        project_id = session.launch.project.id.as_str(),
        project_context = session.launch.project.prompt_context,
        direction = boundary.render(),
        snapshot_synced_at = session.launch.pm_snapshot_synced_at,
        wave = wave_name,
        session_id = session.id,
        lifecycle_phase = session.lifecycle_phase.as_str(),
        phase_epoch = session.phase_epoch,
        gate_cycle = session.gate_cycle,
        interaction_policy = session.phase_plan().interaction_policy.as_str(),
        gate_proposal = gate_proposal,
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
    use std::path::PathBuf;
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use loopflow_test_support::TestRepo;
    use time::OffsetDateTime;

    use super::{
        ci_fix_seed, handle_body_failure, infra_blocked_reason, prepare_task_flow_step,
        progress_summary, resume_task_phase, run_task_session_with, settle_ci_fix_turn, CiFixWake,
        PreparedTaskStep,
    };
    use crate::chat::types::{ConversationEvent, Lifecycle, TurnUsage};
    use crate::child_session::{ChildProcessReservation, ChildRef};
    use crate::durable::RunLease;
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Harness, SendCurrentOutcome};
    use crate::id::WaveId;
    use crate::project_session::{ProjectSession, ProjectSessionId, ProjectSessionStatus};
    use crate::session_context::{
        LinearIssueId, LinearIssueSnapshot, LinearProjectId, LinearProjectSnapshot,
        ProjectLaunchReceipt, TaskLaunchReceipt,
    };
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        AfterMerge, CiIncident, CiObservation, CiState, GithubPr, PmWritebackState, PrPublication,
        TaskEventKind, TaskGateProposal, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr, TaskPrId,
        TaskSession, TaskSessionId, TaskSessionStatus,
    };
    use crate::wave::playhead::Playhead;
    use crate::wave::Wave;

    struct ScriptedHarness {
        accepts_current_send: bool,
        /// Overrides what the active Turn answers, so a test can script a
        /// live-capable provider whose Turn rejects, loses its response, or
        /// faults — shapes a bare "supports steering" bool cannot express.
        steer_outcome: Option<SendCurrentOutcome>,
        sent: Vec<String>,
        interrupts: usize,
        fail_send: bool,
        fail_interrupt: bool,
    }

    struct RunnerUsageHarness {
        events: tokio::sync::mpsc::UnboundedSender<ConversationEvent>,
        inputs: usize,
    }

    #[async_trait]
    impl Harness for RunnerUsageHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> Result<()> {
            self.inputs += 1;
            if self.inputs == 1 {
                self.events
                    .send(ConversationEvent::TurnStarted {
                        turn_id: "spending-turn".to_string(),
                    })
                    .unwrap();
                self.events
                    .send(ConversationEvent::TurnCompleted {
                        turn_id: "spending-turn".to_string(),
                        status: Lifecycle::Completed,
                    })
                    .unwrap();
                self.events
                    .send(ConversationEvent::TurnUsage {
                        turn_id: "spending-turn".to_string(),
                        usage: TurnUsage {
                            input_tokens: 321,
                            output_tokens: 45,
                            ..TurnUsage::default()
                        },
                    })
                    .unwrap();
            } else {
                self.events
                    .send(ConversationEvent::Error {
                        code: "script_complete".to_string(),
                        message: "end the runner fixture".to_string(),
                    })
                    .unwrap();
            }
            Ok(())
        }

        async fn interrupt(&mut self) -> Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }

        fn provider_session_id(&self) -> Option<String> {
            Some("runner-provider-session".to_string())
        }
    }

    impl ScriptedHarness {
        fn new(accepts_current_send: bool) -> Self {
            Self {
                accepts_current_send,
                steer_outcome: None,
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

        async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
            if let Some(outcome) = self.steer_outcome.clone() {
                // Only a confirmed Sent means the provider took the text; a
                // rejected or lost send must not record a delivery.
                if matches!(outcome, SendCurrentOutcome::Sent { .. }) {
                    self.sent.push(content.to_string());
                }
                return outcome;
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

    async fn seed_conformance_session(
        store: SharedStore,
        provider: &str,
        worktree: PathBuf,
        directive_text: Option<&str>,
    ) -> (TaskSession, ChildProcessReservation) {
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
            status: TaskSessionStatus::Waiting,
            status_reason: "ready for provider".to_string(),
            status_at: now,
            worktree,
            workspace_slug: format!("test-{provider}"),
            lifecycle: crate::task::TaskLifecyclePlan::standard("task"),
            lifecycle_phase: crate::task::TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: provider.to_string(),
            provider: provider.to_string(),
            provider_session_id: Some("provider-session".to_string()),
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
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
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        store.create_task_session(&session, &pr).await.unwrap();
        if let Some(text) = directive_text {
            let work = store
                .work_for_child(&ChildRef::Task(session.id.clone()))
                .await
                .unwrap();
            store
                .append_steer(&work, crate::durable::Author::User, text, None)
                .await
                .unwrap();
        }
        session.begin_generation(format!("task-{provider}"));
        let reservation = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        (session, reservation)
    }

    async fn conformance_session(provider: &str) -> (SharedStore, TaskSession, RunLease) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.keep().join("registry.db");
        let store = Arc::new(open_store(&StorageConfig::sqlite(path)).await.unwrap());
        let (mut session, reservation) = seed_conformance_session(
            store.clone(),
            provider,
            PathBuf::from(format!("/repo.{provider}")),
            None,
        )
        .await;
        let lease = store
            .resolve_run_lease(reservation.run_token.clone())
            .await
            .unwrap();
        if let Some(process) = &mut session.latest_process {
            process.state = crate::child_session::ChildLeaseState::Active;
        }
        session.set_status(TaskSessionStatus::Running, "provider active");
        store
            .activate_task_process_for_run(&session, &lease)
            .await
            .unwrap();
        (store, session, lease)
    }

    #[tokio::test]
    async fn task_runner_records_reported_turn_usage() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let repo = TestRepo::new();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(ledger.home().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let (session, reservation) = seed_conformance_session(
            store.clone(),
            "codex",
            repo.path().to_path_buf(),
            Some("record this provider turn"),
        )
        .await;
        let lease = store
            .resolve_run_lease(reservation.run_token.clone())
            .await
            .unwrap();
        crate::journal::emit(
            repo.path(),
            crate::journal::LfNode::Run,
            crate::journal::LfEventType::Started,
            crate::journal::LfEventFields::default(),
        );

        run_task_session_with(
            store,
            session.id,
            &lease,
            Box::new(|name, _approval, events| {
                assert_eq!(name, "codex");
                Ok(Box::new(RunnerUsageHarness { events, inputs: 0 }))
            }),
        )
        .await
        .unwrap();

        let trace_store = crate::journal::open_ledger().unwrap();
        let launches = trace_store.agent_launches_since(0).unwrap();
        assert_eq!(launches.len(), 1);
        assert_eq!(launches[0].provider, "codex");
        assert_eq!(launches[0].model, None);
        let turns = trace_store
            .agent_turns_for_launches(&[launches[0].id.clone()])
            .unwrap();
        let spending_turns = turns
            .iter()
            .filter(|turn| turn.provider_input_tokens.is_some())
            .collect::<Vec<_>>();
        assert_eq!(spending_turns.len(), 1);
        assert_eq!(spending_turns[0].provider_input_tokens, Some(321));
        assert_eq!(spending_turns[0].provider_output_tokens, Some(45));

        crate::journal::emit(
            repo.path(),
            crate::journal::LfNode::Run,
            crate::journal::LfEventType::Completed,
            crate::journal::LfEventFields::default(),
        );
    }

    #[tokio::test]
    async fn ci_settlement_records_the_first_fresh_repaired_head() {
        let (store, mut session, lease) = conformance_session("codex").await;
        let mut observed_pr = store
            .active_task_pr(&session.id)
            .await
            .unwrap()
            .expect("active PR");
        let now = OffsetDateTime::now_utc();
        observed_pr.publication = Some(PrPublication {
            requested_at: now,
            after_merge: AfterMerge::Review,
            next_slug: None,
            github: Some(GithubPr {
                number: 42,
                url: "https://github.com/owner/repo/pull/42".to_string(),
                head_sha: Some("fresh-repaired-head".to_string()),
            }),
        });
        observed_pr.ci_observation = Some(CiObservation {
            head_sha: "cached-failed-head".to_string(),
            state: CiState::Failing,
            failing_checks: Vec::new(),
            observed_at: now,
        });
        let incident = CiIncident {
            identity: "github:ci:owner/repo:42:cached-failed-head:test".to_string(),
            task_session_id: session.id.clone(),
            pr_id: observed_pr.id.clone(),
            repo: "owner/repo".to_string(),
            pr_number: 42,
            failed_head_sha: "cached-failed-head".to_string(),
            repaired_head_sha: None,
            failure_set: vec!["test".to_string()],
            provider_completed_at: None,
            poll_observed_at: Some(now),
            webhook_received_at: None,
            claimed_run_id: None,
            responded_at: None,
            green_at: None,
            merged_at: None,
            blocked_at: None,
            blocked_reason: None,
            created_at: now,
            updated_at: now,
        };
        store.observe_ci_incident(&incident).await.unwrap();
        let wake = CiFixWake {
            incident_identity: incident.identity.clone(),
            pr_number: 42,
            head_sha: incident.failed_head_sha.clone(),
            failing_checks: Vec::new(),
        };

        settle_ci_fix_turn(
            &store,
            &mut session,
            &lease,
            &wake,
            Some(&observed_pr),
            Some("cached-failed-head"),
            Lifecycle::Completed,
            None,
        )
        .await
        .unwrap();
        store
            .mark_ci_incident_repaired(&incident.identity, "later-unrelated-head", now)
            .await
            .unwrap();

        let row = store
            .ci_incidents_since(OffsetDateTime::UNIX_EPOCH, None, None)
            .await
            .unwrap()
            .into_iter()
            .find(|row| row.incident.identity == incident.identity)
            .expect("incident remains queryable");
        assert_eq!(
            row.incident.repaired_head_sha.as_deref(),
            Some("fresh-repaired-head")
        );
    }

    async fn prepared_gate_review(
        lifecycle: TaskLifecyclePlan,
    ) -> (
        tempfile::TempDir,
        SharedStore,
        TaskSession,
        RunLease,
        Playhead,
        PreparedTaskStep,
    ) {
        let repo = tempfile::tempdir().unwrap();
        for args in [
            ["init", "-b", "main"].as_slice(),
            ["config", "user.email", "loopflow@example.com"].as_slice(),
            ["config", "user.name", "Loopflow Test"].as_slice(),
        ] {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.path().join("README.md"), "review evidence\n").unwrap();
        assert!(std::process::Command::new("git")
            .args(["add", "README.md"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        assert!(std::process::Command::new("git")
            .args(["commit", "-m", "evidence"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());

        let (store, mut session, lease) = conformance_session("codex").await;
        let work = store
            .work_for_child(&ChildRef::Task(session.id.clone()))
            .await
            .unwrap();
        store
            .append_steer(
                &work,
                crate::durable::Author::User,
                "Prepare the Task for review",
                None,
            )
            .await
            .unwrap();
        session.worktree = repo.path().to_path_buf();
        session.lifecycle = lifecycle;
        session.lifecycle_phase = TaskLifecyclePhase::Gate;
        session.phase_epoch = 3;
        session.gate_cycle = 1;
        session.gate_proposal = Some(TaskGateProposal {
            status: TaskSessionStatus::Waiting,
            reason: "prove the delivered behavior".to_string(),
        });
        store
            .update_task_session_for_run(&session, &lease)
            .await
            .unwrap();
        let flow = resume_task_phase(&session).unwrap();
        let prepared =
            prepare_task_flow_step(&store, &mut session, &lease, "test-wave", &flow, None)
                .await
                .unwrap();
        (repo, store, session, lease, flow, prepared)
    }

    #[test]
    fn progress_summary_bounds_wave_visible_text() {
        let summary = progress_summary(&"x".repeat(2_500));
        assert_eq!(summary.chars().count(), 2_000);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn infra_blocked_reason_names_capability_and_keeps_pr_attached() {
        // Provider outage: the capability and safe next action are visible,
        // and the attached PR is named so a resume recovers onto the same PR.
        let reason = infra_blocked_reason(
            "provider",
            "provider turn failed; resume when the provider recovers",
            Some(900),
        );
        assert!(reason.contains("blocked by provider"), "reason: {reason}");
        assert!(
            reason.contains("resume when the provider recovers"),
            "reason: {reason}"
        );
        assert!(reason.contains("#900 stays attached"), "reason: {reason}");

        // GitHub observation failure uses the same shape with its capability.
        let gh = infra_blocked_reason("github-observation", "gh pr checks: HTTP 502", Some(7));
        assert!(gh.contains("blocked by github-observation"), "reason: {gh}");
        assert!(gh.contains("#7 stays attached"), "reason: {gh}");

        // No PR attached (e.g. a no-PR task that still hit an infra failure):
        // the reason names the capability without a PR note.
        let no_pr = infra_blocked_reason("provider", "turn failed", None);
        assert!(no_pr.contains("blocked by provider"), "reason: {no_pr}");
        assert!(!no_pr.contains("stays attached"), "reason: {no_pr}");
    }

    #[tokio::test]
    async fn headless_interactive_step_routes_parent_attention() {
        let (_repo, store, session, _lease, _flow, prepared) =
            prepared_gate_review(TaskLifecyclePlan::headless("task")).await;
        let parent = store
            .work_for_child(&ChildRef::Project(session.project_session_id.clone()))
            .await
            .unwrap();

        assert_eq!(
            prepared.attention,
            Some(crate::durable::AttentionRoute::Parent(parent))
        );
        assert!(prepared.position.feedback);
        assert_eq!(prepared.position.step, "demo");
        assert!(prepared.turn.input.contains("ordinary Steers"));
        assert!(prepared
            .turn
            .input
            .contains("no approval or changes-requested disposition"));
    }

    #[tokio::test]
    async fn standard_interactive_step_routes_user_attention() {
        let (_repo, _store, _session, _lease, _flow, prepared) =
            prepared_gate_review(TaskLifecyclePlan::standard("task")).await;

        assert_eq!(
            prepared.attention,
            Some(crate::durable::AttentionRoute::User)
        );
        assert!(prepared.position.feedback);
        assert!(prepared.turn.input.contains("authenticated User"));
    }

    #[tokio::test]
    async fn ci_fix_seed_carries_the_pr_and_the_failing_checks() {
        let (_store, session, _lease) = conformance_session("codex").await;
        let now = time::OffsetDateTime::now_utc();
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: "ship".to_string(),
            branch: "jack/ship".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: Some(crate::task::PrPublication {
                requested_at: now,
                after_merge: crate::task::AfterMerge::Review,
                next_slug: None,
                github: Some(crate::task::GithubPr {
                    number: 916,
                    url: "https://github.com/loopflow/loopflow/pull/916".to_string(),
                    head_sha: Some("headsha".to_string()),
                }),
            }),
            merge_commit: None,
            abandoned_at: None,
            // The observation has already moved on to a different head and a
            // different failure — exactly the drift that used to reach the seed.
            ci_observation: Some(crate::task::CiObservation {
                head_sha: "movedhead".to_string(),
                state: crate::task::CiState::Failing,
                failing_checks: vec![crate::task::CiCheck {
                    name: "some-other-check".to_string(),
                    url: Some("https://ci/other".to_string()),
                }],
                observed_at: now,
            }),
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        // The runner loads the `ci-fix` flow by name; it must be a registered builtin.
        assert!(
            crate::engine::builtins::get_builtin_flow("ci-fix").is_some(),
            "the ci-fix builtin flow must resolve"
        );
        let wake = super::CiFixWake {
            incident_identity: "github:ci:test/repo:916:headsha:deadbeef".to_string(),
            pr_number: 916,
            head_sha: "headsha".to_string(),
            failing_checks: vec![crate::task::CiCheck {
                name: "rust-test".to_string(),
                url: Some("https://ci/rust".to_string()),
            }],
        };
        let seed = ci_fix_seed(&session, &pr, &wake, "product");
        // The skill resolves the exact failure from the injected metadata.
        assert!(seed.contains("#916"), "seed names the PR");
        assert!(seed.contains("jack/ship"), "seed names the branch");
        assert!(seed.contains("headsha"), "seed names the head commit");
        assert!(
            seed.contains("rust-test"),
            "seed names the failing leaf check"
        );
        assert!(seed.contains("https://ci/rust"), "seed carries the log URL");
        assert!(
            seed.contains("ci-fix skill"),
            "seed points at the ci-fix skill"
        );

        // The seed follows the claimed incident, not the mutable observation row.
        // When they disagree the incident wins, or a body repairs a failure other
        // than the one that woke it.
        assert!(
            !seed.contains("movedhead"),
            "seed must not carry the observation's newer head"
        );
        assert!(
            !seed.contains("some-other-check"),
            "seed must not carry the observation's newer failure"
        );
    }

    /// An open PR on head `headsha` with one CI reading. The `current_ci_incident`
    /// mint point is the single gate both the runner's review-preempt check
    /// (`current_ci_incident_identity`, runner.rs) and the idle repair arm
    /// (`arm_ci_fix_wake`) consult, so proving what it warrants proves what would
    /// preempt or reserve a repair Run.
    fn open_pr_with_ci(observation: Option<CiObservation>) -> TaskPr {
        let now = OffsetDateTime::now_utc();
        TaskPr {
            id: TaskPrId::new(),
            task_session_id: TaskSessionId::new(),
            sequence: 1,
            slug: "ship".to_string(),
            branch: "jack/ship".to_string(),
            base_commit: "base".to_string(),
            parent_pr_id: None,
            publication: Some(PrPublication {
                requested_at: now,
                after_merge: AfterMerge::Review,
                next_slug: None,
                github: Some(GithubPr {
                    number: 916,
                    url: "https://github.com/loopflow/loopflow/pull/916".to_string(),
                    head_sha: Some("headsha".to_string()),
                }),
            }),
            merge_commit: None,
            abandoned_at: None,
            ci_observation: observation,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn failing_on(head: &str, checks: Vec<crate::task::CiCheck>) -> CiObservation {
        CiObservation {
            head_sha: head.to_string(),
            state: CiState::Failing,
            failing_checks: checks,
            observed_at: OffsetDateTime::now_utc(),
        }
    }

    /// The runtime wake/preempt gate. A fresh, repairable failure on the current
    /// head warrants an incident (the runner would preempt a parked Feedback or arm
    /// a repair). Green, a stale reading for a past head, and a head red only on a
    /// land-time precondition all warrant nothing — so neither the review-preempt
    /// nor the idle repair path fires for them. This is the composed decision the
    /// runner actually calls; the individual `fresh_ci`/`wake_legal` predicates are
    /// unit-tested in `task::mod`, but their integration through the mint point was
    /// only covered by the deleted CI lifecycle suite.
    #[test]
    fn current_ci_incident_warrants_a_wake_only_for_a_fresh_repairable_failure() {
        let real = crate::task::CiCheck {
            name: "rust-test".to_string(),
            url: Some("https://ci/rust".to_string()),
        };
        let scratch_clear = crate::task::CiCheck {
            name: "scratch-clear".to_string(),
            url: None,
        };

        // A genuine failing required check on the current head: a wake is warranted.
        let actionable = open_pr_with_ci(Some(failing_on("headsha", vec![real.clone()])));
        let incident = crate::ops::task::current_ci_incident(&actionable)
            .expect("a fresh failure warrants a wake");
        assert_eq!(incident.pr_number, 916);
        assert_eq!(incident.failed_head_sha, "headsha");
        assert_eq!(incident.failure_set, vec!["rust-test".to_string()]);

        // Passing: no incident, nothing to preempt for.
        let green = open_pr_with_ci(Some(CiObservation {
            head_sha: "headsha".to_string(),
            state: CiState::Passing,
            failing_checks: Vec::new(),
            observed_at: OffsetDateTime::now_utc(),
        }));
        assert!(crate::ops::task::current_ci_incident(&green).is_none());

        // Stale: the reading is for a head the PR has already moved past. The
        // failure is moot and must never wake or preempt.
        let stale = open_pr_with_ci(Some(failing_on("oldhead", vec![real.clone()])));
        assert!(crate::ops::task::current_ci_incident(&stale).is_none());

        // Red only on a land-time precondition (`scratch-clear`): a repair turn
        // could only delete the reviewer's artifact, so the gate refuses the wake
        // even though the head is genuinely failing. A parked Feedback is not
        // preempted for this reading.
        let land_time_only =
            open_pr_with_ci(Some(failing_on("headsha", vec![scratch_clear.clone()])));
        assert!(crate::ops::task::current_ci_incident(&land_time_only).is_none());

        // But a head failing a land-time precondition *and* a real check still
        // warrants the wake — the mint point must not swallow the actionable
        // failure just because a land-time one rides alongside it.
        let mixed = open_pr_with_ci(Some(failing_on("headsha", vec![scratch_clear, real])));
        assert!(crate::ops::task::current_ci_incident(&mixed).is_some());
    }

    #[tokio::test]
    async fn finish_parked_settles_the_body_without_a_terminal_status() {
        let (store, mut session, lease) = conformance_session("codex").await;
        session.set_status(
            TaskSessionStatus::Waiting,
            "interactive handoff open; waiting for a human",
        );
        store
            .update_task_session_for_run(&session, &lease)
            .await
            .unwrap();

        let outcome = crate::child_session::ChildBodyOutcome::Interrupted {
            reason: session.status_reason.clone(),
        };
        super::finish_parked(&store, &mut session, &lease, None, outcome, None)
            .await
            .unwrap();

        assert_eq!(session.status, TaskSessionStatus::Waiting);
        assert!(!session.status.is_terminal());
        let process = session.latest_process.as_ref().unwrap();
        assert_eq!(
            process.state,
            crate::child_session::ChildLeaseState::Finished
        );
        // The parked body leaves the Session durably non-terminal, so a later
        // resume can reconcile the handoff outcome.
        let persisted = store.get_task_session(&session.id).await.unwrap().unwrap();
        assert_eq!(persisted.status, TaskSessionStatus::Waiting);
    }

    /// A disconnect-class failure with `backup_agent` configured hands the
    /// next generation to the backup, records `BodyHandedOff`, retains the
    /// failed opencode generation, and fences out a late write from the dead
    /// body — all in one test so the recovery contract is visible end-to-end.
    #[tokio::test]
    async fn disconnect_failure_with_backup_hands_off_and_fences_old_writer() {
        let repo = tempfile::tempdir().unwrap();
        let wave_name = "wave-opencode";
        let wave_dir = repo.path().join("wave").join(wave_name);
        std::fs::create_dir_all(&wave_dir).unwrap();
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nbackup_agent: claude:opus\n---\n\n# Goal\n",
        )
        .unwrap();

        let store_path = repo.path().join("registry.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(store_path))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            wave_name.to_string(),
            repo.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();

        let now = OffsetDateTime::from_unix_timestamp(OffsetDateTime::now_utc().unix_timestamp())
            .unwrap();
        let project_snapshot = LinearProjectSnapshot {
            id: LinearProjectId::new("project-opencode").unwrap(),
            slug: "control".to_string(),
            name: "Control".to_string(),
            prompt_context: "test".to_string(),
        };
        let project = ProjectSession {
            id: ProjectSessionId::new(),
            launch: ProjectLaunchReceipt {
                project: project_snapshot.clone(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            status: ProjectSessionStatus::Created,
            status_reason: "reserved".to_string(),
            status_at: now,
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "opencode".to_string(),
            provider: "opencode".to_string(),
            provider_session_id: None,
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project_session(&project).await.unwrap();

        let mut session = TaskSession {
            id: TaskSessionId::new(),
            launch: TaskLaunchReceipt {
                issue: LinearIssueSnapshot {
                    id: LinearIssueId::new("issue-opencode").unwrap(),
                    identifier: "OP-123".to_string(),
                    title: "Conformance".to_string(),
                    description: "test".to_string(),
                },
                project: project_snapshot,
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_session_id: project.id,
            status: TaskSessionStatus::Waiting,
            status_reason: "ready".to_string(),
            status_at: now,
            worktree: PathBuf::from(repo.path().join("worktree").display().to_string()),
            workspace_slug: "test-opencode".to_string(),
            lifecycle: TaskLifecyclePlan::standard("task"),
            lifecycle_phase: TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "opencode:glm-5.2".to_string(),
            provider: "opencode".to_string(),
            provider_session_id: Some("provider-session".to_string()),
            latest_process: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: crate::task::Observation::NotRequired,
        };
        let pr = crate::task::TaskPr {
            id: crate::task::TaskPrId::new(),
            task_session_id: session.id.clone(),
            sequence: 1,
            slug: session.workspace_slug.clone(),
            branch: "test/opencode".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            created_at: now,
            updated_at: now,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
        };
        store.create_task_session(&session, &pr).await.unwrap();
        session.begin_generation("lf-task-opencode".to_string());
        let reservation = store
            .reserve_task_process(&session, TaskSessionStatus::Waiting)
            .await
            .unwrap()
            .unwrap();
        let lease = store
            .resolve_run_lease(reservation.run_token.clone())
            .await
            .unwrap();
        if let Some(process) = &mut session.latest_process {
            process.state = crate::child_session::ChildLeaseState::Active;
        }
        session.set_status(TaskSessionStatus::Running, "provider active");
        store
            .activate_task_process_for_run(&session, &lease)
            .await
            .unwrap();

        // Drive a disconnect-class failure with the backup configured.
        let mut harness = ScriptedHarness::new(false);
        let result = handle_body_failure(
            &store,
            &mut session,
            &lease,
            &mut harness,
            &wave,
            "opencode_disconnected: stream died mid-turn",
            true, // durable side effect → backup is the preferred path
            None,
        )
        .await;

        // The body handed off, not failed — Ok(()) so the supervisor
        // relaunches with the backup agent.
        assert!(result.is_ok(), "handoff should return Ok");

        // The session now carries the backup agent.
        assert_eq!(session.agent, "claude:opus");
        assert_eq!(session.provider, "claude");
        assert_eq!(
            session.provider_session_id, None,
            "provider session cleared on cross-provider handoff"
        );

        // The replacement is reserved in the same Run. The failed provider is
        // retained by the ended Launch, not copied into the next controller.
        let process = session.latest_process.as_ref().expect("process retained");
        assert_eq!(
            process.state,
            crate::child_session::ChildLeaseState::Reserved
        );
        assert_eq!(process.outcome, None);

        // The BodyHandedOff event is in the ledger.
        let events = store.task_events_after(&session.id, 0).await.unwrap();
        assert!(
            events.iter().any(|event| matches!(
                &event.kind,
                TaskEventKind::BodyHandedOff { handoff }
                    if handoff.from_agent == "opencode:glm-5.2"
                        && handoff.to_agent == "claude:opus"
                        && handoff.reason.contains("disconnect-class failure")
            )),
            "BodyHandedOff event must be recorded; events: {events:?}"
        );

        // Fencing: a late write from the dead opencode body is rejected.
        // The process is Finished, so the old lease can no longer update.
        let mut late_session = store.get_task_session(&session.id).await.unwrap().unwrap();
        late_session.status_reason = "late write from dead body".to_string();
        let write_result = store
            .update_task_session_for_run(&late_session, &lease)
            .await;
        assert!(
            write_result.is_err(),
            "a late write from the dead generation must be fenced out"
        );
    }
}
