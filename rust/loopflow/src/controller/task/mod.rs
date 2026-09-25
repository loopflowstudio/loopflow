use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::child::ChildRef;
use crate::controller::wave::playhead::{QueuedInvocation, StepKind};
use crate::durable::{FlowPosition, Steer, TaskWorkerClaim, WorkRef};
use crate::harness::{drain_turn_failure_reason, ApprovalPolicy, Harness};
use crate::planning::ProjectPlan;
use crate::store::SharedStore;
use crate::work::project::Project;
use crate::work::task::{Task, TaskId};
use crate::work::wave::Wave;

/// How often a live Task run checks its comment stream for new steers to inject
/// into the in-flight turn. A skill can run for hours; this is the latency floor
/// for a steer reaching a working provider (the boundary seed is the fallback).
const STEER_POLL_INTERVAL: Duration = Duration::from_secs(5);

struct CommentRefresh(tokio::task::JoinHandle<()>);

impl Drop for CommentRefresh {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Debug)]
struct PreparedTaskStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    seeded_steer_id: i64,
    interrupt_id: i64,
}

async fn load_task(store: &SharedStore, task_id: &TaskId) -> Result<Task> {
    store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} not found"))
}

pub(crate) async fn run(store: SharedStore, task_id: TaskId) -> Result<()> {
    let launch_claim =
        take_task_launch_env(crate::durable::TASK_WORKER_CLAIM_ENV, "Task worker claim")?
            .map(|value| serde_json::from_str::<TaskWorkerClaim>(&value))
            .transpose()
            .map_err(|error| anyhow!("invalid Task worker claim: {error}"))?
            .ok_or_else(|| anyhow!("Task boundary launch is missing its worker claim"))?;
    drive_task(
        store,
        task_id,
        launch_claim,
        Box::new(crate::harness::default_create_harness),
    )
    .await
}

async fn drive_task(
    store: SharedStore,
    task_id: TaskId,
    mut launch_claim: TaskWorkerClaim,
    create_harness: crate::harness::CreateHarness,
) -> Result<()> {
    let (attachment_tx, mut attachment_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if attachment_tx.send(line).is_err() {
                break;
            }
        }
    });
    let mut account =
        take_task_launch_env(crate::ops::TASK_ACCOUNT_ID_ENV, "Task provider account id")?;
    loop {
        let before = store
            .flow_position(&task_id)
            .await?
            .ok_or_else(|| anyhow!("Task has no active Flow"))?;
        if before.claim.as_ref() != Some(&launch_claim) {
            anyhow::bail!("Task driver launch claim is stale");
        }
        // A Flow may start with an operation or a recovered verdict, so the
        // launch can legitimately have no provider route yet.
        if account.is_none()
            && before.current().kind != StepKind::Op
            && !before.has_pending_decision()
        {
            let task = load_task(&store, &task_id).await?;
            let config = crate::engine::config::load_config_or_default(Some(&task.worktree));
            match crate::ops::task::preflight_task_execution(&task.worktree, config.agent()).await {
                Ok(route) => account = Some(route.to_string()),
                Err(error) => {
                    let error = anyhow!(error.to_string());
                    record_claimed_failure(&store, &task_id, &launch_claim, &error).await;
                    return Err(error);
                }
            }
        }
        if let Some(account) = &account {
            std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, account);
        }
        let result = run_task_with(
            store.clone(),
            task_id.clone(),
            &create_harness,
            launch_claim.clone(),
            &mut attachment_rx,
        )
        .await;
        if let Err(error) = &result {
            if let Some(claim) = store
                .flow_position(&task_id)
                .await?
                .and_then(|p| p.claim)
                .filter(|claim| {
                    claim.invocation_id == launch_claim.invocation_id
                        && claim.position_version == launch_claim.position_version
                        && claim.generation == launch_claim.generation
                })
            {
                record_claimed_failure(&store, &task_id, &claim, error).await;
            }
            return result;
        }
        let Some(next) = store.flow_position(&task_id).await? else {
            return Ok(());
        };
        if next.invocation.id != launch_claim.invocation_id
            || next.is_human()
            || next.failure.is_some()
            || (next.cursor == before.cursor)
        {
            return Ok(());
        }
        let owner = crate::journal::current_process_identity()
            .ok_or_else(|| anyhow!("Task driver has no process identity"))?;
        match store
            .claim_task_worker(
                &task_id,
                &next.invocation.id,
                next.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await?
        {
            crate::durable::TaskWorkerClaimOutcome::Claimed(claim) => launch_claim = claim,
            _ => return Ok(()),
        }
    }
}

pub async fn run_worker(task_id: TaskId) -> Result<()> {
    let store = std::sync::Arc::new(
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    run(store, task_id).await
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

fn task_run_spec(
    task: &Task,
    wave: &Wave,
    project: &Project,
    harness: String,
    model: Option<String>,
    surface: &str,
    skill: Option<String>,
) -> crate::run_record::RunSpec {
    crate::run_record::RunSpec {
        harness,
        model,
        surface: surface.to_string(),
        cwd: task.worktree.clone(),
        repo: Some(Path::new(wave.repo()).to_path_buf()),
        worktree: Some(task.worktree.clone()),
        skill,
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
    }
}

async fn run_task_with(
    store: SharedStore,
    task_id: TaskId,
    create_harness: &crate::harness::CreateHarness,
    launch_claim: TaskWorkerClaim,
    attachment_rx: &mut mpsc::UnboundedReceiver<String>,
) -> Result<()> {
    let mut task = load_task(&store, &task_id).await?;
    let wave = owning_wave(&store, &task).await?;
    let project = owning_project(&store, &task).await?;
    let mut flow = store
        .flow_position(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no Flow position", task.id))?;
    if flow.claim.as_ref() != Some(&launch_claim) {
        anyhow::bail!("Task boundary launch claim is stale");
    }
    if flow.has_pending_decision() {
        return recover_task_decision(&store, &mut task, &flow).await;
    }
    if flow.current().kind == StepKind::Op {
        return run_task_op_boundary(store, task, wave, project, flow, launch_claim).await;
    }
    crate::ops::linear_observe::refresh_task_comments(&store, &task).await?;
    let mut prepared = prepare_task_flow_step(&store, &task, wave.name(), &flow).await?;
    let (harness_name, _) = crate::engine::config::parse_agent(
        prepared
            .turn
            .config
            .agent
            .as_deref()
            .expect("Task launch prepared its agent"),
    );
    let capture = crate::run_record::CaptureHandle::begin_with_context(
        task_run_spec(
            &task,
            &wave,
            &project,
            prepared.turn.harness.clone(),
            prepared.turn.model.clone(),
            "headless",
            Some(flow.current().step),
        ),
        &prepared.turn.context,
    )?;
    capture.record_input("initial", &prepared.turn.input);
    capture.record_input("steer_seed_through", &prepared.seeded_steer_id.to_string());
    prepared.turn.config.env.extend(capture.environment());
    capture.mark_spawn_requested();
    let capture = Some(capture);
    let owner = crate::journal::current_process_identity()
        .ok_or_else(|| anyhow!("Task boundary requires a registered Loopflow process identity"))?;
    let bound_claim = store
        .bind_task_worker_run(
            &task.id,
            &launch_claim,
            &capture.as_ref().expect("capture was created").run_id(),
            &owner,
        )
        .await
        .inspect_err(|_| finish_capture(capture.as_ref(), "failed"))?;
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
    harness.set_provider_session_id(None);
    if let Err(error) = harness.start(&prepared.turn.config).await {
        finish_capture(capture.as_ref(), "failed");
        fail_claimed_boundary(&store, &task, &bound_claim, &error.to_string(), true).await?;
        return Err(error);
    }
    if let Some(capture) = &capture {
        capture.set_provider_session_id(harness.provider_session_id());
    }
    let mut steer_cursor = prepared.seeded_steer_id;
    let mut interrupt_cursor = prepared.interrupt_id;
    harness.send_input(&prepared.turn.input).await?;
    drop(prepared);

    println!(
        "task {}> attached; /status, /interrupt, /detach, or type a message/instruction",
        task.plan.identifier
    );
    let comment_store = store.clone();
    let comment_task = task.clone();
    let comment_refresh = tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(15));
        loop {
            tick.tick().await;
            if let Err(error) =
                crate::ops::linear_observe::refresh_task_comments(&comment_store, &comment_task)
                    .await
            {
                tracing::warn!(%error, "Linear comment refresh failed; retaining last confirmed Task direction");
            }
        }
    });
    let _comment_refresh = CommentRefresh(comment_refresh);
    let mut last_text = String::new();
    let mut command_failures = Vec::new();
    // Steers land as durable comments on this Work; a live turn injects any that
    // arrive after its seed was folded. The initial cursor comes from that exact
    // snapshot, so a comment landing between preparation and TurnStarted cannot
    // be mistaken for seeded direction.
    let work = WorkRef::Task(task.id.clone());
    let mut steer_tick = tokio::time::interval(STEER_POLL_INTERVAL);
    steer_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = steer_tick.tick() => {
                let delivered = crate::ops::child::inject_live_steers(
                    &store, &task.id, harness.as_mut(), &mut steer_cursor,
                ).await;
                if let Some(capture) = &capture {
                    for steer in delivered {
                        capture.record_input(&format!("steer_transport_accepted:{}", steer.id), &steer.text);
                    }
                }
                crate::ops::child::observe_interrupt(
                    &store, &work, harness.as_mut(), &mut interrupt_cursor,
                ).await;
            }
            line = attachment_rx.recv(), if !attachment_rx.is_closed() => {
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
                    return finish_claimed_failure(
                        &store,
                        &task,
                        &bound_claim,
                        harness.as_mut(),
                        "provider event stream closed",
                        true,
                        capture.as_ref(),
                    ).await;
                };
                if let Some(capture) = &capture {
                    capture.record_conversation(event.clone());
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
                            let (reason, retryable) = match provider_credential_blocker(&reason) {
                                Some(blocker) => (blocker, false),
                                None => (reason, true),
                            };
                            return finish_claimed_failure(
                                &store,
                                &task,
                                &bound_claim,
                                harness.as_mut(),
                                &reason,
                                retryable,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        if let Some(reason) =
                            execution_blocker_at_handoff(status, &command_failures)
                        {
                            return finish_claimed_failure(
                                &store,
                                &task,
                                &bound_claim,
                                harness.as_mut(),
                                &reason,
                                false,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        flow = store.flow_position(&task.id).await?
                            .ok_or_else(|| anyhow!("Task Flow disappeared during its worker"))?;
                        if flow.claim.as_ref() != Some(&bound_claim) {
                            anyhow::bail!("Task worker was replaced before settlement");
                        }
                        let flow_completed = match finish_task_flow_turn(&mut flow, status) {
                            Ok(completed) => completed,
                            Err(error) => {
                                return finish_claimed_failure(
                                    &store, &task, &bound_claim, harness.as_mut(),
                                    &error.to_string(), false, capture.as_ref(),
                                ).await;
                            }
                        };
                        let latest = load_task(&store, &task.id).await?;
                        task.pm_writeback = latest.pm_writeback;
                        let _ = harness.stop().await;
                        finish_capture(capture.as_ref(), "completed");
                        return finish_claimed_task_boundary(
                            &store,
                            &mut task,
                            &mut flow,
                            &bound_claim,
                            status,
                            flow_completed,
                            &last_text,
                        )
                        .await;
                    }
                    ConversationEvent::Error { code, message, .. } => {
                        let reason = format!("{code}: {message}");
                        let (reason, retryable) = match provider_credential_blocker(&reason) {
                            Some(blocker) => (blocker, false),
                            None => (reason, true),
                        };
                        return finish_claimed_failure(
                            &store,
                            &task,
                            &bound_claim,
                            harness.as_mut(),
                            &reason,
                            retryable,
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

async fn run_task_op_boundary(
    store: SharedStore,
    mut task: Task,
    wave: Wave,
    project: Project,
    mut flow: FlowPosition,
    launch_claim: TaskWorkerClaim,
) -> Result<()> {
    let step = flow.current();
    let context = crate::trace::PreparedTurnContext::from_prompts(
        "Loopflow mechanical Task boundary",
        &step.step,
    );
    let capture = crate::run_record::CaptureHandle::begin_with_context(
        task_run_spec(
            &task,
            &wave,
            &project,
            "loopflow".to_string(),
            None,
            "operation",
            None,
        ),
        &context,
    )?;
    capture.record_input("operation", &step.step);
    let owner = crate::journal::current_process_identity()
        .ok_or_else(|| anyhow!("Task operation requires a registered Loopflow process identity"))?;
    let bound_claim = store
        .bind_task_worker_run(&task.id, &launch_claim, &capture.run_id(), &owner)
        .await?;
    let outcome = run_task_flow_op(&task, &mut flow).await;
    let flow_completed = match outcome {
        Ok(completed) => completed,
        Err(error) => {
            finish_capture(Some(&capture), "failed");
            fail_claimed_boundary(
                &store,
                &task,
                &bound_claim,
                &error.to_string(),
                step.policy.repeat.is_none(),
            )
            .await?;
            return Err(error);
        }
    };
    let result = finish_claimed_task_boundary(
        &store,
        &mut task,
        &mut flow,
        &bound_claim,
        Lifecycle::Completed,
        flow_completed,
        "",
    )
    .await;
    finish_capture(
        Some(&capture),
        if result.is_ok() {
            "completed"
        } else {
            "failed"
        },
    );
    result
}

/// Settle a saved candidate before reclaim replaces its original Run binding.
pub(crate) async fn recover_task_decision(
    store: &SharedStore,
    task: &mut Task,
    position: &FlowPosition,
) -> Result<()> {
    let claim = position
        .claim
        .as_ref()
        .ok_or_else(|| anyhow!("saved Task decision has no originating claim"))?;
    let run_id = claim
        .worker_run_id
        .as_ref()
        .ok_or_else(|| anyhow!("saved Task decision has no originating Run"))?;
    let (directory, _) = crate::run_record::resolve_manifest(
        &crate::store::observability_home_dir(),
        run_id.as_str(),
    )?;
    let snapshot = crate::run_record::read_run_snapshot(&directory)?;
    match snapshot.status() {
        "completed" => {}
        "failed" | "interrupted" => {
            let reason = format!("saved Task decision Run {run_id} {}", snapshot.status());
            store
                .block_task_flow(
                    &task.id,
                    claim,
                    &crate::durable::TaskFlowBlocker {
                        reason: reason.clone(),
                        restart_required: false,
                        observed_at: time::OffsetDateTime::now_utc(),
                    },
                )
                .await?;
            anyhow::bail!(reason);
        }
        _ => anyhow::bail!(
            "saved Task decision is waiting for Run {run_id}; its completion is not recorded"
        ),
    }
    let mut next = position.clone();
    let completed = finish_task_flow_turn(&mut next, Lifecycle::Completed)?;
    finish_claimed_task_boundary(
        store,
        task,
        &mut next,
        claim,
        Lifecycle::Completed,
        completed,
        "Recovered the completed decision Run",
    )
    .await
}

async fn finish_claimed_task_boundary(
    store: &SharedStore,
    task: &mut Task,
    flow: &mut FlowPosition,
    claim: &TaskWorkerClaim,
    status: Lifecycle,
    flow_completed: bool,
    text: &str,
) -> Result<()> {
    let work = WorkRef::Task(task.id.clone());
    if store.work_status(&work).await? != crate::durable::WorkStatus::Ready {
        return Ok(());
    }
    task.updated_at = time::OffsetDateTime::now_utc();

    if status == Lifecycle::Interrupted || !flow_completed {
        return settle_claimed_task_position(store, task, flow, claim, text).await;
    }

    let summary = progress_summary(text);
    store
        .finish_task_flow(
            task,
            claim,
            (!summary.is_empty()).then_some(summary.as_str()),
        )
        .await?;
    Ok(())
}

async fn settle_claimed_task_position(
    store: &SharedStore,
    task: &mut Task,
    flow: &FlowPosition,
    claim: &TaskWorkerClaim,
    text: &str,
) -> Result<()> {
    let mut next = flow.clone();
    next.claim = None;
    next.failure = None;
    next.session_run_id = None;
    next.ready_summary = None;
    next.updated_at = time::OffsetDateTime::now_utc();
    let summary = progress_summary(text);
    let next = store
        .settle_task_worker(
            task,
            claim,
            &next,
            (!summary.is_empty()).then_some(summary.as_str()),
        )
        .await?;
    if next.is_human() {
        let node_id = next
            .current()
            .policy
            .id
            .ok_or_else(|| anyhow!("Task review step has no stable node id"))?;
        checkpoint_worktree_before_human(task, &node_id).await;
        crate::ops::human_session::prepare(store, task, &next).await?;
        return Ok(());
    }
    Ok(())
}

async fn prepare_task_flow_step(
    store: &SharedStore,
    task: &Task,
    wave_name: &str,
    flow: &FlowPosition,
) -> Result<PreparedTaskStep> {
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await?;
    let steers = store.task_steers(&task.id).await?;
    let seeded_steer_id = steers.last().map_or(0, |steer| steer.id);
    let interrupt_id = store.latest_interrupt_id(&work).await?;
    let skill = crate::engine::current_skill(&flow.invocation.steps, &flow.cursor)
        .ok_or_else(|| anyhow!("Task Flow step is not a skill"))?;
    let pr = store
        .active_task_pr(&task.id)
        .await?
        .ok_or_else(|| anyhow!("Task {} has no active PR", task.id))?;
    let project = owning_project(store, task).await?;
    let mut seed = format!(
        "{}\n\n{}",
        task_seed(task, &project.plan, &pr, wave_name, &steers),
        crate::ops::task::task_workspace_context(task, &pr)
            .map_err(|error| anyhow!(error.to_string()))?
    );
    if let Some(direction) = flow.cursor.leaf().progress.direction.as_ref() {
        seed.push_str(&format!(
            "\n\nPrevious step feedback or iteration direction:\n{direction}"
        ));
    }
    if let Some(repeat) = &skill.policy.repeat {
        let edge = skill
            .policy
            .id
            .as_deref()
            .expect("a repeat occurrence has an id");
        let traversals = flow
            .cursor
            .leaf()
            .progress
            .repeats
            .get(edge)
            .copied()
            .unwrap_or(0);
        seed.push_str(&format!(
            "\n\nDecision occurrence {edge}: pass {}. The backward edge returns to {}. Compare the preceding pass's intended progress with its observed results; new evidence counts as progress. Missing prior evidence is an evidence gap, not proof of no progress.",
            u64::from(traversals) + 1, repeat.from,
        ));
        seed.push_str(
            "\n\nBefore finishing, run `lf flow decide iterate \"<remaining work, direction, and evidence>\"` or `lf flow decide advance \"<whole-design proof>\"`. Advance requires the obligations of this decision, not just the latest slice, and leaves later review steps intact. If blocked, run `lf flow blocked \"<reason, attempted direction, and evidence>\"`; this requests an Ask running unblock and waits for human completion. Reread its summary and changed artifacts, then reassess; completing the Ask supplies evidence rather than a navigation decision. Record the decision after edits and verification; missing decisions block advancement.",
        );
    }
    let mut prepared = crate::lf::commands::run::prepare_harness_turn_from_skill_at(
        &skill.skill,
        &seed,
        wave_name,
        None,
        &task.worktree,
    )?;
    let config = crate::engine::config::load_config_or_default(Some(&task.worktree));
    let agent = config.agent();
    prepared.config.agent = Some(agent.to_string());
    prepared.config.write_scope = crate::engine::agent::AgentWriteScope::Worktree;
    prepared.config.execution_boundary = Some(
        crate::ops::task::task_execution_boundary(&task.worktree, agent)
            .map_err(|error| anyhow!(error.to_string()))?,
    );
    prepared.config.skip_permissions = true;
    Ok(PreparedTaskStep {
        turn: prepared,
        seeded_steer_id,
        interrupt_id,
    })
}

pub(crate) async fn complete_human_flow_step(
    store: &SharedStore,
    token: &crate::ops::human_session::FlowSessionToken,
) -> Result<()> {
    if !crate::ops::human_session::token_is_current(store, token).await? {
        anyhow::bail!("review session is stale");
    }
    let expected = store
        .flow_position(&token.task_id)
        .await?
        .ok_or_else(|| anyhow!("review session is no longer waiting"))?;
    let text = expected
        .ready_summary
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            anyhow!("review is not ready; its agent must run `lf session ready` first")
        })?;
    let mut task = load_task(store, &token.task_id).await?;
    let mut position = expected.clone();
    let step = position.current();
    if !step.policy.human
        || step.invocation_id != token.invocation_id
        || step.policy.id.as_deref() != Some(token.node_id.as_str())
        || step.step != token.skill.name
        || step.iteration != token.iteration
        || step.flow != token.flow
    {
        anyhow::bail!("review session no longer matches the Task Flow position");
    }

    position.cursor.leaf_mut().progress.direction = Some(text.to_string());
    let flow_completed = finish_task_flow_turn(&mut position, Lifecycle::Completed)?;
    task.updated_at = time::OffsetDateTime::now_utc();

    if flow_completed {
        store
            .finish_human_task_boundary(&task, &expected, text)
            .await?;
        return Ok(());
    }
    position.session_run_id = None;
    position.ready_summary = None;
    position.updated_at = time::OffsetDateTime::now_utc();
    store
        .complete_human_task_boundary(&task, &expected, &position, text)
        .await?;
    Ok(())
}

pub(crate) async fn ensure_flow_position(
    store: &SharedStore,
    task_id: &TaskId,
    selected_flow: Option<&str>,
) -> Result<FlowPosition> {
    let task = load_task(store, task_id).await?;
    if let Some(current) = store.flow_position(&task.id).await? {
        return Ok(current);
    }
    let selected_flow = selected_flow.ok_or_else(|| {
        anyhow!(
            "Task {} has no active Flow; run `lf task run {} [--flow FLOW]` to select one",
            task.plan.identifier,
            task.plan.identifier
        )
    })?;
    let candidate = start_task_flow(&task, selected_flow)?;
    let candidate = store.set_flow_position(&task.id, candidate).await?;
    if candidate.is_human() {
        let node_id = candidate
            .current()
            .policy
            .id
            .ok_or_else(|| anyhow!("Task review step has no stable node id"))?;
        checkpoint_worktree_before_human(&task, &node_id).await;
        crate::ops::human_session::prepare(store, &task, &candidate).await?;
    }
    Ok(candidate)
}

fn finish_task_flow_turn(position: &mut FlowPosition, status: Lifecycle) -> Result<bool> {
    match status {
        Lifecycle::Interrupted => {
            position.cursor.leaf_mut().progress.verdict = None;
            position.cursor.leaf_mut().route = None;
            Ok(false)
        }
        Lifecycle::Completed => position.cursor.finish(&position.invocation.steps),
        _ => anyhow::bail!("Task flow turn ended with unexpected status {status:?}"),
    }
}

async fn run_task_flow_op(task: &Task, position: &mut FlowPosition) -> Result<bool> {
    let crate::engine::ConcreteStep::Op(op) = position.current_plan() else {
        anyhow::bail!(
            "Task flow step {} is not an operation",
            position.current().step
        );
    };
    let op = op.clone();
    let worktree = task.worktree.clone();
    tokio::task::spawn_blocking(move || {
        crate::ops::execute_flow_ops(&worktree, &op.item, &crate::ops::NullProgress)
    })
    .await
    .map_err(|error| anyhow!("Task flow op worker failed: {error}"))??;
    finish_task_flow_turn(position, Lifecycle::Completed)
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

/// A review FlowStep can outlive this machine's uptime; nothing the Task produced
/// may exist only in the local worktree while it waits. Failure to checkpoint
/// (offline, no remote) must never block the park itself.
async fn checkpoint_worktree_before_human(task: &Task, node_id: &str) {
    if let Err(error) = crate::ops::checkpoint_task_worktree(
        task.worktree.clone(),
        task.plan.identifier.clone(),
        format!("checkpoint: park at review node {node_id}"),
    )
    .await
    {
        tracing::warn!(task = %task.id, %error, "Task parks at a review node without a pushed checkpoint");
    }
}

fn start_task_flow(task: &Task, selected_flow: &str) -> Result<FlowPosition> {
    Ok(FlowPosition {
        task_id: task.id.clone(),
        invocation: QueuedInvocation::load(&task.worktree, selected_flow)?,
        session_run_id: None,
        ready_summary: None,
        cursor: crate::engine::ExecutionCursor {
            index: 0,
            iteration: 0,
            ..Default::default()
        },
        version: 0,
        worker_generation: 0,
        claim: None,
        failure: None,

        updated_at: time::OffsetDateTime::now_utc(),
    })
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
    if line == "/interrupt" {
        harness.interrupt().await?;
        println!("interrupted active provider turn");
    } else {
        let comment_id = crate::ops::linear_observe::publish_task_steer(store, task, line).await?;
        println!("posted to Linear {comment_id}");
    }
    Ok(())
}

async fn record_claimed_failure(
    store: &SharedStore,
    task_id: &TaskId,
    claim: &TaskWorkerClaim,
    error: &anyhow::Error,
) {
    let detail = error.to_string();
    let (message, resumable) = unhandled_failure_receipt(&detail);
    if resumable {
        if let Err(persist_error) = store.release_task_worker(task_id, claim).await {
            tracing::debug!(
                task = %task_id,
                error = %persist_error,
                "Task failure arrived after its worker claim changed"
            );
        }
        return;
    }
    let failure = crate::durable::TaskFlowBlocker {
        reason: message,
        restart_required: false,
        observed_at: time::OffsetDateTime::now_utc(),
    };
    if let Err(persist_error) = store.block_task_flow(task_id, claim, &failure).await {
        tracing::debug!(
            task = %task_id,
            error = %persist_error,
            "Task failure arrived after its worker claim changed"
        );
    }
}

async fn fail_claimed_boundary(
    store: &SharedStore,
    task: &Task,
    claim: &TaskWorkerClaim,
    reason: &str,
    safe_to_retry: bool,
) -> Result<()> {
    if safe_to_retry {
        store.release_task_worker(&task.id, claim).await?;
        return Ok(());
    }
    let failure = crate::durable::TaskFlowBlocker {
        reason: reason.to_string(),
        restart_required: false,
        observed_at: time::OffsetDateTime::now_utc(),
    };
    store.block_task_flow(&task.id, claim, &failure).await?;
    Ok(())
}

async fn finish_claimed_failure(
    store: &SharedStore,
    task: &Task,
    claim: &TaskWorkerClaim,
    harness: &mut dyn Harness,
    reason: &str,
    safe_to_retry: bool,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    fail_claimed_boundary(store, task, claim, reason, safe_to_retry).await?;
    anyhow::bail!(reason.to_string())
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

fn task_seed(
    task: &Task,
    project: &ProjectPlan,
    pr: &crate::work::task::TaskPr,
    wave_name: &str,
    steers: &[Steer],
) -> String {
    let context = crate::ops::render_task_context(task, project, pr, wave_name, steers);
    format!(
        "{context}\n\nYou are the current Task worker. Run the selected immutable Flow from its persisted boundary. This Flow does not choose what a later worker will run. Typed PR and Task operations own publication, landing, rotation, and completion. `lf pr abandon` discards only this PR. If this PR already merged out of band and follow-up work remains, `lf pr next [slug]` rotates to the next serial PR, carrying committed and uncommitted follow-up forward."
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
    ledger: crate::journal::TestLedgerGuard,
}

#[cfg(test)]
impl TestLfBinGuard {
    fn pin() -> Self {
        let ledger = crate::journal::TestLedgerGuard::new();
        let previous = std::env::var_os("LF_BIN");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self { previous, ledger }
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
        completed_boundary_failure, execution_blocker_at_handoff, task_seed,
        unhandled_failure_receipt,
    };
    use crate::chat::types::Lifecycle;
    use crate::controller::wave::playhead::StepKind;
    use crate::durable::{
        Author, FlowPosition, RunId, TaskWorkerClaimOutcome, TaskWorkerOwner, WorkRef,
    };
    use crate::engine::agent::AgentConfig;
    use crate::harness::{Harness, SendCurrentOutcome};
    use crate::id::{ExecId, TraceId};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        Observation, PmWritebackState, Task, TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;

    #[derive(Default)]
    struct UnusedHarness {
        stopped: bool,
    }

    #[derive(Default)]
    struct RecordingControlHarness {
        steers: Vec<String>,
        interrupts: usize,
    }

    #[tokio::test]
    async fn feature_repeats_the_whole_slice_then_stops_for_delivery() {
        let (_, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "feature").unwrap();
        assert_eq!(position.current().step, "kickoff");
        super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        assert!(position.is_human());
        assert_eq!(position.current().step, "review-design");
        position.cursor.progress.direction = Some("Design clarified with the human".into());
        super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        for pass in 0..10 {
            for expected in ["implement", "compress", "review-slice", "concept-review"] {
                assert_eq!(position.current().step, expected);
                assert!(
                    !super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap()
                );
            }
            assert_eq!(position.current().step, "loop-decide");
            position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
                decision: if pass < 9 {
                    crate::engine::transitions::FlowDecision::Iterate
                } else {
                    crate::engine::transitions::FlowDecision::Advance
                },
                summary: if pass < 9 {
                    "repair the demonstrated gap"
                } else {
                    "all design claims demonstrated"
                }
                .into(),
            });
            assert!(!super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap());
        }
        assert!(position.is_human());
        assert_eq!(position.current().step, "demo");
        assert_eq!(position.cursor.iteration, 9);
        assert!(position.cursor.progress.verdict.is_none());
        position.cursor.progress.direction = Some("Human requested a delivery correction".into());
        super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        assert_eq!(position.current().step, "loop-decide");
        assert!(position.cursor.progress.verdict.is_none());
        assert_eq!(position.cursor.iteration, 9);
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Human requested a delivery correction".into(),
        });
        assert!(!super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap());
        assert_eq!(position.current().step, "implement");
        assert_eq!(
            position.cursor.progress.direction.as_deref(),
            Some("Human requested a delivery correction")
        );
        for _ in 0..4 {
            super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        }
        assert_eq!(position.current().step, "loop-decide");
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Continue the human-requested revision".into(),
        });
        super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        assert_eq!(position.current().step, "implement");
        assert_eq!(position.cursor.progress.repeats["decide"], 10);
    }

    #[tokio::test]
    async fn loop_requires_a_decision_and_discards_it_on_interrupt() {
        let (_, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "feature").unwrap();
        position.cursor.index = 6;
        assert!(
            super::finish_task_flow_turn(&mut position, Lifecycle::Completed)
                .unwrap_err()
                .to_string()
                .contains("requires a decision")
        );
        assert_eq!(position.cursor.index, 6);
        let review = &mut position.cursor.progress;
        review.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "discard on interrupt".into(),
        });
        assert!(!super::finish_task_flow_turn(&mut position, Lifecycle::Interrupted).unwrap());
        assert_eq!(position.cursor.index, 6);
        assert!(position.cursor.progress.verdict.is_none());
    }

    struct SliceHarness {
        store: SharedStore,
        task_id: TaskId,
        events: tokio::sync::mpsc::UnboundedSender<crate::chat::types::ConversationEvent>,
        seen: std::sync::Arc<std::sync::Mutex<Vec<(String, RunId)>>>,
        restart: bool,
    }

    #[async_trait::async_trait]
    impl Harness for SliceHarness {
        async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
            let position = self.store.flow_position(&self.task_id).await?.unwrap();
            let run = position.claim.unwrap().worker_run_id.unwrap();
            assert_eq!(
                config.env.get(crate::durable::RUN_ID_ENV),
                Some(&run.to_string())
            );
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> anyhow::Result<()> {
            let position = self.store.flow_position(&self.task_id).await?.unwrap();
            if self.restart {
                let owner = position.claim.as_ref().unwrap().owner.clone();
                let mut replacement = position.clone();
                replacement.invocation.id = "replacement-invocation".into();
                replacement.version = 0;
                replacement.claim = None;
                let task = super::load_task(&self.store, &self.task_id).await?;
                self.store
                    .restart_task_flow(&task, "fixture-checkpoint")
                    .await?;
                let replacement = self
                    .store
                    .set_flow_position(&self.task_id, replacement)
                    .await?;
                self.store
                    .claim_task_worker(
                        &self.task_id,
                        &replacement.invocation.id,
                        replacement.version,
                        &owner,
                        time::OffsetDateTime::now_utc(),
                    )
                    .await?;
                anyhow::bail!("late provider failure from the replaced invocation");
            }
            let step = position.current();
            let routing = matches!(position.current_plan(), crate::engine::ConcreteStep::Xor(_));
            let run = position.claim.unwrap().worker_run_id.unwrap();
            self.seen
                .lock()
                .unwrap()
                .push((step.step.clone(), run.clone()));
            if position.cursor.iteration > 0 {
                assert!(content.contains("repair the demonstrated gap"));
            }
            if routing {
                assert!(content.contains("lf flow route"));
                self.store
                    .record_flow_route(&self.task_id, &run, "selected")
                    .await?;
            }
            if step.policy.repeat.is_some() {
                self.store
                    .record_flow_verdict(
                        &self.task_id,
                        &run,
                        &crate::engine::transitions::FlowVerdict {
                            decision: if position.cursor.iteration == 0 {
                                crate::engine::transitions::FlowDecision::Iterate
                            } else {
                                crate::engine::transitions::FlowDecision::Advance
                            },
                            summary: "repair the demonstrated gap".into(),
                        },
                    )
                    .await?;
            }
            self.events
                .send(crate::chat::types::ConversationEvent::TurnCompleted {
                    turn_id: "fixture-turn".into(),
                    status: Lifecycle::Completed,
                })?;
            Ok(())
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

    #[test]
    fn task_decision_recovery_requires_the_original_successful_run() {
        let guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            for routing in [false, true] {
                for status in ["completed", "failed", "interrupted", "running", "missing"] {
                    let (store, mut task, _) = human_task_fixture().await;
                    let mut flow = super::start_task_flow(&task, "pursue").unwrap();
                    flow.invocation.steps.truncate(5);
                    flow.cursor.index = 4;
                    if routing {
                        flow.cursor.index = 0;
                        flow.invocation.steps = vec![crate::engine::ConcreteStep::Xor(
                            crate::engine::ConcreteXor {
                                router: crate::engine::Skill::named("xor-route"),
                                paths: std::collections::HashMap::from([(
                                    "done".into(),
                                    crate::engine::ConcretePath {
                                        description: "finish".into(),
                                        steps: vec![],
                                    },
                                )]),
                                flow_parents: vec![],
                            },
                        )];
                    }
                    let flow = store.set_flow_position(&task.id, flow).await.unwrap();
                    let owner = TaskWorkerOwner {
                        trace_id: TraceId::new(),
                        exec_id: ExecId::new(),
                        pid: 303,
                        started_at: 1_700_000_000,
                    };
                    let TaskWorkerClaimOutcome::Claimed(claim) = store
                        .claim_task_worker(
                            &task.id,
                            &flow.invocation.id,
                            flow.version,
                            &owner,
                            time::OffsetDateTime::now_utc(),
                        )
                        .await
                        .unwrap()
                    else {
                        panic!("recovery claim")
                    };
                    let capture = crate::run_record::CaptureHandle::begin_at(
                        guard.ledger.home(),
                        crate::run_record::RunSpec {
                            harness: "proof".into(),
                            model: None,
                            surface: "headless".into(),
                            cwd: task.worktree.clone(),
                            repo: None,
                            worktree: None,
                            skill: Some("loop-decide".into()),
                            subjects: vec![],
                        },
                    )
                    .unwrap();
                    let run = if status == "missing" {
                        RunId::new()
                    } else {
                        capture.run_id()
                    };
                    store
                        .bind_task_worker_run(&task.id, &claim, &run, &owner)
                        .await
                        .unwrap();
                    if routing {
                        store
                            .record_flow_route(&task.id, &run, "done")
                            .await
                            .unwrap();
                    } else {
                        store
                            .record_flow_verdict(
                                &task.id,
                                &run,
                                &crate::engine::transitions::FlowVerdict {
                                    decision: crate::engine::transitions::FlowDecision::Advance,
                                    summary: "candidate, not completion".into(),
                                },
                            )
                            .await
                            .unwrap();
                    }
                    if !matches!(status, "running" | "missing") {
                        capture.finish(status).unwrap();
                    }
                    let saved = store.flow_position(&task.id).await.unwrap().unwrap();
                    let result = super::recover_task_decision(&store, &mut task, &saved).await;
                    let after = store.flow_position(&task.id).await.unwrap();
                    if status == "completed" {
                        result.unwrap();
                        assert!(after.is_none());
                        assert!(super::recover_task_decision(&store, &mut task, &saved)
                            .await
                            .is_err());
                    } else {
                        assert!(result.is_err());
                        let after = after.unwrap();
                        assert_eq!(after.cursor.index, saved.cursor.index);
                        if matches!(status, "failed" | "interrupted") {
                            assert!(!after.has_pending_decision());
                            assert!(after.failure.unwrap().reason.contains(status));
                            assert!(after.claim.is_none());
                        } else {
                            assert_eq!(after, saved);
                        }
                    }
                }
            }
        });
    }

    #[test]
    fn driver_runs_fresh_slice_turns_until_the_flow_finishes() {
        let guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, task, _) = runtime.block_on(human_task_fixture());
        crate::journal::with_runtime(&task.worktree, &["task-driver-proof".into()], || {
            runtime.block_on(async {
                use crate::pm::test_server::{self, json_response};
                use serde_json::json;
                let (url, _) = test_server::spawn((0..11).map(|_| json_response(
                    axum::http::StatusCode::OK,
                    json!({"data": {"issue": {
                        "updatedAt": "2026-09-24T00:00:00Z", "title": task.plan.title,
                        "description": task.plan.description,
                        "comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}
                    }}}),
                )).collect()).await;
                store.upsert_provider_token(&crate::store::ProviderToken {
                    provider: "linear".into(), access_token: "fixture-token".into(),
                    refresh_token: None, oauth_client_id: None, expires_at: None,
                    login: None, updated_at: 1, credential_type: crate::store::CredentialType::OAuth,
                }).await.unwrap();
                let definitions = task.worktree.join(".lf/flows");
                std::fs::create_dir_all(&definitions).unwrap();
                std::fs::write(definitions.join("nested-slice.yaml"), "- xor:\n    paths:\n      selected:\n        flow: saved-body\n        description: pursue the approved design\n").unwrap();
                std::fs::write(definitions.join("saved-body.yaml"), "- step:\n    name: implement\n    id: work\n- compress\n- review-slice\n- concept-review\n- step:\n    name: loop-decide\n    id: decide\n    repeat:\n      from: work\n").unwrap();
                let flow = super::start_task_flow(&task, "nested-slice").unwrap();
                // Delete the sources before routing: every possible path was captured.
                std::fs::remove_file(definitions.join("nested-slice.yaml")).unwrap();
                std::fs::remove_file(definitions.join("saved-body.yaml")).unwrap();
                let flow = store.set_flow_position(&task.id, flow).await.unwrap();
                let owner = crate::journal::current_process_identity().unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("driver claim") };
                let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                let create: crate::harness::CreateHarness = Box::new({
                    let store = store.clone();
                    let task_id = task.id.clone();
                    let seen = seen.clone();
                    move |_, _, events| Ok(Box::new(SliceHarness {
                        store: store.clone(), task_id: task_id.clone(), events, seen: seen.clone(), restart: false,
                    }))
                });
                // The simulated provider uses a preflighted fixture route.
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                crate::ops::pm::PM_TEST_CONTEXT.scope(crate::ops::pm::PmTestContext {
                    path: guard.ledger.home().join("fixture.db"), store: store.clone(), graphql_url: url,
                }, super::drive_task(store.clone(), task.id.clone(), claim, create)).await.unwrap();
                let turns = seen.lock().unwrap().clone();
                assert_eq!(turns.iter().map(|(step, _)| step.as_str()).collect::<Vec<_>>(),
                    ["xor-route", "implement", "compress", "review-slice", "concept-review", "loop-decide", "implement", "compress", "review-slice", "concept-review", "loop-decide"]);
                assert_eq!(turns.iter().map(|(_, run)| run).collect::<std::collections::HashSet<_>>().len(), 11);
                assert!(store.flow_position(&task.id).await.unwrap().is_none());
                let execution = crate::ops::task_execution::task_execution(&store, &task.id).await.unwrap();
                assert!(execution.reason.contains("Flow nested-slice finished"));

                // Recover a completed decision Run before replacing its claim.
                // Settlement consumes its saved result without another provider.
                let mut flow = super::start_task_flow(&task, "pursue").unwrap();
                flow.invocation.steps.truncate(5);
                flow.cursor.index = 4;
                let flow = store.set_flow_position(&task.id, flow).await.unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("recovery fixture claim") };
                let capture = crate::run_record::CaptureHandle::begin_at(
                    guard.ledger.home(), crate::run_record::RunSpec {
                        harness: "proof".into(), model: None, surface: "headless".into(),
                        cwd: task.worktree.clone(), repo: None, worktree: None,
                        skill: Some("loop-decide".into()), subjects: vec![],
                    },
                ).unwrap();
                let run = capture.run_id();
                store.bind_task_worker_run(&task.id, &claim, &run, &owner).await.unwrap();
                store.record_flow_verdict(&task.id, &run, &crate::engine::transitions::FlowVerdict {
                    decision: crate::engine::transitions::FlowDecision::Advance,
                    summary: "saved whole-design proof".into(),
                }).await.unwrap();
                capture.finish("completed").unwrap();
                let saved = store.flow_position(&task.id).await.unwrap().unwrap();
                super::recover_task_decision(&store, &mut task.clone(), &saved).await.unwrap();
                assert!(store.flow_position(&task.id).await.unwrap().is_none());

                // A late error cannot release a replacement invocation's claim,
                // even when restart reuses its version and generation.
                let flow = super::start_task_flow(&task, "pursue").unwrap();
                let flow = store.set_flow_position(&task.id, flow).await.unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("late failure fixture claim") };
                let create: crate::harness::CreateHarness = Box::new({
                    let store = store.clone();
                    let task_id = task.id.clone();
                    move |_, _, events| Ok(Box::new(SliceHarness {
                        store: store.clone(), task_id: task_id.clone(), events,
                        seen: Default::default(), restart: true,
                    }))
                });
                let (url, _) = test_server::spawn(vec![json_response(
                    axum::http::StatusCode::OK,
                    json!({"data": {"issue": {
                        "updatedAt": "2026-09-24T00:00:00Z", "title": task.plan.title,
                        "description": task.plan.description,
                        "comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}
                    }}}),
                )]).await;
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                let error = crate::ops::pm::PM_TEST_CONTEXT.scope(crate::ops::pm::PmTestContext {
                    path: guard.ledger.home().join("fixture.db"), store: store.clone(), graphql_url: url,
                }, super::drive_task(store.clone(), task.id.clone(), claim.clone(), create)).await.unwrap_err();
                assert!(error.to_string().contains("late provider failure"));
                let replacement = store.flow_position(&task.id).await.unwrap().unwrap();
                assert!(replacement.failure.is_none());
                let replacement_claim = replacement.claim.expect("replacement worker remains claimed");
                assert_eq!(replacement_claim.position_version, claim.position_version);
                assert_eq!(replacement_claim.generation, claim.generation);
                assert_ne!(replacement_claim.invocation_id, claim.invocation_id);
                Ok(())
            })
        }).unwrap();
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
            self.stopped = true;
            Ok(())
        }

        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    #[async_trait::async_trait]
    impl Harness for RecordingControlHarness {
        async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_current(&mut self, content: &str) -> SendCurrentOutcome {
            self.steers.push(content.to_string());
            SendCurrentOutcome::Sent {
                provider_turn_id: "turn-test".to_string(),
            }
        }

        async fn interrupt(&mut self) -> anyhow::Result<()> {
            self.interrupts += 1;
            Ok(())
        }

        async fn stop(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    async fn human_task_fixture() -> (SharedStore, Task, FlowPosition) {
        let repository =
            std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .unwrap();
        // The Task worktree must never be the real checkout: parking at a
        // review node checkpoint-commits the worktree, and a test must not
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
        let base_commit = String::from_utf8(
            std::process::Command::new("git")
                .current_dir(&worktree)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        let database = tempfile::tempdir().unwrap().keep();
        let database = database.join("registry.db");
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
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
                name: "Review Task proof".to_string(),
                prompt_context: "Prove durable review steps.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("human-task-issue").unwrap(),
                identifier: "TEST-1".to_string(),
                title: "Review Task proof".to_string(),
                description: "Stop at review_kickoff.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: worktree.clone(),
            workspace_slug: "human-task-proof".to_string(),
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
            base_commit,
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
        let mut flow = super::start_task_flow(&task, "task-design").unwrap();
        flow.cursor.index = 1;
        (store, task, flow)
    }

    #[tokio::test]
    async fn provider_failure_releases_the_exact_worker_claim() {
        let (store, task, _) = human_task_fixture().await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let initial = store
            .set_flow_position(&task.id, flow.clone())
            .await
            .unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 301,
            started_at: 1_700_000_000,
        };
        let claim = match store
            .claim_task_worker(
                &task.id,
                &initial.invocation.id,
                initial.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let claim = store
            .bind_task_worker_run(&task.id, &claim, &RunId::new(), &owner)
            .await
            .unwrap();
        let mut harness = UnusedHarness::default();

        let error = super::finish_claimed_failure(
            &store,
            &task,
            &claim,
            &mut harness,
            "opencode_disconnected: provider stream ended",
            true,
            None,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("provider stream ended"));
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        assert!(harness.stopped);
        let position = store.flow_position(&task.id).await.unwrap().unwrap();
        assert!(position.claim.is_none());
        assert!(position.failure.is_none());
    }

    #[test]
    fn normal_task_completion_preserves_delivery_permission_and_run_network_failures() {
        let commit = completed_boundary_failure(
            &["lf".into(), "commit".into(), "-m".into(), "ship".into(), "-p".into()],
            Lifecycle::Failed,
            Some(
                "fatal: Unable to create '/repo/.git/worktrees/task/index.lock': Operation not permitted",
            ),
            Some(128),
        )
        .unwrap();
        let run = completed_boundary_failure(
            &[
                "lf".into(),
                "--as".into(),
                "project:proj_1".into(),
                ":".into(),
                "Review this".into(),
            ],
            Lifecycle::Failed,
            Some("network access is disabled by policy"),
            Some(1),
        )
        .unwrap();
        let reason = execution_blocker_at_handoff(Lifecycle::Completed, &[commit, run])
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
    async fn prebind_task_failure_releases_its_claim_without_shared_failure_state() {
        let (store, task, mut flow) = human_task_fixture().await;
        flow.cursor.index = 0;
        let position = store.set_flow_position(&task.id, flow).await.unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 302,
            started_at: 1_700_000_000,
        };
        let claim = match store
            .claim_task_worker(
                &task.id,
                &position.invocation.id,
                position.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };

        super::record_claimed_failure(
            &store,
            &task.id,
            &claim,
            &anyhow::anyhow!("provider stream closed"),
        )
        .await;

        let events = store.recent_task_events(&task.id, 10).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, TaskEventKind::Started);
        let position = store.flow_position(&task.id).await.unwrap().unwrap();
        assert!(position.claim.is_none());
        assert!(position.failure.is_none());
    }

    #[tokio::test]
    async fn posted_direction_on_an_idle_task_does_not_start_advancement() {
        let (store, task, _) = human_task_fixture().await;
        assert!(store.flow_position(&task.id).await.unwrap().is_none());
        let id = crate::ops::linear_observe::tests::with_posted_comment(
            &store,
            &task,
            "keep the API",
            crate::ops::linear_observe::publish_task_steer(&store, &task, "keep the API"),
        )
        .await
        .unwrap();
        assert_eq!(id, "comment-1");
        assert!(store.flow_position(&task.id).await.unwrap().is_none());
        let steers = store.task_steers(&task.id).await.unwrap();
        assert_eq!(steers.len(), 1);
        assert!(steers[0].text.contains("keep the API"));
    }

    #[tokio::test]
    async fn comment_sync_recovers_history_and_deduplicates_edits_and_webhooks() {
        let (store, task, _) = human_task_fixture().await;
        let now = time::OffsetDateTime::now_utc();
        let observation = |issue_revision: &str, comment_revision: &str, body: &str| {
            crate::pm::IssueObservation {
                revision: issue_revision.into(),
                title: task.plan.title.clone(),
                description: task.plan.description.clone(),
                comments: vec![crate::pm::IssueComment {
                    author_name: None,
                    id: "c-1".into(),
                    revision: Some(comment_revision.into()),
                    body: body.into(),
                    author_id: Some("my-account".into()),
                }],
            }
        };
        let first = observation(
            "2026-09-23T00:00:00Z",
            "2026-09-22T00:00:00Z",
            "first advice",
        );
        let imported = crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first.clone(),
            "my-account",
            now,
        )
        .await
        .unwrap();
        assert_eq!(imported.follow_ups_created.len(), 1);
        assert!(crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first.clone(),
            "my-account",
            now
        )
        .await
        .unwrap()
        .follow_ups_created
        .is_empty());
        // A stale issue revision can carry a newer comment edit.
        let edited = observation(
            "2026-09-21T00:00:00Z",
            "2026-09-24T00:00:00Z",
            "revised advice",
        );
        assert_eq!(
            crate::ops::linear_observe::reconcile_linear_observation(
                &store,
                &task,
                edited,
                "my-account",
                now
            )
            .await
            .unwrap()
            .follow_ups_created
            .len(),
            1
        );
        let event = crate::webhook::WebhookEvent::Comment {
            author_name: None,
            issue_id: task.plan.id.as_str().into(),
            comment_id: "c-1".into(),
            revision: Some("2026-09-24T00:00:00Z".into()),
            body: "revised advice".into(),
            author_id: Some("my-account".into()),
        };
        assert_eq!(
            crate::webhook::ingest_event(&store, event, "my-account", now)
                .await
                .unwrap(),
            crate::webhook::WebhookOutcome::Comment { delivered: false }
        );
        assert!(crate::ops::linear_observe::reconcile_linear_observation(
            &store,
            &task,
            first,
            "my-account",
            now
        )
        .await
        .unwrap()
        .follow_ups_created
        .is_empty());
        let steers = store.task_steers(&task.id).await.unwrap();
        assert_eq!(steers.len(), 2);
        assert!(steers[1].text.contains("revised advice"));
    }

    #[tokio::test]
    async fn stale_task_child_cannot_rewrite_a_replacement_human_boundary() {
        let (store, task, human_flow) = human_task_fixture().await;
        let autonomous = super::start_task_flow(&task, "task-design").unwrap();
        let first = store
            .set_flow_position(&task.id, autonomous.clone())
            .await
            .unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 303,
            started_at: 1_700_000_000,
        };
        let stale_claim = match store
            .claim_task_worker(
                &task.id,
                &first.invocation.id,
                first.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };

        store.restart_task_flow(&task, "deadbeef").await.unwrap();
        let replacement = store
            .set_flow_position(&task.id, human_flow.clone())
            .await
            .unwrap();

        let create_harness: crate::harness::CreateHarness =
            Box::new(|_, _, _| panic!("stale child must fail before creating a harness"));
        let error = super::run_task_with(
            store.clone(),
            task.id.clone(),
            &create_harness,
            stale_claim,
            &mut tokio::sync::mpsc::unbounded_channel().1,
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("launch claim is stale"));
        assert_eq!(
            store.flow_position(&task.id).await.unwrap(),
            Some(replacement)
        );
    }

    #[tokio::test]
    async fn restart_only_failure_cannot_be_cleared_as_a_retry() {
        let (store, task, _) = human_task_fixture().await;
        let mut blocked = super::start_task_flow(&task, "task-design").unwrap();
        blocked.failure = Some(crate::durable::TaskFlowBlocker {
            reason: "old Flow definition is unavailable".to_string(),
            restart_required: true,
            observed_at: time::OffsetDateTime::now_utc(),
        });
        let blocked = store.set_flow_position(&task.id, blocked).await.unwrap();

        let error = store.retry_task_flow(&task.id, &blocked).await.unwrap_err();

        assert!(error.to_string().contains("explicit Flow restart"));
        assert_eq!(store.flow_position(&task.id).await.unwrap(), Some(blocked));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn control_events_after_seed_preparation_remain_live() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let work = WorkRef::Task(task.id.clone());
        let seeded = store
            .append_steer(&work, Author::User, "seeded direction")
            .await
            .unwrap();
        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &flow)
            .await
            .unwrap();
        assert_eq!(prepared.seeded_steer_id, seeded.id);
        assert!(prepared.turn.input.contains("seeded direction"));

        let late = store
            .append_steer(&work, Author::User, "late direction")
            .await
            .unwrap();
        let late_interrupt = store.append_interrupt(&work).await.unwrap();
        let mut harness = RecordingControlHarness::default();
        let mut steer_cursor = prepared.seeded_steer_id;
        let mut interrupt_cursor = prepared.interrupt_id;

        crate::ops::child::inject_live_steers(&store, &task.id, &mut harness, &mut steer_cursor)
            .await;
        crate::ops::child::observe_interrupt(&store, &work, &mut harness, &mut interrupt_cursor)
            .await;

        assert_eq!(harness.steers, vec!["late direction"]);
        assert_eq!(steer_cursor, late.id);
        assert_eq!(harness.interrupts, 1);
        assert_eq!(interrupt_cursor, late_interrupt);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn interrupted_step_restarts_in_place_with_fresh_direction() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, mut flow) = human_task_fixture().await;
        let interrupted_step = flow.current().step.clone();

        assert!(!super::finish_task_flow_turn(&mut flow, Lifecycle::Interrupted).unwrap());
        store
            .append_steer(
                &WorkRef::Task(task.id.clone()),
                Author::User,
                "direction after interrupt",
            )
            .await
            .unwrap();

        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &flow)
            .await
            .unwrap();

        assert_eq!(flow.current().step, interrupted_step);
        assert!(prepared.turn.input.contains("direction after interrupt"));
    }

    #[tokio::test]
    async fn one_mechanical_op_advances_one_persisted_boundary_without_a_provider() {
        let (_store, task, _) = human_task_fixture().await;
        let flow_dir = task.worktree.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::write(
            flow_dir.join("two-ops.yaml"),
            "- op: rebase --plan\n- op: rebase --plan\n",
        )
        .unwrap();
        let mut flow = super::start_task_flow(&task, "two-ops").unwrap();
        assert_eq!(flow.current().kind, StepKind::Op);
        assert!(!super::run_task_flow_op(&task, &mut flow).await.unwrap());
        assert_eq!(flow.current().index, 1);
        assert_eq!(flow.current().kind, StepKind::Op);
        assert!(super::run_task_flow_op(&task, &mut flow).await.unwrap());
        assert_eq!(flow.cursor.index, 2);
        assert_eq!(flow.cursor.iteration, 0);
    }

    #[tokio::test]
    async fn active_task_invocation_ignores_later_flow_and_skill_edits() {
        let (store, task, _) = human_task_fixture().await;
        let flow_dir = task.worktree.join(".lf/flows");
        let skill_dir = task.worktree.join(".lf/skills");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- original-proof\n- op: rebase --plan\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Original\n\nExecute the definition captured at Flow start.\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "persisted-proof").unwrap();
        store
            .set_flow_position(&task.id, flow.clone())
            .await
            .unwrap();

        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- replacement-proof\n- op: doctor\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Mutated\n\nThis must not affect the active invocation.\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("replacement-proof.md"),
            "# Replacement\n\nA future invocation may use this.\n",
        )
        .unwrap();

        let persisted = store.flow_position(&task.id).await.unwrap().unwrap();
        let crate::engine::ConcreteStep::Skill(active_skill) = persisted.current_plan() else {
            panic!("active first step is a skill")
        };
        assert_eq!(active_skill.skill.name, "original-proof");
        assert!(active_skill
            .skill
            .content
            .as_deref()
            .unwrap()
            .contains("captured at Flow start"));
        let crate::engine::ConcreteStep::Op(active_op) = &persisted.invocation.steps[1] else {
            panic!("active second step is an op")
        };
        assert_eq!(active_op.item.command, "rebase");
        assert_eq!(active_op.item.args, ["--plan"]);

        let future = super::start_task_flow(&task, "persisted-proof").unwrap();
        let crate::engine::ConcreteStep::Skill(future_skill) = future.current_plan() else {
            panic!("future first step is a skill")
        };
        assert_eq!(future_skill.skill.name, "replacement-proof");
        let crate::engine::ConcreteStep::Op(future_op) = &future.invocation.steps[1] else {
            panic!("future second step is an op")
        };
        assert_eq!(future_op.item.command, "doctor");
    }

    async fn park_human_task(
        store: &SharedStore,
        task: &Task,
        flow: &FlowPosition,
    ) -> crate::ops::human_session::FlowSessionToken {
        if store.flow_position(&task.id).await.unwrap().is_none() {
            store
                .set_flow_position(&task.id, flow.clone())
                .await
                .unwrap();
        }
        let position = store.flow_position(&task.id).await.unwrap().unwrap();
        let step = position.current();
        let crate::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
            panic!("human position must select a Skill")
        };
        crate::ops::human_session::FlowSessionToken {
            task_id: task.id.clone(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.policy.id.unwrap(),
            skill: planned.skill.clone(),
            iteration: position.cursor.iteration,
        }
    }

    #[tokio::test]
    async fn restarting_a_human_node_reuses_the_same_task_position() {
        let (store, task, flow) = human_task_fixture().await;
        let original = park_human_task(&store, &task, &flow).await;
        let run_id = RunId::new();
        let mut position = store.flow_position(&task.id).await.unwrap().unwrap();
        position.session_run_id = Some(run_id.clone());
        position.ready_summary = Some("ready".to_string());
        store.set_flow_position(&task.id, position).await.unwrap();
        let persisted = store.flow_position(&task.id).await.unwrap().unwrap();
        let restarted_flow = super::ensure_flow_position(&store, &task.id, None)
            .await
            .unwrap();
        assert_eq!(restarted_flow, persisted);
        let recovered = park_human_task(&store, &task, &restarted_flow).await;

        assert_eq!(recovered, original);
        assert_eq!(store.human_task_flow_positions().await.unwrap().len(), 1);
        let recovered = store.flow_position(&task.id).await.unwrap().unwrap();
        assert_eq!(recovered.session_run_id, Some(run_id));
        assert_eq!(recovered.ready_summary.as_deref(), Some("ready"));
    }

    #[tokio::test]
    async fn stale_human_decisions_cannot_target_a_replacement_invocation() {
        let (store, task, flow) = human_task_fixture().await;
        let stale = park_human_task(&store, &task, &flow).await;
        let mut replacement = store.flow_position(&task.id).await.unwrap().unwrap();
        let step = replacement.current();
        replacement.invocation = crate::durable::test_flow_invocation(
            &step.flow,
            replacement.cursor.index as u32,
            &step.step,
            step.policy.id.as_deref(),
            true,
        );
        let replacement = store
            .set_flow_position(&task.id, replacement)
            .await
            .unwrap();

        assert!(super::complete_human_flow_step(&store, &stale)
            .await
            .is_err());
        let current = store.flow_position(&task.id).await.unwrap().unwrap();
        assert_eq!(current.invocation.id, replacement.invocation.id);
        assert_eq!(current.version, replacement.version);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn claimed_autonomous_boundary_settles_once_at_the_human_node() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, mut task, _) = human_task_fixture().await;
        let mut flow = super::start_task_flow(&task, "task-design").unwrap();
        let initial = store
            .set_flow_position(&task.id, flow.clone())
            .await
            .unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 101,
            started_at: 1_700_000_000,
        };
        let claim = match store
            .claim_task_worker(
                &task.id,
                &initial.invocation.id,
                initial.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let claim = store
            .bind_task_worker_run(&task.id, &claim, &RunId::new(), &owner)
            .await
            .unwrap();
        flow = store.flow_position(&task.id).await.unwrap().unwrap();
        let completed = super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap();
        assert!(!completed);
        super::finish_claimed_task_boundary(
            &store,
            &mut task,
            &mut flow,
            &claim,
            Lifecycle::Completed,
            completed,
            "design ready",
        )
        .await
        .unwrap();

        let settled = store.flow_position(&task.id).await.unwrap().unwrap();
        assert!(settled.is_human());
        assert!(settled.claim.is_none());
        assert_eq!(settled.version, initial.version + 1);
        assert_eq!(store.human_task_flow_positions().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn final_skill_completion_removes_the_flow_without_restarting_it() {
        let (store, mut task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "task-design").unwrap();
        position.invocation.steps.truncate(1);
        position.cursor.iteration = 3;
        let position = store.set_flow_position(&task.id, position).await.unwrap();
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid: 101,
            started_at: 1_700_000_000,
        };
        let claim = match store
            .claim_task_worker(
                &task.id,
                &position.invocation.id,
                position.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        };
        let claim = store
            .bind_task_worker_run(&task.id, &claim, &RunId::new(), &owner)
            .await
            .unwrap();
        let mut position = store.flow_position(&task.id).await.unwrap().unwrap();
        let completed = super::finish_task_flow_turn(&mut position, Lifecycle::Completed).unwrap();
        assert!(completed);
        assert_eq!(position.cursor.iteration, 3);
        super::finish_claimed_task_boundary(
            &store,
            &mut task,
            &mut position,
            &claim,
            Lifecycle::Completed,
            completed,
            "done",
        )
        .await
        .unwrap();

        assert!(store.flow_position(&task.id).await.unwrap().is_none());
        assert_eq!(
            store
                .work_status(&WorkRef::Task(task.id.clone()))
                .await
                .unwrap(),
            crate::durable::WorkStatus::Ready
        );
    }

    async fn ready_review(store: &SharedStore, task: &Task, feedback: &str) {
        let mut position = store.flow_position(&task.id).await.unwrap().unwrap();
        position.ready_summary = Some(feedback.into());
        store.set_flow_position(&task.id, position).await.unwrap();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes LF_BIN while assembling the real next prompt
    async fn nested_review_returns_feedback_before_the_outer_decision() {
        let _lf_bin = super::TestLfBinGuard::pin();
        use crate::engine::transitions::{FlowDecision, FlowVerdict};
        use crate::engine::{ConcretePath, ConcreteStep, ConcreteXor, Skill};
        let (store, task, _) = human_task_fixture().await;
        let mut flow = super::start_task_flow(&task, "pursue").unwrap();
        let body = flow.invocation.steps.clone();
        let suffix = body[0].clone();
        flow.invocation.steps = vec![
            ConcreteStep::Xor(ConcreteXor {
                router: Skill::named("xor-route"),
                paths: [(
                    "work".into(),
                    ConcretePath {
                        description: "implementation and review".into(),
                        steps: body,
                    },
                )]
                .into(),
                flow_parents: vec![],
            }),
            suffix,
        ];
        flow.cursor.route = Some("work".into());
        super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap();
        for _ in 0..4 {
            super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap();
        }
        flow.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "ready to demonstrate".into(),
        });
        super::finish_task_flow_turn(&mut flow, Lifecycle::Completed).unwrap();
        assert!(flow.is_human());
        assert_eq!(flow.current().step, "demo");
        let token = park_human_task(&store, &task, &flow).await;
        assert!(super::complete_human_flow_step(&store, &token)
            .await
            .is_err());
        let notes = task.worktree.join("scratch/search");
        std::fs::create_dir_all(&notes).unwrap();
        let observation = "The human lost their place when clearing a search. Agreed: keep results on one screen. Next: implement clearing without a navigation change and prove focus stays in the search field.";
        std::fs::write(
            notes.join("feedback.md"),
            format!("# Search feedback\n\n{observation}\n\nDesign: [search design](design.md)\n"),
        )
        .unwrap();
        let design =
            "The search field and results share one screen; clearing preserves keyboard focus.";
        std::fs::write(notes.join("design.md"), design).unwrap();
        let feedback = "See scratch/search/feedback.md and scratch/search/design.md; implement the agreed search interaction";
        ready_review(&store, &task, feedback).await;
        let mut ordinary = flow.cursor.clone();
        ordinary.leaf_mut().progress.direction = Some(feedback.into());
        ordinary.finish(&flow.invocation.steps).unwrap();
        super::complete_human_flow_step(&store, &token)
            .await
            .unwrap();
        let mut saved = store.flow_position(&task.id).await.unwrap().unwrap();
        assert_eq!(saved.cursor, ordinary);
        assert_eq!(saved.current().step, "loop-decide");
        assert_eq!(saved.cursor.iteration, 0);
        assert_eq!(
            saved.cursor.leaf().progress.direction.as_deref(),
            Some(feedback)
        );
        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &saved)
            .await
            .unwrap();
        assert!(prepared.turn.input.contains(feedback));
        assert!(prepared.turn.input.contains(observation));
        assert!(prepared.turn.input.contains(design));
        assert!(saved.cursor.leaf().progress.verdict.is_none());
        assert!(super::complete_human_flow_step(&store, &token)
            .await
            .is_err());
        assert_eq!(store.flow_position(&task.id).await.unwrap().unwrap(), saved);
        saved.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Iterate,
            summary: "Implement scratch/search/design.md using the findings and proof in scratch/search/feedback.md".into(),
        });
        super::finish_task_flow_turn(&mut saved, Lifecycle::Completed).unwrap();
        assert_eq!(saved.current().step, "implement");
        assert_eq!(saved.cursor.iteration, 1);
        let implementation =
            super::prepare_task_flow_step(&store, &task, "human-task-proof", &saved)
                .await
                .unwrap();
        assert!(implementation.turn.input.contains(observation));
        assert!(implementation.turn.input.contains(design));
        assert!(implementation
            .turn
            .input
            .contains("Implement scratch/search/design.md"));
        assert_eq!(
            std::fs::read_to_string(notes.join("design.md")).unwrap(),
            design
        );
        for _ in 0..4 {
            super::finish_task_flow_turn(&mut saved, Lifecycle::Completed).unwrap();
        }
        saved.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "revision demonstrated".into(),
        });
        super::finish_task_flow_turn(&mut saved, Lifecycle::Completed).unwrap();
        store.set_flow_position(&task.id, saved).await.unwrap();
        let later = park_human_task(&store, &task, &flow).await;
        assert_ne!(later.iteration, token.iteration);
        ready_review(&store, &task, "the interaction works").await;
        super::complete_human_flow_step(&store, &later)
            .await
            .unwrap();
        let mut saved = store.flow_position(&task.id).await.unwrap().unwrap();
        saved.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "human feedback and proof agree".into(),
        });
        super::finish_task_flow_turn(&mut saved, Lifecycle::Completed).unwrap();
        assert_eq!(saved.cursor.index, 1);
        assert!(saved.cursor.child.is_none());
    }

    #[tokio::test]
    async fn completing_a_final_review_finishes_the_flow() {
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "design clarified").await;
        super::complete_human_flow_step(&store, &token)
            .await
            .unwrap();
        assert!(store.flow_position(&task.id).await.unwrap().is_none());
        assert!(!crate::ops::human_session::token_is_current(&store, &token)
            .await
            .unwrap());
        assert!(store.recent_task_events(&task.id, 10).await.unwrap().iter().any(|event|
            matches!(&event.kind, TaskEventKind::FlowFinished { summary, .. } if summary == "design clarified")
        ));
    }

    #[tokio::test]
    async fn concurrent_review_completions_settle_once() {
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "review feedback").await;
        let (first, second) = tokio::join!(
            super::complete_human_flow_step(&store, &token),
            super::complete_human_flow_step(&store, &token),
        );
        assert_ne!(first.is_ok(), second.is_ok());
        assert!(!crate::ops::human_session::token_is_current(&store, &token)
            .await
            .unwrap());
    }

    #[test]
    fn task_seed_uses_the_current_chapter_plan() {
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
            project_id: crate::work::project::ProjectId::new(),
            worktree: "/tmp/task".into(),
            workspace_slug: "ship-it".to_string(),
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
        assert!(seed.contains("Chapter plan snapshot synced at: 22"));
        assert!(!seed.contains("metric-portfolio"));
        assert!(!seed.contains("project-owned-metrics"));
    }
}
