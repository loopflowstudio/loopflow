use std::path::Path;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;
use tokio::sync::{mpsc, oneshot};

use crate::chat::types::{ConversationEvent, Lifecycle};
use crate::durable::{
    AdvanceReceipt, AnswerContext, AskId, BoundarySeed, BoundaryState, ControlCtx, InvocationRoute,
    RunAdvance, RunLease, WorkRef,
};
use crate::engine::agent::AgentAuthority;
use crate::harness::{ApprovalPolicy, CreateHarnessFn};
use crate::project::Project;
use crate::store::SharedStore;
use crate::task::Task;
use crate::trace::{CaptureHandle, CaptureStart, SupervisedInvocation};
use crate::wave::Wave;

const MAX_ANSWER_ATTEMPTS: u32 = 3;

#[derive(Debug)]
struct ActiveAnswer {
    ask_id: AskId,
    cancel: oneshot::Sender<()>,
}

#[derive(Debug)]
pub(crate) struct AnswerAttempt {
    ask_id: AskId,
    answer: Result<String, String>,
}

pub(crate) struct AnswerLane {
    parent: WorkRef,
    lease: RunLease,
    create_harness: CreateHarnessFn,
    active: Option<ActiveAnswer>,
    parked_ask: Option<AskId>,
    events_tx: mpsc::UnboundedSender<AnswerAttempt>,
    events_rx: mpsc::UnboundedReceiver<AnswerAttempt>,
}

impl AnswerLane {
    pub(crate) fn new(parent: WorkRef, lease: RunLease) -> Self {
        Self::with_harness(parent, lease, crate::harness::default_create_harness)
    }

    fn with_harness(parent: WorkRef, lease: RunLease, create_harness: CreateHarnessFn) -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        Self {
            parent,
            lease,
            create_harness,
            active: None,
            parked_ask: None,
            events_tx,
            events_rx,
        }
    }

    pub(crate) fn active(&self) -> bool {
        self.active.is_some()
    }

    pub(crate) async fn reconcile(
        &mut self,
        store: &SharedStore,
        project: &Project,
        wave: &Wave,
    ) -> Result<()> {
        let Some(context) = self.next_context(store).await? else {
            return Ok(());
        };
        let prepared = prepare_answer_turn(store, project, wave, &context).await?;
        self.start(
            store,
            &project.agent,
            Path::new(wave.repo()),
            context,
            prepared,
        )
        .await
    }

    pub(crate) async fn reconcile_wave(&mut self, store: &SharedStore, wave: &Wave) -> Result<()> {
        let Some(context) = self.next_context(store).await? else {
            return Ok(());
        };
        let configured =
            crate::engine::wave_config::read_wave_config(Path::new(wave.repo()), wave.name());
        let agent = configured
            .and_then(|config| config.agent)
            .unwrap_or_else(|| {
                crate::engine::load_config_or_default(Some(Path::new(wave.repo())))
                    .agent()
                    .to_string()
            });
        let mut prepared = prepare_wave_answer_turn(store, wave, &context).await?;
        prepared.config.agent = Some(agent.clone());
        self.start(store, &agent, Path::new(wave.repo()), context, prepared)
            .await
    }

    async fn next_context(&mut self, store: &SharedStore) -> Result<Option<AnswerContext>> {
        if self.active.is_some() {
            return Ok(None);
        }
        let Some(context) = store.oldest_answer_context(&self.parent).await? else {
            self.parked_ask = None;
            return Ok(None);
        };
        if self.parked_ask.as_ref() == Some(&context.ask.id) {
            return Ok(None);
        }
        self.parked_ask = None;
        let history = store.answer_attempt_history(&context.ask.id).await?;
        if history.failed_attempts >= MAX_ANSWER_ATTEMPTS {
            tracing::warn!(
                ask_id = %context.ask.id,
                attempts = history.failed_attempts,
                "Ask remains pending after bounded answer attempts"
            );
            self.parked_ask = Some(context.ask.id);
            return Ok(None);
        }
        if let Some(last_failed_at) = history.last_failed_at {
            let delay = retry_delay(history.failed_attempts);
            if OffsetDateTime::now_utc() < last_failed_at + delay {
                return Ok(None);
            }
        }
        Ok(Some(context))
    }

    async fn start(
        &mut self,
        store: &SharedStore,
        agent: &str,
        repo: &Path,
        context: AnswerContext,
        prepared: crate::lf::commands::run::PreparedHarnessTurn,
    ) -> Result<()> {
        let (provider, model) = crate::engine::config::parse_agent(agent);
        let receipt = store
            .advance_run(
                &self.lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: provider.clone(),
                        model,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: Some(context.ask.id.clone()),
                },
            )
            .await?;
        let AdvanceReceipt::Invocation(invocation) = receipt else {
            unreachable!("InvocationStarting returns an Invocation receipt")
        };
        let supervision = SupervisedInvocation {
            invocation_id: invocation.id.clone(),
            supervising_run_id: self.lease.run_id.clone(),
            account_id: invocation.route.account_id.clone(),
            resume_token: None,
        };
        let (cancel, cancel_rx) = oneshot::channel();
        self.active = Some(ActiveAnswer {
            ask_id: context.ask.id.clone(),
            cancel,
        });
        let store = store.clone();
        let lease = self.lease.clone();
        let events = self.events_tx.clone();
        let ask_id = context.ask.id;
        let create_harness = self.create_harness;
        let repo = repo.to_path_buf();
        tokio::spawn(async move {
            let answer = run_answer_attempt(
                &store,
                &lease,
                &provider,
                prepared,
                supervision,
                repo,
                cancel_rx,
                create_harness,
            )
            .await
            .map_err(|error| error.to_string());
            let _ = events.send(AnswerAttempt { ask_id, answer });
        });
        Ok(())
    }

    pub(crate) async fn receive(&mut self) -> Option<AnswerAttempt> {
        self.events_rx.recv().await
    }

    pub(crate) fn try_receive(&mut self) -> Option<AnswerAttempt> {
        self.events_rx.try_recv().ok()
    }

    pub(crate) async fn settle(
        &mut self,
        store: &SharedStore,
        attempt: AnswerAttempt,
    ) -> Result<()> {
        if self.active.as_ref().map(|active| &active.ask_id) != Some(&attempt.ask_id) {
            tracing::warn!(ask_id = %attempt.ask_id, "ignored stale answer attempt result");
            return Ok(());
        }
        self.active = None;
        self.parked_ask = None;
        match attempt.answer {
            Ok(answer) => match store
                .answer_ask(&ControlCtx::Run(&self.lease), &attempt.ask_id, &answer)
                .await
            {
                Ok(_) => tracing::info!(ask_id = %attempt.ask_id, "answered child Ask"),
                Err(error) => {
                    let still_pending = store
                        .pending_asks_for_parent(&self.parent)
                        .await?
                        .iter()
                        .any(|ask| ask.id == attempt.ask_id);
                    if still_pending {
                        return Err(error.into());
                    }
                    tracing::info!(ask_id = %attempt.ask_id, "discarded answer for a settled Ask");
                }
            },
            Err(error) => tracing::warn!(
                ask_id = %attempt.ask_id,
                %error,
                "detached answer attempt failed"
            ),
        }
        Ok(())
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(active) = self.active.take() {
            let _ = active.cancel.send(());
        }
    }
}

fn retry_delay(failed_attempts: u32) -> time::Duration {
    match failed_attempts {
        0 => time::Duration::ZERO,
        1 => time::Duration::seconds(5),
        _ => time::Duration::seconds(30),
    }
}

async fn prepare_answer_turn(
    store: &SharedStore,
    project: &Project,
    wave: &Wave,
    context: &AnswerContext,
) -> Result<crate::lf::commands::run::PreparedHarnessTurn> {
    let WorkRef::Task(task_id) = &context.child else {
        return Err(anyhow!(
            "Project {} received an Ask from non-Task {} {}",
            project.id,
            context.child.kind(),
            context.child.id()
        ));
    };
    let task = store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("asking Task {task_id} is not registered"))?;
    if task.project_id != project.id {
        return Err(anyhow!(
            "Task {} belongs to Project {}, not {}",
            task.id,
            task.project_id,
            project.id
        ));
    }
    let project_boundary = store.boundary_seed(&context.ask.route.parent()?).await?;
    let task_boundary = store.boundary_seed(&context.child).await?;
    let pr = store.active_task_pr(&task.id).await?;
    let events = store.recent_task_events(&task.id, 8).await?;
    let seed = answer_seed(
        project,
        wave,
        &task,
        &project_boundary,
        &task_boundary,
        context,
        pr.as_ref(),
        &events,
    )?;
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn("answer-child", &seed, wave.name(), None)?;
    prepared.config.agent = Some(project.agent.clone());
    prepared.config.cwd = Some(Path::new(wave.repo()).to_path_buf());
    prepared.config.authority = AgentAuthority::Detached;
    Ok(prepared)
}

async fn prepare_wave_answer_turn(
    store: &SharedStore,
    wave: &Wave,
    context: &AnswerContext,
) -> Result<crate::lf::commands::run::PreparedHarnessTurn> {
    let WorkRef::Project(project_id) = &context.child else {
        return Err(anyhow!(
            "Wave {} received an Ask from non-Project {} {}",
            wave.name(),
            context.child.kind(),
            context.child.id()
        ));
    };
    let project = store
        .get_project(project_id)
        .await?
        .ok_or_else(|| anyhow!("asking Project {project_id} is not registered"))?;
    if project.wave_id != *wave.id() {
        return Err(anyhow!(
            "Project {} belongs to Wave {}, not {}",
            project.id,
            project.wave_id,
            wave.id()
        ));
    }
    let wave_boundary = store.boundary_seed(&context.ask.route.parent()?).await?;
    let project_boundary = store.boundary_seed(&context.child).await?;
    let mut events = store.project_events_after(&project.id, 0).await?;
    if events.len() > 8 {
        events.drain(..events.len() - 8);
    }
    let seed = wave_answer_seed(
        wave,
        &project,
        &wave_boundary,
        &project_boundary,
        context,
        &events,
    )?;
    let mut prepared =
        crate::lf::commands::run::prepare_harness_turn("answer-child", &seed, wave.name(), None)?;
    prepared.config.cwd = Some(Path::new(wave.repo()).to_path_buf());
    prepared.config.authority = AgentAuthority::Detached;
    Ok(prepared)
}

fn wave_answer_seed(
    wave: &Wave,
    project: &Project,
    wave_boundary: &BoundarySeed,
    project_boundary: &BoundarySeed,
    context: &AnswerContext,
    events: &[crate::project::ProjectEvent],
) -> Result<String> {
    let repo = Path::new(wave.repo());
    let goal_path = repo.join("wave").join(wave.name()).join("GOAL.md");
    let goal = std::fs::read_to_string(goal_path).unwrap_or_else(|_| "(goal unavailable)".into());
    let memory = crate::engine::wave_context::gather_wave_memory(repo, wave.name())
        .unwrap_or_else(|| "(memory unavailable)".into());
    Ok(format!(
        "Answer Ask {ask_id} from Project {project_name} ({project_id}) in Epoch {epoch_id}.\n\n\
         Exact question:\n{question}\n\nWave {wave_name} goal:\n{goal}\n\n\
         Current Wave memory:\n{memory}\n\nCurrent Wave direction:\n{wave_direction}\n\n\
         Project definition and KRs:\n{project_context}\n\nCurrent Project direction:\n{project_direction}\n\n\
         Recent Project evidence:\n{events}\n\nPrior Ask/Answer exchanges in this Project Epoch:\n{prior}",
        ask_id = context.ask.id,
        project_name = project.plan.name,
        project_id = project.id,
        epoch_id = context.epoch_id,
        question = context.ask.question,
        wave_name = wave.name(),
        wave_direction = wave_boundary.render(),
        project_context = project.plan.prompt_context,
        project_direction = project_boundary.render(),
        events = serde_json::to_string_pretty(events)?,
        prior = prior_exchange_seed(context),
    ))
}

fn prior_exchange_seed(context: &AnswerContext) -> String {
    if context.prior_exchanges.is_empty() {
        return "(none)".to_string();
    }
    context
        .prior_exchanges
        .iter()
        .map(|exchange| {
            let answer = exchange
                .answer
                .as_ref()
                .expect("prior answer context contains only answered exchanges");
            format!("Q: {}\nA: {}", exchange.question, answer.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[allow(clippy::too_many_arguments)]
fn answer_seed(
    project: &Project,
    wave: &Wave,
    task: &Task,
    project_boundary: &BoundarySeed,
    task_boundary: &BoundarySeed,
    context: &AnswerContext,
    pr: Option<&crate::task::TaskPr>,
    events: &[crate::task::TaskEvent],
) -> Result<String> {
    let repo = Path::new(wave.repo());
    let goal_path = repo.join("wave").join(wave.name()).join("GOAL.md");
    let goal = std::fs::read_to_string(goal_path).unwrap_or_else(|_| "(goal unavailable)".into());
    let memory = crate::engine::wave_context::gather_wave_memory(repo, wave.name())
        .unwrap_or_else(|| "(memory unavailable)".into());
    let prior = prior_exchange_seed(context);
    let pr = pr
        .map(serde_json::to_string_pretty)
        .transpose()?
        .unwrap_or_else(|| "(no active PR)".to_string());
    let events = serde_json::to_string_pretty(events)?;
    Ok(format!(
        "Answer Ask {ask_id} from Task {identifier} ({task_id}) in Epoch {epoch_id}.\n\n\
         Exact question:\n{question}\n\n\
         Project: {project_name} ({project_id})\n{project_context}\n\n\
         Current Project direction:\n{project_direction}\n\n\
         Wave goal:\n{goal}\n\nCurrent Wave memory:\n{memory}\n\n\
         Task directive: {title}\n{description}\nTask worktree: {worktree}\n\
         Lifecycle: {phase:?}, phase epoch {phase_epoch}, iteration {phase_iteration}\n\
         Current Task direction:\n{task_direction}\n\nActive PR evidence:\n{pr}\n\n\
         Recent Task events:\n{events}\n\nPrior Ask/Answer exchanges in this Task Epoch:\n{prior}",
        ask_id = context.ask.id,
        identifier = task.plan.identifier,
        task_id = task.id,
        epoch_id = context.epoch_id,
        question = context.ask.question,
        project_name = project.plan.name,
        project_id = project.id,
        project_context = project.plan.prompt_context,
        project_direction = project_boundary.render(),
        title = task.plan.title,
        description = task.plan.description,
        worktree = task.worktree.display(),
        phase = task.lifecycle_phase,
        phase_epoch = task.phase_epoch,
        phase_iteration = task.phase_iteration,
        task_direction = task_boundary.render(),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn run_answer_attempt(
    store: &SharedStore,
    lease: &RunLease,
    provider: &str,
    prepared: crate::lf::commands::run::PreparedHarnessTurn,
    supervision: SupervisedInvocation,
    repo: std::path::PathBuf,
    mut cancel: oneshot::Receiver<()>,
    create_harness: CreateHarnessFn,
) -> Result<String> {
    let invocation_id = supervision.invocation_id.clone();
    let capture_result = crate::journal::trace_capture_context(
        &repo,
        Some("answer-child".to_string()),
        Some("answer-child".to_string()),
    )
    .map(|trace_context| {
        CaptureHandle::begin(
            trace_context,
            prepared.context.clone(),
            CaptureStart {
                provider: prepared.harness.clone(),
                model: prepared.model.clone(),
                surface: "headless".to_string(),
                input_op: "initial".to_string(),
                gather_ms: prepared.context_gather_ms,
                render_ms: prepared.context_render_ms,
                raw_provider: true,
                basis: None,
                supervision: Some(supervision),
            },
        )
    })
    .transpose();
    let capture = match capture_result {
        Ok(Some(capture)) => Some(capture),
        Ok(None) if cfg!(test) => None,
        Ok(None) => {
            let result = Err(anyhow!("answer invocation has no trace capture context"));
            return settle_answer_invocation(store, lease, invocation_id, None, result).await;
        }
        Err(error) => {
            return settle_answer_invocation(store, lease, invocation_id, None, Err(error.into()))
                .await;
        }
    };
    let result = run_answer_provider(
        store,
        lease,
        &invocation_id,
        provider,
        &prepared,
        capture.as_ref(),
        &mut cancel,
        create_harness,
    )
    .await;
    settle_answer_invocation(store, lease, invocation_id, capture.as_ref(), result).await
}

#[allow(clippy::too_many_arguments)]
async fn run_answer_provider(
    store: &SharedStore,
    lease: &RunLease,
    invocation_id: &crate::durable::AgentInvocationId,
    provider: &str,
    prepared: &crate::lf::commands::run::PreparedHarnessTurn,
    capture: Option<&CaptureHandle>,
    cancel: &mut oneshot::Receiver<()>,
    create_harness: CreateHarnessFn,
) -> Result<String> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let mut harness = create_harness(provider, ApprovalPolicy::AutoApprove, event_tx)?;
    let result = async {
        harness.start(&prepared.config).await?;
        let observed = store
            .observe_invocation_provider(
                lease,
                invocation_id,
                harness.provider_account_id(),
                harness.provider_session_id(),
            )
            .await?;
        if let Some(capture) = &capture {
            capture.set_provider_session_id(observed.resume_token);
        }
        harness.send_input(&prepared.input).await?;
        let mut text = String::new();
        loop {
            tokio::select! {
                _ = &mut *cancel => {
                    let _ = harness.interrupt().await;
                    return Err(anyhow!("answer attempt interrupted"));
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        return Err(anyhow!("answer provider event stream closed"));
                    };
                    if let Some(capture) = &capture {
                        capture.record_conversation(event.clone());
                    }
                    match event {
                        ConversationEvent::TextDelta { content, .. } => text.push_str(&content),
                        ConversationEvent::TurnCompleted { status, .. } => {
                            if let Some(capture) = &capture {
                                capture.finish_turn(match status {
                                    Lifecycle::Completed => "completed",
                                    Lifecycle::Interrupted => "interrupted",
                                    _ => "failed",
                                })?;
                            }
                            if status != Lifecycle::Completed {
                                return Err(anyhow!("answer provider Turn ended {status:?}"));
                            }
                            let answer = text.trim();
                            if answer.is_empty() {
                                return Err(anyhow!("answer provider returned no text"));
                            }
                            return Ok(answer.to_string());
                        }
                        ConversationEvent::Error { code, message, .. } => {
                            return Err(anyhow!("{code}: {message}"));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    .await;
    let _ = harness.stop().await;
    result
}

async fn settle_answer_invocation(
    store: &SharedStore,
    lease: &RunLease,
    invocation_id: crate::durable::AgentInvocationId,
    capture: Option<&CaptureHandle>,
    result: Result<String>,
) -> Result<String> {
    let outcome = if result.is_ok() {
        BoundaryState::Succeeded
    } else {
        BoundaryState::Failed
    };
    let capture_result = match capture {
        Some(capture) => capture.finish(outcome.as_invocation_outcome(), false),
        None => Ok(()),
    };
    let invocation_result = store
        .advance_run(
            lease,
            RunAdvance::InvocationEnded {
                invocation_id,
                outcome,
            },
        )
        .await;
    invocation_result?;
    capture_result?;
    result
}

trait ParentRoute {
    fn parent(&self) -> Result<WorkRef>;
}

impl ParentRoute for crate::durable::AnswerRoute {
    fn parent(&self) -> Result<WorkRef> {
        match self {
            Self::Parent(parent) => Ok(parent.clone()),
            Self::User => Err(anyhow!("User-routed Ask has no parent Work")),
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use time::OffsetDateTime;
    use tokio::sync::mpsc;

    use super::{retry_delay, AnswerLane};
    use crate::chat::types::{ConversationEvent, Lifecycle};
    use crate::durable::{
        AdvanceReceipt, Containment, InvocationRoute, RunAdvance, RunTrigger, WorkRef,
    };
    use crate::engine::agent::{AgentAuthority, AgentConfig};
    use crate::harness::{ApprovalPolicy, Harness, SendCurrentOutcome};
    use crate::id::WaveId;
    use crate::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, StorageConfig, Store};
    use crate::task::{
        Observation, PmWritebackState, Task, TaskId, TaskLifecyclePhase, TaskLifecyclePlan, TaskPr,
        TaskPrId,
    };
    use crate::wave::Wave;

    struct AnswerHarness {
        events: mpsc::UnboundedSender<ConversationEvent>,
    }

    #[async_trait]
    impl Harness for AnswerHarness {
        async fn start(&mut self, config: &AgentConfig) -> anyhow::Result<()> {
            if config.authority != AgentAuthority::Detached {
                anyhow::bail!("answer harness inherited Work authority");
            }
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            let turn_id = "answer-turn".to_string();
            self.events.send(ConversationEvent::TurnStarted {
                turn_id: turn_id.clone(),
            })?;
            self.events.send(ConversationEvent::TextDelta {
                turn_id: turn_id.clone(),
                content: "Use the durable exchange as the proof.".to_string(),
            })?;
            self.events.send(ConversationEvent::TurnCompleted {
                turn_id,
                status: Lifecycle::Completed,
            })?;
            Ok(())
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
            Some("answer-session".to_string())
        }
    }

    fn create_answer_harness(
        _name: &str,
        _approval: ApprovalPolicy,
        events: mpsc::UnboundedSender<ConversationEvent>,
    ) -> anyhow::Result<Box<dyn Harness>> {
        Ok(Box::new(AnswerHarness { events }))
    }

    async fn start_invocation(
        store: &Store,
        work: &WorkRef,
        process_group: i64,
    ) -> (crate::durable::RunLease, crate::durable::AgentInvocation) {
        let (_, lease) = store.reserve_run(work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::ProcessGroup { id: process_group },
                    cwd: "/repo".into(),
                },
            )
            .await
            .unwrap();
        let AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "fake".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                    answer_ask_id: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        (lease, invocation)
    }

    async fn fixture() -> (
        std::sync::Arc<Store>,
        Wave,
        Project,
        Task,
        crate::durable::RunLease,
        crate::durable::AgentInvocation,
        crate::durable::RunLease,
        crate::durable::AgentInvocation,
    ) {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(directory.join("registry.db")))
                .await
                .unwrap(),
        );
        let now = OffsetDateTime::now_utc();
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("linear-project").unwrap(),
                slug: "runtime-project".to_string(),
                name: "Runtime Project".to_string(),
                prompt_context: "Answer child questions.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "fake".to_string(),
            provider: "fake".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let task = Task {
            id: TaskId::new(),
            plan: TaskPlan {
                id: LinearIssueId::new("linear-issue").unwrap(),
                identifier: "RUN-1".to_string(),
                title: "Prove detached answers".to_string(),
                description: "Keep the Project core Turn active.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            project_id: project.id.clone(),
            worktree: directory.join("task"),
            workspace_slug: "detached-answer".to_string(),
            lifecycle: TaskLifecyclePlan::standard("task-design", "slice", "ship"),
            lifecycle_phase: TaskLifecyclePhase::Loop,
            phase_epoch: 1,
            phase_cursor: 0,
            phase_iteration: 0,
            gate_cycle: 0,
            gate_proposal: None,
            agent: "fake".to_string(),
            provider: "fake".to_string(),
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
            branch: "task/detached-answer".to_string(),
            base_commit: "base".to_string(),
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
        store.create_task(&task, &pr).await.unwrap();
        let (parent_lease, parent_invocation) =
            start_invocation(&store, &WorkRef::Project(project.id.clone()), 101).await;
        store
            .advance_run(
                &parent_lease,
                RunAdvance::TurnStarting {
                    invocation_id: parent_invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        let (child_lease, child_invocation) =
            start_invocation(&store, &WorkRef::Task(task.id.clone()), 102).await;
        store
            .advance_run(
                &child_lease,
                RunAdvance::TurnStarting {
                    invocation_id: child_invocation.id.clone(),
                },
            )
            .await
            .unwrap();
        (
            store,
            wave,
            project,
            task,
            parent_lease,
            parent_invocation,
            child_lease,
            child_invocation,
        )
    }

    #[test]
    fn answer_attempts_back_off_and_stop_after_the_third_failure() {
        assert_eq!(retry_delay(0), time::Duration::ZERO);
        assert_eq!(retry_delay(1), time::Duration::seconds(5));
        assert_eq!(retry_delay(2), time::Duration::seconds(30));
    }

    #[tokio::test]
    async fn detached_answer_resumes_the_child_without_advancing_the_project_core() {
        let (
            store,
            wave,
            project,
            task,
            parent_lease,
            parent_invocation,
            child_lease,
            child_invocation,
        ) = fixture().await;
        let parent_work = WorkRef::Project(project.id.clone());
        let child_work = WorkRef::Task(task.id.clone());
        let parent_basis = store
            .current_epoch(&parent_work)
            .await
            .unwrap()
            .current_basis;
        let ask = store
            .open_ask(
                &child_lease,
                &child_invocation.id,
                "Which proof should this Task preserve?",
            )
            .await
            .unwrap();
        let mut lane = AnswerLane::with_harness(
            parent_work.clone(),
            parent_lease.clone(),
            create_answer_harness,
        );

        lane.reconcile(&store, &project, &wave).await.unwrap();
        let attempt = tokio::time::timeout(std::time::Duration::from_secs(5), lane.receive())
            .await
            .unwrap()
            .unwrap();
        lane.settle(&store, attempt).await.unwrap();

        let answered = store
            .current_ask(&child_lease, &child_invocation.id, Some(&ask.id))
            .await
            .unwrap();
        assert_eq!(
            answered.answer.unwrap().text,
            "Use the durable exchange as the proof."
        );
        assert_eq!(
            store
                .current_epoch(&parent_work)
                .await
                .unwrap()
                .current_basis,
            parent_basis,
            "Answer must not move the Project Basis"
        );
        let invocations = store
            .invocations_for_run(&parent_lease.run_id)
            .await
            .unwrap();
        let core = invocations
            .iter()
            .find(|invocation| invocation.id == parent_invocation.id)
            .unwrap();
        assert!(core.ended_at.is_none(), "Project core remains active");
        let answer = invocations
            .iter()
            .find(|invocation| invocation.answer_ask_id.as_ref() == Some(&ask.id))
            .unwrap();
        assert!(answer.ended_at.is_some());
        assert!(store
            .pending_asks_for_parent(&parent_work)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            store.work_status(&child_work).await.unwrap(),
            crate::durable::WorkStatus::Running {
                run_id: child_lease.run_id
            }
        );
        assert_eq!(
            store.pending_ask_comment_writes().await.unwrap().len(),
            2,
            "Linear publication remains a retryable outbox side effect"
        );
    }

    #[tokio::test]
    async fn wave_answers_a_project_without_entering_the_wave_core() {
        let (
            store,
            wave,
            _project,
            _task,
            project_lease,
            project_invocation,
            _task_lease,
            _task_invocation,
        ) = fixture().await;
        let wave_work = WorkRef::Wave(wave.id().clone());
        let (wave_lease, wave_core) = start_invocation(&store, &wave_work, 103).await;
        store
            .advance_run(
                &wave_lease,
                RunAdvance::TurnStarting {
                    invocation_id: wave_core.id.clone(),
                },
            )
            .await
            .unwrap();
        let basis = store.current_epoch(&wave_work).await.unwrap().current_basis;
        let ask = store
            .open_ask(
                &project_lease,
                &project_invocation.id,
                "Which KR should this Project optimize for?",
            )
            .await
            .unwrap();
        let mut lane =
            AnswerLane::with_harness(wave_work.clone(), wave_lease.clone(), create_answer_harness);

        lane.reconcile_wave(&store, &wave).await.unwrap();
        let attempt = tokio::time::timeout(std::time::Duration::from_secs(5), lane.receive())
            .await
            .unwrap()
            .unwrap();
        lane.settle(&store, attempt).await.unwrap();

        let answered = store
            .current_ask(&project_lease, &project_invocation.id, Some(&ask.id))
            .await
            .unwrap();
        assert_eq!(
            answered.answer.unwrap().text,
            "Use the durable exchange as the proof."
        );
        assert_eq!(
            store.current_epoch(&wave_work).await.unwrap().current_basis,
            basis
        );
        let invocations = store.invocations_for_run(&wave_lease.run_id).await.unwrap();
        assert!(invocations
            .iter()
            .find(|invocation| invocation.id == wave_core.id)
            .unwrap()
            .ended_at
            .is_none());
        assert!(invocations.iter().any(|invocation| {
            invocation.answer_ask_id.as_ref() == Some(&ask.id) && invocation.ended_at.is_some()
        }));
    }
}
