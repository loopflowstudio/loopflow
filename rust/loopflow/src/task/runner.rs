use std::io::BufRead;
use std::path::Path;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::child::ChildRef;
use crate::durable::{render_steers, AskId, FlowPosition, RunId, Steer};
use crate::harness::{drain_turn_failure_reason, ApprovalPolicy, Harness};
use crate::planning::ProjectPlan;
use crate::project::Project;
use crate::store::SharedStore;
use crate::task::{PrPhase, Task, TaskEventKind, TaskGateProposal, TaskId, TaskLifecyclePhase};
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

#[derive(Debug)]
struct PreparedTaskStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    position: FlowPosition,
}

pub(crate) async fn run(store: SharedStore, task_id: TaskId) -> Result<()> {
    let result = run_task_with(
        store.clone(),
        task_id.clone(),
        Box::new(crate::harness::default_create_harness),
    )
    .await;
    if let Err(error) = &result {
        record_unhandled_failure(&store, &task_id, error).await;
    }
    result
}

fn take_task_launch_env(key: &'static str, label: &str) -> Result<Option<String>> {
    let Some(value) = std::env::var_os(key) else {
        return Ok(None);
    };
    std::env::remove_var(key);
    value
        .into_string()
        .map(Some)
        .map_err(|_| anyhow!("{label} is not valid UTF-8"))
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

async fn run_task_with(
    store: SharedStore,
    task_id: TaskId,
    create_harness: crate::harness::CreateHarness,
) -> Result<()> {
    let mut task = store
        .get_task(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} not found"))?;
    let wave = owning_wave(&store, &task).await?;
    let project = owning_project(&store, &task).await?;
    store.update_task(&task).await?;
    store
        .append_task_event(&task.id, &TaskEventKind::Started)
        .await?;
    let mut flow = resume_task_phase(&task)?;
    let Some(mut prepared) =
        prepare_task_flow_step(&store, &mut task, wave.name(), &mut flow, None).await?
    else {
        return Ok(());
    };
    let (harness_name, _) = crate::engine::config::parse_agent(&task.agent);
    let capture = crate::run_record::CaptureHandle::begin(crate::run_record::RunSpec {
        harness: prepared.turn.harness.clone(),
        model: prepared.turn.model.clone(),
        surface: "headless".to_string(),
        cwd: Path::new(&task.worktree).to_path_buf(),
        repo: Some(Path::new(wave.repo()).to_path_buf()),
        worktree: Some(Path::new(&task.worktree).to_path_buf()),
        skill: flow.current().map(|step| step.step.clone()),
        subjects: vec![
            crate::run_record::SubjectAttribution::declared(format!("wave:{}", wave.name())),
            crate::run_record::SubjectAttribution::declared(format!(
                "project:{}",
                project.plan.slug
            )),
            crate::run_record::SubjectAttribution::declared(format!(
                "task:{}",
                task.plan.identifier
            )),
        ],
    })?;
    capture.record_input("initial", &prepared.turn.input);
    prepared.turn.config.env.extend(capture.environment());
    capture.mark_spawn_requested();
    let capture = Some(capture);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)
        .inspect_err(|_| {
            finish_capture(capture.as_ref(), "failed");
        })?;
    let requested_account =
        take_task_launch_env(crate::ops::TASK_ACCOUNT_ID_ENV, "Task provider account id")?
            .map(|value| crate::store::ProviderAccountId::parse(&value))
            .transpose()
            .map_err(|reason| anyhow!("invalid Task provider account route: {reason}"))?;
    harness.set_provider_account_id(requested_account);
    harness.set_provider_session_id(
        take_task_launch_env(crate::ops::TASK_RESUME_TOKEN_ENV, "Task resume token")?
            .or_else(|| task.provider_session_id.clone()),
    );
    if let Err(error) = harness.start(&prepared.turn.config).await {
        finish_capture(capture.as_ref(), "failed");
        return Err(error);
    }
    task.provider = harness_name;
    task.provider_session_id = harness.provider_session_id();
    if let Err(error) = store.update_task(&task).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    let mut state_fingerprint = task_state_fingerprint(&task)?;
    let mut gate_fingerprint = if task.lifecycle_phase == TaskLifecyclePhase::Finally {
        Some(task_gate_fingerprint(&task)?)
    } else {
        None
    };

    if let Some(capture) = &capture {
        capture.set_provider_session_id(task.provider_session_id.clone());
    }
    start_prepared_task_step(
        &store,
        &mut task,
        harness.as_mut(),
        &mut flow,
        None,
        prepared,
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
        "task {}> attached; /status, /interrupt, /detach, or type a message/instruction",
        task.plan.identifier
    );
    let mut last_text = String::new();
    let mut command_failures = Vec::new();
    'runner: loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(
                        &store,
                        &task,
                        harness.as_mut(),
                        line,
                    ).await?;
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut task,
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
                    store.update_task(&task).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnStarted { .. } => {
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
                    }
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            let reason = drain_turn_failure_reason(
                                &mut event_rx,
                                "provider turn failed",
                            );
                            return finish_body_failure(
                                &store,
                                &mut task,
                                harness.as_mut(),
                                &reason,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        if execution_blocker_at_handoff(status, &command_failures).is_some() {
                            return finish_execution_blocked(
                                &store,
                                &mut task,
                                harness.as_mut(),
                                &command_failures,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        let mut flow_iteration_completed =
                            finish_task_flow_turn(&mut flow, status)?;
                        let mut finally_ops_ran = false;
                        if !flow_iteration_completed
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
                        let latest = store
                            .get_task(&task.id)
                            .await?
                            .ok_or_else(|| anyhow!("Task {} disappeared", task.id))?;
                        sync_task_state(&mut task, &latest);
                        record_task_flow_position(&mut task, &flow)?;
                        store.update_task(&task).await?;
                        if flow_iteration_completed
                                && task.lifecycle_phase == TaskLifecyclePhase::First
                            {
                                task.enter_loop()?;
                                store.update_task(&task).await?;
                                flow = resume_task_phase(&task)?;
                                flow_iteration_completed = false;
                                state_fingerprint = task_state_fingerprint(&task)?;
                                gate_fingerprint = None;
                                last_text.clear();
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
                                    store.update_task(&task).await?;
                                    let Some(()) = start_resumed_task_phase(
                                        &store,
                                        &mut task,
                                        harness.as_mut(),
                                        &mut flow,
                                        wave.name(),
                                        capture.as_ref(),
                                    )
                                    .await?
                                    else {
                                        return park_task_at_human(
                                            Some(harness.as_mut()),
                                            capture.as_ref(),
                                        )
                                        .await;
                                    };
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
                                    wave.name(),
                                    &mut flow,
                                    capture.as_ref().map(crate::run_record::CaptureHandle::run_id),
                                )
                                .await?
                                else {
                                    return park_task_at_human(
                                        Some(harness.as_mut()),
                                        capture.as_ref(),
                                    )
                                    .await;
                                };
                                start_prepared_task_step(
                                    &store,
                                    &mut task,
                                    harness.as_mut(),
                                    &mut flow,
                                    capture.as_ref(),
                                    prepared,
                                )
                                .await?;
                                continue 'runner;
                            }
                            let summary = progress_summary(&last_text);
                            let latest = store
                                .get_task(&task.id)
                                .await?
                                .ok_or_else(|| anyhow!("Task {} disappeared", task.id))?;
                            sync_task_state(&mut task, &latest);
                            let observed_pr = crate::ops::task::reconcile_task_pr(
                                &store,
                                &mut task,
                            )
                            .await
                            .map_err(|error| anyhow!(error.to_string()))?;
                            let (stopped_done, stopped_reason) = if let Some(proposal) = approved_gate {
                                (proposal.done, proposal.reason)
                            } else if status == Lifecycle::Interrupted {
                                (
                                    false,
                                    "Task flow step interrupted; waiting for resume or another instruction".to_string(),
                                )
                            } else if let Some(pr) = observed_pr
                                .as_ref()
                                .filter(|pr| pr.phase() == PrPhase::Open)
                            {
                                (false, crate::ops::task::open_pr_wait_reason(pr))
                            } else {
                                let next_fingerprint = task_state_fingerprint(&task)?;
                                if next_fingerprint != state_fingerprint {
                                    state_fingerprint = next_fingerprint;
                                    store.update_task(&task).await?;
                                    let Some(prepared) = prepare_task_flow_step(
                                        &store,
                                        &mut task,
                                        wave.name(),
                                        &mut flow,
                                        capture
                                            .as_ref()
                                            .map(crate::run_record::CaptureHandle::run_id),
                                    )
                                    .await?
                                    else {
                                        return park_task_at_human(
                                            Some(harness.as_mut()),
                                            capture.as_ref(),
                                        )
                                        .await;
                                    };
                                    start_prepared_task_step(
                                        &store,
                                        &mut task,
                                        harness.as_mut(),
                                        &mut flow,
                                        capture.as_ref(),
                                        prepared,
                                    )
                                    .await?;
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
                                task.enter_finally(TaskGateProposal {
                                    done: stopped_done,
                                    reason: stopped_reason,
                                })?;
                                gate_fingerprint = Some(task_gate_fingerprint(&task)?);
                                store.update_task(&task).await?;
                                let Some(()) = start_resumed_task_phase(
                                    &store,
                                    &mut task,
                                    harness.as_mut(),
                                    &mut flow,
                                    wave.name(),
                                    capture.as_ref(),
                                )
                                .await?
                                else {
                                    return park_task_at_human(
                                        Some(harness.as_mut()),
                                        capture.as_ref(),
                                    )
                                    .await;
                                };
                                last_text.clear();
                                continue 'runner;
                            }
                            store.update_task(&task).await?;
                            let _ = harness.stop().await;
                            finish_capture(
                                capture.as_ref(),
                                if status == Lifecycle::Interrupted {
                                    "interrupted"
                                } else {
                                    "completed"
                                },
                            );
                            if !summary.is_empty() {
                                store.append_task_event(
                                    &task.id,
                                    &TaskEventKind::Progress {
                                        summary: summary.clone(),
                                    },
                                ).await?;
                            }
                            if stopped_done {
                                store.complete_task(&task, None).await?;
                            }
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message, .. } => {
                        let reason = format!("{code}: {message}");
                        return finish_body_failure(
                            &store,
                            &mut task,
                            harness.as_mut(),
                            &reason,
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
    wave_name: &str,
    flow: &Playhead,
) -> Result<PreparedTaskStep> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await?;
    let steers = store.work_steers(&work).await?;
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
    store.update_task(task).await?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = owning_project(store, task).await?;
    let seed = task_seed(task, &project.plan, &pr, wave_name, &steers);
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
    })
}

async fn prepare_task_flow_step(
    store: &SharedStore,
    task: &mut Task,
    wave_name: &str,
    flow: &mut Playhead,
    source_run_id: Option<RunId>,
) -> Result<Option<PreparedTaskStep>> {
    loop {
        let prepared = prepare_task_flow_step_once(store, task, wave_name, flow).await?;
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
            .set_flow_position(&prepared.position.work, prepared.position.clone())
            .await?;
        let placement = store.placement(&prepared.position.work).await?;
        let ask = store
            .create_ask(
                crate::durable::AskOrigin {
                    work: prepared.position.work.clone(),
                    source_run_id: source_run_id.clone(),
                    home_id: placement.home_id,
                    cwd: task.worktree.clone(),
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
                complete_human_task_step(store, task, flow).await?;
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
                        &prepared.position.work,
                        crate::durable::Author::User,
                        &format!("Human flow node {node_id} did not accept the step: {reason}"),
                    )
                    .await?;
                task.phase_cursor = preceding_autonomous_step(flow, prepared.position.step_index)?;
                task.phase_iteration += 1;
                task.updated_at = time::OffsetDateTime::now_utc();
                store.update_task(task).await?;
                *flow = resume_task_phase(task)?;
            }
            _ => anyhow::bail!("human Task Ask {} has an invalid terminal result", ask.id),
        }
    }
}

async fn complete_human_task_step(
    store: &SharedStore,
    task: &mut Task,
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
    store.update_task(task).await?;
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
    task: &mut Task,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
) -> Result<()> {
    open_task_flow_body(flow, task)?;
    harness.send_input(&prepared.input).await?;
    Ok(())
}

async fn start_prepared_task_step(
    store: &SharedStore,
    task: &mut Task,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    capture: Option<&crate::run_record::CaptureHandle>,
    prepared: PreparedTaskStep,
) -> Result<()> {
    store
        .set_flow_position(&prepared.position.work, prepared.position.clone())
        .await?;
    debug_assert!(!prepared.position.human);
    if let Some(capture) = capture {
        capture.record_input("queued", &prepared.turn.input);
    }
    start_task_flow_turn(task, harness, flow, prepared.turn).await
}

async fn start_resumed_task_phase(
    store: &SharedStore,
    task: &mut Task,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    wave_name: &str,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<Option<()>> {
    *flow = resume_task_phase(task)?;
    let Some(prepared) = prepare_task_flow_step(
        store,
        task,
        wave_name,
        flow,
        capture.map(crate::run_record::CaptureHandle::run_id),
    )
    .await?
    else {
        return Ok(None);
    };
    start_prepared_task_step(store, task, harness, flow, capture, prepared).await?;
    Ok(Some(()))
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
/// Settle the harness launch on every terminal path.
fn finish_capture(capture: Option<&crate::run_record::CaptureHandle>, outcome: &str) {
    let Some(capture) = capture else { return };
    if let Err(error) = capture.finish(outcome) {
        tracing::warn!(%error, "failed to finish Task Run record");
    }
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
    harness: Option<&mut dyn Harness>,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "completed");
    if let Some(harness) = harness {
        let _ = harness.stop().await;
    }
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

fn task_gate_fingerprint(task: &Task) -> Result<String> {
    let state = crate::engine::git::material_worktree_state(Path::new(&task.worktree))?;
    Ok(hex::encode(Sha256::digest(state.as_bytes())))
}

async fn handle_attachment(
    store: &SharedStore,
    task: &Task,
    harness: &mut dyn Harness,
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
    let target = ChildRef::Task(task.id.clone());
    if line == "/interrupt" {
        harness.interrupt().await?;
        println!("interrupted active provider turn");
    } else {
        let work = store.work_for_child(&target).await?;
        let receipt = store
            .append_steer(&work, crate::durable::Author::User, line)
            .await?;
        harness.send_input(line).await?;
        println!("queued {}", receipt.steer.id);
    }
    Ok(())
}

async fn record_unhandled_failure(store: &SharedStore, task_id: &TaskId, error: &anyhow::Error) {
    let detail = error.to_string();
    let (message, resumable) = unhandled_failure_receipt(&detail);
    if store
        .latest_task_event(task_id)
        .await
        .ok()
        .flatten()
        .is_some_and(|event| {
            matches!(
                event.kind,
                TaskEventKind::Failed {
                    error,
                    resumable: recorded_resumable,
                } if (error == detail || error == message) && recorded_resumable == resumable
            )
        })
    {
        return;
    }
    let event = TaskEventKind::Failed {
        error: message,
        resumable,
    };
    if let Err(persist_error) = store.append_task_event(task_id, &event).await {
        tracing::error!(
            task = %task_id,
            error = %persist_error,
            "Task failure receipt did not persist"
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

async fn finish_failed(
    store: &SharedStore,
    task: &mut Task,
    harness: &mut dyn Harness,
    error: &str,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store.update_task(task).await?;
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::Failed {
                error: error.to_string(),
                resumable: true,
            },
        )
        .await?;
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
    harness: &mut dyn Harness,
    failures: &[String],
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    let reason = execution_blocked_reason(failures);
    finish_nonresumable(store, task, harness, &reason, capture).await
}

async fn finish_nonresumable(
    store: &SharedStore,
    task: &mut Task,
    harness: &mut dyn Harness,
    reason: &str,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store.update_task(task).await?;
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::Failed {
                error: reason.to_string(),
                resumable: false,
            },
        )
        .await?;
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

/// Name the infrastructure capability that blocked a Task body while keeping
/// any attached PR visible for a later resume.
fn infra_blocked_reason(capability: &str, detail: &str, pr_number: Option<u32>) -> String {
    let pr_note = pr_number
        .map(|n| format!(" Pull request #{n} stays attached."))
        .unwrap_or_default();
    format!("Task execution blocked by {capability}: {detail}.{pr_note}")
}

/// Stop the body and record an infrastructure failure (provider outage, GitHub
/// observation failure), keeping the active PR attached so a resume after the
/// capability recovers picks up the same PR. Returns `Ok(())` after the atomic
/// Task failure receipt settles the Run, so the outer boundary does not record
/// the same failure twice.
async fn finish_infra_blocked(
    store: &SharedStore,
    task: &mut Task,
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
    store.update_task(task).await?;
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::Failed {
                error: reason,
                resumable: true,
            },
        )
        .await?;
    Ok(())
}

async fn finish_body_failure(
    store: &SharedStore,
    task: &mut Task,
    harness: &mut dyn Harness,
    reason: &str,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    if let Some(blocker) = provider_credential_blocker(reason) {
        return finish_nonresumable(store, task, harness, &blocker, capture).await;
    }
    if store.active_task_pr(&task.id).await?.is_some() {
        finish_capture(capture, "failed");
        return finish_infra_blocked(store, task, harness, "provider", reason).await;
    }
    finish_failed(store, task, harness, reason, capture).await
}

fn task_seed(
    task: &Task,
    project: &ProjectPlan,
    pr: &crate::task::TaskPr,
    wave_name: &str,
    steers: &[Steer],
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
        "Advance Linear task {identifier}: {title}\n\n{description}\n\nLinear Project: {project} ({project_id})\n{project_context}\n\n{direction}\n\nTask directive snapshot synced at: {task_snapshot_synced_at}\nProject definition snapshot synced at: {project_snapshot_synced_at}\nWave: {wave}\nTask: {task_id}\nLifecycle phase: {lifecycle_phase} (epoch {phase_epoch}, iteration {phase_iteration}, gate cycle {gate_cycle})\n{gate_proposal}\nWorktree: {worktree}\nPR {pr_sequence}: {pr_branch}\nBase commit: {base_commit}\n{placement}\n\nThis PR owns one serial branch. The pinned finally flow owns landing and Task completion. `lf pr abandon` discards only this PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward. Watched landing owns automatic post-merge rotation.",
        identifier = task.plan.identifier,
        title = task.plan.title,
        description = task.plan.description,
        project = project.name,
        project_id = project.id.as_str(),
        project_context = project.prompt_context,
        direction = render_steers(steers),
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

/// Prepare the skill's REAL harness turn for a human flow gate — the same turn
/// `lf <skill>` builds (its system prompt, context, config), with only the
/// settlement contract appended to the input. The Ask session then IS the skill
/// run, not a generic agent handed the skill's task text (which self-approved).
pub(crate) async fn flow_step_harness_turn(
    store: &SharedStore,
    ask: &crate::durable::Ask,
) -> Result<crate::lf::commands::run::PreparedHarnessTurn> {
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
    let steers = store.work_steers(&ask.origin.work).await?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = owning_project(store, &task).await?;
    let wave = owning_wave(store, &task).await?;
    let seed = task_seed(&task, &project.plan, &pr, wave.name(), &steers);
    let mut prepared =
        crate::lf::commands::run::prepare_interactive_harness_turn(skill, &seed, wave.name())?;
    prepared.input = human_flow_settlement(&prepared.input, skill, node_id, &ask.id);
    Ok(prepared)
}

/// Append only the settlement contract to the skill's real input. The skill's
/// own harness governs the review; the ask system owns just how the flow advances.
fn human_flow_settlement(input: &str, skill: &str, node_id: &str, ask_id: &AskId) -> String {
    format!(
        "{input}\n\n<lf:human-flow-node>\nThis `{skill}` run is the actual writable Task step at human node `{node_id}`, not an advisory review. Settle explicitly before leaving:\n- `lf ask resolve {ask_id} \"<concise verified summary>\"` accepts the step\n- `lf ask decline {ask_id} \"<reason>\"` returns to the preceding autonomous step\n- `lf ask release {ask_id} \"<reason>\"` leaves the node waiting\nA final response, clean exit, Ctrl-D, or window close never advances the flow.\n</lf:human-flow-node>"
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
mod planning_tests {
    use super::{
        completed_boundary_failure, execution_blocker_at_handoff, human_flow_settlement,
        preceding_autonomous_step, sync_task_state, task_seed, unhandled_failure_receipt,
    };
    use crate::chat::types::Lifecycle;
    use crate::durable::{AskBody, AskId, AskResult, AskTarget, RunId};
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

    async fn human_task_fixture() -> (SharedStore, Task, Playhead) {
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
        let database = database.join("registry.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(database.clone()))
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
        let flow = super::resume_task_phase(&task).unwrap();
        (store, task, flow)
    }

    #[test]
    fn human_flow_settlement_wraps_the_skill_input() {
        let prompt = human_flow_settlement(
            "the real review-design harness input",
            "review-design",
            "review_kickoff",
            &AskId::parse("ask_00000000000000000000000000000001").unwrap(),
        );

        // The skill's own input is preserved verbatim; only settlement is appended.
        assert!(prompt.starts_with("the real review-design harness input"));
        assert!(prompt.contains("<lf:human-flow-node>"));
        assert!(prompt.contains("lf ask resolve ask_00000000000000000000000000000001"));
        assert!(prompt.contains("lf ask decline ask_00000000000000000000000000000001"));
        assert!(prompt.contains("lf ask release ask_00000000000000000000000000000001"));
    }

    #[tokio::test]
    async fn provider_failure_records_planning() {
        let (store, mut task, _) = human_task_fixture().await;
        let mut harness = UnusedHarness;

        super::finish_body_failure(
            &store,
            &mut task,
            &mut harness,
            "opencode_disconnected: provider stream ended",
            None,
        )
        .await
        .unwrap();

        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(matches!(
            &events.last().unwrap().kind,
            TaskEventKind::Failed {
                error,
                resumable: true,
            } if error.contains("provider stream ended")
        ));
    }

    #[test]
    fn normal_task_completion_preserves_delivery_permission_and_ask_network_failures() {
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
            Some("network access is disabled by policy"),
            Some(1),
        )
        .unwrap();
        let reason = execution_blocker_at_handoff(Lifecycle::Completed, &[commit, ask])
            .expect("normal task_complete with unresolved capability failures is blocked");

        assert!(reason.contains(".git/worktrees/task/index.lock"));
        assert!(reason.contains("Operation not permitted"));
        assert!(reason.contains("network access is disabled by policy"));
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

    #[tokio::test]
    async fn unhandled_task_failure_is_a_planning_receipt() {
        let (store, task, _) = human_task_fixture().await;

        super::record_unhandled_failure(
            &store,
            &task.id,
            &anyhow::anyhow!("provider stream closed"),
        )
        .await;

        let events = store.recent_task_events(&task.id, 10).await.unwrap();
        assert!(matches!(
            &events[0].kind,
            TaskEventKind::Failed { error, resumable: true }
                if error == "task process failed: provider stream closed"
        ));
    }

    async fn settle_human_task(store: &SharedStore, result: AskResult) -> crate::durable::Ask {
        let ask = store
            .pending_asks(&AskTarget::User)
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let claim = store.claim_ask(&ask.id).await.unwrap();
        assert!(claim.needs_launch);
        store
            .mark_ask_presented(&ask.id, &claim.run_id)
            .await
            .unwrap();
        let ask = store
            .settle_ask(&ask.id, &claim.run_id, result)
            .await
            .unwrap();
        ask
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn released_human_task_attempt_keeps_the_same_node_parked() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, mut flow) = human_task_fixture().await;
        let parent_run_id = RunId::new();
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut flow,
            Some(parent_run_id.clone()),
        )
        .await
        .unwrap()
        .is_none());
        let ask = store
            .pending_asks(&AskTarget::User)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(ask.origin.source_run_id, Some(parent_run_id));
        let claim = store.claim_ask(&ask.id).await.unwrap();
        assert!(claim.needs_launch);
        store
            .release_ask(&ask.id, &claim.run_id, Some("not finished"))
            .await
            .unwrap();

        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut flow,
            None
        )
        .await
        .unwrap()
        .is_none());
        let queued = store.pending_asks(&AskTarget::User).await.unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].id, ask.id);
        assert_eq!(task.phase_cursor, 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn restarted_human_task_reuses_the_queued_ask_without_sql_run_authority() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, mut flow) = human_task_fixture().await;
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut flow,
            None
        )
        .await
        .unwrap()
        .is_none());
        let original = store
            .pending_asks(&AskTarget::User)
            .await
            .unwrap()
            .remove(0);
        let mut restarted_flow = super::resume_task_phase(&task).unwrap();

        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut restarted_flow,
            None,
        )
        .await
        .unwrap()
        .is_none());
        let recovered = store.pending_asks(&AskTarget::User).await.unwrap();
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].id, original.id);
        assert_eq!(recovered[0].origin, original.origin);
        let claim = store.claim_ask(&recovered[0].id).await.unwrap();
        assert!(claim.needs_launch);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn human_task_node_queues_without_starting_a_provider_and_resolve_advances() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, mut flow) = human_task_fixture().await;
        let prepared =
            super::prepare_task_flow_step(&store, &mut task, "human-task-proof", &mut flow, None)
                .await
                .unwrap();
        assert!(prepared.is_none());
        let queued = store.pending_asks(&AskTarget::User).await.unwrap();
        assert_eq!(queued.len(), 1);
        assert!(matches!(
            &queued[0].request,
            AskBody::FlowStep { node_id, skill, .. }
                if node_id == "review_kickoff" && skill == "review-design"
        ));
        let turn = super::flow_step_harness_turn(&store, &queued[0])
            .await
            .unwrap();
        // The session runs the skill's real harness turn; only the settlement
        // contract is appended — no generic-agent reviewer-mode injection.
        assert!(turn.input.contains("<lf:human-flow-node>"));
        assert!(turn.input.contains("lf ask resolve"));

        settle_human_task(
            &store,
            AskResult::Resolved {
                summary: "design accepted".to_string(),
            },
        )
        .await;
        let prepared =
            super::prepare_task_flow_step(&store, &mut task, "human-task-proof", &mut flow, None)
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
        let (store, mut task, mut flow) = human_task_fixture().await;
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut flow,
            None
        )
        .await
        .unwrap()
        .is_none());
        let declined = settle_human_task(
            &store,
            AskResult::Declined {
                reason: "narrow the design".to_string(),
            },
        )
        .await;

        let prepared =
            super::prepare_task_flow_step(&store, &mut task, "human-task-proof", &mut flow, None)
                .await
                .unwrap()
                .expect("decline returns to autonomous kickoff");
        assert_eq!(prepared.position.step, "kickoff");
        assert_eq!(task.phase_cursor, 0);
        assert_eq!(task.phase_iteration, 1);
        let steers = store.work_steers(&declined.origin.work).await.unwrap();
        assert!(steers
            .iter()
            .any(|steer| steer.text.contains("narrow the design")));

        super::open_task_flow_body(&mut flow, &task).unwrap();
        assert!(!super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap());
        super::record_task_flow_position(&mut task, &flow).unwrap();
        store.update_task(&task).await.unwrap();
        assert!(super::prepare_task_flow_step(
            &store,
            &mut task,
            "human-task-proof",
            &mut flow,
            None
        )
        .await
        .unwrap()
        .is_none());
        let queued = store.pending_asks(&AskTarget::User).await.unwrap();
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
        let seed = task_seed(&task, &project, &pr, "wave", &[]);

        assert!(seed.contains("Current project name"));
        assert!(seed.contains("Current project definition"));
        assert!(seed.contains("Task directive snapshot synced at: 11"));
        assert!(seed.contains("Project definition snapshot synced at: 22"));
        assert!(!seed.contains("metric-portfolio"));
        assert!(!seed.contains("project-owned-metrics"));
    }
}
