use std::collections::VecDeque;
use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::child::{ChildBodyHandoff, ChildRef};
use crate::child_control::{
    absorb_run_control, apply_input as apply_child_input, input_is_current,
    send_outstanding_steers, CommandStop, PendingInput,
};
use crate::durable::{AttentionRoute, Basis, BoundarySeed, FlowPosition, RunLease};
use crate::engine::wave_config::read_wave_config;
use crate::harness::{
    classify_disconnect_recovery, drain_turn_failure_reason, ApprovalPolicy, Harness,
    RecoveryDecision,
};
use crate::planning::ProjectPlan;
use crate::project::Project;
use crate::provider_account::recovery::{
    capability_key, plan_run_route_recovery, settle_route_recovery, stop_launch_for_recovery,
    ExactRoute, RecoveryChoice, RecoverySettlement, RecoveryStopOutcome,
};
use crate::store::SharedStore;
use crate::task::{
    CiCheck, FeedbackReviewer, Observation, PrPhase, Task, TaskEventKind, TaskGateProposal, TaskId,
    TaskLifecyclePhase,
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

pub(crate) async fn run(store: SharedStore, task_id: TaskId, lease: &RunLease) -> Result<()> {
    let result = run_task_with(
        store.clone(),
        task_id.clone(),
        lease,
        Box::new(crate::harness::default_create_harness),
    )
    .await;
    if let Err(error) = &result {
        record_unhandled_failure(&store, &task_id, lease, error).await;
    }
    result
}

async fn owning_wave(store: &SharedStore, task: &Task) -> Result<Wave> {
    store
        .get_wave(&task.wave_id)
        .await?
        .ok_or_else(|| anyhow!("owning Wave {} is not registered", task.wave_id))
}

async fn owning_project(store: &SharedStore, task: &Task) -> Result<Project> {
    store
        .get_project(&task.project_id)
        .await?
        .ok_or_else(|| anyhow!("owning Project {} is not registered", task.project_id))
}

async fn spawn_failover(
    store: &SharedStore,
    task: &Task,
    lease: &RunLease,
    route: &ExactRoute,
) -> Result<()> {
    let tmux_name = format!(
        "lf-task-{}-{}",
        &task.id.as_str()[3..11],
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    crate::ops::launch_in_run(
        store,
        lease,
        crate::ops::RunLaunch {
            work: crate::durable::WorkRef::Task(task.id.clone()),
            wave_id: task.wave_id.clone(),
            cwd: task.worktree.clone(),
            tmux_name,
            agent: route.agent.agent(),
            account_id: route.account_id.clone(),
            resume_token: task.provider_session_id.clone(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| anyhow!(error.to_string()))
}

async fn run_task_with(
    store: SharedStore,
    task_id: TaskId,
    lease: &RunLease,
    create_harness: crate::harness::CreateHarness,
) -> Result<()> {
    let mut task = store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} not found"))?;
    let wave = owning_wave(&store, &task).await?;
    store.update_task_for_run(&task, lease).await?;
    store
        .append_task_event_for_run(&task.id, lease, &TaskEventKind::Started)
        .await?;
    let target = ChildRef::Task(task.id.clone());
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
    let mut run_control = crate::trace::ControlLaunch {
        run_id: run.id,
        home_id: run.home_id,
        account_id: launch.route.account_id.clone(),
        containment: launch.containment.clone(),
        resume_token: launch.resume_token.clone(),
        opaque_basis: launch.opaque_basis.clone(),
    };
    // Typed current-head evidence selects ci-fix before ordinary lifecycle work.
    // The exact Run claim is the crash/recovery fence; no command row mediates it.
    let mut ci_fix_wake = arm_ci_fix_wake(&store, &task, lease).await?;
    let mut flow = if ci_fix_wake.is_some() {
        Playhead::new(QueuedInvocation::load(&task.worktree, "ci-fix")?).0
    } else {
        resume_task_phase(&task)?
    };
    let prepared = prepare_task_flow_step(
        &store,
        &mut task,
        lease,
        wave.name(),
        &flow,
        ci_fix_wake.as_ref(),
    )
    .await?;
    let (harness_name, _) = crate::engine::config::parse_agent(&task.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(task.provider_session_id.clone());
    let requested_account = launch
        .route
        .account_id
        .as_deref()
        .map(crate::store::ProviderAccountId::parse)
        .transpose()
        .map_err(|reason| anyhow!("invalid Launch account route: {reason}"))?;
    harness.set_provider_account_id(requested_account);
    store.validate_run_lease(lease).await?;
    harness.start(&prepared.turn.config).await?;
    task.provider = harness_name;
    task.provider_session_id = harness.provider_session_id();
    let launch = store
        .observe_launch_provider(
            lease,
            &launch.id,
            harness.provider_account_id(),
            task.provider_session_id.clone(),
        )
        .await?;
    run_control.account_id = launch.route.account_id.clone();
    run_control.resume_token = launch.resume_token.clone();
    if let Err(error) = store.update_task_for_run(&task, lease).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    let mut state_fingerprint = task_state_fingerprint(&task)?;
    let mut iteration_start_head = pr_head_for_task(&store, &task).await?;
    let mut gate_fingerprint = if task.lifecycle_phase == TaskLifecyclePhase::Finally {
        Some(task_gate_fingerprint(&task)?)
    } else {
        None
    };

    let mut pending = VecDeque::new();
    let mut feedback_open = prepared.attention.is_some();
    // Record this body's turns the way `flowloop/wave.rs` does. Without it a
    // Task's spend reaches no store at all: the provider runs in this
    // process, so no child `lf` records on its behalf.
    let capture = flow.current().and_then(|step| {
        let context = crate::journal::trace_capture_context(
            Path::new(&task.worktree),
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
        capture.set_provider_session_id(task.provider_session_id.clone());
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
        apply_next_pending(&store, &task, lease, harness.as_mut(), &mut pending).await?;
    if !provider_turn_active {
        start_task_flow_turn(
            &store,
            &mut task,
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
        task.plan.identifier
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
                    handle_attachment(&store, &task, lease, line).await?;
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
                        &mut task,
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
                        && current_ci_incident_identity(&store, &task).await?.is_some()
                    {
                        harness.interrupt().await?;
                        feedback_preempted = true;
                    }
                    None
                } else if ci_fix_wake.is_none() {
                    arm_ci_fix_wake(&store, &task, lease).await?
                } else {
                    None
                };
                if let Some(wake) = wake {
                    active_basis = start_ci_fix_flow(
                        &store,
                        &mut task,
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
                        &task,
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
                    apply_input(&store, &task, lease, harness.as_mut(), close).await?;
                    active_basis = boundary.basis;
                    provider_turn_active = true;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut task,
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
                if provider_session_id != task.provider_session_id {
                    task.provider_session_id = provider_session_id;
                    store.update_task_for_run(&task, lease).await?;
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
                                &mut task,
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
                            let wake = arm_ci_fix_wake(&store, &task, lease).await?;
                            if let Some(wake) = wake {
                                active_basis = start_ci_fix_flow(
                                    &store,
                                    &mut task,
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
                                &task,
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
                        let mut finally_ops_ran = false;
                        if !flow_iteration_completed
                            && ci_fix_wake.is_none()
                            && flow.current().is_some_and(|step| step.kind == StepKind::Op)
                        {
                            let current_fingerprint = task_gate_fingerprint(&task)?;
                            if gate_fingerprint.as_ref() == Some(&current_fingerprint) {
                                flow_iteration_completed =
                                    run_task_flow_ops(&task, &mut flow).await?;
                                finally_ops_ran = true;
                            } else {
                                // Mechanical finish is the side-effect boundary. A final-flow
                                // skill that changed material work must return through Loop and
                                // be reviewed before any op may publish or land it.
                                flow_iteration_completed = true;
                            }
                        }
                        if flow_turn_active || feedback_body_completed {
                            let latest = store
                                .get_task(&task.id)
                                .await?
                                .ok_or_else(|| {
                                    anyhow!("Task {} disappeared", task.id)
                                })?;
                            sync_task_state(&mut task, &latest);
                            if ci_fix_wake.is_none() {
                                record_task_flow_position(&mut task, &flow)?;
                            }
                            store.update_task_for_run(&task, lease).await?;
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
                                        &mut task,
                                        lease,
                                    )
                                    .await
                                    .map_err(|error| anyhow!(error.to_string()))?;
                                let _ = harness.stop().await;
                                return settle_ci_fix_turn(
                                    &store,
                                    &mut task,
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
                                && task.lifecycle_phase == TaskLifecyclePhase::First
                            {
                                task.enter_loop()?;
                                store.update_task_for_run(&task, lease).await?;
                                flow = resume_task_phase(&task)?;
                                flow_iteration_completed = false;
                                state_fingerprint = task_state_fingerprint(&task)?;
                                gate_fingerprint = None;
                                last_text.clear();
                            }
                            while let Some(input) = pending.pop_front() {
                                if !pending_input_is_current(&store, &task, lease, &input).await? {
                                    continue;
                                }
                                if resume_interrupted_flow {
                                    open_task_flow_body(&mut flow, &task)?;
                                    flow_turn_active = true;
                                }
                                apply_input(
                                    &store,
                                    &task,
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
                                && task.lifecycle_phase == TaskLifecyclePhase::Finally
                            {
                                let next_gate_fingerprint = task_gate_fingerprint(&task)?;
                                if !finally_ops_ran
                                    && gate_fingerprint.as_ref() != Some(&next_gate_fingerprint)
                                {
                                    state_fingerprint = task_state_fingerprint(&task)?;
                                    gate_fingerprint = None;
                                    task.enter_loop()?;
                                    store.update_task_for_run(&task, lease).await?;
                                    let started = start_resumed_task_phase(
                                        &store,
                                        &mut task,
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
                                Some(task.approved_gate_proposal()?)
                            } else {
                                None
                            };
                            if !flow_iteration_completed && status != Lifecycle::Interrupted {
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    wave.name(),
                                    &flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?;
                                let started = start_prepared_task_step(
                                    &store,
                                    &mut task,
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
                                .get_task(&task.id)
                                .await?
                                .ok_or_else(|| anyhow!("Task {} disappeared", task.id))?;
                            sync_task_state(&mut task, &latest);
                            let observed_pr = crate::ops::task::reconcile_task_pr_for_run(
                                &store,
                                &mut task,
                                lease,
                            )
                            .await
                            .map_err(|error| anyhow!(error.to_string()))?;
                            // Reconcile keeps the cached PR row through a GitHub
                            // outage and names the failure on the task. For a
                            // turn that just ran, that degraded reading is an
                            // infrastructure blocker: it could not have verified
                            // a repair.
                            let github_degraded = match &task.observation {
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
                            // this PR as merged and waits on its explicit Task Gate
                            // Feedback checkpoint, which is false of an abandoned one.
                            let merged_completing_pr = observed_pr.as_ref().is_some_and(|pr| {
                                pr.phase() == crate::task::PrPhase::Merged
                                    && pr.after_merge()
                                        == crate::task::AfterMerge::CompleteTask
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
                                store.active_task_pr(&task.id).await?.is_none()
                            } else {
                                false
                            };
                            let (stopped_done, stopped_reason) = if let Some(proposal) = approved_gate {
                                (proposal.done, proposal.reason)
                            } else if status == Lifecycle::Interrupted {
                                (
                                    false,
                                    "Task flow step interrupted; waiting for resume or another instruction".to_string(),
                                )
                            } else if merged_completing_pr {
                                // The PR merged to complete the Task, but its authored Task
                                // Gate Feedback checkpoint is still open. Wait for explicit
                                // continuation before completion; do not rotate another PR.
                                let number = observed_pr
                                    .as_ref()
                                    .and_then(|pr| pr.github())
                                    .map(|github| github.number);
                                let reason = match number {
                                    Some(number) => format!(
                                        "pull request #{number} merged; awaiting Task Gate Feedback continuation before completion"
                                    ),
                                    None => "pull request merged; awaiting Task Gate Feedback continuation before completion"
                                        .to_string(),
                                };
                                (false, reason)
                            } else if needs_rotation {
                                crate::ops::task::ensure_working_pr_for_run(
                                    &store,
                                    &mut task,
                                    lease,
                                )
                                .await
                                .map_err(|error| anyhow!(error.to_string()))?;
                                store.update_task_for_run(&task, lease).await?;
                                let prepared = prepare_task_flow_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    wave.name(),
                                    &flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?;
                                let started = start_prepared_task_step(
                                    &store,
                                    &mut task,
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
                                    let (_disposition, reason) =
                                        crate::ops::task::decide_open_pr_status(
                                            pr,
                                            github_degraded.as_deref(),
                                            head_advanced,
                                        );
                                    (false, reason)
                                }
                            } else {
                                let next_fingerprint = task_state_fingerprint(&task)?;
                                if next_fingerprint != state_fingerprint {
                                    state_fingerprint = next_fingerprint;
                                    store.update_task_for_run(&task, lease).await?;
                                    let prepared = prepare_task_flow_step(
                                        &store,
                                        &mut task,
                                        lease,
                                        wave.name(),
                                        &flow,
                                        ci_fix_wake.as_ref(),
                                    )
                                    .await?;
                                    let started = start_prepared_task_step(
                                        &store,
                                        &mut task,
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
                                    false,
                                    "Task flow completed without a PR or any worktree change; another automatic iteration would spin".to_string(),
                                )
                            };
                            if task.lifecycle_phase == TaskLifecyclePhase::Loop
                                && status != Lifecycle::Interrupted
                            {
                                let waiting_for_ci = observed_pr.as_ref().is_some_and(|pr| {
                                    pr.phase() == PrPhase::Open
                                        && pr.merge_request().is_some()
                                        && !pr.merge_checks_passed()
                                });
                                task.enter_finally(TaskGateProposal {
                                    done: stopped_done,
                                    reason: stopped_reason,
                                })?;
                                if waiting_for_ci {
                                    let number = observed_pr
                                        .as_ref()
                                        .and_then(|pr| pr.github())
                                        .map(|github| github.number);
                                    let reason = match number {
                                        Some(number) => format!(
                                            "pull request #{number} is waiting for fresh passing required checks before its requested merge"
                                        ),
                                        None => "pull request is waiting for fresh passing required checks before its requested merge"
                                            .to_string(),
                                    };
                                    tracing::info!(task = %task.id, %reason, "Task waiting for CI");
                                    return finish_parked(
                                        &store,
                                        &mut task,
                                        lease,
                                        Some(harness.as_mut()),
                                        crate::durable::BoundaryState::Succeeded,
                                        capture.as_ref(),
                                    )
                                    .await;
                                }
                                gate_fingerprint = Some(task_gate_fingerprint(&task)?);
                                store.update_task_for_run(&task, lease).await?;
                                let started = start_resumed_task_phase(
                                    &store,
                                    &mut task,
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
                            store.update_task_for_run(&task, lease).await?;
                            let _ = harness.stop().await;
                            if !summary.is_empty() {
                                store.append_task_event_for_run(
                                    &task.id,
                                    lease,
                                    &TaskEventKind::Progress {
                                        summary: summary.clone(),
                                    },
                                ).await?;
                            }
                            if stopped_done {
                                store.append_task_event_for_run(
                                    &task.id,
                                    lease,
                                    &TaskEventKind::Completed { summary },
                                ).await?;
                            }
                            let launch = store.current_launch(lease).await?.ok_or_else(|| {
                                anyhow!("Task Run {} has no Launch to finish", lease.run_id)
                            })?;
                            store.advance_run(
                                lease,
                                crate::durable::RunAdvance::LaunchEnded {
                                    launch_id: launch.id,
                                    outcome: crate::durable::BoundaryState::Succeeded,
                                },
                            ).await?;
                            if stopped_done {
                                store.done(lease, &active_basis).await?;
                            } else {
                                store.finish_task_run(
                                    &task,
                                    lease,
                                    crate::durable::BoundaryState::Succeeded,
                                ).await?;
                            }
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message, .. } => {
                        let reason = format!("{code}: {message}");
                        return fail_and_maybe_relaunch(
                            &store,
                            &mut task,
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
    task: &mut Task,
    lease: &RunLease,
    wave_name: &str,
    flow: &Playhead,
    ci_fix: Option<&CiFixWake>,
) -> Result<PreparedTaskStep> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
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
    store.update_task_for_run(task, lease).await?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = owning_project(store, task).await?;
    // The `ci-fix` step gets the failure seed from the typed incident claimed by
    // this Run; every other Task-flow step gets the standard task seed. The flow
    // and incident are chosen together, so a `ci-fix` step without one is invalid.
    let seed = match (step.step.as_str(), ci_fix) {
        ("ci-fix", Some(wake)) => format!(
            "{}\n\n{}",
            ci_fix_seed(task, &pr, wake, wave_name),
            boundary.render()
        ),
        ("ci-fix", None) => {
            anyhow::bail!(
                "Task {} is running the ci-fix flow with no claimed ci-fix wake",
                task.id
            )
        }
        _ => task_seed(task, &project.plan, &pr, wave_name, &boundary),
    };
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave_name, None)?;
    prepared.config.agent = Some(task.agent.clone());
    let skill = crate::engine::load_skill(&step.step, Path::new(&task.worktree))?;
    let attention = if step.feedback {
        let route = match task.phase_plan().reviewer {
            FeedbackReviewer::User => AttentionRoute::User,
            FeedbackReviewer::Parent => AttentionRoute::Parent(
                store
                    .work_for_child(&ChildRef::Project(task.project_id.clone()))
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
        store.update_task_for_run(task, lease).await?;
        Some(route)
    } else {
        None
    };
    let position = FlowPosition {
        work,
        epoch_id: boundary.basis.epoch_id.clone(),
        flow: task.phase_plan().flow.clone(),
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

fn open_task_flow_body(flow: &mut Playhead, task: &Task) -> Result<()> {
    let step = flow
        .current()
        .ok_or_else(|| anyhow!("Task flow has no current step"))?;
    if step.kind != StepKind::Skill {
        anyhow::bail!("Task flow step {} is not a skill", step.step);
    }
    flow.start_body(BodyProvenance::for_step(&step, &task.worktree))?;
    Ok(())
}

async fn start_task_flow_turn(
    _store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    open_task_flow_body(flow, task)?;
    apply_input(_store, task, lease, harness, &prepared.input).await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn start_prepared_task_step(
    store: &SharedStore,
    task: &mut Task,
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
    start_task_flow_turn(store, task, lease, harness, flow, prepared.turn).await?;
    Ok(StartedTaskStep {
        feedback: prepared.attention.is_some(),
        provider_turn_active: true,
        basis: Some(prepared.basis),
    })
}

#[allow(clippy::too_many_arguments)]
async fn start_resumed_task_phase(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    wave_name: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<StartedTaskStep> {
    *flow = resume_task_phase(task)?;
    let prepared = prepare_task_flow_step(store, task, lease, wave_name, flow, None).await?;
    start_prepared_task_step(store, task, lease, harness, flow, capture, prepared).await
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

async fn run_task_flow_ops(task: &Task, flow: &mut Playhead) -> Result<bool> {
    if task.lifecycle_phase != TaskLifecyclePhase::Finally {
        anyhow::bail!(
            "Task {} {} flow reached an op; only finally flows may run mechanical ops",
            task.id,
            task.lifecycle_phase.as_str()
        );
    }
    while let Some(step) = flow.current() {
        if step.kind != StepKind::Op {
            return Ok(false);
        }
        let definition = crate::engine::load_flow(&step.flow, &task.worktree)?;
        let items = crate::engine::expand_flow(&definition, &task.worktree)?;
        let op = match items.get(step.index as usize) {
            Some(crate::engine::ConcreteStep::Op(op)) => op.clone(),
            Some(item) => {
                anyhow::bail!(
                    "Task flow step {} was planned as an op but expanded to {item:?}",
                    step.step
                )
            }
            None => anyhow::bail!("Task flow op {} is outside its expanded flow", step.step),
        };
        let body = BodyProvenance::for_step(&step, &task.worktree);
        let body_id = body.body_id.clone();
        flow.start_body(body)?;
        let worktree = task.worktree.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::ops::execute_flow_ops(&worktree, &op.item, &crate::ops::NullProgress)
        })
        .await
        .map_err(|error| anyhow!("Task flow op worker failed: {error}"))?;
        if let Err(error) = result {
            flow.finish_body(&body_id, StepOutcome::Failed, &error.to_string())?;
            return Err(anyhow!(error.to_string()));
        }
        let events = flow.finish_body(&body_id, StepOutcome::Completed, "completed")?;
        if events
            .iter()
            .any(|event| matches!(event, PlayheadEvent::InvocationCompleted { .. }))
        {
            return Ok(true);
        }
    }
    Ok(true)
}

/// End a parked body while Work stays open. The caller supplies
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
    task: &mut Task,
    lease: &RunLease,
    harness: Option<&mut dyn Harness>,
    outcome: crate::durable::BoundaryState,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "completed");
    if let Some(harness) = harness {
        let _ = harness.stop().await;
    }
    store.finish_task_run(task, lease, outcome).await?;
    Ok(())
}

fn record_task_flow_position(task: &mut Task, flow: &Playhead) -> Result<()> {
    let root = flow
        .stack
        .first()
        .ok_or_else(|| anyhow!("Task flow has no root invocation"))?;
    if root.flow != task.phase_plan().flow {
        anyhow::bail!(
            "Task {} {} flow is {:?}, but its playhead is {:?}",
            task.id,
            task.lifecycle_phase.as_str(),
            task.phase_plan().flow,
            root.flow
        );
    }
    task.phase_cursor = root.cursor;
    task.phase_iteration = root.iteration;
    task.updated_at = time::OffsetDateTime::now_utc();
    Ok(())
}

fn resume_task_phase(task: &Task) -> Result<Playhead> {
    let (flow, _) = Playhead::resume_root(
        QueuedInvocation::load(&task.worktree, &task.phase_plan().flow)?,
        task.phase_cursor,
        task.phase_iteration,
    )?;
    Ok(flow)
}

fn sync_task_state(task: &mut Task, latest: &Task) {
    task.pm_writeback = latest.pm_writeback.clone();
    task.gate_proposal = latest.gate_proposal.clone();
}

fn task_state_fingerprint(task: &Task) -> Result<String> {
    let state = crate::engine::git::worktree_state(Path::new(&task.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

/// The active PR's current head SHA, or `None` when there is no active PR.
/// Captured at iteration boundaries as a GitHub-side progress baseline so the
/// runner can tell a no-change ci-fix (head unchanged) from a push (head
/// advanced) without relying on worktree churn.
async fn pr_head_for_task(store: &SharedStore, task: &Task) -> Result<Option<String>> {
    Ok(store
        .active_task_pr(&task.id)
        .await?
        .and_then(|pr| pr.github().map(|g| g.head_sha.clone()))
        .flatten())
}

fn task_gate_fingerprint(task: &Task) -> Result<String> {
    let state = crate::engine::git::material_worktree_state(Path::new(&task.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

async fn pending_input_is_current(
    _store: &SharedStore,
    _task: &Task,
    _lease: &RunLease,
    input: &PendingInput,
) -> Result<bool> {
    input_is_current(input).await
}

async fn apply_next_pending(
    store: &SharedStore,
    task: &Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    pending: &mut VecDeque<PendingInput>,
) -> Result<bool> {
    while let Some(input) = pending.pop_front() {
        if !pending_input_is_current(store, task, lease, &input).await? {
            continue;
        }
        apply_input(store, task, lease, harness, &input.text).await?;
        return Ok(true);
    }
    Ok(false)
}

async fn handle_attachment(
    store: &SharedStore,
    task: &Task,
    lease: &RunLease,
    line: String,
) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await?;
        println!(
            "{}  {:?}",
            task.plan.identifier,
            store.work_status(&work).await?
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
    let target = ChildRef::Task(task.id.clone());
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
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    wake: &CiFixWake,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<Basis> {
    *flow = Playhead::new(QueuedInvocation::load(&task.worktree, "ci-fix")?).0;
    let wave = owning_wave(store, task).await?;
    let prepared =
        prepare_task_flow_step(store, task, lease, wave.name(), flow, Some(wake)).await?;
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    let basis = prepared.basis;
    start_task_flow_turn(store, task, lease, harness, flow, prepared.turn).await?;
    Ok(basis)
}

async fn record_unhandled_failure(
    store: &SharedStore,
    task_id: &TaskId,
    lease: &RunLease,
    error: &anyhow::Error,
) {
    let Ok(Some(task)) = store.get_task(task_id).await else {
        return;
    };
    let Ok(work) = store.work_for_child(&ChildRef::Task(task.id.clone())).await else {
        return;
    };
    if lease.work != work {
        return;
    }
    let message = format!("task process failed: {error}");
    let _ = store
        .append_task_event_for_run(
            &task.id,
            lease,
            &TaskEventKind::Failed {
                error: message.clone(),
                resumable: true,
            },
        )
        .await;
    let _ = store
        .finish_task_run(&task, lease, crate::durable::BoundaryState::Failed)
        .await;
}

async fn apply_input(
    store: &SharedStore,
    _task: &Task,
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

async fn finish_failed(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    error: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store
        .append_task_event_for_run(
            &task.id,
            lease,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
    store
        .finish_task_run(task, lease, crate::durable::BoundaryState::Failed)
        .await?;
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
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    capability: &str,
    detail: &str,
) -> Result<()> {
    let _ = harness.stop().await;
    let pr_number = store
        .active_task_pr(&task.id)
        .await?
        .and_then(|pr| pr.github().map(|g| g.number));
    let reason = infra_blocked_reason(capability, detail, pr_number);
    if let Some(pr) = store.active_task_pr(&task.id).await? {
        store
            .mark_ci_incidents_blocked(&pr.id, time::OffsetDateTime::now_utc(), &reason)
            .await?;
    }
    store
        .finish_task_run(task, lease, crate::durable::BoundaryState::Failed)
        .await?;
    Ok(())
}

/// Recover a retryable body failure through the Run's next exact route after
/// PRD-38 permits replacement and the current containment stops positively.
#[allow(clippy::too_many_arguments)] // capture is a terminal-path output, not a knob
async fn handle_body_failure(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<Option<(RunLease, ExactRoute)>> {
    finish_capture(capture, "failed");
    let wave_config = read_wave_config(Path::new(wave.repo()), wave.name());
    let backup_agent = wave_config.as_ref().and_then(|c| c.backup_agent.as_deref());
    let decision = classify_disconnect_recovery(
        reason,
        &task.agent,
        turn_had_durable_side_effect,
        backup_agent,
    );

    let route_recovery_permitted = matches!(
        decision,
        RecoveryDecision::HandoffToBackup { .. } | RecoveryDecision::AllowRetry
    ) || (matches!(decision, RecoveryDecision::Normal)
        && !turn_had_durable_side_effect
        && crate::engine::agent::classify_retryable_agent_failure(reason).is_some());

    if route_recovery_permitted {
        let launch = store
            .current_launch(lease)
            .await?
            .ok_or_else(|| anyhow!("Task Run {} has no Launch to hand back", lease.run_id))?;
        let current_route = ExactRoute::try_from(&launch.route)?;
        let stopped = match stop_launch_for_recovery(store, lease, harness).await? {
            RecoveryStopOutcome::Stopped(stopped) => stopped,
            RecoveryStopOutcome::Fenced { error, stop } => {
                tracing::error!(task = %task.id, containment = ?stop.containment, %error, "Task recovery left the Run fenced");
                return Ok(None);
            }
        };
        let choice = plan_run_route_recovery(store, lease, backup_agent).await?;
        let failure = match &choice {
            RecoveryChoice::Launch(_) => reason.to_string(),
            RecoveryChoice::AwaitCapability { reasons } => format!(
                "{reason}; waiting on provider route capability: {}",
                capability_key(reasons)
            ),
        };
        store
            .append_task_event_for_run(
                &task.id,
                lease,
                &TaskEventKind::Failed {
                    error: failure,
                    resumable: true,
                },
            )
            .await?;
        store.update_task_for_run(task, lease).await?;
        return match settle_route_recovery(store, lease, stopped, choice).await? {
            RecoverySettlement::Launch {
                lease: rotated,
                route,
            } => {
                let agent = route.agent.agent();
                let provider = route.agent.provider.clone();
                let handoff = ChildBodyHandoff {
                    from_agent: task.agent.clone(),
                    to_agent: agent.clone(),
                    from_provider: task.provider.clone(),
                    to_provider: provider.clone(),
                    reason: format!("route recovery after {reason}"),
                };
                if current_route.agent.provider != route.agent.provider
                    || current_route.account_id != route.account_id
                {
                    task.provider_session_id = None;
                }
                task.agent = agent;
                task.provider = provider;
                store.update_task_for_run(task, &rotated).await?;
                store
                    .append_task_event_for_run(
                        &task.id,
                        &rotated,
                        &TaskEventKind::BodyHandedOff { handoff },
                    )
                    .await?;
                Ok(Some((rotated, route)))
            }
            RecoverySettlement::AwaitCapability { wait } => {
                tracing::info!(task = %task.id, wait = %wait.id, "Task waiting for a provider route capability");
                Ok(None)
            }
        };
    }

    match decision {
        RecoveryDecision::Stop => {
            let non_convergence = format!(
                "{reason}; not replay-safe (durable side effects this turn) and no backup agent configured"
            );
            finish_failed(store, task, lease, harness, &non_convergence, None)
                .await
                .map(|_| None)
        }
        RecoveryDecision::AllowRetry => finish_failed(store, task, lease, harness, reason, None)
            .await
            .map(|_| None),
        RecoveryDecision::Normal => {
            // Not a disconnect-class failure — a provider outage during a
            // PR/ci-fix iteration is an infrastructure blocker: keep the PR
            // attached and block actionably so a resume when the provider
            // recovers picks up the same PR. Without a PR, fall back to the
            // generic failed path.
            if store.active_task_pr(&task.id).await?.is_some() {
                return finish_infra_blocked(store, task, lease, harness, "provider", reason)
                    .await
                    .map(|_| None);
            }
            finish_failed(store, task, lease, harness, reason, None)
                .await
                .map(|_| None)
        }
        RecoveryDecision::HandoffToBackup { .. } => unreachable!(
            "backup handoff is consumed by route recovery before ordinary failure handling"
        ),
    }
}

#[allow(clippy::too_many_arguments)]
async fn fail_and_maybe_relaunch(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    let Some((rotated, route)) = handle_body_failure(
        store,
        task,
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
    spawn_failover(store, task, &rotated, &route).await
}

async fn finish_abandoned(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    _reason: String,
) -> Result<()> {
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    store
        .finish_task_run(task, lease, crate::durable::BoundaryState::Interrupted)
        .await?;
    Ok(())
}

async fn finish_command_stop(
    store: &SharedStore,
    task: &mut Task,
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
            store
                .finish_task_run(task, lease, crate::durable::BoundaryState::Interrupted)
                .await?;
            Ok(())
        }
        CommandStop::Abandoned(reason) => {
            finish_abandoned(store, task, lease, harness, reason).await
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
async fn current_ci_incident_identity(store: &SharedStore, task: &Task) -> Result<Option<String>> {
    Ok(store
        .active_task_pr(&task.id)
        .await?
        .as_ref()
        .and_then(crate::ops::task::current_ci_incident)
        .map(|incident| incident.identity))
}

async fn arm_ci_fix_wake(
    store: &SharedStore,
    task: &Task,
    lease: &crate::durable::RunLease,
) -> Result<Option<CiFixWake>> {
    let Some(pr) = store.active_task_pr(&task.id).await? else {
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
    task: &mut Task,
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
    let reason = match observed_pr.filter(|pr| pr.phase() == PrPhase::Open) {
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
            // Reconcile names a degraded read on the task; for a turn that just
            // ran, that reading could not have verified a repair.
            let degraded = match &task.observation {
                Observation::Degraded { reason, .. } => Some(reason.as_str()),
                _ => None,
            };
            let (disposition, reason) =
                crate::ops::task::decide_open_pr_status(pr, degraded, head_advanced);
            if matches!(
                disposition,
                Some(crate::ops::task::OpenPrDisposition::ObservationDegraded)
                    | Some(crate::ops::task::OpenPrDisposition::NeedsDirection)
            ) {
                store
                    .mark_ci_incidents_blocked(&pr.id, time::OffsetDateTime::now_utc(), &reason)
                    .await?;
            }
            reason
        }
        Some(_) => format!(
            "ci-fix turn on pull request #{} was interrupted; the repair resumes on resume",
            wake.pr_number
        ),
        None => format!(
            "pull request #{} settled or is no longer attached; the ci-fix wake no longer applies",
            wake.pr_number
        ),
    };

    tracing::info!(task = %task.id, %reason, "ci-fix Run settled");
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
        crate::durable::BoundaryState::Interrupted
    } else {
        crate::durable::BoundaryState::Succeeded
    };
    finish_parked(store, task, lease, None, outcome, capture).await
}

/// The seed for a `ci-fix` turn: the PR the skill must repair plus the failing
/// required checks (names + log URLs) so it resolves the exact failure on the
/// current head without re-deriving it.
///
/// The selected incident is immutable even after the PR's current observation
/// moves on, so the seed and settlement name the same failed head.
fn ci_fix_seed(task: &Task, pr: &crate::task::TaskPr, wake: &CiFixWake, wave_name: &str) -> String {
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
         Wave: {wave}\nTask: {task_id}\nWorktree: {worktree}\n\n\
         Push fixes to the same branch; do not open a new PR or rotate the serial branch. When the push lands, the Task returns to waiting on the new head.",
        identifier = task.plan.identifier,
        number = number,
        url = url,
        branch = pr.branch,
        head = head,
        failing = if failing.is_empty() { "- (none reported)".to_string() } else { failing },
        wave = wave_name,
        task_id = task.id,
        worktree = task.worktree.display(),
    )
}

fn task_seed(
    task: &Task,
    project: &ProjectPlan,
    pr: &crate::task::TaskPr,
    wave_name: &str,
    boundary: &BoundarySeed,
) -> String {
    let placement = pr
        .parent_pr_id
        .as_ref()
        .map(|parent| format!("Stack parent PR: {parent} (land the parent first)"))
        .unwrap_or_else(|| "Stack parent PR: none (rooted on main)".to_string());
    let gate_proposal = task
        .gate_proposal
        .as_ref()
        .map(|proposal| {
            format!(
                "Gate proposal: {} — {}",
                if proposal.done { "done" } else { "continue" },
                proposal.reason
            )
        })
        .unwrap_or_else(|| "Gate proposal: none".to_string());
    format!(
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\n{direction}\n\nTask directive snapshot synced at: {task_snapshot_synced_at}\nProject definition snapshot synced at: {project_snapshot_synced_at}\nWave: {wave}\nTask: {task_id}\nLifecycle phase: {lifecycle_phase} (epoch {phase_epoch}, gate cycle {gate_cycle})\nFeedback reviewer: {reviewer}\n{gate_proposal}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}\n\nThis PR owns one serial branch. The pinned finally flow owns landing and Task completion. `lf pr abandon` discards only this PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward. The runner owns branch rotation between PRs.",
        identifier = task.plan.identifier,
        title = task.plan.title,
        description = task.plan.description,
        project = project.name,
        project_id = project.id.as_str(),
        project_context = project.prompt_context,
        direction = boundary.render(),
        task_snapshot_synced_at = task.plan.pm_snapshot_synced_at,
        project_snapshot_synced_at = project.pm_snapshot_synced_at,
        wave = wave_name,
        task_id = task.id,
        lifecycle_phase = task.lifecycle_phase.as_str(),
        phase_epoch = task.phase_epoch,
        gate_cycle = task.gate_cycle,
        reviewer = task.phase_plan().reviewer.as_str(),
        gate_proposal = gate_proposal,
        worktree = task.worktree.display(),
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
mod planning_tests {
    use super::task_seed;
    use crate::durable::{Basis, BoundarySeed, EpochId};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::task::{
        Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
        TaskPrId,
    };

    #[test]
    fn task_seed_uses_the_current_parent_project_definition() {
        let now = time::OffsetDateTime::UNIX_EPOCH;
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("issue-1").unwrap(),
                identifier: "INF-123".to_string(),
                title: "Ship it".to_string(),
                description: "Task direction".to_string(),
                pm_snapshot_synced_at: 11,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: crate::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
            lifecycle: TaskLifecyclePlan::standard("task"),
            lifecycle_phase: TaskLifecyclePhase::Iterate,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
            observation: Observation::NotRequired,
        };
        let pr = TaskPr {
            id: TaskPrId::new(),
            task_id: task.id.clone(),
            sequence: 1,
            slug: "ship-it".to_string(),
            branch: "jack/ship-it".to_string(),
            base_commit: "deadbeef".to_string(),
            parent_pr_id: None,
            publication: None,
            merge_commit: None,
            abandoned_at: None,
            ci_observation: None,
            github_observation: None,
            linear_attachment_id: None,
            linear_comment_id: None,
            linear_link_error: None,
            created_at: now,
            updated_at: now,
        };
        let project = ProjectPlan {
            id: LinearProjectId::new("project-1").unwrap(),
            slug: "runtime".to_string(),
            name: "Current project name".to_string(),
            prompt_context: "Current project definition".to_string(),
            pm_snapshot_synced_at: 22,
        };
        let boundary = BoundarySeed {
            basis: Basis {
                epoch_id: EpochId::new(),
                revision: 0,
            },
            steers: Vec::new(),
        };

        let seed = task_seed(&task, &project, &pr, "wave", &boundary);

        assert!(seed.contains("Current project name"));
        assert!(seed.contains("Current project definition"));
        assert!(seed.contains("Task directive snapshot synced at: 11"));
        assert!(seed.contains("Project definition snapshot synced at: 22"));
    }
}
