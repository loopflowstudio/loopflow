use std::io::BufRead;
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::child::ChildRef;
use crate::durable::{FlowSession, Steer, TaskWorkerClaim, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{ConcreteSkill, ExecutionContext};
use crate::harness::{drain_turn_failure_reason, ApprovalPolicy, Harness};
use crate::lf::commands::flow::{StepEnd, StepLauncher};
use crate::planning::ProjectPlan;
use crate::store::SharedStore;
use crate::work::project::Project;
use crate::work::task::{Task, TaskId, TaskPrId};
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
    task_pr_id: TaskPrId,
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
    // Consuming the launch capability must not erase the restriction on Task
    // descendants changing the machine installation, including operation steps.
    std::env::set_var(crate::run_record::TASK_ORIGIN_ENV, "1");
    let owner = crate::journal::current_process_identity()
        .ok_or_else(|| anyhow!("Task worker requires a registered Exec"))?;
    let launch_claim = store
        .sqlite
        .handoff_task_worker(&task_id, &launch_claim, &owner)?;
    drive_task(
        store,
        task_id,
        launch_claim,
        Box::new(crate::harness::default_create_harness),
    )
    .await
}

/// Drive the Task's Flow through the shared executor with this process's
/// claim. A step that ends without a result releases the position; a
/// decision Run that failed without a verdict opens its one unblock Session.
async fn drive_task(
    store: SharedStore,
    task_id: TaskId,
    launch_claim: TaskWorkerClaim,
    create_harness: crate::harness::CreateHarness,
) -> Result<()> {
    let (attachment_tx, attachment_rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if attachment_tx.send(line).is_err() {
                break;
            }
        }
    });
    let account =
        take_task_launch_env(crate::ops::TASK_ACCOUNT_ID_ENV, "Task provider account id")?;
    let flow = store
        .task_flow(&task_id)
        .await?
        .ok_or_else(|| anyhow!("Task has no active Flow"))?;
    if flow.claim.as_ref() != Some(&launch_claim) {
        anyhow::bail!("Task driver launch claim is stale");
    }
    let launcher = TaskLauncher {
        store: store.clone(),
        task_id: task_id.clone(),
        create_harness,
        attachment_rx: tokio::sync::Mutex::new(attachment_rx),
        route: tokio::sync::Mutex::new(ProviderRoute {
            agent: None,
            account,
        }),
    };
    let result =
        crate::lf::commands::flow::drive(store.clone(), flow, Some(launch_claim), &launcher).await;
    if result.is_err() {
        if let Some(blocked) = store.task_flow(&task_id).await? {
            if blocked
                .failure
                .as_ref()
                .is_some_and(|failure| failure.captured.is_some())
                && blocked.is_decision()
            {
                let task = load_task(&store, &task_id).await?;
                crate::ops::human_session::task_unblock(&store, &task, &blocked).await?;
            }
        }
    }
    result.map(|_| ())
}

pub async fn run_worker(task_id: TaskId) -> Result<()> {
    crate::ops::task_destination::require_worker_destination()?;
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
    flow: &FlowSession,
    task_pr_id: TaskPrId,
) -> Result<crate::run_record::RunSpec> {
    let step = flow.current();
    let mut membership = crate::run_record::RunFlowStep::of(flow)?;
    membership.task_pr_id = Some(task_pr_id);
    Ok(crate::run_record::RunSpec {
        harness,
        model,
        surface: "headless".to_string(),
        cwd: task.worktree.clone(),
        repo: Some(Path::new(wave.repo()).to_path_buf()),
        worktree: Some(task.worktree.clone()),
        skill: Some(step.step),
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
        flow: crate::run_record::RunFlowMembership::Step(membership),
        work: None,
    })
}

/// The provider account the worker's steps run on; re-resolved when the
/// Task's agent changes between steps.
struct ProviderRoute {
    agent: Option<String>,
    account: Option<String>,
}

/// The Task worker's launcher: the shared executor's steps run in this
/// process's harness with live steers, interrupts and terminal attachment;
/// a review parks with its worktree checkpointed and its Session launched.
struct TaskLauncher {
    store: SharedStore,
    task_id: TaskId,
    create_harness: crate::harness::CreateHarness,
    attachment_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<String>>,
    route: tokio::sync::Mutex<ProviderRoute>,
}

/// How a Task step that produced no result ends: a retryable failure releases
/// the position unless a launched decision Run must be inspected first.
fn step_failure(reason: String, retryable: bool, inspect: bool) -> anyhow::Error {
    if retryable && !inspect {
        StepEnd::Released(reason).into()
    } else {
        anyhow!(reason)
    }
}

impl TaskLauncher {
    async fn account_for(&self, task: &Task, skill: &ConcreteSkill) -> Result<String> {
        let agent = crate::ops::task::resolve_task_agent(
            &task.worktree,
            task.agent.as_deref(),
            Some(&skill.skill),
        );
        let mut route = self.route.lock().await;
        if route
            .agent
            .as_ref()
            .is_some_and(|previous| previous != &agent)
        {
            route.account = None;
        }
        if route.account.is_none() {
            let selected = crate::ops::task::preflight_task_execution(&task.worktree, &agent)
                .await
                .map_err(|error| {
                    let (reason, resumable) = unhandled_failure_receipt(&error.to_string());
                    step_failure(reason, resumable, false)
                })?;
            route.account = Some(selected.to_string());
        }
        route.agent = Some(agent);
        Ok(route.account.clone().expect("route was just resolved"))
    }
}

#[async_trait]
impl StepLauncher for TaskLauncher {
    async fn review(&self, flow: &FlowSession, _skill: &ConcreteSkill) -> Result<Option<String>> {
        let task = load_task(&self.store, &self.task_id).await?;
        park_at_review(&self.store, &task, flow).await?;
        Ok(None)
    }

    async fn launch(
        &self,
        flow: &FlowSession,
        skill: &ConcreteSkill,
        _ctx: &ExecutionContext,
        claim: Option<&TaskWorkerClaim>,
    ) -> Result<Option<String>> {
        let store = &self.store;
        let task = load_task(store, &self.task_id).await?;
        let wave = owning_wave(store, &task).await?;
        let project = owning_project(store, &task).await?;
        let account = self.account_for(&task, skill).await?;
        crate::ops::linear_observe::refresh_task_comments(store, &task).await?;
        let mut prepared = prepare_task_flow_step(store, &task, wave.name(), flow).await?;
        prepared.turn.config.env.insert(
            crate::ops::flow_run::FLOW_STEP_ENV.to_string(),
            crate::ops::flow_run::ActiveStep::of(flow).env_value()?,
        );
        let (harness_name, _) = crate::engine::config::parse_agent(
            prepared
                .turn
                .config
                .agent
                .as_deref()
                .expect("Task launch prepared its agent"),
        );
        let attempt = flow
            .current_attempt
            .as_ref()
            .ok_or_else(|| anyhow!("Task step has no reserved Run"))?;
        let publish = {
            let captured = attempt.captured;
            let store = store.clone();
            let id = flow.id().to_owned();
            let version = flow.version;
            let claim = claim.cloned();
            let (harness, model) = (prepared.turn.harness.clone(), prepared.turn.model.clone());
            move |_artifact: &String| {
                store.sqlite.publish_attempt(
                    &id,
                    version,
                    captured,
                    claim.as_ref(),
                    &harness,
                    model.as_deref(),
                )
            }
        };
        let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
            task_run_spec(
                &task,
                &wave,
                &project,
                prepared.turn.harness.clone(),
                prepared.turn.model.clone(),
                flow,
                prepared.task_pr_id.clone(),
            )?,
            attempt.run_id.clone(),
            None,
            &prepared.turn.context,
            publish,
        )?;
        capture.claim_conversation_driver()?;
        prepared.turn.config.session_driver = capture.session_driver();
        prepared.turn.config.flow_selection = capture.flow_turn_selection()?;
        prepared.turn.config.resume_token = capture.conversation_resume_token()?;
        capture.record_input("initial", &prepared.turn.input);
        capture.record_input("steer_seed_through", &prepared.seeded_steer_id.to_string());
        prepared.turn.config.env.extend(capture.environment());
        capture.mark_spawn_requested();
        let fail = |reason: &str, retryable: bool| {
            finish_capture(&capture, "failed");
            step_failure(reason.to_string(), retryable, flow.is_decision())
        };
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let mut harness =
            (self.create_harness)(&harness_name, ApprovalPolicy::AutoApprove, event_tx).map_err(
                |error| {
                    let (reason, resumable) = unhandled_failure_receipt(&error.to_string());
                    fail(&reason, resumable)
                },
            )?;
        let requested_account = crate::store::ProviderAccountId::parse(&account)
            .map_err(|reason| anyhow!("invalid Task provider account route: {reason}"))?;
        harness.set_provider_account_id(Some(requested_account));
        harness.set_provider_session_id(prepared.turn.config.resume_token.clone());
        if let Err(error) = harness.start(&prepared.turn.config).await {
            return Err(fail(&error.to_string(), true));
        }
        capture.observe_provider(harness.provider_session_id(), harness.provider_account_id());
        let mut steer_cursor = prepared.seeded_steer_id;
        let mut interrupt_cursor = prepared.interrupt_id;
        if let Err(error) = harness.send_input(&prepared.turn.input).await {
            let _ = harness.stop().await;
            let (reason, resumable) = unhandled_failure_receipt(&error.to_string());
            return Err(fail(&reason, resumable));
        }
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
        // Steers land as durable comments on this Work; a live turn injects any that
        // arrive after its seed was folded. The initial cursor comes from that exact
        // snapshot, so a comment landing between preparation and TurnStarted cannot
        // be mistaken for seeded direction.
        let work = WorkRef::Task(task.id.clone());
        let mut attachment_rx = self.attachment_rx.lock().await;
        let mut steer_tick = tokio::time::interval(STEER_POLL_INTERVAL);
        steer_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut activity_tick = tokio::time::interval(crate::run_record::activity::SAMPLE_INTERVAL);
        activity_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = activity_tick.tick() => {
                    capture.observe_activity(harness.process_id()).await;
                }
                _ = steer_tick.tick() => {
                    let delivered = crate::ops::child::inject_live_steers(
                        store, &task.id, harness.as_mut(), &mut steer_cursor,
                    ).await;
                    for steer in delivered {
                        capture.record_input(&format!("steer_transport_accepted:{}", steer.id), &steer.text);
                    }
                    crate::ops::child::observe_interrupt(
                        store, &work, harness.as_mut(), &mut interrupt_cursor,
                    ).await;
                }
                line = attachment_rx.recv(), if !attachment_rx.is_closed() => {
                    if let Some(line) = line {
                        handle_attachment(store, &task, harness.as_mut(), line).await?;
                    }
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        let _ = harness.stop().await;
                        return Err(fail("provider event stream closed", true));
                    };
                    capture.observe_provider(harness.provider_session_id(), harness.provider_account_id());
                    capture.record_conversation(event.clone());
                    match event {
                        ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                        ConversationEvent::TurnCompleted { status, .. } => {
                            let _ = harness.stop().await;
                            if status == Lifecycle::Failed {
                                let reason = drain_turn_failure_reason(
                                    &mut event_rx,
                                    "provider turn failed",
                                );
                                let (reason, retryable) = match provider_credential_blocker(&reason) {
                                    Some(blocker) => (blocker, false),
                                    None => (reason, true),
                                };
                                return Err(fail(&reason, retryable));
                            }
                            return match status {
                                Lifecycle::Interrupted => {
                                    finish_capture(&capture, "interrupted");
                                    Err(StepEnd::Interrupted.into())
                                }
                                Lifecycle::Completed => {
                                    finish_capture(&capture, "completed");
                                    Ok(Some(progress_summary(&last_text)))
                                }
                                other => Err(fail(
                                    &format!("Task flow turn ended with unexpected status {other:?}"),
                                    false,
                                )),
                            };
                        }
                        ConversationEvent::Error { code, message, .. } => {
                            let reason = format!("{code}: {message}");
                            let (reason, retryable) = match provider_credential_blocker(&reason) {
                                Some(blocker) => (blocker, false),
                                None => (reason, true),
                            };
                            let _ = harness.stop().await;
                            return Err(fail(&reason, retryable));
                        }
                        ConversationEvent::TurnStarted { .. }
                        | ConversationEvent::ItemCompleted { .. }
                        | ConversationEvent::ItemStarted { .. }
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
}

async fn prepare_task_flow_step(
    store: &SharedStore,
    task: &Task,
    wave_name: &str,
    flow: &FlowSession,
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
    if let Some(repeat) = &skill.repeat {
        let edge = skill.id.as_deref().expect("a repeat occurrence has an id");
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
    }
    if let Some(output) = flow
        .current_step()
        .and_then(crate::engine::flow_output::FlowOutput::for_step_instructions)
    {
        seed.push_str(&output);
    }
    let agent = crate::ops::task::resolve_task_agent(
        &task.worktree,
        task.agent.as_deref(),
        Some(&skill.skill),
    );
    let mut prepared = crate::lf::commands::run::prepare_harness_turn_from_skill_at(
        &skill.skill,
        &seed,
        wave_name,
        None,
        &task.worktree,
        Some(&agent),
    )?;
    prepared.config.write_scope = crate::engine::agent::AgentWriteScope::Worktree;
    prepared.config.execution_boundary = Some(
        crate::ops::task::task_execution_boundary(&task.worktree, &agent)
            .map_err(|error| anyhow!(error.to_string()))?,
    );
    prepared.config.skip_permissions = true;
    Ok(PreparedTaskStep {
        task_pr_id: pr.id,
        turn: prepared,
        seeded_steer_id,
        interrupt_id,
    })
}

/// The human completed the Task's review; the store advances the cursor with
/// the feedback, or ends the Flow when the review was its last step.
pub(crate) async fn complete_human_flow_step(
    store: &SharedStore,
    token: &crate::ops::human_session::FlowSessionToken,
    expected: &FlowSession,
) -> Result<()> {
    if !crate::ops::human_session::token_is_current(store, token).await? {
        anyhow::bail!("review session is stale");
    }
    crate::ops::human_session::require_current_review_actor(store, expected).await?;
    let text = expected
        .ready_summary
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| {
            anyhow!("review is not ready; its agent must run `lf session ready` first")
        })?;
    let step = expected.current();
    if !step.human
        || step.invocation_id != token.invocation_id
        || step.id.as_deref() != Some(token.node_id.as_str())
        || step.step != token.skill.name
        || step.iteration != token.iteration
        || step.flow != token.flow
    {
        anyhow::bail!("review session no longer matches the Task Flow position");
    }
    store
        .complete_task_review(&token.task_id, expected, text)
        .await?;
    Ok(())
}

/// The Task's Flow to launch: the one it points at, or `selected_flow` when
/// the Task has none or points at a different Flow. Selecting a different
/// Flow replaces the current one in one transaction. A blocked decision opens
/// its unblock Session; a review without its Session parks.
pub(crate) async fn ensure_flow_position(
    store: &SharedStore,
    task_id: &TaskId,
    selected_flow: Option<&str>,
) -> Result<FlowSession> {
    let task = load_task(store, task_id).await?;
    let current = match (store.task_flow(&task.id).await?, selected_flow) {
        (Some(current), Some(selected)) if current.invocation.flow != selected => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (Some(current), _) => current,
        (None, Some(selected)) => {
            store
                .start_task_flow(&task.id, start_task_flow(&task, selected)?)
                .await?
        }
        (None, None) => anyhow::bail!(
            "Task {} has no active Flow; run `lf task run {} [--flow FLOW]` to select one",
            task.plan.identifier,
            task.plan.identifier
        ),
    };
    if current
        .failure
        .as_ref()
        .is_some_and(|failure| failure.captured.is_some())
        && current.is_decision()
    {
        crate::ops::human_session::task_unblock(store, &task, &current).await?;
        return Ok(current);
    }
    if current.is_human() && current.pending_session_id.is_none() {
        return park_at_review(store, &task, &current).await;
    }
    Ok(current)
}

/// Park the Task's Flow at its review: the review Session exists, the
/// worktree is checkpointed, and the Session's terminal is launched.
async fn park_at_review(
    store: &SharedStore,
    task: &Task,
    flow: &FlowSession,
) -> Result<FlowSession> {
    let flow = store.reserve_task_review(flow.id(), flow.version).await?;
    let node_id = flow
        .current()
        .id
        .ok_or_else(|| anyhow!("Task review step has no stable node id"))?;
    checkpoint_worktree_before_human(task, &node_id).await;
    crate::ops::human_session::prepare(store, task, &flow).await?;
    Ok(flow)
}

/// Settle the harness launch on every terminal path.
fn finish_capture(capture: &crate::run_record::CaptureHandle, outcome: &str) {
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

/// A fresh invocation of `selected_flow` for the Task, at its first step.
fn start_task_flow(task: &Task, selected_flow: &str) -> Result<FlowSession> {
    Ok(FlowSession {
        parent_id: None,
        invocation: QueuedInvocation::load(&task.worktree, selected_flow)?,
        cursor: crate::engine::ExecutionCursor::default(),
        version: 0,
        task_id: Some(task.id.clone()),
        wave_id: Some(task.wave_id.clone()),
        cwd: task.worktree.clone(),
        message: None,
        model: None,
        current_attempt: None,
        pending_session_id: None,
        ready_summary: None,
        worker_generation: 0,
        claim: None,
        failure: None,
        finished: false,
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
    _ambient: crate::test_ambient::EnvGuard,
    previous: Option<std::ffi::OsString>,
    ledger: crate::journal::TestLedgerGuard,
}

#[cfg(test)]
impl TestLfBinGuard {
    fn pin() -> Self {
        let ledger = crate::journal::TestLedgerGuard::new();
        let ambient = crate::test_ambient::EnvGuard::new();
        let previous = std::env::var_os("LF_BIN");
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        Self {
            _ambient: ambient,
            previous,
            ledger,
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
    use super::{task_seed, unhandled_failure_receipt};
    use crate::chat::types::Lifecycle;
    use crate::durable::{
        Author, FlowSession, TaskWorkerClaim, TaskWorkerClaimOutcome, TaskWorkerOwner, WorkRef,
    };
    use crate::engine::agent::AgentConfig;
    use crate::engine::invocation::StepKind;
    use crate::harness::{Harness, SendCurrentOutcome};
    use crate::id::{ExecId, TraceId};
    use crate::ops::human_session::{self, SessionFlowMembership};
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::run_record::SessionTitleSource;
    use crate::store::{SharedStore, StorageConfig};
    use crate::work::project::{Project, ProjectId};
    use crate::work::task::{
        Observation, PmWritebackState, Task, TaskEventKind, TaskId, TaskPr, TaskPrId,
    };
    use crate::work::wave::Wave;

    /// Settle one completed step of a position in memory: the engine's traversal.
    fn finish(position: &mut FlowSession) -> anyhow::Result<bool> {
        position.cursor.finish(&position.invocation.steps)
    }

    /// An interrupted step keeps its position and discards its candidate.
    fn interrupt(position: &mut FlowSession) -> bool {
        let leaf = position.cursor.leaf_mut();
        leaf.progress.verdict = None;
        leaf.route = None;
        false
    }

    async fn claim(
        store: &SharedStore,
        task: &Task,
        flow: &FlowSession,
        pid: u32,
    ) -> TaskWorkerClaim {
        let owner = TaskWorkerOwner {
            trace_id: TraceId::new(),
            exec_id: ExecId::new(),
            pid,
            started_at: 1_700_000_000,
        };
        match store
            .claim_task_worker(
                &task.id,
                &flow.invocation.id,
                flow.version,
                &owner,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .unwrap()
        {
            TaskWorkerClaimOutcome::Claimed(claim) => claim,
            outcome => panic!("unexpected claim outcome: {outcome:?}"),
        }
    }

    /// The Run the claim reserved for the current step.
    async fn reserved_run(store: &SharedStore, task: &Task) -> String {
        store
            .task_flow(&task.id)
            .await
            .unwrap()
            .unwrap()
            .current_attempt
            .expect("the claim reserved the step's Run")
            .run_id
    }

    /// Start `flow` as the Task's Flow and park it when its first step is a review.
    async fn parked(store: &SharedStore, task: &Task, flow: FlowSession) -> FlowSession {
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        if flow.is_human() {
            return super::park_at_review(store, task, &flow).await.unwrap();
        }
        flow
    }

    /// Stand in for Linear so a driven step can refresh its Task comments.
    async fn linear_stub(
        store: &SharedStore,
        task: &Task,
        home: &std::path::Path,
    ) -> crate::ops::pm::PmTestContext {
        use crate::pm::test_server::{self, json_response};
        let (url, _) = test_server::spawn(
            (0..4)
                .map(|_| {
                    json_response(
                        axum::http::StatusCode::OK,
                        serde_json::json!({"data": {"issue": {
                            "updatedAt": "2026-09-24T00:00:00Z", "title": task.plan.title,
                            "description": task.plan.description,
                            "comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}
                        }}}),
                    )
                })
                .collect(),
        )
        .await;
        store
            .upsert_provider_token(&crate::store::ProviderToken {
                provider: "linear".into(),
                access_token: "fixture-token".into(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: None,
                updated_at: 1,
                credential_type: crate::store::CredentialType::OAuth,
            })
            .await
            .unwrap();
        crate::ops::pm::PmTestContext {
            path: home.join("fixture.db"),
            store: store.clone(),
            graphql_url: url,
        }
    }

    /// A harness whose provider stream closes as soon as the turn is sent.
    fn closing_harness() -> crate::harness::CreateHarness {
        Box::new(|_, _, _events| Ok(Box::new(UnusedHarness::default())))
    }

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
        finish(&mut position).unwrap();
        assert!(position.is_human());
        assert_eq!(position.current().step, "review-design");
        position.cursor.progress.direction = Some("Design clarified with the human".into());
        finish(&mut position).unwrap();
        for pass in 0..10 {
            for expected in ["implement", "compress", "rebase", "realign"] {
                assert_eq!(position.current().step, expected);
                assert!(!finish(&mut position).unwrap());
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
            assert!(!finish(&mut position).unwrap());
        }
        assert_eq!(position.current().step, "pr-publish");
        assert!(!position.is_human());
        assert!(!finish(&mut position).unwrap());
        assert!(position.is_human());
        assert_eq!(position.current().step, "demo");
        assert_eq!(position.cursor.iteration, 9);
        assert!(position.cursor.progress.verdict.is_none());
        position.cursor.progress.direction = Some("Human requested a delivery correction".into());
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "loop-decide");
        assert!(position.cursor.progress.verdict.is_none());
        assert_eq!(position.cursor.iteration, 9);
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Human requested a delivery correction".into(),
        });
        assert!(!finish(&mut position).unwrap());
        assert_eq!(position.current().step, "implement");
        assert_eq!(
            position.cursor.progress.direction.as_deref(),
            Some("Human requested a delivery correction")
        );
        for _ in 0..4 {
            finish(&mut position).unwrap();
        }
        assert_eq!(position.current().step, "loop-decide");
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Iterate,
            summary: "Continue the human-requested revision".into(),
        });
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "implement");
        assert_eq!(position.cursor.progress.repeats["decide"], 10);
        // Final Advance is the only edge into queue and landing. This traverses
        // the authored plan without invoking any publication operation.
        for _ in 0..4 {
            finish(&mut position).unwrap();
        }
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "Revision proved".into(),
        });
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "pr-publish");
        finish(&mut position).unwrap();
        assert_eq!(position.current().step, "demo");
        finish(&mut position).unwrap();
        position.cursor.progress.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "Human feedback addressed".into(),
        });
        finish(&mut position).unwrap();
        for expected in ["compress", "rebase", "realign", "gate", "pr land -c"] {
            assert_eq!(position.current().step, expected);
            let finished = finish(&mut position).unwrap();
            assert_eq!(finished, expected == "pr land -c");
        }
    }

    #[tokio::test]
    async fn loop_requires_a_decision_and_discards_it_on_interrupt() {
        let (_, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "feature").unwrap();
        while !position.is_decision() {
            assert!(!finish(&mut position).unwrap());
        }
        assert_eq!(position.current().step, "loop-decide");
        let decision_index = position.cursor.index;
        assert!(finish(&mut position)
            .unwrap_err()
            .to_string()
            .contains("requires a decision"));
        assert_eq!(position.cursor.index, decision_index);
        let review = &mut position.cursor.progress;
        review.verdict = Some(crate::engine::transitions::FlowVerdict {
            decision: crate::engine::transitions::FlowDecision::Advance,
            summary: "discard on interrupt".into(),
        });
        assert!(!interrupt(&mut position));
        assert_eq!(position.cursor.index, decision_index);
        assert!(position.cursor.progress.verdict.is_none());
    }

    struct SliceHarness {
        store: SharedStore,
        task_id: TaskId,
        events: tokio::sync::mpsc::UnboundedSender<crate::chat::types::ConversationEvent>,
        seen: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
        restart: bool,
        session_ready: bool,
        failure: Option<&'static str>,
        expected_input: Option<(Vec<&'static str>, std::sync::Arc<tokio::sync::Notify>)>,
    }

    fn review_commands() -> Vec<crate::chat::types::ConversationItem> {
        [
            ("inspect-history", "saved fixture: operation not permitted: ps\nAttributeError: 'list' object has no attribute 'get'", 1),
            ("delivery", "fatal: cannot create index.lock: Permission denied", 128),
            ("delivery-retry", "committed", 0),
        ].into_iter().map(|(id, output, exit_code)| crate::chat::types::ConversationItem::Command {
            id: id.into(), command: vec![id.into()], cwd: "/fixture".into(),
            status: if exit_code == 0 { Lifecycle::Completed } else { Lifecycle::Failed },
            output: Some(output.into()), exit_code: Some(exit_code), duration_ms: Some(1),
        }).collect()
    }

    #[async_trait::async_trait]
    impl Harness for SliceHarness {
        async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
            let position = self.store.task_flow(&self.task_id).await?.unwrap();
            let run = position.review_artifact_key().unwrap();
            assert_eq!(
                config.env.get(crate::durable::RUN_ID_ENV),
                Some(&run.to_string())
            );
            assert!(config.env.contains_key(crate::ops::flow_run::FLOW_STEP_ENV));
            if let Some((session, driver)) = &config.session_driver {
                // This harness simulates a finite provider. Give recovery exact
                // child-exit evidence instead of treating a missing socket as death.
                let mut child = tokio::process::Command::new("sleep")
                    .arg("60")
                    .kill_on_drop(true)
                    .spawn()?;
                let pid = child.id().unwrap();
                let started = crate::journal::process_started_at(pid)?.unwrap();
                self.store
                    .sqlite
                    .record_session_provider_process(session, driver, pid, started)?;
                child.kill().await?;
                child.wait().await?;
            }
            Ok(())
        }

        async fn send_input(&mut self, content: &str) -> anyhow::Result<()> {
            let position = self.store.task_flow(&self.task_id).await?.unwrap();
            if let Some((expected_input, resumed)) = &self.expected_input {
                for expected in expected_input {
                    assert!(
                        content.contains(expected),
                        "missing resumed direction: {expected}"
                    );
                }
                assert!(position.cursor.leaf().progress.verdict.is_none());
                resumed.notified().await;
            }
            if self.restart {
                let owner = position.claim.as_ref().unwrap().owner.clone();
                let mut replacement = position.clone();
                replacement.invocation.id = "replacement-invocation".into();
                replacement.version = 0;
                replacement.claim = None;
                let task = super::load_task(&self.store, &self.task_id).await?;
                let stopped = self
                    .store
                    .release_flow(position.id(), position.version, position.claim.as_ref())
                    .await?;
                self.store
                    .restart_task_flow(&task, Some(&stopped), "fixture-checkpoint")
                    .await?;
                let replacement = self
                    .store
                    .start_task_flow(&self.task_id, replacement)
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
            let run = position.review_artifact_key().unwrap().clone();
            let (dir, _) = crate::run_record::resolve_manifest(
                &crate::store::observability_home_dir(),
                run.as_str(),
            )?;
            // Inspect the actual worker while input is pending and no native ID exists.
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                loop {
                    let events =
                        std::fs::read_to_string(dir.join("events.jsonl")).unwrap_or_default();
                    if events.lines().any(|line| {
                        serde_json::from_str::<serde_json::Value>(line).is_ok_and(|event| {
                            event["type"] == "provider_account_selected"
                                && event["account_id"] == "selected-account"
                        })
                    }) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("running Task must record its selected account before a session ID");
            assert!(!dir.join("terminal.json").exists());
            assert!(crate::run_record::read_provider_session(&dir)?.is_none());
            self.session_ready = true;
            self.seen
                .lock()
                .unwrap()
                .push((step.step.clone(), run.clone()));
            if position.cursor.iteration > 0 {
                assert!(content.contains("repair the demonstrated gap"));
                let parent = self
                    .store
                    .flow(position.parent_id.as_ref().unwrap())
                    .await?
                    .unwrap();
                assert_eq!(parent.task_id, position.task_id);
                assert_eq!(parent.wave_id, position.wave_id);
                assert!(parent.claim.is_none());
                assert_eq!(parent.current().step, "loop-decide");
                assert!(self
                    .store
                    .reserve_attempt(parent.id(), parent.version, None)
                    .await
                    .is_err());
            }
            let actor = self.store.sqlite.test_flow_turn(&run);
            if step.step == "concept-review" {
                for item in review_commands() {
                    self.events
                        .send(crate::chat::types::ConversationEvent::ItemCompleted {
                            turn_id: "fixture-turn".into(),
                            item,
                        })?;
                }
            }
            if routing {
                assert!(content.contains("declared JSON object"));
                self.store
                    .sqlite
                    .test_output(&actor, &serde_json::json!({"path": "selected"}))?;
            }
            if let Some(failure) = self.failure {
                if failure == "malformed" {
                    self.store.sqlite.test_output(
                        &actor,
                        &serde_json::json!({"decision":"advance", "summary":""}),
                    )?;
                } else if failure == "disconnected" {
                    self.store.sqlite.test_finish_flow_turn(&actor, "failed");
                    anyhow::bail!("decision provider disconnected");
                }
                self.store.sqlite.test_finish_flow_turn(&actor, "completed");
                self.events
                    .send(crate::chat::types::ConversationEvent::TurnCompleted {
                        turn_id: "failed-decision".into(),
                        status: Lifecycle::Completed,
                    })?;
                return Ok(());
            }
            if step.repeat.is_some() {
                self.store.sqlite.test_decision_output(
                    &actor,
                    &crate::engine::transitions::FlowVerdict {
                        decision: if position.cursor.iteration == 0 {
                            crate::engine::transitions::FlowDecision::Iterate
                        } else {
                            crate::engine::transitions::FlowDecision::Advance
                        },
                        summary: "repair the demonstrated gap".into(),
                    },
                )?;
            }
            self.store.sqlite.test_finish_flow_turn(&actor, "completed");
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
            self.session_ready.then(|| "task-provider-session".into())
        }
        fn provider_account_id(&self) -> Option<crate::store::ProviderAccountId> {
            Some(crate::store::ProviderAccountId::parse("selected-account").unwrap())
        }
    }

    fn pursue_decision(task: &Task) -> FlowSession {
        let mut flow = super::start_task_flow(task, "pursue").unwrap();
        while !flow.is_decision() {
            assert!(!finish(&mut flow).unwrap());
        }
        assert_eq!(flow.current().step, "loop-decide");
        flow
    }

    #[test]
    fn task_decision_recovery_requires_the_original_successful_run() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            for routing in [false, true] {
                for status in ["completed", "failed", "interrupted"] {
                    // Each case has its own Home: the Run's row and record share it.
                    let guard = super::TestLfBinGuard::pin();
                    let (store, task, _) =
                        human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
                    let mut flow = pursue_decision(&task);
                    flow.invocation.steps.truncate(flow.cursor.index + 1);
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
                    let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                    let held = claim(&store, &task, &flow, 303).await;
                    // The claim reserved the decision Run; its launch publishes it.
                    let run = reserved_run(&store, &task).await;
                    let publish = {
                        let (store, id, version, held) = (
                            store.clone(),
                            flow.id().to_owned(),
                            flow.version,
                            held.clone(),
                        );
                        move |run: &String| {
                            store.sqlite.publish_attempt(
                                &id,
                                version,
                                store.sqlite.captured_sequence(run).unwrap().unwrap(),
                                Some(&held),
                                "proof",
                                None,
                            )
                        }
                    };
                    let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
                        crate::run_record::RunSpec {
                            harness: "proof".into(),
                            model: None,
                            surface: "headless".into(),
                            cwd: task.worktree.clone(),
                            repo: None,
                            worktree: None,
                            skill: Some("loop-decide".into()),
                            subjects: vec![],
                            flow: crate::run_record::RunFlowMembership::Step(
                                crate::run_record::RunFlowStep::of(&flow).unwrap(),
                            ),
                            work: None,
                        },
                        run.clone(),
                        None,
                        &crate::trace::PreparedTurnContext::from_prompts("system", "decide"),
                        publish,
                    )
                    .unwrap();
                    let actor = store.sqlite.test_flow_turn(&run);
                    if routing {
                        store
                            .sqlite
                            .test_output(&actor, &serde_json::json!({"path": "done"}))
                            .unwrap();
                    } else {
                        store
                            .sqlite
                            .test_decision_output(
                                &actor,
                                &crate::engine::transitions::FlowVerdict {
                                    decision: crate::engine::transitions::FlowDecision::Advance,
                                    summary: "candidate, not completion".into(),
                                },
                            )
                            .unwrap();
                    }
                    store.sqlite.test_finish_flow_turn(&actor, status);
                    capture.finish(status).unwrap();
                    let saved = store.task_flow(&task.id).await.unwrap().unwrap();
                    // The driver settles the saved candidate from its Run's row; a
                    // successful Run completes the step without another provider.
                    let result = super::drive_task(
                        store.clone(),
                        task.id.clone(),
                        held.clone(),
                        closing_harness(),
                    )
                    .await;
                    let after = store.task_flow(&task.id).await.unwrap();
                    if status == "completed" {
                        result.unwrap();
                        assert!(after.is_none());
                        assert!(super::drive_task(
                            store.clone(),
                            task.id.clone(),
                            held,
                            closing_harness()
                        )
                        .await
                        .is_err());
                    } else {
                        assert!(result.is_err());
                        let after = after.unwrap();
                        assert_eq!(after.cursor.index, saved.cursor.index);
                        assert!(!after.has_pending_decision());
                        let failure = after.failure.as_ref().unwrap();
                        assert_eq!(
                            failure.captured,
                            store.sqlite.captured_sequence(&run).unwrap()
                        );
                        assert!(after.claim.is_none());
                        let (session, feedback) =
                            crate::ops::human_session::task_unblock(&store, &task, &after)
                                .await
                                .unwrap();
                        assert!(feedback.is_none());
                        assert_eq!(
                            crate::ops::human_session::task_unblock(&store, &task, &after)
                                .await
                                .unwrap()
                                .0,
                            session
                        );
                        let execution =
                            crate::ops::task_execution::task_execution(&store, &task.id)
                                .await
                                .unwrap();
                        assert_eq!(
                            execution.state,
                            crate::ops::task_execution::TaskExecutionState::Blocked
                        );
                        assert_eq!(
                            execution.captured,
                            store.sqlite.captured_sequence(&run).unwrap()
                        );
                    }
                }
            }
            // A dead driver cannot establish the provider outcome. Reclaim preserves
            // the unresolved input and refuses to settle or replay it.
            let guard = super::TestLfBinGuard::pin();
            let (store, task, _) =
                human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
            let mut flow = super::start_task_flow(&task, "pursue").unwrap();
            flow.cursor.index = 3;
            let flow = store.start_task_flow(&task.id, flow).await.unwrap();
            let dead = claim(&store, &task, &flow, 304).await;
            let run = reserved_run(&store, &task).await;
            store
                .sqlite
                .publish_attempt(
                    flow.id(),
                    flow.version,
                    store.sqlite.captured_sequence(&run).unwrap().unwrap(),
                    Some(&dead),
                    "proof",
                    None,
                )
                .unwrap();
            let replacement = store
                .reclaim_task_worker(
                    &task.id,
                    &dead,
                    &TaskWorkerOwner {
                        trace_id: TraceId::new(),
                        exec_id: ExecId::new(),
                        pid: 305,
                        started_at: 1_700_000_000,
                    },
                    time::OffsetDateTime::now_utc(),
                )
                .await
                .unwrap();
            assert!(super::drive_task(
                store.clone(),
                task.id.clone(),
                replacement,
                closing_harness()
            )
            .await
            .is_err());
            let unresolved = store.task_flow(&task.id).await.unwrap().unwrap();
            assert_eq!(unresolved.cursor, flow.cursor);
            let input = unresolved.current_attempt.as_ref().unwrap();
            assert_eq!(input.run_id, run);
            assert_eq!(input.outcome, None);
            store.sqlite.assert_no_historical_runs();
            let session = store.session_for_artifact(&run).await.unwrap().unwrap();
            assert!(store
                .sqlite
                .session_history(&session.id, 0, 0)
                .unwrap()
                .iter()
                .all(|event| event.kind == crate::session::SessionEventKind::Captured));
        });
    }

    #[test]
    fn task_agent_driver_runs_fresh_slice_turns_until_the_flow_finishes() {
        let guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, task, _) = runtime.block_on(human_task_fixture_at(
            &guard.ledger.home().join("loopflow.db"),
        ));
        runtime
            .block_on(store.set_task_agent(&task.id, "claude:sonnet"))
            .unwrap();
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
                std::fs::write(definitions.join("saved-body.yaml"), "- step:\n    name: implement\n    id: work\n- compress\n- realign\n- concept-review\n- step:\n    name: loop-decide\n    id: decide\n    repeat:\n      from: work\n").unwrap();
                let flow = super::start_task_flow(&task, "nested-slice").unwrap();
                // Delete the sources before routing: every possible path was captured.
                std::fs::remove_file(definitions.join("nested-slice.yaml")).unwrap();
                std::fs::remove_file(definitions.join("saved-body.yaml")).unwrap();
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
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
                        store: store.clone(), task_id: task_id.clone(), events, seen: seen.clone(), restart: false, session_ready: false, failure: None, expected_input: None,
                    }))
                });
                // The simulated provider uses a preflighted fixture route.
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                crate::ops::pm::PM_TEST_CONTEXT.scope(crate::ops::pm::PmTestContext {
                    path: guard.ledger.home().join("fixture.db"), store: store.clone(), graphql_url: url,
                }, super::drive_task(store.clone(), task.id.clone(), claim, create)).await.unwrap();
                let turns = seen.lock().unwrap().clone();
                for (_, run) in &turns {
                    let (dir, _) = crate::run_record::resolve_manifest(guard.ledger.home(), run.as_str()).unwrap();
                    let session = crate::run_record::read_provider_session(&dir).unwrap().unwrap();
                    assert_eq!(session.provider_session_id, "task-provider-session");
                    assert_eq!(session.account_id.unwrap().as_str(), "selected-account");
                }
                for (_, run) in turns.iter().filter(|(step, _)| step == "concept-review") {
                    let (dir, _) = crate::run_record::resolve_manifest(guard.ledger.home(), run.as_str()).unwrap();
                    let events = std::fs::read_to_string(dir.join("events.jsonl")).unwrap();
                    let commands = events.lines().map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                        .filter(|event| event["type"] == "conversation" && event["event"]["type"] == "item_completed")
                        .map(|event| event["event"]["item"].clone()).collect::<Vec<_>>();
                    assert_eq!(commands, review_commands().into_iter().map(|item| serde_json::to_value(item).unwrap()).collect::<Vec<_>>());
                }
                assert_eq!(turns.iter().map(|(step, _)| step.as_str()).collect::<Vec<_>>(),
                    ["xor-route", "implement", "compress", "realign", "concept-review", "loop-decide", "implement", "compress", "realign", "concept-review", "loop-decide"]);
                assert_eq!(turns.iter().map(|(_, run)| run).collect::<std::collections::HashSet<_>>().len(), 11);
                let memberships = turns.iter().map(|(_, input)| {
                    store.sqlite.session_for_artifact(input).unwrap().unwrap().flow_session_id.unwrap()
                }).collect::<Vec<_>>();
                assert!(memberships[..6].iter().all(|id| id == flow.id()), "routing and template composition keep the root owner");
                assert!(memberships[6..].iter().all(|id| id == &memberships[6]));
                let pass = store.flow(&memberships[6]).await.unwrap().unwrap();
                assert_eq!(pass.parent_id.as_deref(), Some(flow.id()));
                assert!(pass.finished);
                for (_, run) in &turns {
                    let (_, manifest) = crate::run_record::resolve_manifest(guard.ledger.home(), run.as_str()).unwrap();
                    assert_eq!(manifest.harness, "claude");
                    assert_eq!(manifest.model.as_deref(), Some("sonnet"));
                }
                assert!(store.task_flow(&task.id).await.unwrap().is_none());
                let execution = crate::ops::task_execution::task_execution(&store, &task.id).await.unwrap();
                assert!(execution.reason.contains("Flow nested-slice finished"));

                // Recover a completed decision Run before replacing its claim.
                // Settlement consumes its saved result without another provider.
                let mut flow = pursue_decision(&task);
                flow.invocation.steps.truncate(flow.cursor.index + 1);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("recovery fixture claim") };
                let run = reserved_run(&store, &task).await;
                store.sqlite.publish_attempt(flow.id(), flow.version,
                store.sqlite.captured_sequence(&run).unwrap().unwrap(), Some(&claim), "proof", None).unwrap();
                let actor = store.sqlite.test_flow_turn(&run);
                store.sqlite.test_decision_output(&actor, &crate::engine::transitions::FlowVerdict {
                    decision: crate::engine::transitions::FlowDecision::Advance,
                    summary: "saved whole-design proof".into(),
                }).unwrap();
                store.sqlite.test_finish_flow_turn(&actor, "completed");
                super::drive_task(store.clone(), task.id.clone(), claim, closing_harness()).await.unwrap();
                assert!(store.task_flow(&task.id).await.unwrap().is_none());

                // A late error cannot release a replacement invocation's claim,
                // even when restart reuses its version and generation.
                let flow = pursue_decision(&task);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("late failure fixture claim") };
                let create: crate::harness::CreateHarness = Box::new({
                    let store = store.clone();
                    let task_id = task.id.clone();
                    move |_, _, events| Ok(Box::new(SliceHarness {
                        store: store.clone(), task_id: task_id.clone(), events,
                        seen: Default::default(), restart: true, session_ready: false, failure: None, expected_input: None,
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
                assert!(error.to_string().contains("late provider failure"), "{error:#}");
                let replacement = store.task_flow(&task.id).await.unwrap().unwrap();
                assert!(replacement.failure.is_none());
                let replacement_claim = replacement.claim.expect("replacement worker remains claimed");
                assert_eq!(replacement_claim.position_version, claim.position_version);
                assert_eq!(replacement_claim.generation, claim.generation);
                assert_ne!(replacement_claim.invocation_id, claim.invocation_id);
                assert!(crate::ops::human_session::list(&store, &crate::session::SessionFilter::default()).await.unwrap().is_empty(),
                    "a rejected late decision failure cannot open an unblock Session");
                Ok(())
            })
        }).unwrap();
    }

    #[test]
    fn managed_flow_retry_releases_only_dead_native_selection_and_consumes_its_successor() {
        use crate::session::SessionEventKind;
        use clap::Parser;
        let guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, task, _) = runtime.block_on(human_task_fixture_at(
            &guard.ledger.home().join("loopflow.db"),
        ));
        crate::journal::with_runtime(&task.worktree, &["managed-native-retry-proof".into()], || {
            let (flow, session, prior) = runtime.block_on(async {
                let mut flow = super::start_task_flow(&task, "task-design").unwrap();
                flow.invocation.steps.truncate(1);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                // The orchestration owner is synthetic; the native exit below is
                // an actual owned process. This proves managed policy, not Codex.
                let claim = claim(&store, &task, &flow, u32::MAX).await;
                let run = reserved_run(&store, &task).await;
                store.sqlite.publish_attempt(flow.id(), flow.version,
                store.sqlite.captured_sequence(&run).unwrap().unwrap(), Some(&claim), "codex", None).unwrap();
                let session = store.sqlite.session_for_artifact(&run).unwrap().unwrap();
                let exec = crate::journal::current_exec_id().unwrap();
                let driver = store.sqlite.claim_session_driver(&session.id, None, &exec, true).unwrap();
                let mut selection = store.sqlite.flow_turn_selection(&run).unwrap().unwrap();
                selection.caller_token = Some("managed-unfinished".into());
                let start = store.sqlite.record_session_turn_origin(&session.id, "native-thread", "unfinished", driver.provider_generation, &exec).unwrap();
                store.sqlite.select_flow_turn(&selection, &session.id, &driver, start).unwrap();
                let mut engine = std::process::Command::new("sleep").arg("60").spawn().unwrap();
                let started = crate::journal::process_started_at(engine.id()).unwrap().unwrap();
                store.sqlite.record_session_provider_process(&session.id, &driver, engine.id(), started).unwrap();
                engine.kill().unwrap();
                engine.wait().unwrap();
                store.sqlite.release_session_driver(&session.id, &driver).unwrap();
                let prior = store.sqlite.session_history(&session.id, 0, 100).unwrap();
                (flow, session, prior)
            });
            // Exercise the public managed dispatch. Its --retry must survive
            // delegation; normal Task adoption policy still refuses this branch.
            let cli = crate::lf::Cli::parse_from(["lf"]);
            let error = crate::lf::commands::flow::control("resume", &[flow.id().into(), "--retry".into()], &cli).unwrap_err();
            assert!(error.to_string().contains("active PR expects branch"), "{error:#}");
            runtime.block_on(async {
                let retried = store.task_flow(&task.id).await.unwrap().unwrap();
                assert_eq!(retried.id(), flow.id());
                assert_eq!(retried.cursor, flow.cursor);
                assert!(retried.current_attempt.is_none() && retried.claim.is_none() && retried.failure.is_none());
                assert_eq!(store.sqlite.session_history(&session.id, 0, 100).unwrap(), prior);
                store.sqlite.assert_no_historical_runs();
                let owner = crate::journal::current_process_identity().unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(&task.id, retried.id(), retried.version, &owner, time::OffsetDateTime::now_utc()).await.unwrap() else { panic!("retry claim") };
                let next = reserved_run(&store, &task).await;
                assert_eq!(store.sqlite.session_for_artifact(&next).unwrap().unwrap().id, session.id);
                store.sqlite.publish_attempt(retried.id(), retried.version,
                store.sqlite.captured_sequence(&next).unwrap().unwrap(), Some(&claim), "codex", None).unwrap();
                let expected = store.sqlite.session_driver(&session.id).unwrap();
                let driver = store.sqlite.claim_session_driver(&session.id, expected.as_ref(), &owner.exec_id, true).unwrap();
                let mut selection = store.sqlite.flow_turn_selection(&next).unwrap().unwrap();
                selection.caller_token = Some("managed-retry".into());
                let start = store.sqlite.record_session_turn_origin(&session.id, "native-thread", "retry", driver.provider_generation, &owner.exec_id).unwrap();
                store.sqlite.select_flow_turn(&selection, &session.id, &driver, start).unwrap();
                let completion = store.sqlite.record_session_event(&session.id, "native-thread", "retry", SessionEventKind::Completed, &serde_json::json!({"status":"completed"})).unwrap();
                super::drive_task(store.clone(), task.id.clone(), claim, closing_harness()).await.unwrap();
                assert!(store.task_flow(&task.id).await.unwrap().is_none());
                assert!(store.flow(retried.id()).await.unwrap().unwrap().finished);
                let conn = rusqlite::Connection::open(guard.ledger.home().join("loopflow.db")).unwrap();
                let consumed: Vec<i64> = conn.prepare("SELECT session_event FROM flow_events WHERE flow_id=?1 AND kind='consumed'").unwrap()
                    .query_map([flow.id()], |row| row.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
                assert_eq!(consumed, vec![completion]);
                store.sqlite.assert_no_historical_runs();
            });
            Ok(())
        }).unwrap();
    }

    #[test]
    fn task_decision_live_unblock_returns_feedback_without_navigation() {
        let _guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, task, _) = runtime.block_on(human_task_fixture_at(
            &_guard.ledger.home().join("loopflow.db"),
        ));
        crate::journal::with_runtime(&task.worktree, &["live-unblock-proof".into()], || {
            runtime.block_on(async {
                let flow = pursue_decision(&task);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let owner = crate::journal::current_process_identity().unwrap();
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
                    panic!("decision claim")
                };
                let run = reserved_run(&store, &task).await;
                let publish = {
                    let (store, id, version, claim) = (
                        store.clone(),
                        flow.id().to_owned(),
                        flow.version,
                        claim.clone(),
                    );
                    move |run: &String| {
                        store.sqlite.publish_attempt(
                            &id,
                            version,
                            store.sqlite.captured_sequence(run).unwrap().unwrap(),
                            Some(&claim),
                            "proof",
                            None,
                        )
                    }
                };
                let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
                    crate::run_record::RunSpec {
                        harness: "proof".into(),
                        model: None,
                        surface: "headless".into(),
                        cwd: task.worktree.clone(),
                        repo: None,
                        worktree: None,
                        skill: Some("loop-decide".into()),
                        subjects: vec![],
                        flow: crate::run_record::RunFlowMembership::Step(
                            crate::run_record::RunFlowStep::of(&flow).unwrap(),
                        ),
                        work: None,
                    },
                    run.clone(),
                    None,
                    &crate::trace::PreparedTurnContext::from_prompts("system", "decide"),
                    publish,
                )
                .unwrap();
                capture.record_input("initial", "Fixture decision is active");
                std::env::set_var(crate::durable::RUN_ID_ENV, run.as_str());
                std::env::set_var(crate::run_record::RUN_DIR_ENV, capture.artifact_dir());
                let before = store.task_flow(&task.id).await.unwrap().unwrap();
                let key = crate::ops::human_session::task_unblock_key(&before).unwrap();
                let ask = async {
                    crate::ops::human_session::ask_once(
                        &store,
                        &key,
                        "Choose the consumer to replace",
                        Some("unblock"),
                    )
                    .await
                    .unwrap_or_else(|error| panic!("live decision Ask failed: {error:#}"))
                };
                let human = async {
                    let session = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                        loop {
                            let sessions = crate::ops::human_session::list(
                                &store,
                                &crate::session::SessionFilter::default(),
                            )
                            .await
                            .unwrap();
                            if let Some(session) = sessions
                                .into_iter()
                                .find(|s| s.kind == crate::ops::human_session::SessionKind::Ask)
                            {
                                break session;
                            }
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                    })
                    .await
                    .unwrap();
                    let (execution, graph) =
                        crate::ops::task_execution::task_execution_and_flow(&store, &task.id)
                            .await
                            .unwrap();
                    assert_eq!(
                        execution.state,
                        crate::ops::task_execution::TaskExecutionState::Blocked
                    );
                    assert_eq!(
                        execution.captured,
                        store.sqlite.captured_sequence(&run).unwrap()
                    );
                    assert!(execution.reason.contains(&session.id));
                    assert!(execution.reason.contains("without Task resume"));
                    let crate::ops::task_flow::TaskFlowRecord::Pinned(graph) = graph else {
                        panic!("pinned")
                    };
                    assert_eq!(graph.reason, execution.reason);
                    assert_eq!(graph.execution, execution.state);
                    assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), before);
                    std::env::set_var(
                        crate::durable::RUN_ID_ENV,
                        store
                            .session(&session.id)
                            .await
                            .unwrap()
                            .unwrap()
                            .artifact_key
                            .as_str(),
                    );
                    std::env::set_var(
                        crate::ops::human_session::HUMAN_SESSION_ENV,
                        serde_json::to_string(&crate::ops::human_session::HumanSessionToken::Ask {
                            id: session.id.clone(),
                        })
                        .unwrap(),
                    );
                    crate::ops::human_session::mark_ready(&store, "Switch the existing reader")
                        .await
                        .unwrap();
                    crate::ops::human_session::complete(&store, &session.id)
                        .await
                        .unwrap();
                };
                let (feedback, ()) = tokio::join!(ask, human);
                assert_eq!(feedback, "Switch the existing reader");
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), before);
                assert_eq!(
                    crate::ops::task_execution::task_execution(&store, &task.id)
                        .await
                        .unwrap()
                        .state,
                    crate::ops::task_execution::TaskExecutionState::Running
                );
                assert!(before.cursor.leaf().progress.verdict.is_none());
                std::env::remove_var(crate::durable::RUN_ID_ENV);
                std::env::remove_var(crate::run_record::RUN_DIR_ENV);
                std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
                Ok(())
            })
        })
        .unwrap();
    }

    #[test]
    fn task_decision_driver_failures_open_one_unblock_and_reassess_feedback() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let repo = tempfile::tempdir().unwrap();
        for failure in ["missing", "malformed", "disconnected"] {
            let guard = super::TestLfBinGuard::pin();
            crate::journal::with_runtime(repo.path(), &["decision-failure-proof".into()], || {
            runtime.block_on(async {
            use crate::pm::test_server::{self, json_response};
            use serde_json::json;
                let (store, task, _) = human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
                let (url, _) = test_server::spawn((0..3).map(|_| json_response(
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
                let flow = pursue_decision(&task);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                let context = crate::ops::pm::PmTestContext {
                    path: guard.ledger.home().join("fixture.db"), store: store.clone(), graphql_url: url,
                };
                let create: crate::harness::CreateHarness = Box::new({
                    let store = store.clone(); let task_id = task.id.clone(); let seen = seen.clone();
                    move |_, _, events| Ok(Box::new(SliceHarness {
                        store: store.clone(), task_id: task_id.clone(), events,
                        seen: seen.clone(), restart: false, session_ready: false, failure: Some(failure), expected_input: None,
                    }))
                });
                let owner = TaskWorkerOwner {
                    trace_id: TraceId::new(), exec_id: ExecId::new(),
                    pid: std::process::id(), started_at: 1_700_000_000,
                };
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("decision claim") };
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                let error = crate::ops::pm::PM_TEST_CONTEXT.scope(context,
                    super::drive_task(store.clone(), task.id.clone(), claim, create),
                ).await.unwrap_err();
                assert!(error.to_string().contains(if failure == "disconnected" { "disconnected" } else { "structured output validation exhausted" }), "{error:#}");
                let blocked = store.task_flow(&task.id).await.unwrap().unwrap();
                let run = seen.lock().unwrap().last().unwrap().1.clone();
                assert_eq!(blocked.failure.as_ref().unwrap().captured, store.sqlite.captured_sequence(&run).unwrap());
                // A completed Run whose step wanted a decision fails the step, not
                // the Run; a disconnected provider fails the Run itself.
                assert_eq!(
                    store.sqlite.input_history(run.as_str()).unwrap().recorded_outcome.unwrap(),
                    if failure == "disconnected" { "failed" } else { "completed" }
                );
                assert!(blocked.claim.is_none());
                assert_eq!(blocked.cursor.index, flow.cursor.index);
                let (execution, graph) = crate::ops::task_execution::task_execution_and_flow(&store, &task.id).await.unwrap();
                assert_eq!(execution.state, crate::ops::task_execution::TaskExecutionState::Blocked);
                assert_eq!(execution.captured, store.sqlite.captured_sequence(&run).unwrap());
                let crate::ops::task_flow::TaskFlowRecord::Pinned(graph) = graph else { panic!("pinned Flow") };
                assert_eq!(graph.execution, execution.state);
                assert_eq!(graph.reason, execution.reason);
                let sessions = crate::ops::human_session::list(&store, &crate::session::SessionFilter::default()).await.unwrap();
                let sessions = sessions.iter().filter(|s| s.work.as_ref() == Some(&WorkRef::Task(task.id.clone()))).collect::<Vec<_>>();
                assert_eq!(sessions.len(), 1, "the driver must open unblock before any retry");
                let (session, feedback) = crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap();
                assert!(feedback.is_none());
                assert_eq!(crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap().0, session);
                assert_eq!(sessions[0].id, session);
                std::env::set_var(crate::durable::RUN_ID_ENV, store.session(&sessions[0].id).await.unwrap().unwrap().artifact_key.as_str());
                std::env::set_var(crate::ops::human_session::HUMAN_SESSION_ENV,
                    serde_json::to_string(&crate::ops::human_session::HumanSessionToken::Ask { id: session.clone() }).unwrap());
                let feedback = "Capability repaired; reassess the evidence before choosing navigation";
                crate::ops::human_session::mark_ready(&store, feedback).await.unwrap();
                crate::ops::human_session::complete(&store, &session).await.unwrap();
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), blocked);
                let (_, returned) = crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap();
                assert_eq!(returned.as_deref(), Some(feedback));
                let retried = store.retry_flow(&blocked.invocation.id, returned.as_deref()).await.unwrap();
                assert_eq!(retried.cursor.index, blocked.cursor.index);
                assert!(retried.cursor.leaf().progress.verdict.is_none());
                assert_eq!(retried.cursor.leaf().progress.direction.as_deref(), Some(feedback));
                assert!(retried.failure.is_none());
                let wave = super::owning_wave(&store, &task).await.unwrap();
                let prepared = super::prepare_task_flow_step(&store, &task, wave.name(), &retried).await.unwrap();
                assert!(prepared.turn.input.contains(feedback));
                std::env::remove_var(crate::durable::RUN_ID_ENV);
                std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
            Ok(())
            })
            }).unwrap();
        }
    }

    #[test]
    fn task_decision_public_resume_preserves_feedback_after_adoption_refusal() {
        let guard = super::TestLfBinGuard::pin();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let repo = tempfile::tempdir().unwrap();
        crate::journal::with_runtime(repo.path(), &["decision-failure-proof".into()], || {
        runtime.block_on(async {
            use crate::pm::test_server::{self, json_response};
            use serde_json::json;
            let failure = "missing";
                let (store, task, _) = human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
                store.set_task_agent(&task.id, "claude:sonnet").await.unwrap();
                let (url, _) = test_server::spawn((0..3).map(|_| json_response(
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
                let flow = pursue_decision(&task);
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                let context = crate::ops::pm::PmTestContext {
                    path: guard.ledger.home().join("fixture.db"), store: store.clone(), graphql_url: url,
                };
                let create: crate::harness::CreateHarness = Box::new({
                    let store = store.clone(); let task_id = task.id.clone(); let seen = seen.clone();
                    move |_, _, events| Ok(Box::new(SliceHarness {
                        store: store.clone(), task_id: task_id.clone(), events,
                        seen: seen.clone(), restart: false, session_ready: false, failure: Some(failure), expected_input: None,
                    }))
                });
                let owner = TaskWorkerOwner {
                    trace_id: TraceId::new(), exec_id: ExecId::new(),
                    pid: std::process::id(), started_at: 1_700_000_000,
                };
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("decision claim") };
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                let error = crate::ops::pm::PM_TEST_CONTEXT.scope(context,
                    super::drive_task(store.clone(), task.id.clone(), claim, create),
                ).await.unwrap_err();
                assert!(error.to_string().contains(if failure == "disconnected" { "disconnected" } else { "structured output validation exhausted" }), "{error:#}");
                let blocked = store.task_flow(&task.id).await.unwrap().unwrap();
                let run = seen.lock().unwrap().last().unwrap().1.clone();
                assert_eq!(blocked.failure.as_ref().unwrap().captured, store.sqlite.captured_sequence(&run).unwrap());
                assert_eq!(store.sqlite.input_history(run.as_str()).unwrap().recorded_outcome.unwrap(), "completed");
                assert!(blocked.claim.is_none());
                assert_eq!(blocked.cursor.index, flow.cursor.index);
                let (execution, graph) = crate::ops::task_execution::task_execution_and_flow(&store, &task.id).await.unwrap();
                assert_eq!(execution.state, crate::ops::task_execution::TaskExecutionState::Blocked);
                assert_eq!(execution.captured, store.sqlite.captured_sequence(&run).unwrap());
                let crate::ops::task_flow::TaskFlowRecord::Pinned(graph) = graph else { panic!("pinned Flow") };
                assert_eq!(graph.execution, execution.state);
                assert_eq!(graph.reason, execution.reason);
                let sessions = crate::ops::human_session::list(&store, &crate::session::SessionFilter::default()).await.unwrap();
                let sessions = sessions.iter().filter(|s| s.work.as_ref() == Some(&WorkRef::Task(task.id.clone()))).collect::<Vec<_>>();
                assert_eq!(sessions.len(), 1, "the driver must open unblock before any retry");
                let (session, feedback) = crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap();
                assert!(feedback.is_none());
                assert_eq!(crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap().0, session);
                assert_eq!(sessions[0].id, session);
                let error = crate::ops::task::continue_task_async(&task.plan.identifier, None, None, None, false).await.unwrap_err();
                assert!(error.to_string().contains("Complete unblock Session"), "{error}");
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), blocked);
                std::env::set_var(crate::durable::RUN_ID_ENV, store.session(&sessions[0].id).await.unwrap().unwrap().artifact_key.as_str());
                std::env::set_var(crate::ops::human_session::HUMAN_SESSION_ENV,
                    serde_json::to_string(&crate::ops::human_session::HumanSessionToken::Ask { id: session.clone() }).unwrap());
                let feedback = "Capability repaired; reassess the evidence before choosing navigation";
                crate::ops::human_session::mark_ready(&store, feedback).await.unwrap();
                crate::ops::human_session::complete(&store, &session).await.unwrap();
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), blocked);
                let (_, returned) = crate::ops::human_session::task_unblock(&store, &task, &blocked).await.unwrap();
                assert_eq!(returned.as_deref(), Some(feedback));
                std::env::remove_var(crate::durable::RUN_ID_ENV);
                std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
                let reason = "Keep the published API while replacing the reader";
                let error = crate::ops::linear_observe::tests::with_posted_comment(
                    &store, &task, reason,
                    crate::ops::task::continue_task_async(&task.plan.identifier, Some(reason.into()), None, None, false),
                ).await.unwrap_err();
                assert!(error.to_string().contains("active PR expects branch"), "{error}");
                let retried = store.task_flow(&task.id).await.unwrap().unwrap();
                assert_eq!(retried.cursor.index, blocked.cursor.index);
                assert!(retried.cursor.leaf().progress.verdict.is_none());
                assert_eq!(retried.cursor.leaf().progress.direction.as_deref(), Some(feedback));
                assert!(retried.failure.is_none());
                assert!(store.task_steers(&task.id).await.unwrap().iter().any(|s| s.text.contains(reason)));
                assert!(store.sqlite.test_decision_output(&ExecId::new(), &crate::engine::transitions::FlowVerdict {
                    decision: crate::engine::transitions::FlowDecision::Advance,
                    summary: "late failed Run must not navigate".into(),
                }).is_err());
                assert!(std::process::Command::new("git").current_dir(&task.worktree)
                    .args(["checkout", "-b", "test/human-task-proof"]).output().unwrap().status.success());

                // Only provider/PM/session effects are simulated. Public resume owns
                // adoption, reconciliation, account selection and the worker claim.
                let account_home = guard.ledger.home().join("accounts/claude/fixture-account");
                std::fs::create_dir_all(&account_home).unwrap();
                std::fs::write(account_home.join(".credentials.json"),
                    r#"{"claudeAiOauth":{"accessToken":"fixture-token","expiresAt":4102444800000}}"#).unwrap();
                let account = crate::provider_account::new_account(
                    crate::provider_auth::Provider::Claude,
                    crate::store::ProviderAccountId::parse("fixture-account").unwrap(), account_home, None);
                store.upsert_provider_account(&account).await.unwrap();
                store.set_provider_route(&crate::profile::ProviderRoute {
                    scope: crate::profile::RouteScope::Default, provider: crate::provider_auth::Provider::Claude,
                    accounts: vec![account.account_id.clone()], created_at: 1, updated_at: 1,
                }).await.unwrap();
                let (launch_tx, mut launch_rx) = tokio::sync::mpsc::unbounded_channel();
                let resumed = std::sync::Arc::new(tokio::sync::Notify::new());
                let resume = async {
                    let result = crate::ops::TEST_TASK_LAUNCH.scope(launch_tx,
                        crate::ops::task::continue_task_async(&task.plan.identifier, None, None, None, false)).await;
                    resumed.notify_one();
                    result
                };
                let worker = async {
                    let launch = launch_rx.recv().await.unwrap();
                    let claim = launch.environment.iter().find(|(key, _)| key == crate::durable::TASK_WORKER_CLAIM_ENV).unwrap();
                    let claim = serde_json::from_str(&claim.1).unwrap();
                    std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
                    let create: crate::harness::CreateHarness = Box::new({
                        let store = store.clone(); let task_id = task.id.clone(); let seen = seen.clone(); let resumed = resumed.clone();
                        move |_, _, events| Ok(Box::new(SliceHarness {
                            store: store.clone(), task_id: task_id.clone(), events, seen: seen.clone(),
                            restart: false, session_ready: false, failure: Some("missing"), expected_input: Some((vec![feedback, reason], resumed.clone())),
                        }))
                    });
                    let (url, _) = test_server::spawn(vec![json_response(axum::http::StatusCode::OK,
                        json!({"data": {"issue": {"updatedAt": "2026-09-24T00:00:00Z", "title": task.plan.title,
                        "description": task.plan.description, "comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}}}}))]).await;
                    crate::ops::pm::PM_TEST_CONTEXT.scope(crate::ops::pm::PmTestContext {
                        path: guard.ledger.home().join("loopflow.db"), store: store.clone(), graphql_url: url,
                    }, super::drive_task(store.clone(), task.id.clone(), claim, create)).await.unwrap_err()
                };
                let (resumed, error) = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                    tokio::join!(resume, worker)
                }).await.unwrap();
                resumed.unwrap();
                assert!(error.to_string().contains("structured output validation exhausted"), "{error:#}");
                let turns = seen.lock().unwrap().clone();
                assert_eq!(turns.len(), 4);
                assert_ne!(turns[0].1, turns[1].1);
                let (_, manifest) = crate::run_record::resolve_manifest(guard.ledger.home(), turns[1].1.as_str()).unwrap();
                assert_eq!(manifest.harness, "claude");
                assert_eq!(manifest.model.as_deref(), Some("sonnet"));
                let final_position = store.task_flow(&task.id).await.unwrap().unwrap();
                assert_eq!(final_position.cursor.index, blocked.cursor.index);
                assert_eq!(final_position.invocation.id, blocked.invocation.id);
                assert!(final_position.cursor.leaf().progress.verdict.is_none());
                std::env::remove_var(crate::durable::RUN_ID_ENV);
                std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
            Ok(())
        })
        }).unwrap();
    }

    #[test]
    #[ignore = "requires a configured Codex account and an explicitly selected source lf"]
    fn task_decision_live_policy_blocks_two_empty_replacement_passes() {
        let binary = std::env::var_os("LOOPFLOW_LIVE_TASK_LF").expect("source lf path");
        let account_home =
            std::env::var_os("LOOPFLOW_LIVE_CODEX_HOME").expect("configured Codex Home");
        let output = std::path::PathBuf::from(
            std::env::var_os("LOOPFLOW_LIVE_TASK_OUTPUT").expect("evidence directory"),
        );
        assert!(!output.exists(), "retain previous live evidence");
        let guard = super::TestLfBinGuard::pin();
        std::env::set_var("LF_BIN", &binary);
        std::env::set_var("LF_HOME", &output);
        guard.ledger.set_db_path(output.join("loopflow.db"));
        std::fs::create_dir_all(&output).unwrap();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (store, mut task, _) =
            runtime.block_on(human_task_fixture_at(&output.join("loopflow.db")));
        assert!(std::process::Command::new("git")
            .current_dir(&task.worktree)
            .args(["checkout", "-b", "test/human-task-proof"])
            .output()
            .unwrap()
            .status
            .success());
        task.plan.title = "Synthetic convergence policy acceptance".into();
        task.plan.description = "Switch consumer.display to candidate.name and delete legacy.py. Two recorded passes have replaced nothing. Inspect their executed proof and semantic replacement evidence using the captured loop-decide skill. This is a disposable fixture; change no other checkout or Task.".into();
        runtime.block_on(store.update_task(&task)).unwrap();
        runtime
            .block_on(store.set_task_agent(&task.id, "codex"))
            .unwrap();
        let scratch = task.worktree.join("scratch");
        std::fs::create_dir_all(&scratch).unwrap();
        std::fs::write(scratch.join("design.md"), &task.plan.description).unwrap();
        std::fs::write(
            task.worktree.join("legacy.py"),
            "def read_name(record):\n    return record['name']\n",
        )
        .unwrap();
        std::fs::write(
            task.worktree.join("consumer.py"),
            "from legacy import read_name\n\ndef display(record):\n    return read_name(record)\n",
        )
        .unwrap();
        std::fs::write(
            task.worktree.join("candidate.py"),
            "def name(record):\n    return record['name']\n",
        )
        .unwrap();
        let proof = || {
            let result = std::process::Command::new("uv")
                .current_dir(&task.worktree)
                .args([
                    "run",
                    "python",
                    "-c",
                    "from consumer import display; assert display({'name': 'Ada'}) == 'Ada'",
                ])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
        };
        proof();
        std::fs::write(
            task.worktree.join("adapter.py"),
            "from candidate import name\n\ndef unused_name(record):\n    return name(record)\n",
        )
        .unwrap();
        proof();
        std::fs::write(scratch.join("passes.md"), "# Synthetic fixture passes\n\nPass 1: candidate.py added +2/-0 non-test lines. Executed `uv run python -c \"from consumer import display; assert display({'name': 'Ada'}) == 'Ada'\"`: exit 0. No consumer switched, no predecessor deleted. Direction: switch consumer.display and delete legacy.py.\n\nPass 2: adapter.py added +4/-0 non-test lines. The same command executed: exit 0. No consumer switched, no predecessor deleted. consumer.py still imports legacy.read_name; candidate.name and adapter.unused_name have no real consumer. No changed requirement or environment blocker. The accepted target remains unmet.\n").unwrap();
        let definitions = task.worktree.join(".lf/flows");
        std::fs::create_dir_all(&definitions).unwrap();
        std::fs::write(definitions.join("convergence-proof.yaml"), "- step:\n    id: work\n    name: implement\n- step:\n    id: decide\n    name: loop-decide\n    repeat:\n      from: work\n").unwrap();
        let result = crate::journal::with_runtime(
            &task.worktree,
            &["managed-policy-proof".into()],
            || {
                runtime.block_on(async {
                use crate::pm::test_server::{self, json_response};
                use serde_json::json;
                let account = crate::provider_account::new_account(
                    crate::provider_auth::Provider::Codex,
                    crate::store::ProviderAccountId::parse("live-proof").unwrap(), account_home.into(), None);
                store.upsert_provider_account(&account).await.unwrap();
                // Prove the actual provider tool shell before asking it to judge
                // policy. No absolute lf command or supplied verdict can pass.
                let (probe_tx, mut probe_rx) = tokio::sync::mpsc::unbounded_channel();
                let mut probe = crate::harness::default_create_harness("codex",
                    crate::harness::ApprovalPolicy::AutoApprove, probe_tx).unwrap();
                probe.set_provider_account_id(Some(account.account_id.clone()));
                probe.start(&AgentConfig {
                    agent: Some("codex".into()), cwd: Some(task.worktree.clone()),
                    skip_permissions: true,
                    ..AgentConfig::default()
                }).await.unwrap();
                probe.send_input("Use your actual shell tool to execute `command -v lf > scratch/tool-executable.txt`, `lf task status TEST-1 --json > scratch/tool-status.json`, and `uv run python -c 'import os,json; from pathlib import Path; Path(\"scratch/tool-context.json\").write_text(json.dumps({k:os.environ.get(k) for k in [\"LF_HOME\",\"LF_DB_PATH\"]}))'`. Do not alter environment variables, use an absolute lf command, create wrappers, or repair failures. Report results and stop. This is only a disposable shell routing probe; take no Flow action.").await.unwrap();
                let mut probe_events = std::fs::File::create(output.join("shell-probe.jsonl")).unwrap();
                let probe_result = tokio::time::timeout(std::time::Duration::from_secs(120), async {
                    while let Some(event) = probe_rx.recv().await {
                        serde_json::to_writer(&mut probe_events, &event).unwrap();
                        std::io::Write::write_all(&mut probe_events, b"\n").unwrap();
                        match event {
                            crate::chat::types::ConversationEvent::TurnCompleted { status, .. } => {
                                assert_eq!(status, Lifecycle::Completed);
                                return;
                            }
                            crate::chat::types::ConversationEvent::Error { message, .. } => panic!("shell probe: {message}"),
                            _ => {}
                        }
                    }
                    panic!("shell probe disconnected");
                }).await;
                probe.stop().await.unwrap();
                probe_result.expect("shell probe exceeded two minutes");
                let executable = std::fs::read_to_string(scratch.join("tool-executable.txt")).unwrap();
                assert_eq!(std::fs::canonicalize(executable.trim()).unwrap(), std::fs::canonicalize(&binary).unwrap());
                let context: serde_json::Value = serde_json::from_slice(&std::fs::read(scratch.join("tool-context.json")).unwrap()).unwrap();
                assert_eq!(context["LF_HOME"], output.to_str().unwrap());
                assert_eq!(context["LF_DB_PATH"], output.join("loopflow.db").to_str().unwrap());
                let status: serde_json::Value = serde_json::from_slice(&std::fs::read(scratch.join("tool-status.json")).unwrap()).unwrap();
                assert_eq!(status["agent"], "codex");
                println!("Provider shell read TEST-1: {}; Home {}; database {}", executable.trim(), output.display(), output.join("loopflow.db").display());
                store.upsert_provider_token(&crate::store::ProviderToken {
                    provider: "linear".into(), access_token: "fixture-token".into(),
                    refresh_token: None, oauth_client_id: None, expires_at: None, login: None,
                    updated_at: 1, credential_type: crate::store::CredentialType::OAuth,
                }).await.unwrap();
                let (url, _) = test_server::spawn(vec![json_response(axum::http::StatusCode::OK,
                    json!({"data": {"issue": {"updatedAt": "2026-09-27T00:00:00Z", "title": task.plan.title,
                    "description": task.plan.description, "comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}}}}))]).await;
                let mut flow = super::start_task_flow(&task, "convergence-proof").unwrap();
                flow.cursor.index = 1;
                let flow = store.start_task_flow(&task.id, flow).await.unwrap();
                let owner = crate::journal::current_process_identity().unwrap();
                let TaskWorkerClaimOutcome::Claimed(claim) = store.claim_task_worker(
                    &task.id, &flow.invocation.id, flow.version, &owner, time::OffsetDateTime::now_utc(),
                ).await.unwrap() else { panic!("live decision claim") };
                std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "live-proof");
                let drive = crate::ops::pm::PM_TEST_CONTEXT.scope(crate::ops::pm::PmTestContext {
                    path: output.join("loopflow.db"), store: store.clone(), graphql_url: url,
                }, super::drive_task(store.clone(), task.id.clone(), claim,
                    Box::new(crate::harness::default_create_harness)));
                let observe = async {
                    loop {
                        let sessions = crate::ops::human_session::list(&store, &crate::session::SessionFilter::default()).await.unwrap();
                        let sessions = sessions.into_iter().filter(|s| s.work.as_ref() == Some(&WorkRef::Task(task.id.clone()))).collect::<Vec<_>>();
                        if let Some(session) = sessions.first() {
                            assert_eq!(sessions.len(), 1);
                            let before = store.task_flow(&task.id).await.unwrap().unwrap();
                            assert!(before.failure.is_none(), "provider failure is not a policy judgment");
                            let execution = crate::ops::task_execution::task_execution(&store, &task.id).await.unwrap();
                            assert_eq!(execution.state, crate::ops::task_execution::TaskExecutionState::Blocked);
                            assert_eq!(before.cursor.index, 1);
                            assert!(before.cursor.leaf().progress.verdict.is_none());
                            std::fs::write(scratch.join("blocked.json"), serde_json::to_vec_pretty(&execution).unwrap()).unwrap();
                            std::env::set_var(crate::durable::RUN_ID_ENV, store.session(&session.id).await.unwrap().unwrap().artifact_key.as_str());
                            std::env::set_var(crate::ops::human_session::HUMAN_SESSION_ENV,
                                serde_json::to_string(&crate::ops::human_session::HumanSessionToken::Ask { id: session.id.clone() }).unwrap());
                            crate::ops::human_session::mark_ready(&store, "Synthetic fixture cleanup: the actual policy chose Blocked and the managed Task projected it with one unblock Session. This fixture objective is complete; advance to end this disposable Flow. This is not Jack approval or LOO-319 acceptance.").await.unwrap();
                            let completion = tokio::process::Command::new(&binary)
                                .args(["session", "complete", &session.id])
                                .current_dir(&task.worktree).output().await.unwrap();
                            assert!(completion.status.success(), "{}", String::from_utf8_lossy(&completion.stderr));
                            assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), before);
                            std::env::remove_var(crate::durable::RUN_ID_ENV);
                            std::env::remove_var(crate::ops::human_session::HUMAN_SESSION_ENV);
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    }
                };
                tokio::time::timeout(std::time::Duration::from_secs(300), async {
                    tokio::try_join!(drive, async { observe.await; Ok(()) })
                }).await.map_err(|_| anyhow::anyhow!("managed policy proof exceeded five minutes"))??;
                assert!(store.task_flow(&task.id).await.unwrap().is_none());
                Ok(())
            })
            },
        );
        println!(
            "Managed policy evidence: {}; fixture worktree: {}",
            output.display(),
            task.worktree.display()
        );
        result.unwrap();
    }

    #[async_trait::async_trait]
    impl Harness for UnusedHarness {
        async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            Ok(())
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

    async fn human_task_fixture() -> (SharedStore, Task, FlowSession) {
        let (store, task, position, _) = human_task_fixture_with_database().await;
        (store, task, position)
    }

    async fn human_task_fixture_with_database(
    ) -> (SharedStore, Task, FlowSession, std::path::PathBuf) {
        let database = tempfile::tempdir().unwrap().keep().join("registry.db");
        let (store, task, position) = human_task_fixture_at(&database).await;
        (store, task, position, database)
    }

    async fn human_task_fixture_at(database: &std::path::Path) -> (SharedStore, Task, FlowSession) {
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
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.to_path_buf()))
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
                flow: "feature".into(),
                status: crate::pm::ProjectStatus::Started,
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
            agent: None,
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
        let guard = super::TestLfBinGuard::pin();
        let (store, task, _) =
            human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let initial = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &initial, 301).await;
        std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
        let linear = linear_stub(&store, &task, guard.ledger.home()).await;

        // The provider stream closes as soon as the turn is sent: a retryable
        // failure on a non-decision step releases the position.
        let error = crate::ops::pm::PM_TEST_CONTEXT
            .scope(
                linear,
                super::drive_task(store.clone(), task.id.clone(), held, closing_harness()),
            )
            .await
            .unwrap_err();

        assert!(
            error.to_string().contains("provider event stream closed"),
            "{error:#}"
        );
        assert!(store
            .task_events_after(&task.id, 0)
            .await
            .unwrap()
            .is_empty());
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(position.claim.is_none());
        assert!(position.failure.is_none());
        assert!(position.current_attempt.is_none());
        assert_eq!(position.cursor, initial.cursor);
        assert!(store.task_started(&task.id).await.unwrap());
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
        let guard = super::TestLfBinGuard::pin();
        let (store, task, mut flow) =
            human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
        flow.cursor.index = 0;
        let position = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &position, 302).await;
        std::env::set_var(crate::ops::TASK_ACCOUNT_ID_ENV, "fixture-account");
        let linear = linear_stub(&store, &task, guard.ledger.home()).await;
        let create: crate::harness::CreateHarness =
            Box::new(|_, _, _| anyhow::bail!("provider stream closed"));

        let error = crate::ops::pm::PM_TEST_CONTEXT
            .scope(
                linear,
                super::drive_task(store.clone(), task.id.clone(), held, create),
            )
            .await
            .unwrap_err();

        assert!(error.to_string().contains("provider stream closed"));
        assert!(store
            .recent_task_events(&task.id, 10)
            .await
            .unwrap()
            .is_empty());
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(position.claim.is_none());
        assert!(position.failure.is_none());
    }

    #[tokio::test]
    async fn posted_direction_on_an_idle_task_does_not_start_advancement() {
        let (store, task, _) = human_task_fixture().await;
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        let id = crate::ops::linear_observe::tests::with_posted_comment(
            &store,
            &task,
            "keep the API",
            crate::ops::linear_observe::publish_task_steer(&store, &task, "keep the API"),
        )
        .await
        .unwrap();
        assert_eq!(id, "comment-1");
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
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
                    created_at: None,
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
            .start_task_flow(&task.id, autonomous.clone())
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

        let stopped = store
            .release_flow(
                &stale_claim.invocation_id,
                stale_claim.position_version,
                Some(&stale_claim),
            )
            .await
            .unwrap();
        store
            .restart_task_flow(&task, Some(&stopped), "deadbeef")
            .await
            .unwrap();
        let replacement = store
            .start_task_flow(&task.id, human_flow.clone())
            .await
            .unwrap();

        let create_harness: crate::harness::CreateHarness =
            Box::new(|_, _, _| panic!("stale child must fail before creating a harness"));
        let error = super::drive_task(store.clone(), task.id.clone(), stale_claim, create_harness)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("launch claim is stale"));
        assert_eq!(store.task_flow(&task.id).await.unwrap(), Some(replacement));
    }

    #[tokio::test]
    async fn restart_only_failure_cannot_be_cleared_as_a_retry() {
        let (store, task, _) = human_task_fixture().await;
        let started = super::start_task_flow(&task, "task-design").unwrap();
        let started = store.start_task_flow(&task.id, started).await.unwrap();
        let blocked = store
            .fail_flow(
                started.id(),
                started.version,
                None,
                &crate::durable::TaskFlowBlocker {
                    captured: None,
                    reason: "old Flow definition is unavailable".to_string(),
                    restart_required: true,
                    observed_at: time::OffsetDateTime::now_utc(),
                },
            )
            .await
            .unwrap();

        let error = store.retry_flow(blocked.id(), None).await.unwrap_err();

        assert!(error.to_string().contains("explicit Flow restart"));
        assert_eq!(store.task_flow(&task.id).await.unwrap(), Some(blocked));
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
    #[allow(clippy::await_holding_lock)] // isolates review Run capture and executable resolution
    async fn task_agent_changed_during_worker_applies_to_next_review() {
        let _guard = super::TestLfBinGuard::pin();
        let (store, task, _) = human_task_fixture().await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let mut flow = store.start_task_flow(&task.id, flow).await.unwrap();
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
            panic!("fixture worker must acquire its claim");
        };
        store
            .set_task_agent(&task.id, "claude:sonnet")
            .await
            .unwrap();
        finish(&mut flow).unwrap();
        // The checkpoint onto the review releases the claim; parking reserves
        // the review Run with the Task's current choice.
        store
            .checkpoint_flow(
                flow.id(),
                flow.version,
                &flow.cursor,
                Some(&claim),
                Some("ready"),
            )
            .await
            .unwrap();
        let next = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(next.is_human());
        assert!(next.claim.is_none());
        let next = super::park_at_review(&store, &task, &next).await.unwrap();
        let review = store
            .session(&crate::ops::human_session::flow_id(&next).unwrap())
            .await
            .unwrap()
            .unwrap();
        assert!(!review.input_published);
        assert_eq!(
            (review.provider.as_deref(), review.model.as_deref()),
            (Some("claude"), Some("sonnet"))
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // isolates Run capture and executable resolution
    async fn task_agent_survives_refresh_and_selects_autonomous_and_human_steps() {
        let _guard = super::TestLfBinGuard::pin();
        let (store, task, mut flow) = human_task_fixture().await;
        std::fs::create_dir_all(task.worktree.join(".lf")).unwrap();
        std::fs::write(task.worktree.join(".lf/config.yaml"), "agent: codex\n").unwrap();
        let crate::engine::ConcreteStep::Skill(step) = &mut flow.invocation.steps[1] else {
            panic!("review skill");
        };
        step.skill.agent = Some("codex:step-model".into());
        step.skill.default_agent = Some("claude:haiku".into());

        store
            .set_task_agent(&task.id, "claude:sonnet")
            .await
            .unwrap();
        // A planning write made from an older snapshot cannot undo the choice.
        store.update_task(&task).await.unwrap();
        let resumed = store.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(resumed.agent.as_deref(), Some("claude:sonnet"));
        let prepared = super::prepare_task_flow_step(&store, &resumed, "human-task-proof", &flow)
            .await
            .unwrap();
        assert_eq!(prepared.turn.config.agent.as_deref(), Some("claude:sonnet"));
        assert_eq!(prepared.turn.harness, "claude");
        assert_eq!(prepared.turn.model.as_deref(), Some("sonnet"));
        assert_eq!(
            prepared.task_pr_id,
            store.active_task_pr(&task.id).await.unwrap().unwrap().id
        );
        // Parking at the review reserves its Run; the choice lands on that row.
        let mut flow = store.start_task_flow(&task.id, flow.clone()).await.unwrap();
        flow = store
            .reserve_task_review(flow.id(), flow.version)
            .await
            .unwrap();
        let review_id = crate::ops::human_session::flow_id(&flow).unwrap();
        let agent = crate::ops::human_session::select_review_agent(&store, &resumed, &flow)
            .await
            .unwrap();
        assert_eq!(agent, "claude:sonnet");
        let review = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(
            (review.provider.as_deref(), review.model.as_deref()),
            (Some("claude"), Some("sonnet"))
        );

        // A later choice re-targets the same unlaunched Run in place.
        store
            .set_task_agent(&task.id, "codex:replacement")
            .await
            .unwrap();
        crate::ops::human_session::retarget_prepared_task_review(&store, &task)
            .await
            .unwrap();
        let retargeted = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(retargeted.cursor, flow.cursor);
        let retargeted_run = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(retargeted_run.artifact_key, review.artifact_key);
        assert!(!retargeted_run.input_published);
        assert_eq!(
            (
                retargeted_run.provider.as_deref(),
                retargeted_run.model.as_deref()
            ),
            (Some("codex"), Some("replacement"))
        );
        // A launched Run keeps the provider it launched with.
        store
            .sqlite
            .publish_review_run(
                &review_id,
                store
                    .sqlite
                    .captured_sequence(&retargeted_run.artifact_key)
                    .unwrap()
                    .unwrap(),
                retargeted.version,
                "codex",
                Some("replacement"),
            )
            .unwrap();
        store.set_task_agent(&task.id, "claude:opus").await.unwrap();
        crate::ops::human_session::retarget_prepared_task_review(&store, &task)
            .await
            .unwrap();
        let published = store.session(&review_id).await.unwrap().unwrap();
        assert_eq!(published.model.as_deref(), Some("replacement"));

        // Without a Task choice, either form of captured step default beats config.
        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &flow)
            .await
            .unwrap();
        assert_eq!(
            prepared.turn.config.agent.as_deref(),
            Some("codex:step-model")
        );
        let crate::engine::ConcreteStep::Skill(step) = &mut flow.invocation.steps[1] else {
            unreachable!()
        };
        step.skill.agent = None;
        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &flow)
            .await
            .unwrap();
        assert_eq!(prepared.turn.config.agent.as_deref(), Some("claude:haiku"));
        let crate::engine::ConcreteStep::Skill(step) = &mut flow.invocation.steps[1] else {
            unreachable!()
        };
        step.skill.default_agent = None;
        let prepared = super::prepare_task_flow_step(&store, &task, "human-task-proof", &flow)
            .await
            .unwrap();
        assert_eq!(prepared.turn.config.agent.as_deref(), Some("codex"));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn interrupted_step_restarts_in_place_with_fresh_direction() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, mut flow) = human_task_fixture().await;
        let interrupted_step = flow.current().step.clone();

        assert!(!interrupt(&mut flow));
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
        let guard = super::TestLfBinGuard::pin();
        let (store, task, _) =
            human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
        let flow_dir = task.worktree.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::write(
            flow_dir.join("two-ops.yaml"),
            "- cmd: rebase --plan\n- cmd: rebase --plan\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "two-ops").unwrap();
        assert_eq!(flow.current().kind, StepKind::Command);
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &flow, 404).await;
        assert!(!store.task_started(&task.id).await.unwrap());
        let released = store
            .release_flow(flow.id(), flow.version, Some(&held))
            .await
            .unwrap();
        assert!(!store.task_started(&task.id).await.unwrap());
        let held = claim(&store, &task, &released, 405).await;
        let create: crate::harness::CreateHarness =
            Box::new(|_, _, _| panic!("an operation step needs no provider"));

        super::drive_task(store.clone(), task.id.clone(), held, create)
            .await
            .unwrap();

        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        let ended = store.flow(flow.id()).await.unwrap().unwrap();
        assert!(ended.finished);
        assert_eq!(ended.cursor.index, 2);
        assert_eq!(ended.cursor.iteration, 0);
        store.sqlite.assert_no_historical_runs();
        assert!(store.task_started(&task.id).await.unwrap());
        let conn = rusqlite::Connection::open(guard.ledger.home().join("loopflow.db")).unwrap();
        let results: Vec<(i64, String)> = conn
            .prepare("SELECT node,outcome FROM flow_events WHERE flow_id=?1 AND kind='operation_completed' ORDER BY seq")
            .unwrap()
            .query_map([flow.id()], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            results,
            vec![(0, "completed".into()), (1, "completed".into())]
        );
        assert!(conn
            .execute(
                "UPDATE tasks SET started_at=started_at+1 WHERE id=?1",
                [task.id.as_str()]
            )
            .is_err());
    }

    #[tokio::test]
    async fn shared_driver_parks_after_releasing_its_claim_at_a_review() {
        let guard = super::TestLfBinGuard::pin();
        let (store, task, _) =
            human_task_fixture_at(&guard.ledger.home().join("loopflow.db")).await;
        let flow_dir = task.worktree.join(".lf/flows");
        std::fs::create_dir_all(&flow_dir).unwrap();
        std::fs::write(
            flow_dir.join("op-then-review.yaml"),
            "- op: rebase --plan\n- step:\n    name: review-design\n    id: review\n    human: true\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "op-then-review").unwrap();
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &flow, 404).await;
        let create: crate::harness::CreateHarness =
            Box::new(|_, _, _| panic!("parking a review starts no provider"));
        super::drive_task(store.clone(), task.id.clone(), held, create)
            .await
            .unwrap();

        let parked = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(parked.is_human());
        assert!(parked.claim.is_none() && parked.failure.is_none());
        let session = store
            .session(parked.pending_session_id.as_ref().unwrap())
            .await
            .unwrap()
            .unwrap();
        let run = session.clone();
        assert_eq!(run.task_id, Some(task.id));
        assert_eq!(run.flow_session_id.as_deref(), Some(flow.id()));
        assert_eq!(session.artifact_key, run.artifact_key);
        assert!(!run.input_published);
    }

    #[tokio::test]
    async fn selecting_another_flow_replaces_only_the_managed_invocation() {
        let (store, task, _) = human_task_fixture().await;
        let original = super::ensure_flow_position(&store, &task.id, Some("code"))
            .await
            .unwrap();
        let contribution = super::start_task_flow(&task, "code").unwrap();
        let contribution = store.create_flow(contribution).await.unwrap();
        let replacement = super::ensure_flow_position(&store, &task.id, Some("pursue"))
            .await
            .unwrap();
        assert_ne!(original.id(), replacement.id());
        assert!(store.flow(original.id()).await.unwrap().unwrap().finished);
        assert_eq!(
            store.flow(contribution.id()).await.unwrap().unwrap(),
            contribution
        );
        assert_eq!(
            store.task_flow(&task.id).await.unwrap().unwrap(),
            replacement
        );
        assert_eq!(
            super::ensure_flow_position(&store, &task.id, Some("pursue"))
                .await
                .unwrap(),
            replacement
        );
        assert!(store
            .reserve_attempt(original.id(), original.version, None)
            .await
            .is_err());
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
            "- original-proof\n- cmd: rebase --plan\n",
        )
        .unwrap();
        std::fs::write(
            skill_dir.join("original-proof.md"),
            "# Original\n\nExecute the definition captured at Flow start.\n",
        )
        .unwrap();
        let flow = super::start_task_flow(&task, "persisted-proof").unwrap();
        store.start_task_flow(&task.id, flow.clone()).await.unwrap();

        std::fs::write(
            flow_dir.join("persisted-proof.yaml"),
            "- replacement-proof\n- cmd: doctor\n",
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

        let persisted = store.task_flow(&task.id).await.unwrap().unwrap();
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
        let crate::engine::ConcreteStep::Command(active_op) = &persisted.invocation.steps[1] else {
            panic!("active second step is an op")
        };
        assert_eq!(active_op.item.command, "rebase");
        assert_eq!(active_op.item.args, ["--plan"]);

        let future = super::start_task_flow(&task, "persisted-proof").unwrap();
        let crate::engine::ConcreteStep::Skill(future_skill) = future.current_plan() else {
            panic!("future first step is a skill")
        };
        assert_eq!(future_skill.skill.name, "replacement-proof");
        let crate::engine::ConcreteStep::Command(future_op) = &future.invocation.steps[1] else {
            panic!("future second step is an op")
        };
        assert_eq!(future_op.item.command, "doctor");
    }

    async fn park_human_task(
        store: &SharedStore,
        task: &Task,
        flow: &FlowSession,
    ) -> crate::ops::human_session::FlowSessionToken {
        if store.task_flow(&task.id).await.unwrap().is_none() {
            parked(store, task, flow.clone()).await;
        }
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        if position.is_human() && position.pending_session_id.is_none() {
            super::park_at_review(store, task, &position).await.unwrap();
        }
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let step = position.current();
        let crate::engine::ConcreteStep::Skill(planned) = position.current_plan() else {
            panic!("human position must select a Skill")
        };
        crate::ops::human_session::FlowSessionToken {
            task_id: task.id.clone(),
            invocation_id: position.invocation.id.clone(),
            flow: step.flow,
            node_id: step.id.unwrap(),
            skill: planned.skill.clone(),
            iteration: position.cursor.iteration,
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes fixture executable and Home.
    async fn restarting_a_human_node_reuses_the_same_task_position() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let original = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "ready").await;
        let persisted = store.task_flow(&task.id).await.unwrap().unwrap();
        let run_id = persisted.review_artifact_key().cloned().unwrap();
        let restarted_flow = super::ensure_flow_position(&store, &task.id, None)
            .await
            .unwrap();
        assert_eq!(restarted_flow, persisted);
        let recovered = park_human_task(&store, &task, &restarted_flow).await;

        assert_eq!(recovered, original);
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        let recovered = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(recovered.review_artifact_key(), Some(&run_id));
        assert_eq!(recovered.ready_summary.as_deref(), Some("ready"));
    }

    #[tokio::test]
    async fn review_actions_preserve_selected_attempt_across_replacement() {
        use crate::ops::human_session::action_test::{LookupPause, NativeClients};

        let _lf_bin = super::TestLfBinGuard::pin();
        for action in ["complete", "rename", "open"] {
            for selector_kind in ["run", "prefix", "session"] {
                let (store, task, flow) = human_task_fixture().await;
                let position = parked(&store, &task, flow).await;
                let id = human_session::flow_id(&position).unwrap();
                let (position, first) = store.reserve_review_run(&position).await.unwrap();
                let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
                    crate::run_record::RunSpec {
                        harness: "codex".into(),
                        model: None,
                        surface: "tui".into(),
                        cwd: task.worktree.clone(),
                        repo: None,
                        worktree: None,
                        skill: None,
                        subjects: Vec::new(),
                        flow: crate::run_record::RunFlowMembership::Step(
                            crate::run_record::RunFlowStep::of(&position).unwrap(),
                        ),
                        work: None,
                    },
                    first.artifact_key.clone(),
                    None,
                    &crate::trace::PreparedTurnContext::from_prompts("system", "review"),
                    |run| {
                        store.sqlite.publish_review_run(
                            &id,
                            store.sqlite.captured_sequence(run).unwrap().unwrap(),
                            position.version,
                            "codex",
                            None,
                        )
                    },
                )
                .unwrap();
                store
                    .ready_session(&id, first.captured, "Retain this feedback")
                    .await
                    .unwrap();
                let before = store.task_flow(&task.id).await.unwrap().unwrap();
                let original = store.session(&id).await.unwrap().unwrap();
                let selector = match selector_kind {
                    "run" => first.artifact_key.to_string(),
                    "prefix" => first.artifact_key.as_str()[..20].to_string(),
                    _ => id.clone(),
                };
                let clients = NativeClients::new(&id, std::slice::from_ref(&first.artifact_key));
                let pause = LookupPause::at(action, &selector);
                // Rename starts acquiring this lock after its initial lookup.
                // Replacement owns it until B and retained feedback are published.
                let replacement_lock = human_session::lock_session_launch(&id).unwrap();
                let request = async {
                    match action {
                        "complete" => human_session::complete(&store, &selector).await,
                        "rename" => {
                            human_session::rename(
                                &store,
                                &selector,
                                "Chosen conversation",
                                SessionTitleSource::Human,
                            )
                            .await
                        }
                        _ => {
                            human_session::open(
                                &store,
                                &selector,
                                human_session::OpenMode::Refuse,
                                true,
                            )
                            .await
                        }
                    }
                };
                let replace = async {
                    pause.reached.notified().await;
                    let (reserved, replacement) = store.reserve_review_run(&before).await.unwrap();
                    store
                        .sqlite
                        .publish_review_run(
                            &id,
                            store
                                .sqlite
                                .captured_sequence(&replacement.artifact_key)
                                .unwrap()
                                .unwrap(),
                            reserved.version,
                            "codex",
                            None,
                        )
                        .unwrap();
                    clients.add(&id, &replacement.artifact_key);
                    let position = store.task_flow(&task.id).await.unwrap().unwrap();
                    drop(replacement_lock);
                    pause.proceed.notify_one();
                    (position, replacement)
                };
                let (result, (expected, replacement)) =
                    tokio::time::timeout(std::time::Duration::from_secs(10), async {
                        tokio::join!(request, replace)
                    })
                    .await
                    .unwrap();
                if action == "rename" && selector_kind == "session" {
                    assert_eq!(result.unwrap().title, "Chosen conversation");
                } else {
                    assert!(
                        result.is_err(),
                        "{action} via {selector_kind} selected a replacement"
                    );
                    assert_eq!(
                        store.session(&id).await.unwrap().unwrap().title,
                        original.title
                    );
                }
                assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), expected);
                let session = store.session(&id).await.unwrap().unwrap();
                let current = session.clone();
                assert_eq!(current.artifact_key, replacement.artifact_key);
                assert_eq!(
                    session.ready_summary.as_deref(),
                    Some("Retain this feedback")
                );
                assert!(session.completed_at.is_none());
                assert_eq!(
                    clients.active(),
                    [first.artifact_key.clone(), replacement.artifact_key.clone()].into()
                );
                assert!(store
                    .rename_session(
                        &id,
                        first.captured,
                        "Stale direct write",
                        crate::session::TitleSource::Human,
                    )
                    .await
                    .is_err());

                // Native resume must hold the same lock through handoff, then release
                // it so Complete can settle and stop exactly B. A remains untouched.
                let opened = human_session::open(
                    &store,
                    replacement.artifact_key.as_str(),
                    human_session::OpenMode::Refuse,
                    true,
                )
                .await
                .unwrap();
                assert_eq!(opened.id, replacement.id);
                let completed = human_session::complete(&store, replacement.artifact_key.as_str())
                    .await
                    .unwrap();
                assert_eq!(completed.id, replacement.id);
                assert_eq!(clients.active(), [first.artifact_key.clone()].into());
                assert!(store.task_flow(&task.id).await.unwrap().is_none());
                let closed = store.session(&id).await.unwrap().unwrap();
                assert!(closed.completed_at.is_some());
                assert_eq!(
                    closed.ready_summary.as_deref(),
                    Some("Retain this feedback")
                );
                assert!(human_session::complete(&store, &id).await.is_err());
                let events = store.recent_task_events(&task.id, 20).await.unwrap();
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| matches!(
                            &event.kind, TaskEventKind::FlowFinished { summary, .. }
                                if summary == "Retain this feedback"
                        ))
                        .count(),
                    1
                );
                assert_eq!(store.session_inputs(&id).await.unwrap().len(), 2);
                drop(capture);
            }
        }
    }

    #[tokio::test]
    async fn flow_session_name_and_membership_survive_sql_run_replacement() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let position = parked(&store, &task, flow).await;
        let id = human_session::flow_id(&position).unwrap();
        let first = store.session(&id).await.unwrap().unwrap();
        let named = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Parser review",
            SessionTitleSource::Human,
        )
        .await
        .unwrap();
        assert_eq!(named.id, id);
        assert_eq!(
            store.session(&id).await.unwrap().unwrap().captured,
            first.captured
        );
        assert_eq!(named.work, Some(WorkRef::Task(task.id.clone())));
        assert!(matches!(
            &named.flow_membership,
            SessionFlowMembership::Step { invocation_id, .. }
                if invocation_id == &position.invocation.id
        ));

        let home = crate::store::observability_home_dir();
        let first_dir = crate::run_record::record_dir(&home, &first.artifact_key).unwrap();
        std::fs::create_dir_all(&first_dir).unwrap();
        let manifest = crate::run_record::RunManifest {
            schema_version: 1,
            run_id: first.artifact_key.clone(),
            parent_run_id: None,
            created_at: time::OffsetDateTime::now_utc(),
            harness: "codex".into(),
            model: None,
            surface: "tui".into(),
            cwd: task.worktree.clone(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            flow: Some(crate::run_record::RunFlowMembership::Step(
                crate::run_record::RunFlowStep::of(&position).unwrap(),
            )),
            launch: None,
            context: None,
            runtime_path: None,
            runtime_digest: None,
            host: "test".into(),
            boot_id: None,
        };
        std::fs::write(
            first_dir.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let (_, replacement) = store.reserve_review_run(&position).await.unwrap();
        let retained = human_session::rename(
            &store,
            replacement.artifact_key.as_str(),
            "Generated suggestion",
            SessionTitleSource::Generated,
        )
        .await
        .unwrap();
        assert_eq!(retained.id, id);
        assert_ne!(replacement.captured, first.captured);
        assert_eq!(
            store.session(&id).await.unwrap().unwrap().captured,
            replacement.captured
        );
        assert_eq!(retained.title, "Parser review");
        assert_eq!(retained.title_source, SessionTitleSource::Human);
        assert_eq!(retained.flow_membership, named.flow_membership);
        assert_eq!(retained.ready_summary.as_deref(), Some("Keep this answer"));
        let listed = human_session::list(&store, &crate::session::SessionFilter::default())
            .await
            .unwrap()
            .into_iter()
            .find(|session| session.id == id)
            .unwrap();
        assert_eq!(listed, retained);
        let history = store.session_inputs(&id).await.unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.contains(&first.artifact_key));
        assert!(history.contains(&replacement.artifact_key));
        let historical = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Old actor",
            SessionTitleSource::Human,
        )
        .await
        .unwrap_err();
        assert!(historical
            .to_string()
            .contains(&format!("historical attempt of Session {id}")));
        assert!(!first_dir.join("session-name.json").exists());
        let prefix = &first.artifact_key.as_str()[..16];
        assert!(
            human_session::rename(&store, prefix, "Old prefix", SessionTitleSource::Human)
                .await
                .unwrap_err()
                .to_string()
                .contains("historical attempt of Session")
        );
        assert!(human_session::open(
            &store,
            first.artifact_key.as_str(),
            human_session::OpenMode::Refuse,
            false
        )
        .await
        .is_err());
        assert!(human_session::complete(&store, first.artifact_key.as_str())
            .await
            .is_err());

        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let (_, third) = store.reserve_review_run(&position).await.unwrap();
        ready_review(&store, &task, "Keep this answer").await;
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(position.review_artifact_key(), Some(&third.artifact_key));
        assert!(store
            .ready_session(&id, first.captured, "stale answer")
            .await
            .is_err());
        let previous_actor = std::env::var_os("LF_RUN_ID");
        std::env::set_var("LF_RUN_ID", first.artifact_key.as_str());
        let stale_actor = human_session::require_current_review_actor(&store, &position).await;
        match previous_actor {
            Some(value) => std::env::set_var("LF_RUN_ID", value),
            None => std::env::remove_var("LF_RUN_ID"),
        }
        assert!(stale_actor.is_err());
        store
            .complete_task_review(&task.id, &position, "complete once")
            .await
            .unwrap();
        assert!(store
            .complete_task_review(&task.id, &position, "late completion")
            .await
            .is_err());
        let error = human_session::rename(
            &store,
            first.artifact_key.as_str(),
            "Closed actor",
            SessionTitleSource::Human,
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("historical attempt of Session {id}")));
        assert!(!first_dir.join("session-name.json").exists());
        assert!(human_session::rename(
            &store,
            third.artifact_key.as_str(),
            "Closed actor",
            SessionTitleSource::Human
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("already complete"));
        let closed = store.session(&id).await.unwrap().unwrap();
        let current = closed.clone();
        assert!(closed.completed_at.is_some());
        assert_eq!(closed.title, "Parser review");
        assert_eq!(closed.ready_summary.as_deref(), Some("Keep this answer"));
        assert_eq!(current.artifact_key, third.artifact_key);
        assert_eq!(store.session_inputs(&id).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn review_publication_retry_preserves_identity_and_claims_sql_once() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let position = parked(&store, &task, flow).await;
        let session_id = human_session::flow_id(&position).unwrap();
        let (reserved, run) = store.reserve_review_run(&position).await.unwrap();
        let spec = crate::run_record::RunSpec {
            harness: "codex".into(),
            model: None,
            surface: "tui".into(),
            cwd: task.worktree.clone(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
            flow: crate::run_record::RunFlowMembership::Step(
                crate::run_record::RunFlowStep::of(&reserved).unwrap(),
            ),
            work: None,
        };
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let interrupted = crate::run_record::CaptureHandle::begin_reserved_with_context(
            spec.clone(),
            run.artifact_key.clone(),
            None,
            &context,
            |_| {
                Err(crate::store::StoreError::InvalidAuthority(
                    "interrupted before SQL publication".into(),
                ))
            },
        );
        assert!(interrupted.is_err());
        let dir =
            crate::run_record::record_dir(&crate::store::authority_home_dir(), &run.artifact_key)
                .unwrap();
        let original = std::fs::read(dir.join("manifest.json")).unwrap();
        assert!(!dir.join("terminal.json").exists());
        let (retry, same_run) = store.reserve_review_run(&reserved).await.unwrap();
        assert_eq!(same_run, run);
        assert_eq!(retry, reserved);
        let capture = crate::run_record::CaptureHandle::begin_reserved_with_context(
            spec.clone(),
            run.artifact_key.clone(),
            None,
            &context,
            |id| {
                store.sqlite.publish_review_run(
                    &session_id,
                    store.sqlite.captured_sequence(id).unwrap().unwrap(),
                    retry.version,
                    "codex",
                    None,
                )
            },
        )
        .unwrap();
        assert_eq!(capture.run_id(), run.artifact_key);
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
        assert_eq!(store.session_inputs(&session_id).await.unwrap().len(), 1);
        assert!(
            store
                .session(&session_id)
                .await
                .unwrap()
                .unwrap()
                .input_published
        );
        assert!(
            crate::run_record::CaptureHandle::begin_reserved_with_context(
                spec,
                run.artifact_key.clone(),
                None,
                &context,
                |id| store.sqlite.publish_review_run(
                    &session_id,
                    store.sqlite.captured_sequence(id).unwrap().unwrap(),
                    retry.version,
                    "codex",
                    None
                ),
            )
            .is_err()
        );
        assert!(!dir.join("terminal.json").exists());
        // Publication alone does not establish that a provider started or died.
        // The real Open path must retain this uncertain attempt, not replace it.
        let error = human_session::open(&store, &session_id, human_session::OpenMode::Refuse, true)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("launch status is unresolved"));
        assert_eq!(
            store
                .session(&session_id)
                .await
                .unwrap()
                .unwrap()
                .artifact_key,
            run.artifact_key
        );
        assert_eq!(store.session_inputs(&session_id).await.unwrap().len(), 1);
        capture.finish("interrupted").unwrap();
    }

    #[tokio::test]
    async fn session_list_and_open_preserve_valid_reviews_beside_unreadable_captures() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow, database) = human_task_fixture_with_database().await;
        let position = parked(&store, &task, flow).await;
        let id = human_session::flow_id(&position).unwrap();
        let conn = rusqlite::Connection::open(database).unwrap();
        let mut broken_review_id = None;
        for human in [false, true] {
            let broken_id = TaskId::new();
            conn.execute(
                "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,
                issue_title,issue_description,pm_snapshot_synced_at,pm_writeback_json,
                worktree,workspace_slug,created_at,updated_at)
                SELECT ?1,project_id,?1,?1,issue_title,issue_description,
                pm_snapshot_synced_at,pm_writeback_json,?1,?1,created_at,updated_at
                FROM tasks WHERE id=?2",
                rusqlite::params![broken_id.as_str(), task.id.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO work_placements(task_id,home_id,enabled,placed_at)
                SELECT ?1,home_id,enabled,placed_at FROM work_placements WHERE task_id=?2",
                rusqlite::params![broken_id.as_str(), task.id.as_str()],
            )
            .unwrap();
            let mut broken = position.clone();
            broken.cursor = Default::default();
            broken.task_id = Some(broken_id.clone());
            broken.wave_id = None;
            broken.invocation = crate::durable::test_flow_invocation(
                "broken",
                0,
                "review-design",
                Some("review"),
                human,
            );
            broken.version = 0;
            broken.current_attempt = None;
            broken.pending_session_id = None;
            broken.ready_summary = None;
            let broken = store.start_task_flow(&broken_id, broken).await.unwrap();
            if human {
                store
                    .reserve_task_review(broken.id(), broken.version)
                    .await
                    .unwrap();
                broken_review_id = Some(human_session::flow_id(&broken).unwrap());
            }
            let corrupt = serde_json::json!({"id": broken.invocation.id, "flow": "broken", "steps": "unreadable"}).to_string();
            conn.execute(
                "UPDATE flow_sessions SET invocation_json=?2 WHERE task_id=?1",
                rusqlite::params![broken_id.as_str(), corrupt],
            )
            .unwrap();
            let records = human_session::list(&store, &crate::session::SessionFilter::default())
                .await
                .unwrap();
            assert!(records.iter().any(|record| record.id == id));
            let opened = human_session::open(&store, &id, human_session::OpenMode::Refuse, false)
                .await
                .unwrap();
            assert_eq!(opened.id, id);
            if let Some(broken_id) = &broken_review_id {
                let record = records
                    .iter()
                    .find(|record| &record.id == broken_id)
                    .unwrap();
                assert!(
                    matches!(&record.flow_membership, SessionFlowMembership::Step { flow, .. } if flow == "broken")
                );
                // Inventory exposes recorded metadata; exact open validates the capture.
                assert!(record.actions.iter().any(|action| action.kind
                    == human_session::SessionActionKind::Open
                    && action.unavailable_reason.is_none()));
                assert!(record.actions.iter().any(|action| action.kind
                    == human_session::SessionActionKind::Complete
                    && action.unavailable_reason.is_some()));
                assert!(human_session::open(
                    &store,
                    broken_id,
                    human_session::OpenMode::Refuse,
                    false
                )
                .await
                .unwrap_err()
                .to_string()
                .contains("saved Invocation is unreadable"));
            }
            let retained: String = conn
                .query_row(
                    "SELECT invocation_json FROM flow_sessions WHERE task_id=?1",
                    [broken_id.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(retained, corrupt);
        }
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // Serializes fixture executable and Home.
    async fn stale_human_decisions_cannot_target_a_replacement_invocation() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let stale = park_human_task(&store, &task, &flow).await;
        let mut replacement = store.task_flow(&task.id).await.unwrap().unwrap();
        let step = replacement.current();
        replacement.invocation = crate::durable::test_flow_invocation(
            &step.flow,
            replacement.cursor.index as u32,
            &step.step,
            step.id.as_deref(),
            true,
        );
        store
            .restart_task_flow(
                &task,
                store.task_flow(&task.id).await.unwrap().as_ref(),
                "fixture-checkpoint",
            )
            .await
            .unwrap();
        replacement.version = 0;
        replacement.current_attempt = None;
        replacement.pending_session_id = None;
        replacement.ready_summary = None;
        let replacement = parked(&store, &task, replacement).await;

        assert!(
            super::complete_human_flow_step(&store, &stale, &replacement)
                .await
                .is_err()
        );
        let current = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(current.invocation.id, replacement.invocation.id);
        assert_eq!(current.version, replacement.version);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the guard serializes LF_BIN for the fixture
    async fn claimed_autonomous_boundary_settles_once_at_the_human_node() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, _) = human_task_fixture().await;
        let flow = super::start_task_flow(&task, "task-design").unwrap();
        let initial = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &initial, 101).await;
        let mut flow = store.task_flow(&task.id).await.unwrap().unwrap();
        let completed = finish(&mut flow).unwrap();
        assert!(!completed);
        // The checkpoint onto the review releases the claim in the same write.
        store
            .checkpoint_flow(
                flow.id(),
                flow.version,
                &flow.cursor,
                Some(&held),
                Some("design ready"),
            )
            .await
            .unwrap();
        assert!(store
            .checkpoint_flow(flow.id(), flow.version, &flow.cursor, Some(&held), None)
            .await
            .is_err());

        let settled = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(settled.is_human());
        assert!(settled.claim.is_none());
        assert_eq!(settled.version, initial.version + 1);
        super::park_at_review(&store, &task, &settled)
            .await
            .unwrap();
        assert_eq!(
            store
                .sessions(&crate::session::SessionFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.kind, TaskEventKind::Progress { summary } if summary == "design ready"
        )));
    }

    #[tokio::test]
    async fn final_skill_completion_removes_the_flow_without_restarting_it() {
        let (store, task, _) = human_task_fixture().await;
        let mut position = super::start_task_flow(&task, "task-design").unwrap();
        position.invocation.steps.truncate(1);
        position.cursor.iteration = 3;
        let position = store.start_task_flow(&task.id, position).await.unwrap();
        let held = claim(&store, &task, &position, 101).await;
        let mut position = store.task_flow(&task.id).await.unwrap().unwrap();
        let completed = finish(&mut position).unwrap();
        assert!(completed);
        assert_eq!(position.cursor.iteration, 3);
        store
            .end_flow(position.id(), position.version, Some(&held), "done")
            .await
            .unwrap();

        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        assert_eq!(
            store
                .work_status(&WorkRef::Task(task.id.clone()))
                .await
                .unwrap(),
            crate::durable::WorkStatus::Ready
        );
        let events = store.task_events_after(&task.id, 0).await.unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.kind, TaskEventKind::FlowFinished { summary, .. } if summary == "done"
        )));
    }

    async fn ready_review(store: &SharedStore, task: &Task, feedback: &str) {
        let position = store.task_flow(&task.id).await.unwrap().unwrap();
        let id = crate::ops::human_session::flow_id(&position).unwrap();
        let (position, run) = store.reserve_review_run(&position).await.unwrap();
        store
            .sqlite
            .publish_review_run(&id, run.captured.unwrap(), position.version, "codex", None)
            .unwrap();
        store
            .ready_session(&id, run.captured, feedback)
            .await
            .unwrap();
    }

    async fn finish_test_decision(
        store: &SharedStore,
        flow: &FlowSession,
        verdict: crate::engine::transitions::FlowVerdict,
    ) -> FlowSession {
        let saved = store
            .reserve_attempt(flow.id(), flow.version, None)
            .await
            .unwrap();
        let input = &saved.current_attempt.as_ref().unwrap().run_id;
        store
            .sqlite
            .publish_attempt(
                saved.id(),
                saved.version,
                store.sqlite.captured_sequence(input).unwrap().unwrap(),
                None,
                "proof",
                None,
            )
            .unwrap();
        let actor = store.sqlite.test_flow_turn(input);
        store.sqlite.test_decision_output(&actor, &verdict).unwrap();
        store.sqlite.test_finish_flow_turn(&actor, "completed");
        let mut saved = store.flow(saved.id()).await.unwrap().unwrap();
        finish(&mut saved).unwrap();
        store
            .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
            .await
            .unwrap()
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
        finish(&mut flow).unwrap();
        while !flow.is_decision() {
            assert!(!finish(&mut flow).unwrap());
        }
        flow.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "ready to demonstrate".into(),
        });
        finish(&mut flow).unwrap();
        assert_eq!(flow.current().step, "pr-publish");
        finish(&mut flow).unwrap();
        assert!(flow.is_human());
        assert_eq!(flow.current().step, "demo");
        let token = park_human_task(&store, &task, &flow).await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(super::complete_human_flow_step(&store, &token, &expected)
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
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &token, &expected)
            .await
            .unwrap();
        let mut saved = store.task_flow(&task.id).await.unwrap().unwrap();
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
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        assert!(super::complete_human_flow_step(&store, &token, &expected)
            .await
            .is_err());
        assert_eq!(store.task_flow(&task.id).await.unwrap().unwrap(), saved);
        saved = finish_test_decision(&store, &saved, FlowVerdict {
            decision: FlowDecision::Iterate,
            summary: "Implement scratch/search/design.md using the findings and proof in scratch/search/feedback.md".into(),
        }).await;
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
        while !saved.is_decision() {
            assert!(!finish(&mut saved).unwrap());
            saved = store
                .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
                .await
                .unwrap();
        }
        saved = finish_test_decision(
            &store,
            &saved,
            FlowVerdict {
                decision: FlowDecision::Advance,
                summary: "revision demonstrated".into(),
            },
        )
        .await;
        assert_eq!(saved.current().step, "pr-publish");
        finish(&mut saved).unwrap();
        store
            .checkpoint_flow(saved.id(), saved.version, &saved.cursor, None, None)
            .await
            .unwrap();
        let later = park_human_task(&store, &task, &flow).await;
        assert_ne!(later.iteration, token.iteration);
        ready_review(&store, &task, "the interaction works").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &later, &expected)
            .await
            .unwrap();
        let mut saved = store.task_flow(&task.id).await.unwrap().unwrap();
        saved.cursor.leaf_mut().progress.verdict = Some(FlowVerdict {
            decision: FlowDecision::Advance,
            summary: "human feedback and proof agree".into(),
        });
        finish(&mut saved).unwrap();
        assert_eq!(saved.cursor.index, 1);
        assert!(saved.cursor.child.is_none());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn completing_a_final_review_finishes_the_flow() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "design clarified").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        super::complete_human_flow_step(&store, &token, &expected)
            .await
            .unwrap();
        assert!(store.task_flow(&task.id).await.unwrap().is_none());
        assert!(!crate::ops::human_session::token_is_current(&store, &token)
            .await
            .unwrap());
        assert!(store.recent_task_events(&task.id, 10).await.unwrap().iter().any(|event|
            matches!(&event.kind, TaskEventKind::FlowFinished { summary, .. } if summary == "design clarified")
        ));
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn runtime_child_review_releases_the_worker_and_retains_the_managed_root() {
        let guard = super::TestLfBinGuard::pin();
        let db = guard.ledger.home().join("loopflow.db");
        let (store, task, mut flow) = human_task_fixture_at(&db).await;
        flow.invocation.steps = vec![flow.current_plan().clone()];
        flow.cursor = Default::default();
        flow.invocation
            .steps
            .push(crate::engine::ConcreteStep::Skill(
                crate::engine::ConcreteSkill {
                    skill: crate::engine::Skill::named("decide"),
                    flow_parents: vec![],
                    policy: crate::engine::OccurrencePolicy {
                        id: Some("decision".into()),
                        human: false,
                        repeat: Some(crate::engine::flow::RepeatPolicy {
                            from: flow.current().policy.id.unwrap(),
                        }),
                    },
                },
            ));
        flow.cursor.index = 1;
        let flow = store.start_task_flow(&task.id, flow).await.unwrap();
        let held = claim(&store, &task, &flow, 303).await;
        let input = reserved_run(&store, &task).await;
        store
            .sqlite
            .publish_attempt(
                flow.id(),
                flow.version,
                store.sqlite.captured_sequence(&input).unwrap().unwrap(),
                Some(&held),
                "proof",
                None,
            )
            .unwrap();
        let actor = store.sqlite.test_flow_turn(&input);
        store
            .sqlite
            .test_decision_output(
                &actor,
                &crate::engine::transitions::FlowVerdict {
                    decision: crate::engine::transitions::FlowDecision::Iterate,
                    summary: "review the revision".into(),
                },
            )
            .unwrap();
        store.sqlite.test_finish_flow_turn(&actor, "completed");
        let saved = store.flow(flow.id()).await.unwrap().unwrap();
        let mut cursor = saved.cursor.clone();
        cursor.finish(&saved.invocation.steps).unwrap();
        let child = store
            .checkpoint_flow(saved.id(), saved.version, &cursor, Some(&held), None)
            .await
            .unwrap();
        assert!(child.is_human());
        assert!(child.claim.is_none());
        assert_eq!(
            store.task_flow(&task.id).await.unwrap().unwrap().id(),
            child.id()
        );
        let conn = rusqlite::Connection::open(db).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT current_invocation_id FROM tasks WHERE id=?1",
                [task.id.as_str()],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            flow.id()
        );
        super::park_at_review(&store, &task, &child).await.unwrap();
        ready_review(&store, &task, "revision accepted").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        store
            .complete_task_review(&task.id, &expected, "revision accepted")
            .await
            .unwrap();
        let continued = store.task_flow(&task.id).await.unwrap().unwrap();
        assert_eq!(continued.id(), child.id());
        assert_eq!(continued.current().step, "decide");
        assert_eq!(
            continued.cursor.leaf().progress.direction.as_deref(),
            Some("revision accepted")
        );
        assert!(store
            .complete_task_review(&task.id, &expected, "late completion")
            .await
            .is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // serializes the fixture executable and Home
    async fn concurrent_review_completions_settle_once() {
        let _lf_bin = super::TestLfBinGuard::pin();
        let (store, task, flow) = human_task_fixture().await;
        let token = park_human_task(&store, &task, &flow).await;
        ready_review(&store, &task, "review feedback").await;
        let expected = store.task_flow(&task.id).await.unwrap().unwrap();
        let (first, second) = tokio::join!(
            super::complete_human_flow_step(&store, &token, &expected),
            super::complete_human_flow_step(&store, &token, &expected),
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
            agent: None,
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
            flow: "feature".into(),
            status: crate::pm::ProjectStatus::Started,
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
