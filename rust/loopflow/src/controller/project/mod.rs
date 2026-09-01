use std::io::BufRead;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::child::ChildRef;
use crate::controller::wave::playhead::{
    BodyProvenance, Playhead, PlayheadEvent, QueuedInvocation, StepKind, StepOutcome,
};
use crate::durable::{Steer, WorkStatus};
use crate::harness::{default_create_harness, drain_turn_failure_reason, ApprovalPolicy, Harness};
use crate::store::SharedStore;
use crate::work::project::{ChildEventPayload, Project, ProjectEventKind, ProjectId};
use crate::work::wave::Wave;

mod state;

pub(crate) use state::{automatic_restart_bar, State};

#[derive(Debug, Clone)]
struct ControlledProject {
    work: Project,
    state: State,
}

impl Deref for ControlledProject {
    type Target = Project;

    fn deref(&self) -> &Self::Target {
        &self.work
    }
}

impl DerefMut for ControlledProject {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.work
    }
}

async fn controlled_project(
    store: &SharedStore,
    project_id: &ProjectId,
) -> Result<ControlledProject> {
    let work = store
        .get_project(project_id)
        .await?
        .ok_or_else(|| anyhow!("Project {project_id} not found"))?;
    let state = store
        .project_controller_state(project_id)
        .await?
        .ok_or_else(|| anyhow!("Project {project_id} has no end-to-end controller state"))?;
    Ok(ControlledProject { work, state })
}

#[derive(Debug)]
struct PreparedProjectStep {
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    planning: crate::ops::task_pm::ResolvedProject,
}

pub(crate) async fn run(store: SharedStore, project_id: ProjectId) -> Result<()> {
    let result = run_project_inner(store.clone(), project_id.clone()).await;
    if let Err(error) = &result {
        record_unhandled_failure(&store, &project_id, error).await;
    }
    result
}

async fn owning_wave(store: &SharedStore, project: &ControlledProject) -> Result<Wave> {
    store
        .get_wave(&project.wave_id)
        .await?
        .ok_or_else(|| anyhow!("owning Wave {} is not registered", project.wave_id))
}

async fn run_project_inner(store: SharedStore, project_id: ProjectId) -> Result<()> {
    let mut project = controlled_project(&store, &project_id).await?;
    let wave = owning_wave(&store, &project).await?;
    store.put_project_controller_state(&project.state).await?;
    store
        .append_project_event(&project.id, &ProjectEventKind::Started)
        .await?;
    let observations = consume_task_observations(&store, &mut project).await?;
    let (mut flow, _) = Playhead::new(QueuedInvocation::load(Path::new(wave.repo()), "project")?);
    let mut prepared =
        prepare_project_flow_step(&store, &mut project, &wave, &flow, &observations).await?;
    let mut active_planning = prepared.planning.clone();
    let (harness_name, _) = crate::engine::config::parse_agent(&project.state.agent);
    let capture = crate::run_record::CaptureHandle::begin_with_context(
        crate::run_record::RunSpec {
            harness: prepared.turn.harness.clone(),
            model: prepared.turn.model.clone(),
            surface: "headless".to_string(),
            cwd: Path::new(wave.repo()).to_path_buf(),
            repo: Some(Path::new(wave.repo()).to_path_buf()),
            worktree: Some(Path::new(wave.repo()).to_path_buf()),
            skill: flow.current().map(|step| step.step.clone()),
            subjects: vec![
                crate::run_record::SubjectAttribution::declared(format!("wave:{}", wave.name())),
                crate::run_record::SubjectAttribution::declared(format!(
                    "project:{}",
                    project.plan.slug
                )),
            ],
        },
        &prepared.turn.context,
    )?;
    capture.record_input("initial", &prepared.turn.input);
    prepared.turn.config.env.extend(capture.environment());
    capture.mark_spawn_requested();
    let capture = Some(capture);
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = default_create_harness(&harness_name, ApprovalPolicy::AutoApprove, event_tx)
        .inspect_err(|_| {
            finish_capture(capture.as_ref(), "failed");
        })?;
    harness.set_provider_session_id(project.state.provider_session_id.clone());
    if let Err(error) = harness.start(&prepared.turn.config).await {
        finish_capture(capture.as_ref(), "failed");
        return Err(error);
    }
    project.state.provider = harness_name;
    project.state.provider_session_id = harness.provider_session_id();
    if let Err(error) = store.put_project_controller_state(&project.state).await {
        let _ = harness.stop().await;
        return Err(error.into());
    }
    if let Some(capture) = &capture {
        capture.set_provider_session_id(project.state.provider_session_id.clone());
    }
    start_project_flow_turn(
        &store,
        &mut project,
        harness.as_mut(),
        &mut flow,
        None,
        prepared,
    )
    .await?;
    let mut flow_turn_active = true;

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
    let mut last_text = String::new();
    loop {
        tokio::select! {
            line = attachment_rx.recv() => {
                if let Some(line) = line {
                    if line.trim() == "/interrupt" {
                        harness.interrupt().await?;
                    } else {
                        handle_attachment(&store, &project, line).await?;
                    }
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return finish_failed(
                        &store,
                        &mut project,
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
                if provider_session_id != project.state.provider_session_id {
                    project.state.provider_session_id = provider_session_id;
                    store.put_project_controller_state(&project.state).await?;
                }
                match event {
                    ConversationEvent::TextDelta { content, .. } => last_text.push_str(&content),
                    ConversationEvent::TurnStarted { .. } => {
                    }
                    ConversationEvent::ItemCompleted { .. } => {}
                    ConversationEvent::TurnCompleted { status, .. } => {
                        if status == Lifecycle::Failed {
                            let reason = drain_turn_failure_reason(
                                &mut event_rx,
                                "provider turn failed",
                            );
                            return finish_failed(
                                &store,
                                &mut project,
                                harness.as_mut(),
                                &reason,
                                capture.as_ref(),
                            )
                            .await;
                        }
                        if let Err(error) = verify_control_plane_checkout(Path::new(wave.repo())) {
                            return finish_failed(
                                &store,
                                &mut project,
                                harness.as_mut(),
                                &error.to_string(),
                                capture.as_ref(),
                            )
                            .await;
                        }
                        let flow_iteration_completed = if flow_turn_active {
                            finish_project_flow_turn(&mut flow, status)?
                        } else {
                            false
                        };
                        flow_turn_active = false;
                        if status != Lifecycle::Interrupted {
                            let observations =
                                consume_task_observations(&store, &mut project).await?;
                            if !observations.is_empty() {
                                apply_input(
                                    harness.as_mut(),
                                    format!(
                                        "New supervised Task observations arrived. Continue the same Project iteration:\n{}",
                                        observations.join("\n")
                                    ),
                                ).await?;
                                continue;
                            }
                        }
                        if !flow_iteration_completed && status != Lifecycle::Interrupted {
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut project,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            active_planning = prepared.planning.clone();
                            start_project_flow_turn(
                                &store,
                                &mut project,
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
                            project.state.iteration += 1;
                            store.append_project_event(
                                &project.id,
                                &ProjectEventKind::IterationCompleted {
                                    iteration: project.state.iteration,
                                    summary: summary.clone(),
                                },
                            ).await?;
                        }
                        let mut outcome = inspect_outcome(&store, &project, &active_planning).await?;
                        if status == Lifecycle::Interrupted {
                            outcome.disposition = ProjectDisposition::Wait;
                        }
                        if outcome.disposition == ProjectDisposition::Continue {
                            project.state.last_state_fingerprint = Some(outcome.fingerprint);
                            project.updated_at = time::OffsetDateTime::now_utc();
                            store.put_project_controller_state(&project.state).await?;
                            last_text.clear();
                            let prepared = prepare_project_flow_step(
                                &store,
                                &mut project,
                                &wave,
                                &flow,
                                &[],
                            )
                            .await?;
                            active_planning = prepared.planning.clone();
                            start_project_flow_turn(
                                &store,
                                &mut project,
                                harness.as_mut(),
                                &mut flow,
                                capture.as_ref(),
                                prepared,
                            )
                            .await?;
                            flow_turn_active = true;
                            continue;
                        }
                        project.state.last_state_fingerprint = Some(outcome.fingerprint);
                        store.put_project_controller_state(&project.state).await?;
                        let _ = harness.stop().await;
                        finish_capture(capture.as_ref(), "completed");
                        finish_project_outcome(
                            &store,
                            &project,
                            outcome.disposition,
                            summary,
                        ).await?;
                        return Ok(());
                    }
                    ConversationEvent::Error { code, message, .. } => {
                        let reason = format!("{code}: {message}");
                        return finish_failed(
                            &store,
                            &mut project,
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

async fn finish_project_outcome(
    store: &SharedStore,
    project: &ControlledProject,
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
        store.complete_project(project, &summary).await?;
    } else {
        store.put_project_controller_state(&project.state).await?;
    }
    Ok(())
}

async fn prepare_project_flow_step(
    store: &SharedStore,
    project: &mut ControlledProject,
    wave: &Wave,
    flow: &Playhead,
    observations: &[String],
) -> Result<PreparedProjectStep> {
    let planning = refresh_project_plan(store, project, wave).await?;
    let work = store
        .work_for_child(&ChildRef::Project(project.id.clone()))
        .await?;
    let steers = store.work_steers(&work).await?;
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
    let seed = project_seed(project, wave.name(), &steers, observations, &metric_context);
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn(&step.step, &seed, wave.name(), None)?;
    prepared.config.agent = Some(project.state.agent.clone());
    Ok(PreparedProjectStep {
        turn: prepared,
        planning,
    })
}

async fn refresh_project_plan(
    store: &SharedStore,
    project: &mut ControlledProject,
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
        .adopt_project_plan(&project.id, &plan)
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
    project.work = adopted;
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
    project: &mut ControlledProject,
    harness: &mut dyn Harness,
    flow: &mut Playhead,
    capture: Option<&crate::run_record::CaptureHandle>,
    prepared: PreparedProjectStep,
) -> Result<()> {
    let wave = owning_wave(store, project).await?;
    open_project_flow_body(flow, wave.repo())?;
    if let Some(capture) = capture {
        capture.record_input("queued", &prepared.turn.input);
    }
    apply_input(harness, prepared.turn.input).await?;
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
    project: &ControlledProject,
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
    let target = ChildRef::Project(project.id.clone());
    let work = store.work_for_child(&target).await?;
    let receipt = store
        .append_steer(&work, crate::durable::Author::User, line)
        .await?;
    println!("queued {}", receipt.steer.id);
    Ok(())
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
    project: &ControlledProject,
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
            .is_some_and(|pr| pr.phase() == crate::work::task::PrPhase::Open);
    }
    if has_open_pr {
        return Ok(ProjectOutcome {
            disposition: ProjectDisposition::Wait,
            fingerprint,
        });
    }
    if project.state.last_state_fingerprint.as_deref() == Some(&fingerprint) {
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
    project: &mut ControlledProject,
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
            .consume_task_observation_for_project(&project.id, &observation)
            .await?;
        if inserted {
            prompts.push(serde_json::to_string(event)?);
        }
        project.state.observation_cursor = project.state.observation_cursor.max(observation.id);
    }
    store.put_project_controller_state(&project.state).await?;
    Ok(prompts)
}

async fn apply_input(harness: &mut dyn Harness, input: String) -> Result<()> {
    harness.send_input(&input).await
}

fn finish_capture(capture: Option<&crate::run_record::CaptureHandle>, outcome: &str) {
    let Some(capture) = capture else { return };
    if let Err(error) = capture.finish(outcome) {
        tracing::warn!(%error, "failed to finish Project Run record");
    }
}

async fn finish_failed(
    store: &SharedStore,
    project: &mut ControlledProject,
    harness: &mut dyn Harness,
    error: &str,
    capture: Option<&crate::run_record::CaptureHandle>,
) -> Result<()> {
    finish_capture(capture, "failed");
    let _ = harness.stop().await;
    store.fail_project(project, error).await?;
    anyhow::bail!(error.to_string())
}

async fn record_unhandled_failure(
    store: &SharedStore,
    project_id: &ProjectId,
    error: &anyhow::Error,
) {
    let Ok(Some(project)) = store.get_project(project_id).await else {
        return;
    };
    let message = format!("project runner failed: {error}");
    let _ = store.fail_project(&project, &message).await;
}

fn project_seed(
    project: &ControlledProject,
    wave_name: &str,
    steers: &[Steer],
    observations: &[String],
    metric_context: &str,
) -> String {
    let context = crate::ops::render_project_context(
        project,
        Some(&project.state),
        wave_name,
        steers,
        observations,
        metric_context,
    );
    format!(
        "{context}\n\nRun clarify, pursue, and mutate through the same provider session. Read and update only this Linear Project. Start implementation through Task Work; do not edit repository files from Project Work."
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
    use crate::durable::{Author, WorkStatus};
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::pm::{PmKr, PmProject, ProjectFlowPlan};
    use crate::store::{open_store, SharedStore, StorageConfig};
    use crate::work::project::{Project, ProjectEventKind, ProjectId};
    use crate::work::wave::Wave;

    async fn project_fixture() -> (
        tempfile::TempDir,
        PathBuf,
        SharedStore,
        super::ControlledProject,
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
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let state = super::State {
            project_id: project.id.clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            updated_at: now,
        };
        store.put_project_controller_state(&state).await.unwrap();
        (
            directory,
            database,
            store,
            super::ControlledProject {
                work: project,
                state,
            },
        )
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
        let (_directory, _database, store, mut project) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        store
            .append_steer(
                &work,
                Author::User,
                "Preserve this direction across planning refresh.",
            )
            .await
            .unwrap();
        project.state.observation_cursor = 17;
        store
            .put_project_controller_state(&project.state)
            .await
            .unwrap();
        let prior_event = store
            .append_project_event(
                &project.id,
                &ProjectEventKind::IterationCompleted {
                    iteration: 0,
                    summary: "prior evidence remains durable".to_string(),
                },
            )
            .await
            .unwrap();
        let prior_steers = store.work_steers(&work).await.unwrap();
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
            .adopt_project_plan(&project.id, &refreshed_plan)
            .await
            .unwrap();
        let (same_plan, changed_again) = store
            .adopt_project_plan(&project.id, &refreshed_plan)
            .await
            .unwrap();

        assert!(changed);
        assert!(!changed_again);
        assert_eq!(same_plan.plan, adopted.plan);
        assert_eq!(adopted.id, prior_id);
        let controller = store
            .project_controller_state(&adopted.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(controller.observation_cursor, 17);
        assert_eq!(store.work_steers(&work).await.unwrap(), prior_steers);
        let events = store.project_events_after(&adopted.id, 0).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], prior_event);

        let adopted = super::ControlledProject {
            work: adopted,
            state: controller,
        };
        let seed = super::project_seed(
            &adopted,
            "incident-management",
            &prior_steers,
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
    async fn successful_project_flow_boundary_settles_once_from_work_state() {
        let (_directory, _database, store, mut project) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();
        project.state.iteration = 1;
        store
            .append_project_event(
                &project.id,
                &ProjectEventKind::IterationCompleted {
                    iteration: project.state.iteration,
                    summary: "restored the reported surface".to_string(),
                },
            )
            .await
            .unwrap();
        super::finish_project_outcome(
            &store,
            &project,
            super::ProjectDisposition::Done,
            "restored the reported surface".to_string(),
        )
        .await
        .unwrap();
        super::finish_project_outcome(
            &store,
            &project,
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
    }

    #[tokio::test]
    async fn project_failure_remains_resumable_in_work_state() {
        let (_directory, _database, store, project) = project_fixture().await;
        let work = store
            .work_for_child(&ChildRef::Project(project.id.clone()))
            .await
            .unwrap();

        super::record_unhandled_failure(
            &store,
            &project.id,
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
        assert_eq!(WorkStatus::Ready.reason(), "ready");
    }
}
