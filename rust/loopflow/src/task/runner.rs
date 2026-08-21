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
use crate::durable::{AskId, Basis, BoundarySeed, FlowPosition, RunLease};
use crate::engine::wave_config::read_wave_config;
use crate::harness::{
    classify_disconnect_recovery, drain_turn_failure_reason, ApprovalPolicy, Harness,
    RecoveryDecision,
};
use crate::planning::ProjectPlan;
use crate::project::Project;
use crate::provider_account::recovery::{
    capability_key, plan_run_route_recovery, settle_route_recovery, stop_invocation_for_recovery,
    ExactRoute, RecoveryChoice, RecoverySettlement, RecoveryStopOutcome,
};
use crate::store::SharedStore;
use crate::task::{
    CiCheck, Observation, PrPhase, Task, TaskEventKind, TaskGateProposal, TaskId,
    TaskLifecyclePhase,
};
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

#[derive(Debug)]
struct PreparedTaskStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    position: FlowPosition,
    basis: Basis,
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
    let invocation = store
        .open_invocation(lease)
        .await?
        .ok_or_else(|| anyhow!("Task Run {} has no open Invocation", lease.run_id))?;
    let mut supervision = crate::trace::SupervisedInvocation {
        invocation_id: invocation.id.clone(),
        supervising_run_id: run.id,
        account_id: invocation.route.account_id.clone(),
        resume_token: invocation.resume_token.clone(),
    };
    // Typed current-head evidence selects ci-fix before ordinary lifecycle work.
    // The exact Run claim is the crash/recovery fence; no command row mediates it.
    let mut ci_fix_wake = arm_ci_fix_wake(&store, &task, lease).await?;
    let mut flow = if ci_fix_wake.is_some() {
        Playhead::new(QueuedInvocation::load(&task.worktree, "ci-fix")?).0
    } else {
        resume_task_phase(&task)?
    };
    let Some(prepared) = prepare_task_flow_step(
        &store,
        &mut task,
        lease,
        wave.name(),
        &mut flow,
        ci_fix_wake.as_ref(),
    )
    .await?
    else {
        store
            .advance_run(
                lease,
                crate::durable::RunAdvance::InvocationEnded {
                    invocation_id: invocation.id,
                    outcome: crate::durable::BoundaryState::Succeeded,
                },
            )
            .await?;
        wait_for_parked_run(&store, &lease.run_id).await?;
        return Ok(());
    };
    let (harness_name, _) = crate::engine::config::parse_agent(&task.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(task.provider_session_id.clone());
    let requested_account = invocation
        .route
        .account_id
        .as_deref()
        .map(crate::store::ProviderAccountId::parse)
        .transpose()
        .map_err(|reason| anyhow!("invalid Invocation account route: {reason}"))?;
    harness.set_provider_account_id(requested_account);
    store.validate_run_lease(lease).await?;
    harness.start(&prepared.turn.config).await?;
    task.provider = harness_name;
    task.provider_session_id = harness.provider_session_id();
    let invocation = store
        .observe_invocation_provider(
            lease,
            &invocation.id,
            harness.provider_account_id(),
            task.provider_session_id.clone(),
        )
        .await?;
    let invocation_id = invocation.id.clone();
    let invocation_route = invocation.route.clone();
    supervision.account_id = invocation.route.account_id.clone();
    supervision.resume_token = invocation.resume_token.clone();
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
    // Record this body's turns the way `flowloop/wave.rs` does. Without it a
    // Task's spend reaches no store at all: the provider runs in this
    // process, so no child `lf` records on its behalf.
    let capture = match flow.current() {
        Some(step) => {
            let context = match crate::journal::trace_capture_context(
                Path::new(&task.worktree),
                Some(step.flow.clone()),
                Some(step.step.clone()),
            ) {
                Ok(context) => context,
                Err(error) => {
                    return finish_execution_blocked(
                        &store,
                        &mut task,
                        lease,
                        harness.as_mut(),
                        &[format!("Task trace capture prerequisite missing: {error}")],
                        None,
                    )
                    .await;
                }
            };
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
                    supervision: Some(supervision.clone()),
                },
            ) {
                Ok(capture) => Some(capture),
                Err(error) => {
                    return finish_execution_blocked(
                        &store,
                        &mut task,
                        lease,
                        harness.as_mut(),
                        &[format!(
                            "Loopflow active Turn authority could not be established: {error}"
                        )],
                        None,
                    )
                    .await;
                }
            }
        }
        None => None,
    };
    if let Some(capture) = &capture {
        capture.set_provider_session_id(task.provider_session_id.clone());
    }
    let mut active_basis = start_prepared_task_step(
        &store,
        &mut task,
        lease,
        harness.as_mut(),
        &mut flow,
        capture.as_ref(),
        prepared,
    )
    .await?;
    let mut flow_turn_active = true;
    let mut provider_turn_active = true;

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
    let mut command_failures = Vec::new();
    let mut first_material_recorded = false;
    let mut first_material_warning_emitted = false;
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
                if !first_material_recorded && event.is_material_progress() {
                    let observed_at = time::OffsetDateTime::now_utc();
                    match store.record_first_material_at(lease, observed_at).await {
                        Ok(_) => first_material_recorded = true,
                        Err(error) if !first_material_warning_emitted => {
                            tracing::warn!(
                                task = %task.id,
                                run = %lease.run_id,
                                %error,
                                "Task first-material evidence did not persist; a later material event will retry"
                            );
                            first_material_warning_emitted = true;
                        }
                        Err(_) => {}
                    }
                }
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
                        command_failures.clear();
                    }
                    ConversationEvent::ItemCompleted { item, .. } => {
                        if let ConversationItem::Command {
                            command,
                            status,
                            output,
                            exit_code,
                            ..
                        } = &item
                        {
                            if let Some(failure) = completed_boundary_failure(
                                command,
                                *status,
                                output.as_deref(),
                                *exit_code,
                            ) {
                                if !command_failures.contains(&failure) {
                                    command_failures.push(failure);
                                }
                            }
                        }
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
                        if status == Lifecycle::Failed {
                            let reason = drain_turn_failure_reason(
                                &mut event_rx,
                                "provider turn failed",
                            );
                            return fail_and_maybe_recover(
                                &store,
                                &mut task,
                                lease,
                                &invocation_id,
                                &invocation_route,
                                harness.as_mut(),
                                &wave,
                                &reason,
                                turn_had_durable_side_effect,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        if execution_blocker_at_handoff(status, &command_failures).is_some() {
                            return finish_execution_blocked(
                                &store,
                                &mut task,
                                lease,
                                harness.as_mut(),
                                &command_failures,
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
                        let mut flow_iteration_completed = if flow_turn_active {
                            finish_task_flow_turn(&mut flow, status)?
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
                        if flow_turn_active {
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
                                    let Some(basis) = start_resumed_task_phase(
                                        &store,
                                        &mut task,
                                        lease,
                                        harness.as_mut(),
                                        &mut flow,
                                        wave.name(),
                                        capture.as_ref(),
                                    )
                                    .await?
                                    else {
                                        return park_task_at_human(
                                            &store,
                                            lease,
                                            &invocation_id,
                                            Some(harness.as_mut()),
                                            capture.as_ref(),
                                        )
                                        .await;
                                    };
                                    active_basis = basis;
                                    flow_turn_active = true;
                                    provider_turn_active = true;
                                    last_text.clear();
                                    continue 'runner;
                                }
                                Some(task.approved_gate_proposal()?)
                            } else {
                                None
                            };
                            if !flow_iteration_completed && status != Lifecycle::Interrupted {
                                let Some(prepared) = prepare_task_flow_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    wave.name(),
                                    &mut flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?
                                else {
                                    return park_task_at_human(
                                        &store,
                                        lease,
                                        &invocation_id,
                                        Some(harness.as_mut()),
                                        capture.as_ref(),
                                    )
                                    .await;
                                };
                                active_basis = start_prepared_task_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    capture.as_ref(),
                                    prepared,
                                )
                                .await?;
                                flow_turn_active = true;
                                provider_turn_active = true;
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
                            // A completing merge settles the Task and never rotates.
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
                                let number = observed_pr
                                    .as_ref()
                                    .and_then(|pr| pr.github())
                                    .map(|github| github.number);
                                let reason = match number {
                                    Some(number) => format!(
                                        "pull request #{number} merged to complete the Task"
                                    ),
                                    None => "pull request merged to complete the Task"
                                        .to_string(),
                                };
                                (true, reason)
                            } else if needs_rotation {
                                crate::ops::task::ensure_working_pr_for_run(
                                    &store,
                                    &mut task,
                                    lease,
                                )
                                .await
                                .map_err(|error| anyhow!(error.to_string()))?;
                                store.update_task_for_run(&task, lease).await?;
                                let Some(prepared) = prepare_task_flow_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    wave.name(),
                                    &mut flow,
                                    ci_fix_wake.as_ref(),
                                )
                                .await?
                                else {
                                    return park_task_at_human(
                                        &store,
                                        lease,
                                        &invocation_id,
                                        Some(harness.as_mut()),
                                        capture.as_ref(),
                                    )
                                    .await;
                                };
                                active_basis = start_prepared_task_step(
                                    &store,
                                    &mut task,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    capture.as_ref(),
                                    prepared,
                                )
                                .await?;
                                flow_turn_active = true;
                                provider_turn_active = true;
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
                                    let Some(prepared) = prepare_task_flow_step(
                                        &store,
                                        &mut task,
                                        lease,
                                        wave.name(),
                                        &mut flow,
                                        ci_fix_wake.as_ref(),
                                    )
                                    .await?
                                    else {
                                        return park_task_at_human(
                                            &store,
                                            lease,
                                            &invocation_id,
                                            Some(harness.as_mut()),
                                            capture.as_ref(),
                                        )
                                        .await;
                                    };
                                    active_basis = start_prepared_task_step(
                                        &store,
                                        &mut task,
                                        lease,
                                        harness.as_mut(),
                                        &mut flow,
                                        capture.as_ref(),
                                        prepared,
                                    )
                                    .await?;
                                    flow_turn_active = true;
                                    provider_turn_active = true;
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
                                let Some(basis) = start_resumed_task_phase(
                                    &store,
                                    &mut task,
                                    lease,
                                    harness.as_mut(),
                                    &mut flow,
                                    wave.name(),
                                    capture.as_ref(),
                                )
                                .await?
                                else {
                                    return park_task_at_human(
                                        &store,
                                        lease,
                                        &invocation_id,
                                        Some(harness.as_mut()),
                                        capture.as_ref(),
                                    )
                                    .await;
                                };
                                active_basis = basis;
                                flow_turn_active = true;
                                provider_turn_active = true;
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
                            store.advance_run(
                                lease,
                                crate::durable::RunAdvance::InvocationEnded {
                                    invocation_id: invocation_id.clone(),
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
                        return fail_and_maybe_recover(
                            &store,
                            &mut task,
                            lease,
                            &invocation_id,
                            &invocation_route,
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
                    | ConversationEvent::UsageCheckpoint { .. }
                    | ConversationEvent::SuggestedActions { .. }
                    | ConversationEvent::StatusChanged { .. } => {}
                }
            }
        }
    }
}

async fn prepare_task_flow_step_once(
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
    prepared.config.write_scope = crate::engine::agent::AgentWriteScope::Worktree;
    prepared.config.execution_boundary = Some(
        crate::ops::task::task_execution_boundary(&task.worktree, &task.agent)
            .map_err(|error| anyhow!(error.to_string()))?,
    );
    prepared.config.skip_permissions = true;
    let position = FlowPosition {
        work,
        epoch_id: boundary.basis.epoch_id.clone(),
        flow: task.phase_plan().flow.clone(),
        step: step.step.clone(),
        node_id: step.policy.id.clone(),
        human: step.policy.human,
        step_index: step.index,
        iteration: step.iteration,
        updated_at: time::OffsetDateTime::now_utc(),
    };
    Ok(PreparedTaskStep {
        turn: prepared,
        position,
        basis: boundary.basis,
    })
}

async fn prepare_task_flow_step(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    wave_name: &str,
    flow: &mut Playhead,
    ci_fix: Option<&CiFixWake>,
) -> Result<Option<PreparedTaskStep>> {
    loop {
        let prepared =
            prepare_task_flow_step_once(store, task, lease, wave_name, flow, ci_fix).await?;
        if !prepared.position.human {
            return Ok(Some(prepared));
        }

        let node_id = prepared
            .position
            .node_id
            .clone()
            .ok_or_else(|| anyhow!("human Task flow step has no stable node id"))?;
        checkpoint_worktree_before_human(task, &node_id).await;
        store
            .set_flow_position(lease, prepared.position.clone())
            .await?;
        let run = store.run_by_id(&lease.run_id).await?;
        let origin_cwd = run
            .cwd
            .ok_or_else(|| anyhow!("active Task Run has no execution cwd"))?;
        let ask = store
            .create_ask(
                lease,
                crate::durable::AskOrigin {
                    work: lease.work.clone(),
                    run_id: lease.run_id.clone(),
                    turn_id: None,
                    invocation_id: None,
                    home_id: run.home_id,
                    cwd: origin_cwd,
                },
                crate::durable::AskBody::FlowStep {
                    flow: prepared.position.flow.clone(),
                    node_id: node_id.clone(),
                    skill: prepared.position.step.clone(),
                    iteration: prepared.position.iteration,
                },
                crate::durable::AskTarget::User,
            )
            .await?;

        match (ask.state, ask.result.as_ref()) {
            (crate::durable::AskState::Queued | crate::durable::AskState::Claimed, None) => {
                tracing::info!(task = %task.id, ask = %ask.id, node = %node_id, "Task is waiting at a human flow node");
                return Ok(None);
            }
            (
                crate::durable::AskState::Resolved,
                Some(crate::durable::AskResult::Resolved { .. }),
            ) => {
                complete_human_task_step(store, task, lease, flow).await?;
            }
            (
                crate::durable::AskState::Declined,
                Some(crate::durable::AskResult::Declined { reason }),
            )
            | (
                crate::durable::AskState::Cancelled,
                Some(crate::durable::AskResult::Cancelled { reason }),
            ) => {
                store
                    .append_steer(
                        &lease.work,
                        crate::durable::Author::User,
                        &format!("Human flow node {node_id} did not accept the step: {reason}"),
                        None,
                    )
                    .await?;
                task.phase_cursor = preceding_autonomous_step(flow, prepared.position.step_index)?;
                task.phase_iteration += 1;
                task.updated_at = time::OffsetDateTime::now_utc();
                store.update_task_for_run(task, lease).await?;
                *flow = resume_task_phase(task)?;
            }
            _ => anyhow::bail!("human Task Ask {} has an invalid terminal result", ask.id),
        }
    }
}

async fn complete_human_task_step(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    flow: &mut Playhead,
) -> Result<()> {
    open_task_flow_body(flow, task)?;
    let completed = finish_task_flow_turn(flow, Lifecycle::Completed)?;
    if completed && task.lifecycle_phase == TaskLifecyclePhase::First {
        task.enter_loop()?;
        *flow = resume_task_phase(task)?;
    } else {
        record_task_flow_position(task, flow)?;
    }
    store.update_task_for_run(task, lease).await?;
    Ok(())
}

fn preceding_autonomous_step(flow: &Playhead, current: u32) -> Result<u32> {
    let root = flow
        .stack
        .first()
        .ok_or_else(|| anyhow!("Task flow has no root invocation"))?;
    root.steps
        .iter()
        .enumerate()
        .take(current as usize)
        .rev()
        .find(|(_, step)| step.kind == StepKind::Skill && !step.policy.human)
        .map(|(index, _)| index as u32)
        .ok_or_else(|| anyhow!("human Task flow node has no preceding autonomous skill"))
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
) -> Result<Basis> {
    store
        .set_flow_position(lease, prepared.position.clone())
        .await?;
    debug_assert!(!prepared.position.human);
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    start_task_flow_turn(store, task, lease, harness, flow, prepared.turn).await?;
    Ok(prepared.basis)
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
) -> Result<Option<Basis>> {
    *flow = resume_task_phase(task)?;
    let Some(prepared) = prepare_task_flow_step(store, task, lease, wave_name, flow, None).await?
    else {
        return Ok(None);
    };
    Ok(Some(
        start_prepared_task_step(store, task, lease, harness, flow, capture, prepared).await?,
    ))
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

/// A human Ask can outlive this machine's uptime; nothing the Task produced
/// may exist only in the local worktree while it waits. Failure to checkpoint
/// (offline, no remote) must never block the park itself.
async fn checkpoint_worktree_before_human(task: &Task, node_id: &str) {
    if let Err(error) = crate::ops::checkpoint_task_worktree(
        task.worktree.clone(),
        task.plan.identifier.clone(),
        format!("checkpoint: park at human node {node_id}"),
    )
    .await
    {
        tracing::warn!(task = %task.id, %error, "Task parks at a human node without a pushed checkpoint");
    }
}

async fn park_task_at_human(
    store: &SharedStore,
    lease: &RunLease,
    invocation_id: &crate::durable::AgentInvocationId,
    harness: Option<&mut dyn Harness>,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "completed");
    if let Some(harness) = harness {
        let _ = harness.stop().await;
    }
    store
        .advance_run(
            lease,
            crate::durable::RunAdvance::InvocationEnded {
                invocation_id: invocation_id.clone(),
                outcome: crate::durable::BoundaryState::Succeeded,
            },
        )
        .await?;
    wait_for_parked_run(store, &lease.run_id).await
}

async fn wait_for_parked_run(store: &SharedStore, run_id: &crate::durable::RunId) -> Result<()> {
    loop {
        if store.run_by_id(run_id).await?.state != crate::durable::RunState::Active {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
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
    if task.lifecycle_phase == TaskLifecyclePhase::Finally && task.phase_epoch == latest.phase_epoch
    {
        task.gate_proposal = latest.gate_proposal.clone();
    } else if task.lifecycle_phase != TaskLifecyclePhase::Finally {
        // A gate proposal belongs to one Finally epoch. Copying a newer
        // proposal into a pre-final body forms an invalid Task before the store's
        // phase-epoch fence can discard that stale body's write.
        task.gate_proposal = None;
    }
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
        prepare_task_flow_step_once(store, task, lease, wave.name(), flow, Some(wake)).await?;
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
    match store.current_run(&lease.work).await {
        Ok(Some(run)) if run.id == lease.run_id => {}
        Ok(_) => return,
        Err(store_error) => {
            tracing::error!(
                task = %task_id,
                run = %lease.run_id,
                error = %store_error,
                "cannot inspect Task Run before recording its failure receipt"
            );
            return;
        }
    }
    let (message, resumable) = unhandled_failure_receipt(&error.to_string());
    if let Err(persist_error) = store
        .fail_task_run(task_id, lease, &message, resumable)
        .await
    {
        tracing::error!(
            task = %task_id,
            run = %lease.run_id,
            error = %persist_error,
            "Task failure receipt did not persist; Run remains recoverable"
        );
    }
}

fn unhandled_failure_receipt(detail: &str) -> (String, bool) {
    match provider_credential_blocker(detail) {
        Some(message) => (message, false),
        None => (format!("task process failed: {detail}"), true),
    }
}

fn provider_credential_blocker(detail: &str) -> Option<String> {
    crate::engine::agent::credential_invalidated_failure(detail).map(|_| {
        format!(
            "Task provider credential capability is blocked: {detail}. Reconnect the named managed account before starting a new Run"
        )
    })
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
    store.update_task_for_run(task, lease).await?;
    store.fail_task_run(&task.id, lease, error, true).await?;
    anyhow::bail!(error.to_string())
}

fn completed_boundary_failure(
    command: &[String],
    status: Lifecycle,
    output: Option<&str>,
    exit_code: Option<i32>,
) -> Option<String> {
    if status != Lifecycle::Failed && exit_code.is_none_or(|code| code == 0) {
        return None;
    }
    let output = output?;
    let lower = output.to_ascii_lowercase();
    if ![
        "operation not permitted",
        "permission denied",
        "read-only file system",
        "has no active turn",
        "network access is disabled",
        "network is unreachable",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return None;
    }
    let command = command.join(" ");
    let detail = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("no command error output");
    let detail = detail.chars().take(1_000).collect::<String>();
    let exit = exit_code
        .map(|code| format!(" (exit {code})"))
        .unwrap_or_default();
    Some(format!("`{command}` failed{exit}: {detail}"))
}

async fn finish_execution_blocked(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    failures: &[String],
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    let reason = execution_blocked_reason(failures);
    finish_nonresumable(store, task, lease, harness, &reason, capture).await
}

async fn finish_nonresumable(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    harness: &mut dyn Harness,
    reason: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store.update_task_for_run(task, lease).await?;
    store.fail_task_run(&task.id, lease, reason, false).await?;
    Ok(())
}

fn execution_blocked_reason(failures: &[String]) -> String {
    format!(
        "Task execution boundary is blocked:\n- {}\nCorrect the named filesystem, control-plane, or network capability before starting a new Run.",
        failures.join("\n- ")
    )
}

fn execution_blocker_at_handoff(status: Lifecycle, failures: &[String]) -> Option<String> {
    (status == Lifecycle::Completed && !failures.is_empty())
        .then(|| execution_blocked_reason(failures))
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

/// Stop the body and record an infrastructure failure (provider outage, GitHub
/// observation failure), keeping the active PR attached so a resume after the
/// capability recovers picks up the same PR. Returns `Ok(())` after the atomic
/// Task failure receipt settles the Run, so the outer boundary does not record
/// the same failure twice.
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
    store.update_task_for_run(task, lease).await?;
    store.fail_task_run(&task.id, lease, &reason, true).await?;
    Ok(())
}

/// Recover a retryable body failure through the Run's next exact route after
/// PRD-38 permits replacement and the current containment stops positively.
#[allow(clippy::too_many_arguments)] // capture is a terminal-path output, not a knob
async fn handle_body_failure(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    invocation_id: &crate::durable::AgentInvocationId,
    invocation_route: &crate::durable::InvocationRoute,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<Option<(RunLease, ExactRoute)>> {
    if let Some(blocker) = provider_credential_blocker(reason) {
        return finish_nonresumable(store, task, lease, harness, &blocker, capture)
            .await
            .map(|_| None);
    }
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
        let current_route = ExactRoute::try_from(invocation_route)?;
        let stopped = match stop_invocation_for_recovery(store, lease, invocation_id, harness)
            .await?
        {
            RecoveryStopOutcome::Stopped(stopped) => stopped,
            RecoveryStopOutcome::Fenced { error, stop } => {
                tracing::error!(task = %task.id, containment = ?stop.containment, %error, "Task recovery left the Run fenced");
                return Ok(None);
            }
        };
        let choice = plan_run_route_recovery(store, lease, backup_agent).await?;
        let failure = match &choice {
            RecoveryChoice::Invoke(_) => reason.to_string(),
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
            RecoverySettlement::RecoveryRun {
                lease: recovery_lease,
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
                store.update_task_for_run(task, &recovery_lease).await?;
                store
                    .append_task_event_for_run(
                        &task.id,
                        &recovery_lease,
                        &TaskEventKind::BodyHandedOff { handoff },
                    )
                    .await?;
                Ok(Some((recovery_lease, route)))
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
async fn fail_and_maybe_recover(
    store: &SharedStore,
    task: &mut Task,
    lease: &RunLease,
    invocation_id: &crate::durable::AgentInvocationId,
    invocation_route: &crate::durable::InvocationRoute,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    let Some((recovery_lease, route)) = handle_body_failure(
        store,
        task,
        lease,
        invocation_id,
        invocation_route,
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
    spawn_failover(store, task, &recovery_lease, &route).await
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
            CommandStop::Quiesced => "completed",
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
        CommandStop::Quiesced => {
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
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\n{direction}\n\nTask directive snapshot synced at: {task_snapshot_synced_at}\nProject definition snapshot synced at: {project_snapshot_synced_at}\nWave: {wave}\nTask: {task_id}\nLifecycle phase: {lifecycle_phase} (epoch {phase_epoch}, iteration {phase_iteration}, gate cycle {gate_cycle})\n{gate_proposal}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}\n\nThis PR owns one serial branch. The pinned finally flow owns landing and Task completion. `lf pr abandon` discards only this PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward. The runner owns branch rotation between PRs.",
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
        phase_iteration = task.phase_iteration,
        gate_cycle = task.gate_cycle,
        gate_proposal = gate_proposal,
        worktree = task.worktree.display(),
        pr_sequence = pr.sequence,
        pr_branch = pr.branch,
        base_commit = pr.base_commit,
        placement = placement,
    )
}

pub(crate) async fn flow_step_ask_prompt(
    store: &SharedStore,
    ask: &crate::durable::Ask,
) -> Result<String> {
    let crate::durable::AskBody::FlowStep {
        flow,
        node_id,
        skill,
        iteration,
    } = &ask.request
    else {
        anyhow::bail!("Ask {} is not a flow-step request", ask.id);
    };
    let crate::durable::WorkRef::Task(task_id) = &ask.origin.work else {
        anyhow::bail!("flow-step Ask {} does not belong to a Task", ask.id);
    };
    let task = store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
    if task.phase_plan().flow != *flow {
        anyhow::bail!(
            "flow-step Ask {} names flow {:?}, but Task {} is at {:?}",
            ask.id,
            flow,
            task.id,
            task.phase_plan().flow
        );
    }
    let definition = crate::engine::load_flow(flow, &task.worktree)?;
    let items = crate::engine::expand_flow(&definition, &task.worktree)?;
    let step = items
        .get(task.phase_cursor as usize)
        .and_then(|item| match item {
            crate::engine::ConcreteStep::Skill(step) => Some(step),
            _ => None,
        })
        .ok_or_else(|| anyhow!("Task {} is not at a skill node", task.id))?;
    if !step.policy.human
        || step.policy.id.as_deref() != Some(node_id)
        || step.skill.name != *skill
        || task.phase_iteration != *iteration
    {
        anyhow::bail!(
            "flow-step Ask {} no longer matches the Task playhead",
            ask.id
        );
    }
    let boundary = store.boundary_seed(&ask.origin.work).await?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = owning_project(store, &task).await?;
    let wave = owning_wave(store, &task).await?;
    let seed = task_seed(&task, &project.plan, &pr, wave.name(), &boundary);
    let prepared =
        crate::lf::commands::run::prepare_interactive_harness_turn(skill, &seed, wave.name())?;
    Ok(human_flow_ask_prompt(
        &prepared.input,
        skill,
        node_id,
        &ask.id,
    ))
}

fn human_flow_ask_prompt(input: &str, skill: &str, node_id: &str, ask_id: &AskId) -> String {
    format!(
        "{input}\n\n<lf:human-flow-node>\nThis is the actual writable `{skill}` Task step at human node `{node_id}`, not an advisory review. Work only in the origin Task and settle explicitly before leaving:\n- `lf ask resolve {ask_id} \"<concise verified summary>\"` accepts the step\n- `lf ask decline {ask_id} \"<reason>\"` returns to the preceding autonomous step\n- `lf ask release {ask_id} \"<reason>\"` leaves the node waiting\nA final response, clean exit, Ctrl-D, or window close never advances the flow.\n</lf:human-flow-node>"
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
struct TestLfBinGuard {
    previous: Option<std::ffi::OsString>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(test)]
impl TestLfBinGuard {
    fn pin() -> Self {
        let lock = crate::journal::test_env_lock();
        let previous = std::env::var_os("LF_BIN");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            previous,
            _lock: lock,
        }
    }
}

#[cfg(test)]
impl Drop for TestLfBinGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("LF_BIN", value),
            None => std::env::remove_var("LF_BIN"),
        }
    }
}

#[cfg(test)]
mod shipping_lifecycle_tests;

#[cfg(test)]
mod planning_tests {
    use super::{
        completed_boundary_failure, execution_blocker_at_handoff, human_flow_ask_prompt,
        preceding_autonomous_step, sync_task_state, task_seed, unhandled_failure_receipt,
    };
    use crate::chat::types::Lifecycle;
    use crate::durable::{
        AskBody, AskId, AskResult, AskTarget, AuthenticatedRequest, Basis, BoundarySeed,
        Containment, ControlCtx, EpochId, InvocationRoute, RunAdvance, RunLease, RunTrigger,
    };
    use crate::engine::agent::AgentConfig;
    use crate::engine::OccurrencePolicy;
    use crate::harness::{Harness, SendCurrentOutcome};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::task::{
        Observation, PmWritebackState, Task, TaskEventKind, TaskGateProposal, TaskId,
        TaskLifecyclePhase, TaskLifecyclePlan, TaskPr, TaskPrId,
    };
    use crate::wave::playhead::{Playhead, QueuedInvocation, StepKind, StepPlan};
    use crate::wave::Wave;

    struct UnusedHarness;

    fn step(name: &str, id: Option<&str>, human: bool) -> StepPlan {
        StepPlan {
            name: name.to_string(),
            kind: StepKind::Skill,
            policy: OccurrencePolicy {
                id: id.map(str::to_string),
                human,
            },
        }
    }

    #[test]
    fn decline_skips_prior_human_nodes_when_returning_to_autonomous_work() {
        let (flow, _) = Playhead::new(QueuedInvocation {
            id: "human-decline-proof".to_string(),
            flow: "proof".to_string(),
            steps: vec![
                step("kickoff", None, false),
                step("first-review", Some("first_review"), true),
                step("second-review", Some("second_review"), true),
            ],
        });

        assert_eq!(preceding_autonomous_step(&flow, 2).unwrap(), 0);
    }

    #[async_trait::async_trait]
    impl Harness for UnusedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
            anyhow::bail!("unused test harness must not start")
        }

        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            anyhow::bail!("unused test harness must not receive input")
        }

        async fn send_current(&mut self, _content: &str) -> SendCurrentOutcome {
            SendCurrentOutcome::NotSteerable
        }

        async fn interrupt(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    async fn human_task_fixture() -> (SharedStore, Task, RunLease, Playhead) {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        // The Task worktree must never be the real checkout: parking at a
        // human node checkpoint-commits the worktree, and a test must not
        // commit or push the developer's repo.
        let worktree = tempfile::tempdir().unwrap().keep();
        let git = |args: &[&str]| {
            assert!(std::process::Command::new("git")
                .current_dir(&worktree)
                .args(args)
                .output()
                .unwrap()
                .status
                .success());
        };
        git(&["init", "-q"]);
        git(&[
            "-c",
            "user.email=test@loopflow.dev",
            "-c",
            "user.name=Loopflow Test",
            "commit",
            "--allow-empty",
            "-q",
            "-m",
            "init",
        ]);
        let database = tempfile::tempdir().unwrap().keep();
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(database.join("registry.db")))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "human-task-proof".to_string(),
            repository.display().to_string(),
        );
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("human-task-project").unwrap(),
                slug: "human-task-proof".to_string(),
                name: "Human Task proof".to_string(),
                prompt_context: "Prove durable human flow nodes.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("human-task-issue").unwrap(),
                identifier: "TEST-1".to_string(),
                title: "Human Task proof".to_string(),
                description: "Stop at review_kickoff.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: worktree.clone(),
            workspace_slug: "human-task-proof".to_string(),
            lifecycle: TaskLifecyclePlan::defaults(),
            lifecycle_phase: TaskLifecyclePhase::First,
            phase_epoch: 1,
            phase_cursor: 1,
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
            slug: task.workspace_slug.clone(),
            branch: "test/human-task-proof".to_string(),
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
        store.create_wave(&wave).await.unwrap();
        store.create_project(&project).await.unwrap();
        store.create_task(&task, &pr).await.unwrap();
        let work = store
            .work_for_child(&crate::child::ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let (_, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "human-task-proof".to_string(),
                    },
                    cwd: worktree.clone(),
                },
            )
            .await
            .unwrap();
        let flow = super::resume_task_phase(&task).unwrap();
        (store, task, lease, flow)
    }

    #[test]
    fn human_flow_settlement_contract_follows_the_authored_skill_handoff() {
        let prompt = human_flow_ask_prompt(
            "$review-design\n\n<lf:surface>human present</lf:surface>",
            "review-design",
            "review_kickoff",
            &AskId::parse("ask_00000000000000000000000000000001").unwrap(),
        );

        assert!(prompt.starts_with("$review-design\n"));
        assert!(prompt.contains("<lf:human-flow-node>"));
        assert!(prompt.contains("lf ask resolve ask_00000000000000000000000000000001"));
        assert!(prompt.contains("lf ask decline ask_00000000000000000000000000000001"));
        assert!(prompt.contains("lf ask release ask_00000000000000000000000000000001"));
    }

    #[tokio::test]
    async fn parked_task_supervisor_stays_alive_until_the_run_settles() {
        let (store, task, lease, _) = human_task_fixture().await;
        let waiting_store = store.clone();
        let run_id = lease.run_id.clone();
        let waiter =
            tokio::spawn(async move { super::wait_for_parked_run(&waiting_store, &run_id).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());

        store
            .finish_task_run(&task, &lease, crate::durable::BoundaryState::Succeeded)
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("parked supervisor exits after Run settlement")
            .unwrap()
            .unwrap();
    }

    #[test]
    fn normal_task_completion_preserves_delivery_permission_and_ask_authority_failures() {
        let commit = completed_boundary_failure(
            &["lf".into(), "commit".into(), "-m".into(), "ship".into(), "-p".into()],
            Lifecycle::Failed,
            Some(
                "fatal: Unable to create '/repo/.git/worktrees/task/index.lock': Operation not permitted",
            ),
            Some(128),
        )
        .unwrap();
        let ask = completed_boundary_failure(
            &[
                "lf".into(),
                "ask".into(),
                "--user".into(),
                "Need authority".into(),
            ],
            Lifecycle::Failed,
            Some("AgentInvocation invocation_test has no active Turn"),
            Some(1),
        )
        .unwrap();
        let reason = execution_blocker_at_handoff(Lifecycle::Completed, &[commit, ask])
            .expect("normal task_complete with unresolved capability failures is blocked");

        assert!(reason.contains(".git/worktrees/task/index.lock"));
        assert!(reason.contains("Operation not permitted"));
        assert!(reason.contains("AgentInvocation invocation_test has no active Turn"));
        assert!(reason.contains("before starting a new Run"));
    }

    #[test]
    fn ordinary_failed_probe_is_not_an_execution_boundary_blocker() {
        assert!(completed_boundary_failure(
            &["rg".into(), "missing-pattern".into()],
            Lifecycle::Failed,
            Some(""),
            Some(1),
        )
        .is_none());
    }

    #[test]
    fn rejected_provider_auth_is_a_named_nonresumable_capability_blocker() {
        let (reason, resumable) = unhandled_failure_receipt(
            "Your authentication token has been invalidated (token_invalidated)",
        );

        assert!(!resumable);
        assert!(reason.contains("provider credential capability is blocked"));
        assert!(reason.contains("Reconnect the named managed account"));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn pre_provider_failure_ends_prompt_only_invocation_and_records_task_failure() {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        let database = tempfile::tempdir().unwrap();
        let database_path = database.path().join("registry.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(database_path.clone()))
                .await
                .unwrap(),
        );
        let now = time::OffsetDateTime::now_utc();
        let wave = Wave::new(
            crate::id::WaveId::new(),
            "prompt-only-failure".to_string(),
            repository.display().to_string(),
        );
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("prompt-only-project").unwrap(),
                slug: "prompt-only-failure".to_string(),
                name: "Prompt-only failure".to_string(),
                prompt_context: "Preserve the exact terminal failure.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "unsupported".to_string(),
            provider: "unsupported".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let mut task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("prompt-only-issue").unwrap(),
                identifier: "TEST-PROMPT".to_string(),
                title: "Prompt-only failure".to_string(),
                description: String::new(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: repository.clone(),
            workspace_slug: "prompt-only-failure".to_string(),
            lifecycle: TaskLifecyclePlan::defaults(),
            lifecycle_phase: TaskLifecyclePhase::First,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "unsupported".to_string(),
            provider: "unsupported".to_string(),
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
            slug: task.workspace_slug.clone(),
            branch: "test/prompt-only-failure".to_string(),
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
        store.create_wave(&wave).await.unwrap();
        store.create_project(&project).await.unwrap();
        store.create_task(&task, &pr).await.unwrap();
        let work = store
            .work_for_child(&crate::child::ChildRef::Task(task.id.clone()))
            .await
            .unwrap();
        let (run, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "prompt-only-failure".to_string(),
                    },
                    cwd: repository.clone(),
                },
            )
            .await
            .unwrap();
        let invocation = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "unsupported".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap();
        let crate::durable::AdvanceReceipt::Invocation(invocation) = invocation else {
            unreachable!("InvocationStarting returns an Invocation receipt")
        };
        let connection = rusqlite::Connection::open(&database_path).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_test_task_failure
                 BEFORE INSERT ON task_events
                 WHEN json_extract(NEW.kind_json, '$.kind') = 'failed'
                 BEGIN
                     SELECT RAISE(ABORT, 'injected Task failure write');
                 END;",
            )
            .unwrap();

        let _env_lock = crate::journal::test_env_lock();
        let inherited_invocation = std::env::var_os(crate::durable::AGENT_INVOCATION_ENV);
        std::env::set_var(crate::durable::AGENT_INVOCATION_ENV, invocation.id.as_str());
        let result = super::run(store.clone(), task.id.clone(), &lease).await;
        match inherited_invocation {
            Some(value) => std::env::set_var(crate::durable::AGENT_INVOCATION_ENV, value),
            None => std::env::remove_var(crate::durable::AGENT_INVOCATION_ENV),
        }
        let error = result.expect_err("unsupported provider must fail before its first event");
        assert!(
            error
                .to_string()
                .contains("has no managed account route for the required linked Git"),
            "unexpected startup failure: {error:#}"
        );

        let events = store.recent_task_events(&task.id, 10).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        assert!(store.current_run(&work).await.unwrap().is_some());
        let unsettled: (String, bool) = connection
            .query_row(
                "SELECT outcome, ended_at IS NOT NULL
                 FROM agent_invocations WHERE supervising_run_id=?1",
                [run.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unsettled, ("running".into(), false));

        connection
            .execute_batch("DROP TRIGGER reject_test_task_failure;")
            .unwrap();
        super::record_unhandled_failure(&store, &task.id, &lease, &error).await;

        let events = store.recent_task_events(&task.id, 10).await.unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Failed { error, resumable: true }
                if error.contains("task process failed: Task execution cannot converge")
                    && error.contains("has no managed account route for the required linked Git")
        ));
        assert_eq!(events[1].kind, TaskEventKind::Started);
        assert!(store.current_run(&work).await.unwrap().is_none());

        let invocation: (String, String, bool) = connection
            .query_row(
                "SELECT capture_status, outcome, ended_at IS NOT NULL
                 FROM agent_invocations WHERE supervising_run_id=?1",
                [run.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(invocation, ("prompt_only".into(), "failed".into(), true));

        let (run, lease) = store.reserve_run(&work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "handled-provider-failure".to_string(),
                    },
                    cwd: repository.clone(),
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "unsupported".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap();
        store
            .append_task_event_for_run(&task.id, &lease, &TaskEventKind::Started)
            .await
            .unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER reject_test_task_failure
                 BEFORE INSERT ON task_events
                 WHEN json_extract(NEW.kind_json, '$.kind') = 'failed'
                 BEGIN
                     SELECT RAISE(ABORT, 'injected Task failure write');
                 END;",
            )
            .unwrap();

        let mut harness = UnusedHarness;
        super::finish_failed(
            &store,
            &mut task,
            &lease,
            &mut harness,
            "provider stream closed",
            None,
        )
        .await
        .expect_err("the rejected receipt keeps the handled failure recoverable");
        assert!(store.current_run(&work).await.unwrap().is_some());
        let unsettled: (String, bool) = connection
            .query_row(
                "SELECT outcome, ended_at IS NOT NULL
                 FROM agent_invocations WHERE supervising_run_id=?1",
                [run.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(unsettled, ("running".into(), false));

        connection
            .execute_batch("DROP TRIGGER reject_test_task_failure;")
            .unwrap();
        let handled_error = super::finish_failed(
            &store,
            &mut task,
            &lease,
            &mut harness,
            "provider stream closed",
            None,
        )
        .await
        .expect_err("a handled failure returns its reason after settlement");
        let event_count = store.recent_task_events(&task.id, 10).await.unwrap().len();
        super::record_unhandled_failure(&store, &task.id, &lease, &handled_error).await;
        let events = store.recent_task_events(&task.id, 10).await.unwrap();
        assert_eq!(events.len(), event_count);
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Failed { error, resumable: true }
                if error == "provider stream closed"
        ));
        assert!(store.current_run(&work).await.unwrap().is_none());
        let invocation: (String, String, bool) = connection
            .query_row(
                "SELECT capture_status, outcome, ended_at IS NOT NULL
                 FROM agent_invocations WHERE supervising_run_id=?1",
                [run.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(invocation, ("prompt_only".into(), "failed".into(), true));
    }

    async fn settle_human_task(
        store: &SharedStore,
        result: AskResult,
    ) -> (crate::durable::Ask, RunLease) {
        let user = AuthenticatedRequest::cli();
        let ask = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let claim = store
            .claim_test_ask(&ControlCtx::User(&user), &ask.id)
            .await
            .unwrap();
        assert!(claim.needs_launch);
        let run_lease = store
            .claim_flow_step_run_lease(&ask.id, &claim.invocation_id)
            .await
            .unwrap()
            .unwrap();
        store
            .mark_ask_ready(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        store
            .mark_ask_presented(&ask.id, &claim.invocation_id)
            .await
            .unwrap();
        let ask = store
            .settle_ask(&ask.id, &claim.invocation_id, result)
            .await
            .unwrap();
        assert!(store.validate_run_lease(&run_lease).await.is_err());
        assert!(store.current_run(&ask.origin.work).await.unwrap().is_none());
        let (_, successor) = store
            .reserve_run(&ask.origin.work, RunTrigger::User)
            .await
            .unwrap();
        store
            .advance_run(
                &successor,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "human-task-successor".to_string(),
                    },
                    cwd: ask.origin.cwd.clone(),
                },
            )
            .await
            .unwrap();
        (ask, successor)
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn released_human_task_attempt_keeps_the_same_node_parked() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, lease, mut flow) = human_task_fixture().await;
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &lease,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let user = AuthenticatedRequest::cli();
        let ask = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap()
            .remove(0);
        let claim = store
            .claim_test_ask(&ControlCtx::User(&user), &ask.id)
            .await
            .unwrap();
        assert!(claim.needs_launch);
        let flow_writer = store
            .claim_flow_step_run_lease(&ask.id, &claim.invocation_id)
            .await
            .unwrap()
            .unwrap();
        store
            .release_ask(&ask.id, &claim.invocation_id, Some("not finished"))
            .await
            .unwrap();

        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &flow_writer,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let queued = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, ask.id);
        assert_eq!(task.phase_cursor, 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn restarted_human_task_reuses_the_queued_ask_with_current_run_authority() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, lease, mut flow) = human_task_fixture().await;
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &lease,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let user = AuthenticatedRequest::cli();
        let original = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap()
            .remove(0);
        store
            .finish_task_run(&task, &lease, crate::durable::BoundaryState::Succeeded)
            .await
            .unwrap();
        let (_, successor) = store
            .reserve_run(&original.origin.work, RunTrigger::User)
            .await
            .unwrap();
        store
            .advance_run(
                &successor,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: "human-task-restart".to_string(),
                    },
                    cwd: task.worktree.clone(),
                },
            )
            .await
            .unwrap();
        let mut restarted_flow = super::resume_task_phase(&task).unwrap();

        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &successor,
            "human-task-proof",
            &mut restarted_flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let recovered = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, original.id);
        assert_eq!(recovered[0].origin, original.origin);
        let claim = store
            .claim_test_ask(&ControlCtx::User(&user), &recovered[0].id)
            .await
            .unwrap();
        let flow_writer = store
            .claim_flow_step_run_lease(&recovered[0].id, &claim.invocation_id)
            .await
            .unwrap()
            .expect("recovered flow Ask receives current writer authority");
        store.validate_run_lease(&flow_writer).await.unwrap();
        let invocation = store
            .ask_invocations(&recovered[0].id)
            .await
            .unwrap()
            .into_iter()
            .find(|invocation| invocation.id == claim.invocation_id)
            .unwrap();
        assert_eq!(
            invocation.supervising_run_id.as_ref(),
            Some(&successor.run_id)
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn human_task_node_queues_without_starting_a_provider_and_resolve_advances() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, lease, mut flow) = human_task_fixture().await;
        let prepared = super::prepare_task_flow_step(
            &store,
            &mut task,
            &lease,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap();
        assert!(prepared.is_none());
        let user = AuthenticatedRequest::cli();
        let queued = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap();
        assert_eq!(queued.len(), 1);
        assert!(matches!(
            &queued[0].request,
            AskBody::FlowStep { node_id, skill, .. }
                if node_id == "review_kickoff" && skill == "review-design"
        ));
        assert!(store
            .invocations_for_run(&lease.run_id)
            .await
            .unwrap()
            .is_empty());

        let (_, flow_writer) = settle_human_task(
            &store,
            AskResult::Resolved {
                summary: "design accepted".to_string(),
            },
        )
        .await;
        let prepared = super::prepare_task_flow_step(
            &store,
            &mut task,
            &flow_writer,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .expect("resolved human node advances to autonomous work");
        assert!(!prepared.position.human);
        assert_eq!(task.lifecycle_phase, TaskLifecyclePhase::Loop);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn declined_human_task_node_returns_to_preceding_autonomous_step_with_reason() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, lease, mut flow) = human_task_fixture().await;
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &lease,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let (declined, flow_writer) = settle_human_task(
            &store,
            AskResult::Declined {
                reason: "narrow the design".to_string(),
            },
        )
        .await;

        let prepared = super::prepare_task_flow_step(
            &store,
            &mut task,
            &flow_writer,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .expect("decline returns to autonomous kickoff");
        assert_eq!(prepared.position.step, "kickoff");
        assert_eq!(task.phase_cursor, 0);
        assert_eq!(task.phase_iteration, 1);
        let boundary = store.boundary_seed(&flow_writer.work).await.unwrap();
        assert!(boundary
            .steers
            .iter()
            .any(|steer| steer.text.contains("narrow the design")));

        super::open_task_flow_body(&mut flow, &task).unwrap();
        assert!(!super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap());
        super::record_task_flow_position(&mut task, &flow).unwrap();
        store
            .update_task_for_run(&task, &flow_writer)
            .await
            .unwrap();
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            &flow_writer,
            "human-task-proof",
            &mut flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let user = AuthenticatedRequest::cli();
        let queued = store
            .pending_asks(&ControlCtx::User(&user), &AskTarget::User)
            .await
            .unwrap();
        assert_eq!(queued.len(), 1);
        assert_ne!(queued[0].id, declined.id);
    }

    #[test]
    fn task_state_sync_keeps_gate_proposals_scoped_to_finally_epoch() {
        let now = time::OffsetDateTime::now_utc();
        let first = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("incident-issue").unwrap(),
                identifier: "ENG-125".to_string(),
                title: "Incident".to_string(),
                description: String::new(),
                pm_snapshot_synced_at: 1,
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: crate::id::WaveId::new(),
            project_id: ProjectId::new(),
            worktree: "/tmp/incident".into(),
            workspace_slug: "incident".to_string(),
            lifecycle: TaskLifecyclePlan::standard("incident", "ship-5whys", "ship"),
            lifecycle_phase: TaskLifecyclePhase::First,
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
        let mut finally = first.clone();
        finally.enter_loop().unwrap();
        finally
            .enter_finally(TaskGateProposal {
                done: true,
                reason: "pull request merged".to_string(),
            })
            .unwrap();

        let mut first_body = first.clone();
        sync_task_state(&mut first_body, &finally);
        assert!(first_body.gate_proposal.is_none());
        first_body.validate().unwrap();

        let mut loop_body = first;
        loop_body.enter_loop().unwrap();
        sync_task_state(&mut loop_body, &finally);
        assert!(loop_body.gate_proposal.is_none());
        loop_body.validate().unwrap();

        let mut finally_body = finally.clone();
        finally.gate_proposal = Some(TaskGateProposal {
            done: false,
            reason: "another prevention remains".to_string(),
        });
        sync_task_state(&mut finally_body, &finally);
        assert_eq!(finally_body.gate_proposal, finally.gate_proposal);
        finally_body.validate().unwrap();
    }

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
            lifecycle: TaskLifecyclePlan::standard("task-design", "task", "ship"),
            lifecycle_phase: TaskLifecyclePhase::Loop,
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
