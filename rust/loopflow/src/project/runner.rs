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
    absorb_run_control, apply_input as apply_child_input, send_outstanding_steers,
    take_current_input as take_child_input, CommandStop, PendingInput,
};
use crate::durable::{Basis, BoundarySeed, RunContext, WorkStatus};
use crate::engine::wave_config::read_wave_config;
use crate::harness::{
    classify_disconnect_recovery, default_create_harness, drain_turn_failure_reason,
    ApprovalPolicy, Harness, RecoveryDecision,
};
use crate::project::{ChildEventPayload, Project, ProjectEventKind, ProjectId};
use crate::provider_account::recovery::{
    capability_key, plan_run_route_recovery, settle_route_recovery, stop_invocation_for_recovery,
    ExactRoute, RecoveryChoice, RecoverySettlement, RecoveryStopOutcome,
};
use crate::store::SharedStore;
use crate::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::wave::Wave;

const CONTROL_TICK_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
struct PreparedProjectStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    basis: Basis,
    planning: crate::ops::task_pm::ResolvedProject,
}

pub(crate) async fn run(
    store: SharedStore,
    project_id: ProjectId,
    lease: &RunContext,
) -> Result<()> {
    let result = run_project_inner(store.clone(), project_id.clone(), lease).await;
    if let Err(error) = &result {
        record_unhandled_failure(&store, &project_id, lease, error).await;
    }
    result
}

async fn owning_wave(store: &SharedStore, project: &Project) -> Result<Wave> {
    store
        .get_wave(&project.wave_id)
        .await?
        .ok_or_else(|| anyhow!("owning Wave {} is not registered", project.wave_id))
}

async fn spawn_failover(
    store: &SharedStore,
    project: &Project,
    lease: &RunContext,
    wave: &Wave,
    route: &ExactRoute,
) -> Result<()> {
    let tmux_name = format!(
        "lf-project-{}-{}",
        &project.id.as_str()[3..11],
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    crate::ops::launch_in_run(
        store,
        lease,
        crate::ops::RunLaunch {
            work: crate::durable::WorkRef::Project(project.id.clone()),
            wave_id: project.wave_id.clone(),
            cwd: Path::new(wave.repo()).to_path_buf(),
            tmux_name,
            agent: route.agent.agent(),
            account_id: route.account_id.clone(),
            resume_token: project.provider_session_id.clone(),
        },
    )
    .await
    .map(|_| ())
    .map_err(|error| anyhow!(error.to_string()))
}

async fn run_project_inner(
    store: SharedStore,
    project_id: ProjectId,
    lease: &RunContext,
) -> Result<()> {
    let mut project = store
        .get_project(&project_id)
        .await?
        .ok_or_else(|| anyhow!("Project {project_id} not found"))?;
    let wave = owning_wave(&store, &project).await?;
    store.update_project_for_run(&project, lease).await?;
    store
        .append_project_event_for_run(&project.id, lease, &ProjectEventKind::Started)
        .await?;
    let target = ChildRef::Project(project.id.clone());
    let work = store.work_for_child(&target).await?;
    let run_context = crate::ops::required_run_context(&store).await?;
    if run_context.work != work {
        anyhow::bail!(
            "ambient Run context does not own Project Work {}",
            work.id()
        );
    }
    let run = store
        .current_run(&work)
        .await?
        .ok_or_else(|| anyhow!("Project Work {} has no active Run", work.id()))?;
    let invocation = store
        .open_invocation(lease)
        .await?
        .ok_or_else(|| anyhow!("Project Run {} has no open Invocation", lease.run_id))?;
    let mut supervision = crate::trace::SupervisedInvocation {
        invocation_id: invocation.id.clone(),
        supervising_run_id: run.id,
        account_id: invocation.route.account_id.clone(),
        resume_token: invocation.resume_token.clone(),
    };
    let mut ask_lane = crate::ops::ask::AskLane::new(work.clone(), lease.clone());
    ask_lane.reconcile(&store).await?;
    let mut pending = VecDeque::new();
    let initial_input = take_current_input(&store, &project, lease, &mut pending).await?;
    let observations = consume_task_observations(&store, &mut project, lease).await?;
    let (mut flow, _) = Playhead::new(QueuedInvocation::load(Path::new(wave.repo()), "project")?);
    let prepared =
        prepare_project_flow_step(&store, &mut project, lease, &wave, &flow, &observations).await?;
    let mut active_planning = prepared.planning.clone();
    let (harness_name, _) = crate::engine::config::parse_agent(&project.agent);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)?;
    harness.set_provider_session_id(project.provider_session_id.clone());
    let requested_account = invocation
        .route
        .account_id
        .as_deref()
        .map(crate::store::ProviderAccountId::parse)
        .transpose()
        .map_err(|reason| anyhow!("invalid Invocation account route: {reason}"))?;
    harness.set_provider_account_id(requested_account);
    store.validate_run_context(lease).await?;
    harness.start(&prepared.turn.config).await?;
    project.provider = harness_name;
    project.provider_session_id = harness.provider_session_id();
    let invocation = store
        .observe_invocation_provider(
            lease,
            &invocation.id,
            harness.provider_account_id(),
            project.provider_session_id.clone(),
        )
        .await?;
    let invocation_id = invocation.id.clone();
    let invocation_route = invocation.route.clone();
    supervision.account_id = invocation.route.account_id.clone();
    supervision.resume_token = invocation.resume_token.clone();
    if let Err(error) = store.update_project_for_run(&project, lease).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    let capture = flow.current().and_then(|step| {
        let context = match crate::journal::trace_capture_context(
            Path::new(wave.repo()),
            Some(step.flow.clone()),
            Some(step.step.clone()),
        ) {
            Ok(context) => context,
            Err(error) => {
                tracing::warn!(%error, "failed to establish Project trace capture");
                return None;
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
                tracing::warn!(%error, "failed to establish Project trace capture");
                None
            }
        }
    });
    if let Some(capture) = &capture {
        capture.set_provider_session_id(project.provider_session_id.clone());
    }
    let mut active_basis = prepared.basis.clone();

    let mut flow_turn_active = false;
    let mut input_turn_active = initial_input.is_some();
    let mut provider_turn_active;
    if let Some(input) = initial_input {
        apply_input(&store, &project, lease, harness.as_mut(), input).await?;
        provider_turn_active = true;
    } else if input_turn_active {
        apply_input(
            &store,
            &project,
            lease,
            harness.as_mut(),
            PendingInput::system(prepared.turn.input),
        )
        .await?;
        provider_turn_active = true;
    } else {
        start_project_flow_turn(
            &store,
            &mut project,
            lease,
            harness.as_mut(),
            &mut flow,
            None,
            prepared,
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
        "project {}> attached; /status, /interrupt, /detach, or type an instruction",
        project.plan.slug
    );
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut control_tick = tokio::time::interval(CONTROL_TICK_INTERVAL);
    control_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    spawn_ask_comment_publication(store.clone());
    let mut last_text = String::new();
    let mut turn_had_durable_side_effect = false;
    loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(&store, &project, lease, line).await?;
                }
            }
            _ = poll.tick() => {
                ask_lane.reconcile(&store).await?;
                let active_turn_id = provider_turn_active
                    .then(|| capture.as_ref().map(|capture| capture.current_turn_id()))
                    .flatten();
                if let Some(stop) = absorb_run_control(
                    &store,
                    &run_context,
                    harness.as_mut(),
                    provider_turn_active,
                    active_turn_id.as_deref(),
                ).await? {
                    return finish_command_stop(
                        &store,
                        &mut project,
                        lease,
                        harness.as_mut(),
                        stop,
                        capture.as_ref(),
                    )
                    .await;
                }
                if provider_turn_active {
                    if let Some(capture) = &capture {
                        send_outstanding_steers(
                            &store,
                            &run_context,
                            harness.as_mut(),
                            &capture.current_turn_id(),
                            &active_basis,
                        )
                        .await?;
                    }
                }
            }
            _ = control_tick.tick() => spawn_ask_comment_publication(store.clone()),
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut project,
                        lease,
                        harness.as_mut(),
                        "provider event stream closed",
                        capture.as_ref(),
                    )
                    .await;
                };
                if let Some(capture) = &capture {
                    capture.record_conversation(event.clone());
                }
                let provider_session_id = harness.provider_session_id();
                if provider_session_id != project.provider_session_id {
                    project.provider_session_id = provider_session_id;
                    store.update_project_for_run(&project, lease).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnStarted { .. } => {
                        provider_turn_active = true;
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
                        let was_control_turn = input_turn_active;
                        input_turn_active = false;
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
                            finish_capture(capture.as_ref(), "failed");
                            return fail_and_maybe_recover(
                                &store,
                                &mut project,
                                lease,
                                &invocation_id,
                                &invocation_route,
                                harness.as_mut(),
                                &wave,
                                &reason,
                                turn_had_durable_side_effect,
                            )
                            .await;
                        }
                        if !was_control_turn {
                            if let Err(error) = verify_control_plane_checkout(Path::new(wave.repo())) {
                                return finish_failed(
                                    &store,
                                    &mut project,
                                    lease,
                                    harness.as_mut(),
                                    &error.to_string(),
                                    capture.as_ref(),
                                )
                                .await;
                            }
                        }
                        let flow_iteration_completed = if flow_turn_active {
                            finish_project_flow_turn(&mut flow, status)?
                        } else {
                            false
                        };
                        flow_turn_active = false;
                        if let Some(input) = take_current_input(&store, &project, lease, &mut pending).await? {
                            apply_input(&store, &project, lease, harness.as_mut(), input).await?;
                            input_turn_active = true;
                            provider_turn_active = true;
                            continue;
                        }
                        if status != Lifecycle::Interrupted {
                            let observations =
                                consume_task_observations(&store, &mut project, lease).await?;
                            if !observations.is_empty() {
                                apply_input(
                                    &store,
                                    &project,
                                    lease,
                                    harness.as_mut(),
                                    PendingInput::system(format!(
                                            "New supervised Task observations arrived. Continue the same Project iteration:\n{}",
                                            observations.join("\n")
                                        )),
                                ).await?;
                                provider_turn_active = true;
                                continue;
                            }
                        }
                        if !flow_iteration_completed && status != Lifecycle::Interrupted {
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut project,
                                lease,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            active_planning = prepared.planning.clone();
                            active_basis = prepared.basis.clone();
                            start_project_flow_turn(
                                &store,
                                &mut project,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                capture.as_ref(),
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            provider_turn_active = true;
                            continue;
                        }
                        let summary = bounded_summary(&last_text);
                        if flow_iteration_completed {
                            project.iteration += 1;
                            store.append_project_event_for_run(
                                &project.id,
                                lease,
                                &ProjectEventKind::IterationCompleted {
                                    iteration: project.iteration,
                                    summary: summary.clone(),
                                },
                            ).await?;
                        }
                        let mut outcome = inspect_outcome(&store, &project, &active_planning).await?;
                        if status == Lifecycle::Interrupted {
                            outcome.disposition = ProjectDisposition::Wait;
                        }
                        if outcome.disposition == ProjectDisposition::Continue {
                            project.last_state_fingerprint = Some(outcome.fingerprint);
                            project.updated_at = time::OffsetDateTime::now_utc();
                            store.update_project_for_run(&project, lease).await?;
                            last_text.clear();
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut project,
                                lease,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            active_planning = prepared.planning.clone();
                            active_basis = prepared.basis.clone();
                            start_project_flow_turn(
                                &store,
                                &mut project,
                                lease,
                                harness.as_mut(),
                                &mut flow,
                                capture.as_ref(),
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            provider_turn_active = true;
                            continue;
                        }
                        project.last_state_fingerprint = Some(outcome.fingerprint);
                        store.update_project_for_run(&project, lease).await?;
                        let _ = harness.stop().await;
                        store.advance_run(
                            lease,
                            crate::durable::RunAdvance::InvocationEnded {
                                invocation_id: invocation_id.clone(),
                                outcome: crate::durable::BoundaryState::Succeeded,
                            },
                        ).await?;
                        finish_capture(capture.as_ref(), "completed");
                        if project_run_must_remain_resident(
                            &store,
                            &project,
                            &mut ask_lane,
                            outcome.disposition,
                        ).await? {
                            return run_ask_only_supervisor(
                                &store,
                                &mut project,
                                lease,
                                &run_context,
                                &work,
                                &active_basis,
                                outcome.disposition,
                                summary,
                                &mut ask_lane,
                                &mut attachment_rx,
                            ).await;
                        }
                        finish_project_outcome(
                            &store,
                            &project,
                            lease,
                            &active_basis,
                            outcome.disposition,
                            summary,
                        ).await?;
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message, .. } => {
                        let reason = format!("{code}: {message}");
                        finish_capture(capture.as_ref(), "failed");
                        return fail_and_maybe_recover(
                            &store,
                            &mut project,
                            lease,
                            &invocation_id,
                            &invocation_route,
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
                    | ConversationEvent::UsageCheckpoint { .. }
                    | ConversationEvent::SuggestedActions { .. }
                    | ConversationEvent::StatusChanged { .. } => {}
                }
            }
        }
    }
}

fn spawn_ask_comment_publication(store: SharedStore) {
    tokio::spawn(async move {
        if let Err(error) = crate::ops::publish_pending_ask_comments(&store).await {
            tracing::warn!(%error, "Ask comment outbox publication failed");
        }
    });
}

async fn project_run_must_remain_resident(
    store: &SharedStore,
    project: &Project,
    ask_lane: &mut crate::ops::ask::AskLane,
    disposition: ProjectDisposition,
) -> Result<bool> {
    if ask_lane.reconcile(store).await? {
        return Ok(true);
    }
    if disposition != ProjectDisposition::Wait {
        return Ok(false);
    }
    project_has_running_tasks(store, project).await
}

async fn project_has_running_tasks(store: &SharedStore, project: &Project) -> Result<bool> {
    for task in store.list_tasks(Some(&project.wave_id)).await? {
        if task.project_id != project.id {
            continue;
        }
        let work = store.work_for_child(&ChildRef::Task(task.id)).await?;
        if matches!(store.work_status(&work).await?, WorkStatus::Running { .. }) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
async fn run_ask_only_supervisor(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    run_context: &RunContext,
    work: &crate::durable::WorkRef,
    settled_basis: &Basis,
    disposition: ProjectDisposition,
    summary: String,
    ask_lane: &mut crate::ops::ask::AskLane,
    attachment_rx: &mut mpsc::UnboundedReceiver<String>,
) -> Result<()> {
    let mut poll = tokio::time::interval(Duration::from_millis(200));
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut control_tick = tokio::time::interval(CONTROL_TICK_INTERVAL);
    control_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    spawn_ask_comment_publication(store.clone());
    loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    handle_attachment(store, project, lease, line).await?;
                }
            }
            _ = poll.tick() => {
                match store.run_control(run_context, None).await? {
                    Some(crate::durable::RunControl::Interrupt)
                    | Some(crate::durable::RunControl::Quiesce { .. })
                    | Some(crate::durable::RunControl::Abandon { .. }) => {
                        store.finish_project_run(
                            project,
                            lease,
                            crate::durable::BoundaryState::Interrupted,
                        ).await?;
                        return Ok(());
                    }
                    None => {}
                }
                if ask_lane.reconcile(store).await? {
                    continue;
                }
                let new_direction = store.boundary_seed(work).await?.basis.revision
                    > settled_basis.revision;
                let new_observations = !store.pending_project_observations(&project.id).await?.is_empty();
                if new_direction || new_observations {
                    store.finish_project_run(
                        project,
                        lease,
                        crate::durable::BoundaryState::Succeeded,
                    ).await?;
                    crate::ops::project::wake_project(&project.id)
                        .await
                        .map_err(|error| anyhow!(error.to_string()))?;
                    return Ok(());
                }
                if disposition != ProjectDisposition::Wait
                    || !project_has_running_tasks(store, project).await?
                {
                    finish_project_outcome(
                        store,
                        project,
                        lease,
                        settled_basis,
                        disposition,
                        summary,
                    ).await?;
                    return Ok(());
                }
            }
            _ = control_tick.tick() => spawn_ask_comment_publication(store.clone()),
        }
    }
}

async fn finish_project_outcome(
    store: &SharedStore,
    project: &Project,
    lease: &RunContext,
    basis: &Basis,
    disposition: ProjectDisposition,
    summary: String,
) -> Result<()> {
    if disposition == ProjectDisposition::Done {
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await?;
        if store.work_status(&work).await? == WorkStatus::Done {
            return Ok(());
        }
        store
            .complete_project_run(project, lease, basis, &summary)
            .await?;
    } else {
        store
            .finish_project_run(project, lease, crate::durable::BoundaryState::Succeeded)
            .await?;
    }
    Ok(())
}

async fn prepare_project_flow_step(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    wave: &Wave,
    flow: &Playhead,
    observations: &[String],
) -> Result<PreparedProjectStep> {
    let planning = refresh_project_plan(store, project, lease, wave).await?;
    let work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
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
    let metric_context = crate::ops::metrics::metric_prompt_section(
        "project-owned-metrics",
        crate::ops::metrics::project_metric_portfolio(
            store,
            wave,
            &planning.snapshot.projects,
            project.plan.id.as_str(),
            time::OffsetDateTime::now_utc(),
        )
        .await,
    );
    let seed = project_seed(
        project,
        wave.name(),
        &boundary,
        observations,
        &metric_context,
    );
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave.name(), None)?;
    prepared.config.agent = Some(project.agent.clone());
    Ok(PreparedProjectStep {
        turn: prepared,
        basis: boundary.basis,
        planning,
    })
}

async fn refresh_project_plan(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    wave: &Wave,
) -> Result<crate::ops::task_pm::ResolvedProject> {
    let planning = crate::ops::task_pm::refresh_project(
        Path::new(wave.repo()),
        wave.name(),
        project.plan.id.as_str(),
    )
    .await
    .map_err(|error| {
        anyhow!(
            "Project plan refresh blocked before the next phase: {error}. Project Work {} did not continue on its stale plan; repair Linear planning, then restart it with `lf project run {}`",
            project.id,
            project.plan.id.as_str()
        )
    })?;
    let plan = crate::ops::project::project_plan(&planning.project, planning.snapshot.synced_at)
        .map_err(|error| anyhow!(error.to_string()))?;
    let (adopted, changed) = store
        .adopt_project_plan_for_run(&project.id, &plan, lease)
        .await
        .map_err(|error| {
            anyhow!(
                "Project plan refresh could not be adopted safely: {error}. Project Work {} did not continue on its stale plan; restart it after repairing the planning conflict",
                project.id
            )
        })?;
    if changed {
        tracing::info!(
            project = %project.id,
            linear_project = %project.plan.id.as_str(),
            snapshot = adopted.plan.pm_snapshot_synced_at,
            "adopted refreshed Project planning at a phase boundary"
        );
    }
    *project = adopted;
    Ok(planning)
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
    project: &mut Project,
    lease: &RunContext,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    capture: Option<&crate::trace::CaptureHandle>,
    prepared: PreparedProjectStep,
) -> Result<()> {
    let wave = owning_wave(store, project).await?;
    open_project_flow_body(flow, wave.repo())?;
    if let Some(capture) = capture {
        capture.begin_turn_at("queued", &prepared.turn.input, Some(prepared.basis.clone()))?;
    }
    apply_input(
        store,
        project,
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
    project: &Project,
    lease: &RunContext,
    line: String,
) -> Result<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    if line == "/status" {
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await?;
        println!(
            "{}  {:?}",
            project.plan.slug,
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
    store.validate_run_context(lease).await?;
    let target = ChildRef::Project(project.id.clone());
    if line == "/interrupt" {
        let work = store.work_for_child(&target).await?;
        let run = store
            .current_run(&work)
            .await?
            .ok_or_else(|| anyhow!("Project Work {} has no active Run", work.id()))?;
        let receipt = store.interrupt(None, &work, &run.id).await?;
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
    _store: &SharedStore,
    _project: &Project,
    _lease: &RunContext,
    pending: &mut VecDeque<PendingInput>,
) -> Result<Option<PendingInput>> {
    take_child_input(pending).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProjectDisposition {
    Continue,
    Wait,
    Done,
}

struct ProjectOutcome {
    disposition: ProjectDisposition,
    fingerprint: String,
}

async fn inspect_outcome(
    store: &SharedStore,
    project: &Project,
    planning: &crate::ops::task_pm::ResolvedProject,
) -> Result<ProjectOutcome> {
    let tasks = store
        .list_tasks(Some(&project.wave_id))
        .await?
        .into_iter()
        .filter(|task| task.project_id == project.id)
        .collect::<Vec<_>>();
    let pm_tasks = planning
        .snapshot
        .items
        .iter()
        .filter(|item| item.project_id == project.plan.id.as_str())
        .collect::<Vec<_>>();
    let mut task_states = Vec::with_capacity(tasks.len());
    for task in &tasks {
        let work = store
            .work_for_child(&ChildRef::Task(task.id.clone()))
            .await?;
        task_states.push((
            task.id.clone(),
            store.work_status(&work).await?,
            task.updated_at,
        ));
    }
    let fingerprint_payload = serde_json::json!({
        "project": planning.project,
        "pm_tasks": pm_tasks,
        "tasks": &task_states,
    });
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&fingerprint_payload)?));
    if !planning.project.krs.is_empty() && planning.project.krs.iter().all(|kr| kr.holds) {
        return Ok(ProjectOutcome {
            disposition: ProjectDisposition::Done,
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
        || task_states
            .iter()
            .any(|(_, status, _)| matches!(status, WorkStatus::Running { .. }))
    {
        return Ok(ProjectOutcome {
            disposition: ProjectDisposition::Wait,
            fingerprint,
        });
    }
    if project.last_state_fingerprint.as_deref() == Some(&fingerprint) {
        return Ok(ProjectOutcome {
            disposition: ProjectDisposition::Wait,
            fingerprint,
        });
    }
    Ok(ProjectOutcome {
        disposition: ProjectDisposition::Continue,
        fingerprint,
    })
}

fn verify_control_plane_checkout(repo: &Path) -> Result<()> {
    crate::ops::project::ensure_clean_main(repo, "Project turn")
        .map(|_| ())
        .map_err(|error| anyhow!("Project violated its read-only control-plane boundary: {error}"))
}

async fn consume_task_observations(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
) -> Result<Vec<String>> {
    // The successor consumes the whole project chain: observations addressed to
    // a terminal predecessor the Task was born under are routed here, not
    // stranded on the dead project. The outbox recipient stays the historical
    // owner; this read is the live routing key.
    let observations = store.pending_project_observations(&project.id).await?;
    let mut prompts = Vec::new();
    for observation in observations {
        let event = match &observation.payload {
            ChildEventPayload::Task { event } => event,
            _ => continue,
        };
        let inserted = store
            .consume_task_observation_for_project_for_run(&project.id, &observation, lease)
            .await?;
        if inserted {
            prompts.push(serde_json::to_string(event)?);
        }
        project.observation_cursor = project.observation_cursor.max(observation.id);
    }
    store.update_project_for_run(project, lease).await?;
    Ok(prompts)
}

async fn apply_input(
    store: &SharedStore,
    _project: &Project,
    _lease: &RunContext,
    harness: &mut dyn Harness,
    input: PendingInput,
) -> Result<()> {
    let run_context = crate::ops::required_run_context(store).await?;
    apply_child_input(store, &run_context, harness, input).await
}

fn finish_capture(capture: Option<&crate::trace::CaptureHandle>, outcome: &str) {
    let Some(capture) = capture else { return };
    if let Err(error) = capture.finish(outcome, false) {
        tracing::warn!(%error, "failed to finish Project trace capture");
    }
}

async fn finish_failed(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    harness: &mut dyn Harness,
    error: &str,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store.fail_project_run(project, lease, error).await?;
    anyhow::bail!(error.to_string())
}

/// Recover a retryable body failure through the Run's next exact route after
/// PRD-38 permits replacement and the current containment stops positively.
#[allow(clippy::too_many_arguments)] // Invocation identity is evidence, not a recovery knob.
async fn handle_body_failure(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    invocation_id: &crate::durable::AgentInvocationId,
    invocation_route: &crate::durable::InvocationRoute,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
) -> Result<Option<(RunContext, ExactRoute)>> {
    let wave_config = read_wave_config(Path::new(wave.repo()), wave.name());
    let backup_agent = wave_config.as_ref().and_then(|c| c.backup_agent.as_deref());
    let decision = classify_disconnect_recovery(
        reason,
        &project.agent,
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
                tracing::error!(project = %project.id, containment = ?stop.containment, %error, "Project recovery left the Run fenced");
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
            .append_project_event_for_run(
                &project.id,
                lease,
                &ProjectEventKind::Failed {
                    error: failure,
                    resumable: true,
                },
            )
            .await?;
        store.update_project_for_run(project, lease).await?;
        return match settle_route_recovery(store, lease, stopped, choice).await? {
            RecoverySettlement::RecoveryRun {
                lease: recovery_lease,
                route,
            } => {
                let agent = route.agent.agent();
                let provider = route.agent.provider.clone();
                let handoff = ChildBodyHandoff {
                    from_agent: project.agent.clone(),
                    to_agent: agent.clone(),
                    from_provider: project.provider.clone(),
                    to_provider: provider.clone(),
                    reason: format!("route recovery after {reason}"),
                };
                if current_route.agent.provider != route.agent.provider
                    || current_route.account_id != route.account_id
                {
                    project.provider_session_id = None;
                }
                project.agent = agent;
                project.provider = provider;
                store
                    .update_project_for_run(project, &recovery_lease)
                    .await?;
                store
                    .append_project_event_for_run(
                        &project.id,
                        &recovery_lease,
                        &ProjectEventKind::BodyHandedOff { handoff },
                    )
                    .await?;
                Ok(Some((recovery_lease, route)))
            }
            RecoverySettlement::AwaitCapability { wait } => {
                tracing::info!(project = %project.id, wait = %wait.id, "Project waiting for a provider route capability");
                Ok(None)
            }
        };
    }

    match decision {
        RecoveryDecision::Stop => {
            let non_convergence = format!(
                "{reason}; not replay-safe (durable side effects this turn) and no backup agent configured"
            );
            finish_failed(store, project, lease, harness, &non_convergence, None)
                .await
                .map(|_| None)
        }
        RecoveryDecision::Normal | RecoveryDecision::AllowRetry => {
            finish_failed(store, project, lease, harness, reason, None)
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
    project: &mut Project,
    lease: &RunContext,
    invocation_id: &crate::durable::AgentInvocationId,
    invocation_route: &crate::durable::InvocationRoute,
    harness: &mut dyn Harness,
    wave: &Wave,
    reason: &str,
    turn_had_durable_side_effect: bool,
) -> Result<()> {
    let Some((recovery_lease, route)) = handle_body_failure(
        store,
        project,
        lease,
        invocation_id,
        invocation_route,
        harness,
        wave,
        reason,
        turn_had_durable_side_effect,
    )
    .await?
    else {
        return Ok(());
    };
    spawn_failover(store, project, &recovery_lease, wave, &route).await
}

async fn finish_abandoned(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    harness: &mut dyn Harness,
    _reason: String,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "interrupted");
    let _ = harness.interrupt().await;
    let _ = harness.stop().await;
    store
        .finish_project_run(project, lease, crate::durable::BoundaryState::Interrupted)
        .await?;
    Ok(())
}

async fn finish_command_stop(
    store: &SharedStore,
    project: &mut Project,
    lease: &RunContext,
    harness: &mut dyn Harness,
    stop: CommandStop,
    capture: Option<&crate::trace::CaptureHandle>,
) -> Result<()> {
    match stop {
        CommandStop::Interrupted => {
            finish_capture(capture, "interrupted");
            let _ = harness.stop().await;
            store
                .finish_project_run(project, lease, crate::durable::BoundaryState::Interrupted)
                .await?;
            Ok(())
        }
        CommandStop::Quiesced => {
            finish_capture(capture, "completed");
            let _ = harness.stop().await;
            store
                .finish_project_run(project, lease, crate::durable::BoundaryState::Interrupted)
                .await?;
            Ok(())
        }
        CommandStop::Abandoned(reason) => {
            finish_abandoned(store, project, lease, harness, reason, capture).await
        }
    }
}

async fn record_unhandled_failure(
    store: &SharedStore,
    project_id: &ProjectId,
    lease: &RunContext,
    error: &anyhow::Error,
) {
    let Ok(Some(project)) = store.get_project(project_id).await else {
        return;
    };
    let Ok(work) = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
        .await
    else {
        return;
    };
    if work != lease.work {
        return;
    }
    let message = format!("project runner failed: {error}");
    let _ = store.fail_project_run(&project, lease, &message).await;
}

fn project_seed(
    project: &Project,
    wave_name: &str,
    boundary: &BoundarySeed,
    observations: &[String],
    metric_context: &str,
) -> String {
    let observations = if observations.is_empty() {
        "none".to_string()
    } else {
        observations.join("\n")
    };
    let direction = boundary.render();
    format!(
        "Advance Linear Project {name} ({project_id}) in wave/{wave}.\n\n{context}\n\n{metric_context}\n\nOnly metrics owned by this Project appear above. Cross-owned evidence appears only when the Wave routes it through durable direction. Metrics inform KR judgment; they never check a KR automatically.\n\n{direction}\n\nProject Work: {work_id}\nIteration: {iteration}\nPM snapshot synced at: {synced_at}\nSupervised Task observations:\n{observations}\n\nThe runner plays clarify, pursue, and mutate through this same provider session before it checks authoritative Project and Task state. Read and update only this Linear Project through `lf pm`. Create or select concrete Linear tasks, run file-writing work with `lf task run <issue-id>`, and supervise those Tasks. Do not edit repository files from the Wave home. Return concise phase evidence; the runner decides complete, wait, repeat, or block after the whole flow.",
        name = project.plan.name,
        project_id = project.plan.id.as_str(),
        wave = wave_name,
        context = project.plan.prompt_context,
        metric_context = metric_context,
        work_id = project.id,
        iteration = project.iteration + 1,
        synced_at = project.plan.pm_snapshot_synced_at,
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
    use std::path::PathBuf;

    use anyhow::anyhow;
    use time::OffsetDateTime;

    use crate::child::ChildRef;
    use crate::durable::{Author, BoundaryState, RunAdvance, RunContext, WorkStatus};
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::pm::{PmKr, PmProject, ProjectFlowPlan};
    use crate::project::{Project, ProjectEventKind, ProjectId};
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::wave::Wave;

    async fn project_fixture() -> (
        tempfile::TempDir,
        PathBuf,
        SharedStore,
        Project,
        RunContext,
        crate::durable::AgentInvocation,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(database.clone()))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            "incident-management".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let now = OffsetDateTime::now_utc();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("incident-management-project").unwrap(),
                slug: "incident-management".to_string(),
                name: "Incident Management".to_string(),
                prompt_context: "Restore incidents before prevention.".to_string(),
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
        store.create_project(&project).await.unwrap();
        let reservation = store
            .reserve_project_process(&project, WorkStatus::Ready)
            .await
            .unwrap()
            .unwrap();
        let lease = store.run_context(&reservation.run_id).await.unwrap();
        let invocation = store
            .open_invocation_for_run(&lease.run_id)
            .await
            .unwrap()
            .unwrap();
        (directory, database, store, project, lease, invocation)
    }

    #[test]
    fn project_summary_is_bounded() {
        assert_eq!(
            super::bounded_summary(&"x".repeat(2_500)).chars().count(),
            2_000
        );
    }

    #[tokio::test]
    async fn project_plan_refresh_reaches_the_next_boundary_once_with_all_krs() {
        let (_directory, _database, store, mut project, lease, _invocation) =
            project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        store
            .append_steer(
                &work,
                Author::User,
                "Preserve this direction across planning refresh.",
                None,
            )
            .await
            .unwrap();
        project.observation_cursor = 17;
        store
            .update_project_for_run(&project, &lease)
            .await
            .unwrap();
        let prior_event = store
            .append_project_event_for_run(
                &project.id,
                &lease,
                &ProjectEventKind::IterationCompleted {
                    iteration: 0,
                    summary: "prior evidence remains durable".to_string(),
                },
            )
            .await
            .unwrap();
        let prior_basis = store.boundary_seed(&work).await.unwrap();
        let prior_epoch = store.current_epoch(&work).await.unwrap();
        let prior_id = project.id.clone();
        let refreshed = PmProject {
            id: project.plan.id.as_str().to_string(),
            slug: project.plan.slug.clone(),
            name: "Incident Prevention".to_string(),
            summary: "Prevent recurrence after restoring service.".to_string(),
            definition: "Prevent repeated incidents with evidence from production.".to_string(),
            flows: Some(ProjectFlowPlan::empty()),
            krs: (1..=6)
                .map(|number| PmKr {
                    text: format!("proof {number} holds"),
                    holds: number == 6,
                })
                .collect(),
            initiative_ids: vec!["initiative-1".to_string()],
            team_ids: vec!["team-1".to_string()],
        };
        let refreshed_plan =
            crate::ops::project::project_plan(&refreshed, project.plan.pm_snapshot_synced_at + 1)
                .unwrap();

        let (adopted, changed) = store
            .adopt_project_plan_for_run(&project.id, &refreshed_plan, &lease)
            .await
            .unwrap();
        let (same_plan, changed_again) = store
            .adopt_project_plan_for_run(&project.id, &refreshed_plan, &lease)
            .await
            .unwrap();

        assert!(changed);
        assert!(!changed_again);
        assert_eq!(same_plan.plan, adopted.plan);
        assert_eq!(adopted.id, prior_id);
        assert_eq!(adopted.observation_cursor, 17);
        assert_eq!(store.current_epoch(&work).await.unwrap(), prior_epoch);
        assert_eq!(store.boundary_seed(&work).await.unwrap(), prior_basis);
        let events = store.project_events_after(&adopted.id, 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], prior_event);

        let seed = super::project_seed(
            &adopted,
            "incident-management",
            &prior_basis,
            &[],
            "<lf:project-owned-metrics>\n{\"metrics\":[],\"contract_issues\":[]}\n</lf:project-owned-metrics>",
        );
        assert!(seed.contains("Prevent repeated incidents with evidence from production."));
        assert!(!seed.contains("Restore incidents before prevention."));
        assert!(seed.contains("Preserve this direction across planning refresh."));
        assert!(seed.contains("<lf:project-owned-metrics>"));
        for number in 1..=6 {
            let line = format!(
                "- [{}] proof {number} holds",
                if number == 6 { "x" } else { " " }
            );
            assert_eq!(seed.matches(&line).count(), 1, "missing or repeated {line}");
        }
    }

    #[tokio::test]
    async fn successful_project_flow_boundary_settles_once_without_turn_capture() {
        let (_directory, database, store, mut project, lease, invocation) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        let basis = store.boundary_seed(&work).await.unwrap().basis;
        project.iteration = 1;
        store
            .append_project_event_for_run(
                &project.id,
                &lease,
                &ProjectEventKind::IterationCompleted {
                    iteration: project.iteration,
                    summary: "restored the reported surface".to_string(),
                },
            )
            .await
            .unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::InvocationEnded {
                    invocation_id: invocation.id,
                    outcome: BoundaryState::Succeeded,
                },
            )
            .await
            .unwrap();

        super::finish_project_outcome(
            &store,
            &project,
            &lease,
            &basis,
            super::ProjectDisposition::Done,
            "restored the reported surface".to_string(),
        )
        .await
        .unwrap();
        super::finish_project_outcome(
            &store,
            &project,
            &lease,
            &basis,
            super::ProjectDisposition::Done,
            "restored the reported surface".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
        let events = store.project_events_after(&project.id, 0).await.unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].kind,
            ProjectEventKind::IterationCompleted { iteration: 1, .. }
        ));
        assert!(matches!(events[1].kind, ProjectEventKind::Completed { .. }));
        assert!(!events
            .iter()
            .any(|event| matches!(event.kind, ProjectEventKind::Failed { .. })));

        let connection = rusqlite::Connection::open(database).unwrap();
        let run_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE epoch_id=?1",
                [basis.epoch_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        let turn_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM agent_turns WHERE epoch_id=?1",
                [basis.epoch_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(run_count, 1);
        assert_eq!(turn_count, 0);
    }

    #[tokio::test]
    async fn project_failure_before_success_remains_resumable_and_exact() {
        let (_directory, database, store, project, lease, invocation) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();

        super::record_unhandled_failure(
            &store,
            &project.id,
            &lease,
            &anyhow!(
                "Project plan refresh blocked before the next phase: Linear Project was archived; restore it before restarting Project Work"
            ),
        )
        .await;

        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        let events = store.project_events_after(&project.id, 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].kind,
            ProjectEventKind::Failed { error, resumable: true }
                if error == "project runner failed: Project plan refresh blocked before the next phase: Linear Project was archived; restore it before restarting Project Work"
        ));
        assert_eq!(
            crate::child::work_status_reason(&WorkStatus::Ready),
            "ready"
        );

        let connection = rusqlite::Connection::open(database).unwrap();
        let run_state: String = connection
            .query_row(
                "SELECT state FROM runs WHERE id=?1",
                [lease.run_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        let (invocation_ended, invocation_outcome): (bool, String) = connection
            .query_row(
                "SELECT ended_at IS NOT NULL, outcome FROM agent_invocations WHERE id=?1",
                [invocation.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(run_state, "ended");
        assert!(invocation_ended);
        assert_eq!(invocation_outcome, "failed");
    }

    #[tokio::test]
    async fn project_failure_cannot_bypass_its_atomic_receipt() {
        let (_directory, _database, store, project, lease, _invocation) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();

        let error = store
            .finish_project_run(&project, &lease, BoundaryState::Failed)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("must use fail_project_run"));
        assert!(matches!(
            store.work_status(&work).await.unwrap(),
            WorkStatus::Running { .. }
        ));
        assert!(store
            .project_events_after(&project.id, 0)
            .await
            .unwrap()
            .is_empty());
        assert!(store
            .open_invocation_for_run(&lease.run_id)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn project_failure_receipt_rolls_back_together() {
        let (_directory, database, store, project, lease, invocation) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(
                "CREATE TRIGGER inject_project_failure_receipt_error
                 BEFORE UPDATE OF state ON runs
                 WHEN NEW.state='ended'
                 BEGIN
                    SELECT RAISE(ABORT, 'injected Project failure receipt error');
                 END;",
            )
            .unwrap();
        drop(connection);

        let error = store
            .fail_project_run(&project, &lease, "provider event stream closed")
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("injected Project failure receipt error"));
        assert!(matches!(
            store.work_status(&work).await.unwrap(),
            WorkStatus::Running { .. }
        ));
        assert!(store
            .project_events_after(&project.id, 0)
            .await
            .unwrap()
            .is_empty());
        let connection = rusqlite::Connection::open(database).unwrap();
        let run_state: String = connection
            .query_row(
                "SELECT state FROM runs WHERE id=?1",
                [lease.run_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        let (invocation_ended, invocation_outcome): (bool, String) = connection
            .query_row(
                "SELECT ended_at IS NOT NULL, outcome FROM agent_invocations WHERE id=?1",
                [invocation.id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let observation_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM observation_outbox", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(run_state, "active");
        assert!(!invocation_ended);
        assert_eq!(invocation_outcome, "running");
        assert_eq!(observation_count, 0);
    }
}
